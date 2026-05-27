# Plan 09-03: 性能基准测试与调优

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 CPU 亲和性绑定、基准测试工具、性能调优和生产部署性能文档

**Architecture:** 平台相关的线程绑定模块 (`pipeline/pin.rs`) + Pipeline 原子计数器 + 基准测试 CLI (`--benchmark` 模式)。解码线程创建后立即 pin 到核心，benchmark 模式用 FFmpeg lavfi 合成源模拟 N 路流，通过 Arc<AtomicU64> 收集管线统计。

**Tech Stack:** libc (Linux sched_setaffinity), FFmpeg lavfi (testsrc2), /proc/stat, /proc/self/status

---

### Task 1: CPU 亲和性绑定

**Files:**
- Create: `src/pipeline/pin.rs`
- Modify: `Cargo.toml`, `src/pipeline/mod.rs`, `src/config.rs`

- [ ] **Step 1: 添加 libc 依赖**

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libc = "0.2"
```

- [ ] **Step 2: 创建 pin.rs**

```rust
#[cfg(target_os = "linux")]
pub fn pin_current_thread(core_id: usize) {
    use std::mem;
    unsafe {
        let mut cpuset: libc::cpu_set_t = mem::zeroed();
        libc::CPU_SET(core_id, &mut cpuset);
        libc::pthread_setaffinity_np(libc::pthread_self(), mem::size_of::<libc::cpu_set_t>(), &cpuset);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread(_core_id: usize) {}

pub fn parse_cpu_cores(s: &str) -> Vec<usize> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut cores = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start.parse().unwrap_or(0);
            let end: usize = end.parse().unwrap_or(start);
            cores.extend(start..=end);
        } else if let Ok(c) = part.parse() {
            cores.push(c);
        }
    }
    cores.sort();
    cores.dedup();
    cores
}
```

- [ ] **Step 3: 在 config.rs 的 WorkerConfig 添加 cpu_cores**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_claim_batch_size")]
    pub claim_batch_size: u32,
    #[serde(default = "default_claim_timeout")]
    pub claim_timeout_secs: u64,
    #[serde(default)]
    pub cpu_cores: String,
}
```

- [ ] **Step 4: 修改 Pipeline::start() 接受 core_id 参数**

在 `src/pipeline/mod.rs` 中，修改 `Pipeline::start()` 签名，在解码线程创建后调用 `pin_current_thread`：

```rust
pub fn start(
    config: &StreamConfig,
    stream_id: StreamId,
    shutdown_token: CancellationToken,
    health: Arc<Mutex<StreamHealth>>,
    rules: Arc<RwLock<Vec<RuleConfig>>>,
    core_id: Option<usize>,
) -> Self {
    // ... existing channel creation ...

    let decode_handle = Some(std::thread::spawn(move || {
        if let Some(cid) = core_id {
            pin::pin_current_thread(cid);
        }
        // ... existing decode loop body ...
    }));

    // ... rest of existing Pipeline::start() ...
}
```

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: `Compiling getframe-worker ...` → `Finished`

---

### Task 2: StreamManager 核心分配 + start_pipeline 传递 core_id

**Files:**
- Modify: `src/stream/mod.rs`

- [ ] **Step 1: StreamManager 添加 stream_counter + start_pipeline 传递 core_id**

在 `StreamManager` struct 添加 `stream_counter: Arc<AtomicUsize>`，在 `new()` 中初始化：

```rust
// In StreamManager struct:
pub struct StreamManager {
    pub registry: Arc<StreamRegistry>,
    pub config: Arc<RwLock<AppConfig>>,
    stream_counter: Arc<AtomicUsize>,
}

// In new():
stream_counter: Arc::new(AtomicUsize::new(0)),
```

在 `start_pipeline` 方法中，读取 `cpu_cores` 配置并 round-robin 分配核心：

```rust
pub fn start_pipeline(&self, id: &StreamId) -> bool {
    let info = match self.registry.get(id) {
        Some(info) => info,
        None => return false,
    };

    let cpu_cores_str = std::env::var("GETFRAME_CPU_CORES").unwrap_or_default();
    let cpu_cores = crate::pipeline::pin::parse_cpu_cores(&cpu_cores_str);

    let core_to_pin = if !cpu_cores.is_empty() {
        let idx = self.stream_counter.fetch_add(1, Ordering::Relaxed);
        Some(cpu_cores[idx % cpu_cores.len()])
    } else {
        None
    };

    // ... existing shutdown_token, health_handle, rules setup ...

    let mut pipeline = pipeline::Pipeline::start(
        &info.config, *id, shutdown_token.clone(),
        health_handle.clone(), rules_shared.clone(),
        core_to_pin,
    );

    // ... existing spawn(move { ... }) and registry update ...
}
```

同时更新 `add_stream` 和 `spawn_reconnection_task` 中的 `Pipeline::start` 调用，传递 `core_id = None`。

- [ ] **Step 2: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: `Finished`

---

### Task 3: Pipeline 原子计数器

**Files:**
- Modify: `src/pipeline/mod.rs`

在 Pipeline decode 循环中添加 `AtomicU64` 计数器，benchmark 模式通过 `Arc<AtomicU64>` 共享，生产模式创建计数器但无外部读取。

- [ ] **Step 1: Pipeline struct 添加计数器字段**

```rust
pub struct Pipeline {
    // ... existing fields ...
    pub frames_decoded: Arc<AtomicU64>,
    pub frames_extracted: Arc<AtomicU64>,
}
```

在 `Pipeline::start()` 中初始化：

```rust
let frames_decoded = Arc::new(AtomicU64::new(0));
let frames_extracted = Arc::new(AtomicU64::new(0));

let fd = frames_decoded.clone();
let fe = frames_extracted.clone();

let decode_handle = Some(std::thread::spawn(move || {
    // ... pin if needed ...
    loop {
        // ... decoder.receive() ...
        fd.fetch_add(1, Ordering::Relaxed);
        // ... process frame ...
    }
}));

// In extract callback or after extract
// frames_extracted.fetch_add(1, Ordering::Relaxed);

Self {
    // ... existing fields ...
    frames_decoded,
    frames_extracted,
}
```

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: `Finished`

---

### Task 4: 统计采样器

**Files:**
- Create: `src/benchmark/mod.rs`

- [ ] **Step 1: 创建 benchmark 模块**

写入 `src/benchmark/mod.rs`：

```rust
use std::time::Instant;

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
        if let (Some(prev), Some(pt)) = (self.prev_cpu.as_ref(), self.prev_time) {
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
        let content = std::fs::read_to_string("/proc/self/status").ok()?;
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
```

Run: `New-Item -ItemType Directory -Path "src\benchmark" -Force`

---

### Task 5: 合成视频源 + Benchmark 运行器

**Files:**
- Create: `src/benchmark/synthetic.rs`
- Modify: `src/benchmark/mod.rs`

- [ ] **Step 1: 创建 synthetic.rs**

```rust
use crate::config::StreamConfig;

pub fn create_synthetic_config(index: usize, jpeg_quality: u8) -> StreamConfig {
    StreamConfig {
        name: format!("benchmark-stream-{}", index),
        description: String::new(),
        tags: std::collections::HashMap::new(),
        source_url: "lavfi://testsrc2=size=1920x1080:rate=30:duration=99999".into(),
        source_type: "lavfi".into(),
        stream_type: Some("benchmark".into()),
        extract_interval_seconds: 1.0,
        jpeg_quality,
        ffmpeg_threads: 1,
        rtsp_transport: "tcp".into(),
        storage: None,
        kafka: None,
    }
}
```

- [ ] **Step 2: 在 benchmark/mod.rs 末尾添加 BenchmarkRunner**

```rust
mod synthetic;

use crate::pipeline;
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::types::StreamId;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;

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
```

---

### Task 6: 集成 --benchmark CLI

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: lib.rs 注册 benchmark 模块**

在 `src/lib.rs` 的模块声明中添加 `pub mod benchmark;`。

- [ ] **Step 2: main.rs 添加 --benchmark 分支**

```rust
#[derive(Parser, Debug)]
#[command(name = "getframe-worker")]
struct Cli {
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    #[arg(long)]
    benchmark: bool,

    #[arg(long, default_value = "10")]
    streams: usize,

    #[arg(long, default_value_t = 30.0)]
    duration: f64,

    #[arg(long, default_value_t = 85)]
    jpeg_quality: u8,

    #[arg(long, default_value = "")]
    cpu_cores: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.benchmark {
        let cpu_cores = crate::pipeline::pin::parse_cpu_cores(&cli.cpu_cores);
        let report = crate::benchmark::run_benchmark(
            cli.streams, cli.duration, cli.jpeg_quality, &cpu_cores,
        );
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // ... existing main() body ...
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: `Finished`

---

### Task 7: 性能文档

**Files:**
- Create: `.planning/PERFORMANCE.md`

- [ ] **Step 1: 写入 PERFORMANCE.md 模板**

```markdown
# GetFrame 性能基线

## 硬件平台

| 项目 | 值 |
|------|-----|
| CPU | （待填写） |
| 核心数 | （待填写） |
| 内存 | （待填写） |
| OS | （待填写） |

## 基准测试命令

```bash
cargo run --release -- --benchmark --streams 1 --duration 30
cargo run --release -- --benchmark --streams 10 --duration 30
cargo run --release -- --benchmark --streams 50 --duration 60
cargo run --release -- --benchmark --streams 100 --duration 60
cargo run --release -- --benchmark --streams 50 --duration 60 --cpu-cores "0-7"
```

## 测试结果

### 单流基线

| 指标 | 值 |
|------|-----|
| 解码 FPS | （待填写） |
| CPU 使用率 | （待填写） |
| 内存 | （待填写） |

### 多流扩展

| 流数 | 总解码 FPS | CPU% | 内存/流 | 备注 |
|------|-----------|------|---------|------|
| 1 | | | | |
| 10 | | | | |
| 50 | | | | |
| 100 | | | | |
| 200 | | | | |

### CPU 亲和性对比

| 配置 | 50 流总 FPS | CPU% | 差异 |
|------|------------|------|------|
| 不 pin | | | 基线 |
| pin 到 8 核 | | | |

## 参数调优对比

| 参数 | 默认值 | 测试值 | FPS 变化 | CPU 变化 | 结论 |
|------|--------|--------|---------|---------|------|
| ffmpeg_threads | 1 | 0 | | | |
| jpeg_quality | 85 | 70 | | | |
| jpeg_quality | 85 | 75 | | | |

## 已知瓶颈

（待运行基准测试后填写）

## 生产建议

（待运行基准测试后填写）
```
