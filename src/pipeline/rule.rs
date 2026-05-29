use crate::types::DecodedFrame;
use serde::{Deserialize, Serialize};
use super::filter::SceneDetectFilter;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
#[schema(no_recursion)]
pub enum RuleConfig {
    #[serde(rename = "interval")]
    Interval {
        interval_seconds: f64,
    },
    #[serde(rename = "fps")]
    Fps {
        fps: f64,
    },
    #[serde(rename = "rate_limited")]
    RateLimited {
        rule: Box<RuleConfig>,
        max_per_minute: u64,
    },
    #[serde(rename = "scene_change")]
    SceneChange {
        threshold: f64,
    },
    #[serde(rename = "composite")]
    Composite {
        operator: CompositeOperator,
        rules: Vec<RuleConfig>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum CompositeOperator {
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "all")]
    All,
}

impl RuleConfig {
    #[allow(dead_code)]
    pub fn description(&self) -> String {
        match self {
            RuleConfig::Interval { interval_seconds } => {
                format!("interval/{:.1}s", interval_seconds)
            }
            RuleConfig::Fps { fps } => {
                format!("fps/{:.2}", fps)
            }
            RuleConfig::RateLimited { max_per_minute, .. } => {
                format!("rate-limited/{}mpm", max_per_minute)
            }
            RuleConfig::SceneChange { threshold } => {
                format!("scene-change/{:.2}", threshold)
            }
            RuleConfig::Composite { operator, rules } => {
                let descs: Vec<String> = rules.iter().map(|r| r.description()).collect();
                format!("composite:{}({})", match operator {
                    CompositeOperator::Any => "any",
                    CompositeOperator::All => "all",
                }, descs.join(","))
            }
        }
    }
}

pub trait RuleEvaluator: Send {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool;
    #[allow(dead_code)]
    fn description(&self) -> String;
}

pub fn create_evaluator(config: &RuleConfig, time_base: (i32, i32)) -> Box<dyn RuleEvaluator> {
    match config {
        RuleConfig::Interval { interval_seconds } => {
            Box::new(IntervalEvaluator::new(*interval_seconds, time_base))
        }
        RuleConfig::Fps { fps } => {
            let interval_seconds = 1.0 / fps.max(0.001);
            Box::new(IntervalEvaluator::new(interval_seconds, time_base))
        }
        RuleConfig::RateLimited { rule, max_per_minute } => {
            let inner = create_evaluator(rule, time_base);
            Box::new(RateLimitedEvaluator::new(inner, *max_per_minute))
        }
        RuleConfig::SceneChange { threshold } => {
            Box::new(SceneChangeEvaluator::new(*threshold))
        }
        RuleConfig::Composite { operator, rules } => {
            let inner: Vec<Box<dyn RuleEvaluator>> = rules.iter()
                .map(|r| create_evaluator(r, time_base))
                .collect();
            Box::new(CompositeEvaluator::new(*operator, inner))
        }
    }
}

pub fn has_scene_change_rule(configs: &[RuleConfig]) -> bool {
    configs.iter().any(matches_scene_change)
}

fn matches_scene_change(config: &RuleConfig) -> bool {
    match config {
        RuleConfig::SceneChange { .. } => true,
        RuleConfig::Composite { rules, .. } => rules.iter().any(matches_scene_change),
        _ => false,
    }
}

pub struct IntervalEvaluator {
    #[allow(dead_code)]
    interval_seconds: f64,
    interval_pts: i64,
    last_extracted_pts: Option<i64>,
    frames_evaluated: u64,
    frames_extracted: u64,
}

impl IntervalEvaluator {
    pub fn new(interval_seconds: f64, time_base: (i32, i32)) -> Self {
        let tb = time_base.0 as f64 / time_base.1 as f64;
        let interval_pts = if tb > 0.0 {
            (interval_seconds / tb) as i64
        } else {
            0
        };
        Self {
            interval_seconds,
            interval_pts,
            last_extracted_pts: None,
            frames_evaluated: 0,
            frames_extracted: 0,
        }
    }
}

impl RuleEvaluator for IntervalEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        self.frames_evaluated += 1;
        let should = match self.last_extracted_pts {
            None => true,
            Some(last_pts) => {
                frame.pts.saturating_sub(last_pts) >= self.interval_pts
            }
        };
        if should {
            self.last_extracted_pts = Some(frame.pts);
            self.frames_extracted += 1;
        }
        should
    }

    fn description(&self) -> String {
        format!("interval/{:.1}s", self.interval_seconds)
    }
}

pub struct RateLimitedEvaluator {
    inner: Box<dyn RuleEvaluator>,
    max_per_minute: u64,
    tokens: f64,
    last_refill: std::time::Instant,
}

impl RateLimitedEvaluator {
    pub fn new(inner: Box<dyn RuleEvaluator>, max_per_minute: u64) -> Self {
        Self {
            inner,
            max_per_minute: max_per_minute.max(1),
            tokens: max_per_minute as f64,
            last_refill: std::time::Instant::now(),
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        let rate = self.max_per_minute as f64 / 60.0;
        self.tokens = (self.tokens + elapsed * rate).min(self.max_per_minute as f64);
        self.last_refill = std::time::Instant::now();
    }

    fn consume(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl RuleEvaluator for RateLimitedEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        self.refill();
        if self.inner.should_extract(frame) {
            self.consume()
        } else {
            false
        }
    }

    fn description(&self) -> String {
        format!("rate-limited({}, max={}/min)", self.inner.description(), self.max_per_minute)
    }
}

pub struct SceneChangeEvaluator {
    threshold: f64,
}

impl SceneChangeEvaluator {
    pub fn new(threshold: f64) -> Self {
        Self { threshold: threshold.clamp(0.001, 0.999) }
    }
}

impl RuleEvaluator for SceneChangeEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        match frame.scene_change_score {
            Some(score) => score >= self.threshold,
            None => false,
        }
    }

    fn description(&self) -> String {
        format!("scene-change/{:.2}", self.threshold)
    }
}

pub struct CompositeEvaluator {
    operator: CompositeOperator,
    rules: Vec<Box<dyn RuleEvaluator>>,
}

impl CompositeEvaluator {
    pub fn new(operator: CompositeOperator, rules: Vec<Box<dyn RuleEvaluator>>) -> Self {
        Self { operator, rules }
    }
}

impl RuleEvaluator for CompositeEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        match self.operator {
            CompositeOperator::Any => {
                self.rules.iter_mut().any(|r| r.should_extract(frame))
            }
            CompositeOperator::All => {
                self.rules.iter_mut().all(|r| r.should_extract(frame))
            }
        }
    }

    fn description(&self) -> String {
        let descs: Vec<String> = self.rules.iter().map(|r| r.description()).collect();
        format!("composite:{}({})", match self.operator {
            CompositeOperator::Any => "any",
            CompositeOperator::All => "all",
        }, descs.join(","))
    }
}

pub struct RuleEngine {
    evaluators: Vec<(RuleConfig, Box<dyn RuleEvaluator>)>,
    pub scdet_filter: Option<SceneDetectFilter>,
    scd_enabled: bool,
}

impl RuleEngine {
    pub fn new(configs: &[RuleConfig], time_base: (i32, i32)) -> Self {
        let evaluators = configs.iter()
            .map(|c| (c.clone(), create_evaluator(c, time_base)))
            .collect();
        Self {
            evaluators,
            scdet_filter: None,
            scd_enabled: has_scene_change_rule(configs),
        }
    }

    pub fn evaluate(&mut self, frame: &DecodedFrame) -> bool {
        self.evaluators.iter_mut().any(|(_, eval)| eval.should_extract(frame))
    }

    pub fn rebuild(&mut self, configs: &[RuleConfig], time_base: (i32, i32)) {
        self.evaluators = configs.iter()
            .map(|c| (c.clone(), create_evaluator(c, time_base)))
            .collect();
        self.scd_enabled = has_scene_change_rule(configs);
        if !self.scd_enabled {
            self.scdet_filter = None;
        }
    }

    pub fn scd_enabled(&self) -> bool {
        self.scd_enabled
    }

    pub fn init_scdet_filter(
        &mut self,
        width: u32,
        height: u32,
        pixel_format: FFmpegPixelFormat,
        time_base: FFmpegRational,
    ) {
        if !self.scd_enabled {
            return;
        }
        // Find threshold from config
        let threshold = find_scene_change_threshold(&self.evaluators);
        match SceneDetectFilter::new(width, height, pixel_format, time_base, threshold) {
            Ok(filter) => {
                tracing::info!("Scene detection filter initialized (threshold={:.3})", threshold);
                self.scdet_filter = Some(filter);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to create scene detection filter, disabling SCD");
                self.scd_enabled = false;
            }
        }
    }
}

fn find_scene_change_threshold(evaluators: &[(RuleConfig, Box<dyn RuleEvaluator>)]) -> f64 {
    for (config, _) in evaluators {
        if let RuleConfig::SceneChange { threshold } = config {
            return *threshold;
        }
        if let RuleConfig::Composite { rules, .. } = config {
            for rule in rules {
                if let RuleConfig::SceneChange { threshold } = rule {
                    return *threshold;
                }
            }
        }
    }
    0.3
}

use ffmpeg_next::format::Pixel as FFmpegPixelFormat;
use ffmpeg_next::Rational as FFmpegRational;
