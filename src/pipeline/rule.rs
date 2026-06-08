use crate::types::DecodedFrame;
use serde::{Deserialize, Serialize};
use super::filter::SceneDetectFilter;
use super::comparator::FrameComparator;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMethod {
    PixelDiff,
    PerceptualHash,
    Ssim,
}

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
    #[serde(rename = "static_frame")]
    StaticFrame {
        threshold: f64,
        method: ComparisonMethod,
        #[serde(default)]
        force: bool,
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
            RuleConfig::StaticFrame { threshold, method, force } => {
                let f = if *force { ",force" } else { "" };
                format!("static-frame/{:.3}/{}{f}", threshold, match method {
                    ComparisonMethod::PixelDiff => "pixdiff",
                    ComparisonMethod::PerceptualHash => "phash",
                    ComparisonMethod::Ssim => "ssim",
                })
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
        RuleConfig::StaticFrame { threshold, method, force } => {
            Box::new(StaticFrameEvaluator::new(*threshold, *method, *force))
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

pub fn has_static_frame_rule(configs: &[RuleConfig]) -> bool {
    configs.iter().any(matches_static_frame)
}

fn matches_static_frame(config: &RuleConfig) -> bool {
    match config {
        RuleConfig::StaticFrame { .. } => true,
        RuleConfig::Composite { rules, .. } => rules.iter().any(matches_static_frame),
        _ => false,
    }
}

pub fn find_static_frame_config(
    evaluators: &[(RuleConfig, Box<dyn RuleEvaluator>)],
) -> Option<(f64, ComparisonMethod, bool)> {
    for (config, _) in evaluators {
        if let RuleConfig::StaticFrame { threshold, method, force } = config {
            return Some((*threshold, *method, *force));
        }
        if let RuleConfig::Composite { rules, .. } = config {
            for rule in rules {
                if let RuleConfig::StaticFrame { threshold, method, force } = rule {
                    return Some((*threshold, *method, *force));
                }
            }
        }
    }
    None
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

pub struct StaticFrameEvaluator {
    threshold: f64,
    method: ComparisonMethod,
    force: bool,
}

impl StaticFrameEvaluator {
    pub fn new(threshold: f64, method: ComparisonMethod, force: bool) -> Self {
        Self { threshold, method, force }
    }
}

impl RuleEvaluator for StaticFrameEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        match frame.static_frame_score {
            Some(true) => self.force,
            Some(false) => true,
            None => true,
        }
    }

    fn description(&self) -> String {
        format!("static-frame/{:.3}/{}", self.threshold, match self.method {
            ComparisonMethod::PixelDiff => "pixdiff",
            ComparisonMethod::PerceptualHash => "phash",
            ComparisonMethod::Ssim => "ssim",
        })
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
    pub frame_comparator: Option<FrameComparator>,
    static_frame_enabled: bool,
}

impl RuleEngine {
    pub fn new(configs: &[RuleConfig], time_base: (i32, i32)) -> Self {
        let evaluators = configs.iter()
            .map(|c| (c.clone(), create_evaluator(c, time_base)))
            .collect();
        let static_frame_enabled = has_static_frame_rule(configs);
        Self {
            evaluators,
            scdet_filter: None,
            scd_enabled: has_scene_change_rule(configs),
            frame_comparator: if static_frame_enabled {
                find_static_frame_config(&evaluators).map(|(t, m, _f)| FrameComparator::new(m, t))
            } else {
                None
            },
            static_frame_enabled,
        }
    }

    pub fn static_frame_enabled(&self) -> bool {
        self.static_frame_enabled
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
        self.static_frame_enabled = has_static_frame_rule(configs);
        if self.static_frame_enabled {
            let configs_with_eval: Vec<(RuleConfig, Box<dyn RuleEvaluator>)> = configs.iter()
                .map(|c| (c.clone(), create_evaluator(c, time_base)))
                .collect();
            self.frame_comparator = find_static_frame_config(&configs_with_eval)
                .map(|(t, m, _f)| FrameComparator::new(m, t));
        } else {
            self.frame_comparator = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DecodedFrame;
    use uuid::Uuid;

    fn make_frame(pts: i64, frame_number: u64, scene_score: Option<f64>) -> DecodedFrame {
        DecodedFrame {
            stream_id: Uuid::nil(),
            pts,
            time_base: (1, 30),
            width: 320,
            height: 240,
            y_plane: vec![128; 320 * 240],
            u_plane: vec![128; 320 * 240 / 4],
            v_plane: vec![128; 320 * 240 / 4],
            y_stride: 320,
            u_stride: 160,
            v_stride: 160,
            is_keyframe: true,
            frame_number,
            scene_change_score: scene_score,
            static_frame_score: None,
        }
    }

    #[test]
    fn test_interval_first_frame_always_extracts() {
        let mut ev = IntervalEvaluator::new(1.0, (1, 30));
        assert!(ev.should_extract(&make_frame(0, 0, None)));
    }

    #[test]
    fn test_interval_respects_gap() {
        let mut ev = IntervalEvaluator::new(1.0, (1, 30));
        assert!(ev.should_extract(&make_frame(0, 0, None)));
        assert!(!ev.should_extract(&make_frame(15, 1, None)));
    }

    #[test]
    fn test_interval_extracts_after_gap() {
        let mut ev = IntervalEvaluator::new(1.0, (1, 30));
        ev.should_extract(&make_frame(0, 0, None));
        assert!(ev.should_extract(&make_frame(30, 1, None)));
    }

    #[test]
    fn test_fps_rule() {
        let config = RuleConfig::Fps { fps: 10.0 };
        assert!(config.description().contains("fps"));
        let mut ev = create_evaluator(&config, (1, 30));
        assert!(ev.description().contains("interval"));
        assert!(ev.should_extract(&make_frame(0, 0, None)));
        assert!(!ev.should_extract(&make_frame(2, 1, None)));
        assert!(ev.should_extract(&make_frame(3, 2, None)));
    }

    #[test]
    fn test_scene_change_fires_on_high_score() {
        let mut ev = SceneChangeEvaluator::new(0.3);
        assert!(!ev.should_extract(&make_frame(0, 0, Some(0.1))));
        assert!(ev.should_extract(&make_frame(1, 1, Some(0.5))));
    }

    #[test]
    fn test_scene_change_no_score_returns_false() {
        let mut ev = SceneChangeEvaluator::new(0.3);
        assert!(!ev.should_extract(&make_frame(0, 0, None)));
    }

    #[test]
    fn test_scene_change_threshold_clamping() {
        let ev = SceneChangeEvaluator::new(0.0);
        assert!(ev.threshold >= 0.001);
        let ev = SceneChangeEvaluator::new(1.5);
        assert!(ev.threshold <= 0.999);
    }

    #[test]
    fn test_composite_any() {
        let inner = vec![
            Box::new(IntervalEvaluator::new(10.0, (1, 30))) as Box<dyn RuleEvaluator>,
            Box::new(SceneChangeEvaluator::new(0.3)),
        ];
        let mut ev = CompositeEvaluator::new(CompositeOperator::Any, inner);
        assert!(ev.should_extract(&make_frame(0, 0, None)));
        assert!(ev.should_extract(&make_frame(5, 1, Some(0.9))));
    }

    #[test]
    fn test_composite_all() {
        let inner = vec![
            Box::new(SceneChangeEvaluator::new(0.3)) as Box<dyn RuleEvaluator>,
            Box::new(SceneChangeEvaluator::new(0.5)),
        ];
        let mut ev = CompositeEvaluator::new(CompositeOperator::All, inner);
        assert!(!ev.should_extract(&make_frame(0, 0, Some(0.4))));
        assert!(ev.should_extract(&make_frame(1, 1, Some(0.6))));
    }

    #[test]
    fn test_composite_all_with_interval() {
        let inner = vec![
            Box::new(IntervalEvaluator::new(1.0, (1, 30))) as Box<dyn RuleEvaluator>,
            Box::new(SceneChangeEvaluator::new(0.3)),
        ];
        let mut ev = CompositeEvaluator::new(CompositeOperator::All, inner);
        assert!(ev.should_extract(&make_frame(0, 0, Some(0.5))));
        assert!(!ev.should_extract(&make_frame(15, 1, Some(0.5))));
    }

    #[test]
    fn test_rate_limited_passes_within_limit() {
        let inner = Box::new(IntervalEvaluator::new(0.0, (1, 30)));
        let mut ev = RateLimitedEvaluator::new(inner, 60);
        let fps_30 = (0..30).map(|i| make_frame(i as i64, i as u64, None));
        let extracted: Vec<_> = fps_30.filter(|f| ev.should_extract(f)).collect();
        assert_eq!(extracted.len(), 30, "Should extract all frames within rate limit");
    }

    #[test]
    fn test_interval_description() {
        let ev = IntervalEvaluator::new(5.0, (1, 30));
        assert_eq!(ev.description(), "interval/5.0s");
    }

    #[test]
    fn test_scene_change_description() {
        let ev = SceneChangeEvaluator::new(0.42);
        assert_eq!(ev.description(), "scene-change/0.42");
    }

    #[test]
    fn test_fps_description() {
        let cfg = RuleConfig::Fps { fps: 15.0 };
        assert!(cfg.description().contains("fps"));
    }

    #[test]
    fn test_composite_description() {
        let cfg = RuleConfig::Composite {
            operator: CompositeOperator::Any,
            rules: vec![
                RuleConfig::Interval { interval_seconds: 1.0 },
                RuleConfig::SceneChange { threshold: 0.5 },
            ],
        };
        let desc = cfg.description();
        assert!(desc.contains("composite:any"));
        assert!(desc.contains("interval"));
        assert!(desc.contains("scene-change"));
    }

    #[test]
    fn test_rule_engine_evaluate() {
        let rules = vec![
            RuleConfig::Interval { interval_seconds: 1.0 },
            RuleConfig::SceneChange { threshold: 0.3 },
        ];
        let mut engine = RuleEngine::new(&rules, (1, 30));
        assert!(engine.evaluate(&make_frame(0, 0, Some(0.1))));
        assert!(!engine.evaluate(&make_frame(15, 1, None)));
        assert!(engine.evaluate(&make_frame(30, 2, None)));
        assert!(engine.evaluate(&make_frame(45, 3, Some(0.5))));
    }

    #[test]
    fn test_rule_engine_rebuild() {
        let rules = vec![RuleConfig::Interval { interval_seconds: 0.5 }];
        let mut engine = RuleEngine::new(&rules, (1, 30));
        assert!(engine.evaluate(&make_frame(0, 0, None)));
        assert!(!engine.evaluate(&make_frame(7, 1, None)));

        let new_rules = vec![RuleConfig::Interval { interval_seconds: 0.1 }];
        engine.rebuild(&new_rules, (1, 30));
        assert!(engine.evaluate(&make_frame(15, 2, None)));
    }

    #[test]
    fn test_has_scene_change_rule() {
        let no_scd = vec![RuleConfig::Interval { interval_seconds: 1.0 }];
        assert!(!has_scene_change_rule(&no_scd));

        let with_scd = vec![RuleConfig::SceneChange { threshold: 0.3 }];
        assert!(has_scene_change_rule(&with_scd));

        let nested = vec![RuleConfig::Composite {
            operator: CompositeOperator::Any,
            rules: vec![RuleConfig::SceneChange { threshold: 0.3 }],
        }];
        assert!(has_scene_change_rule(&nested));
    }

    #[test]
    fn test_rule_config_serde_roundtrip() {
        let configs = vec![
            RuleConfig::Interval { interval_seconds: 1.5 },
            RuleConfig::Fps { fps: 10.0 },
            RuleConfig::SceneChange { threshold: 0.4 },
            RuleConfig::RateLimited {
                rule: Box::new(RuleConfig::Interval { interval_seconds: 2.0 }),
                max_per_minute: 30,
            },
            RuleConfig::Composite {
                operator: CompositeOperator::Any,
                rules: vec![
                    RuleConfig::Interval { interval_seconds: 5.0 },
                    RuleConfig::SceneChange { threshold: 0.5 },
                ],
            },
            RuleConfig::StaticFrame {
                threshold: 0.05,
                method: ComparisonMethod::PixelDiff,
                force: false,
            },
            RuleConfig::StaticFrame {
                threshold: 0.15,
                method: ComparisonMethod::PerceptualHash,
                force: true,
            },
        ];
        let json = serde_json::to_string(&configs).unwrap();
        let deserialized: Vec<RuleConfig> = serde_json::from_str(&json).unwrap();
        assert_eq!(configs.len(), deserialized.len());
        for (a, b) in configs.iter().zip(deserialized.iter()) {
            assert_eq!(a.description(), b.description());
        }
    }

    #[test]
    fn test_static_frame_blocks_static() {
        let mut ev = StaticFrameEvaluator::new(0.05, ComparisonMethod::PixelDiff, false);
        let mut frame = make_frame(0, 0, None);
        frame.static_frame_score = Some(true);
        assert!(!ev.should_extract(&frame));
    }

    #[test]
    fn test_static_frame_passes_changed() {
        let mut ev = StaticFrameEvaluator::new(0.05, ComparisonMethod::PixelDiff, false);
        let mut frame = make_frame(0, 0, None);
        frame.static_frame_score = Some(false);
        assert!(ev.should_extract(&frame));
    }

    #[test]
    fn test_static_frame_default_none() {
        let mut ev = StaticFrameEvaluator::new(0.05, ComparisonMethod::PixelDiff, false);
        let frame = make_frame(0, 0, None);
        assert!(ev.should_extract(&frame));
    }

    #[test]
    fn test_static_frame_force_overrides() {
        let mut ev = StaticFrameEvaluator::new(0.05, ComparisonMethod::PixelDiff, true);
        let mut frame = make_frame(0, 0, None);
        frame.static_frame_score = Some(true);
        assert!(ev.should_extract(&frame));
    }

    #[test]
    fn test_has_static_frame_rule() {
        let no_static = vec![RuleConfig::Interval { interval_seconds: 1.0 }];
        assert!(!has_static_frame_rule(&no_static));

        let with_static = vec![RuleConfig::StaticFrame {
            threshold: 0.05, method: ComparisonMethod::PixelDiff, force: false,
        }];
        assert!(has_static_frame_rule(&with_static));

        let nested = vec![RuleConfig::Composite {
            operator: CompositeOperator::Any,
            rules: vec![RuleConfig::StaticFrame {
                threshold: 0.05, method: ComparisonMethod::PixelDiff, force: false,
            }],
        }];
        assert!(has_static_frame_rule(&nested));
    }

    #[test]
    fn test_static_frame_description() {
        let ev = StaticFrameEvaluator::new(0.05, ComparisonMethod::PixelDiff, false);
        assert!(ev.description().contains("static-frame"));
    }

    #[test]
    fn test_time_base_zero_does_not_panic() {
        let ev = IntervalEvaluator::new(1.0, (0, 1));
        assert_eq!(ev.interval_pts, 0);
    }
}
