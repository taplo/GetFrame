# Plan 09-03: 性能基准测试与调优

**Date:** 2026-05-27
**Status:** Draft
**Phase:** 9 (Worker Scaling — 200+/1000+ Streams)

## Goals

1. **CPU 亲和性绑定** — 解码线程 pin 到特定 CPU 核心，减少上下文切换
2. **基准测试工具** — `--benchmark` 模式，用合成视频模拟 N 路流，测量吞吐/CPU/内存
3. **性能调优** — 基于基线数据优化关键参数
4. **文档化** — `PERFORMANCE.md` 记录基线、瓶颈和调优参数

## 1. CPU 亲和性绑定

### 修改点

- 新增配置: `worker.cpu_cores: String`（如 `"0-3,8-11"`），空字符串表示不 pin
- 在 `pipeline::Pipeline::start()` 中，创建解码线程后立即 pin 到核心
- 实现: 平台相关，Linux 用 `libc::sched_setaffinity`

### 核心分配策略

```
pipeline_core_id = stream_index % available_cores
```

其中 `stream_index` 是 StreamManager 中的自增计数器（新增字段），`available_cores` 是解析 `cpu_cores` 配置得到的核心列表长度。

### 代码变更

**新增依赖:**
- `libc` (Linux only, 可选 feature)

**修改文件:**
- `Cargo.toml` — 添加 `[target.'cfg(target_os="linux")'.dependencies] libc = "0.2"`
- `src/config.rs` — `WorkerConfig` 添加 `cpu_cores: String`
- `src/pipeline/mod.rs` — `Pipeline::start()` 中调用 `pin_thread_to_core()`
- `src/pipeline/pin.rs` (新文件) — 平台相关的线程绑定函数

```rust
// src/pipeline/pin.rs
#[cfg(target_os = "linux")]
pub fn pin_current_thread(core_id: usize) {
    // libc::sched_setaffinity
}

#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread(_core_id: usize) {
    // no-op
}
```

## 2. 基准测试工具

### 设计

`getframe-worker --benchmark` 模式：

1. 用 FFmpeg `lavfi` 生成合成 H.264 测试源
2. 启动 N 个并发的解码 pipeline
3. 运行 T 秒
4. 收集性能数据
5. 输出 JSON 报告

### 合成视频源

使用 FFmpeg 内置的 `testsrc2` filter 生成 1920x1080 30fps H.264 视频：

```
lavfi://testsrc2=size=1920x1080:rate=30:duration=99999
```

这个源会持续生成测试视频，包含色彩条和计数器，用于帧验证。

### 命令行接口

```
getframe-worker --benchmark [OPTIONS]

Options:
  -n, --streams <N>        并发流数量 [default: 10]
  -d, --duration <SEC>     测试持续时间（秒）[default: 30]
      --jpeg-quality <Q>   JPEG 质量 [default: 85]
      --cpu-cores <LIST>   核心列表，如 "0-3" [default: ""]
      --json               输出 JSON 格式报告
      --no-storage         跳过 MinIO 上传（仅解码+规则评估）
      --no-kafka           跳过 Kafka 发送
```

### 输出

```
=== Benchmark Results ===
Streams:          10
Duration:         30.0s
CPU cores:        0-7 (8 cores)

Total frames decoded:    14,832
Total frames extracted:    495
Decode FPS (aggregate):  494.4
Avg decode latency:      12.3ms
P99 decode latency:      28.1ms
CPU usage (avg):        67.3%
Memory (max):           1,234 MB
Memory per stream:      123.4 MB
Frame extraction rate:  16.5 fps
```

JSON 模式:
```json
{
  "streams": 10,
  "duration_sec": 30.0,
  "cpu_cores": [0,1,2,3,4,5,6,7],
  "total_frames_decoded": 14832,
  "total_frames_extracted": 495,
  "decode_fps": 494.4,
  "avg_decode_latency_ms": 12.3,
  "p99_decode_latency_ms": 28.1,
  "avg_cpu_pct": 67.3,
  "max_memory_mb": 1234,
  "memory_per_stream_mb": 123.4
}
```

### 实现方案

在 `src/benchmark/mod.rs` 中实现：

```rust
pub struct BenchmarkRunner {
    num_streams: usize,
    duration: Duration,
    jpeg_quality: u8,
    cpu_cores: Vec<usize>,
    enable_storage: bool,
    enable_kafka: bool,
}

impl BenchmarkRunner {
    pub fn run(&self) -> Result<BenchmarkReport>;
}
```

启动 N 个 `pipeline::Pipeline` 实例（使用 lavfi URL），每个在独立线程中运行，通过 channel 收集统计信息。运行时采样 `/proc/self/stat` 获取 CPU/内存。

### 新增文件

- `src/benchmark/mod.rs` — BenchmarkRunner + BenchmarkReport
- `src/benchmark/stats.rs` — CPU/内存采样器
- `src/benchmark/synthetic.rs` — 合成视频源 URL 生成

## 3. 性能调优清单

基于基准测试结果可调整的参数：

| 优先级 | 参数 | 默认值 | 调整方向 | 预期效果 |
|--------|------|--------|----------|----------|
| P0 | `ffmpeg_threads` | 1 | 0 (auto) | 多核解码加速，但可能增加竞争 |
| P0 | CHANNEL_BUFFER_DECODE | 8 | 4-16 | 影响反压强度 |
| P1 | `jpeg_quality` | 85 | 70-80 | 减少 JPEG 编码时间 20-40% |
| P2 | `max_backoff_seconds` | 30 | 10-60 | 重连频率 |
| P2 | `claim_batch_size` | 5 | 10-20 | 水平扩展时抢占效率 |

调优策略：每次改变一个参数，运行基准测试，记录结果。对比前一次基线。

## 4. 文档

创建 `.planning/PERFORMANCE.md`，包含：

1. **硬件平台** — CPU 型号、核心数、内存、OS 版本
2. **基准测试命令** — 完整可复现命令
3. **单流基线** — 1 路流的各项指标
4. **多流扩展** — 10/50/100/200 路流的数据
5. **CPU 亲和性对比** — pin vs 不 pin 的性能差异
6. **参数调优对比** — 各参数调整前后的效果
7. **已知瓶颈** — 当前最受限的资源
8. **生产建议** — 推荐的每个核心流数、内存配置

## 实现边界

### 不在此计划中的内容

- 实际 MinIO/Kafka 集成测试（需外部依赖）
- Docker 性能测试（--benchmark 运行在主机上）
- 自动调优（仅手动调优+文档化）
- GPU/硬件加速（项目范围外）
