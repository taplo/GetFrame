mod synthetic;

use crate::pipeline;
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::types::StreamId;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct CpuStats {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuStats {
    pub fn read() -> Option<CpuStats> {
        let content = std::fs::read_to_string("/proc/stat").ok()?;
        let line = content.lines().next()?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 { return None; }
        Some(CpuStats {
            user: parts[1].parse().unwrap_or(0),
            nice: parts[2].parse().unwrap_or(0),
            system: parts[3].parse().unwrap_or(0),
            idle: parts[4].parse().unwrap_or(0),
            iowait: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
            irq: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
            softirq: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0),
            steal: parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }

    pub fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle
            + self.iowait + self.irq + self.softirq + self.steal
    }
}

pub struct Sampler {
    prev_cpu: Option<CpuStats>,
    prev_time: Option<Instant>,
    pub cpu_samples: Vec<f64>,
}

impl Sampler {
    pub fn new() -> Self {
        Self { prev_cpu: None, prev_time: None, cpu_samples: Vec::new() }
    }

    pub fn sample(&mut self) {
        let now = Instant::now();
        let curr = CpuStats::read();
        if let (Some(prev), Some(_pt)) = (self.prev_cpu.as_ref(), self.prev_time) {
            if let Some(cc) = curr.as_ref() {
                let total_delta = cc.total() - prev.total();
                let idle_delta = cc.idle - prev.idle;
                if total_delta > 0 {
                    let usage = 100.0 * (1.0 - idle_delta as f64 / total_delta as f64);
                    self.cpu_samples.push(usage);
                }
            }
        }
        self.prev_cpu = curr;
        self.prev_time = Some(now);
    }

    pub fn avg_cpu(&self) -> f64 {
        if self.cpu_samples.is_empty() { return 0.0; }
        self.cpu_samples.iter().sum::<f64>() / self.cpu_samples.len() as f64
    }

    pub fn memory_mb() -> u64 {
        let content = match std::fs::read_to_string("/proc/self/status") {
            Ok(c) => c,
            Err(_) => return 0,
        };
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return parts[1].parse().unwrap_or(0);
                }
            }
        }
        0
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkReport {
    pub streams: usize,
    pub duration_secs: f64,
    pub cpu_cores: Vec<usize>,
    pub total_frames_decoded: u64,
    pub total_frames_extracted: u64,
    pub decode_fps: f64,
    pub avg_cpu_pct: f64,
    pub max_memory_mb: u64,
    pub memory_per_stream_mb: f64,
}

pub fn run_benchmark(
    num_streams: usize,
    duration_secs: f64,
    jpeg_quality: u8,
    cpu_cores: &[usize],
) -> BenchmarkReport {
    let duration = std::time::Duration::from_secs_f64(duration_secs);
    let start = std::time::Instant::now();
    let shutdown = CancellationToken::new();

    let mut pipelines = Vec::with_capacity(num_streams);
    for i in 0..num_streams {
        let config = synthetic::create_synthetic_config(i, jpeg_quality);
        let sid = StreamId::new_v4();
        let health = Arc::new(Mutex::new(StreamHealth::new()));
        let rules = Arc::new(RwLock::new(vec![
            RuleConfig::Interval { interval_seconds: config.extract_interval_seconds }
        ]));
        let core_id = if !cpu_cores.is_empty() {
            Some(cpu_cores[i % cpu_cores.len()])
        } else {
            None
        };
        pipelines.push(pipeline::Pipeline::start(
            &config, sid, shutdown.clone(),
            health, rules, core_id,
        ));
    }

    let mut sampler = Sampler::new();
    let sample_interval = std::time::Duration::from_secs(1);
    let mut next_sample = start + sample_interval;

    while start.elapsed() < duration {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let now = Instant::now();
        if now >= next_sample {
            sampler.sample();
            next_sample = now + sample_interval;
        }
    }

    shutdown.cancel();

    let elapsed = start.elapsed().as_secs_f64();
    let max_memory = Sampler::memory_mb();
    let total_decoded: u64 = pipelines.iter().map(|p| p.frames_decoded.load(Ordering::Relaxed)).sum();
    let total_extracted: u64 = pipelines.iter().map(|p| p.frames_extracted.load(Ordering::Relaxed)).sum();

    BenchmarkReport {
        streams: num_streams,
        duration_secs: elapsed,
        cpu_cores: cpu_cores.to_vec(),
        total_frames_decoded: total_decoded,
        total_frames_extracted: total_extracted,
        decode_fps: total_decoded as f64 / elapsed,
        avg_cpu_pct: sampler.avg_cpu(),
        max_memory_mb: max_memory,
        memory_per_stream_mb: if num_streams > 0 { max_memory as f64 / num_streams as f64 } else { 0.0 },
    }
}
