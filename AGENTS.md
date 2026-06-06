# GetFrame 项目指令

## 项目简介

高性能视频抽帧平台，纯 CPU 处理 200-1000+ 路并发 1080P H.264 视频流，支持多源接入（RTSP/RTMP/HLS/文件），通过规则引擎抽帧，帧存 MinIO/S3，元数据推送 Kafka。

## 技术栈

- **语言**: Rust (Edition 2024)
- **视频解码**: FFmpeg libavcodec via ffmpeg-next（库模式，非 CLI 进程）
- **JPEG 编码**: turbojpeg 1.4.0（直接 YUV planes 编码，无需 YUV→RGB 转换）
- **Kafka**: rdkafka (librdkafka bindings)
- **HTTP API**: Axum 0.8
- **前端**: React + TypeScript + Vite + shadcn/ui
- **数据库**: MySQL 8.0 + SQLx
- **对象存储**: MinIO / S3（Claim-Check 模式）
- **容器**: Docker 多阶段构建 + distroless
- **K8s**: Deployments + KEDA

## 关键架构决策

1. **混合并发模型**: OS 线程处理 FFmpeg 解码 + tokio async 处理网络 I/O
2. **有界通道反压**: 流水线各阶段间使用 bounded channel 形成反压链
3. **Claim-Check 模式**: 图片存 MinIO/S3，Kafka 只传元数据+S3 链接
4. **核心绑定调度**: 每核 round-robin 调度 6-7 路流（TODO: 实测 200 流在 16 核上，核心绑定导致解码延迟从 104ms→183ms. 当 thread_count ≫ core_count 时，内核调度器自由调度优于静态绑定，待后续优化）
5. **Guaranteed QoS**: `limits.cpu = requests.cpu` + CPU Manager static policy

## 开发与部署环境

### VM (开发/编译, 192.168.3.122)

| 属性 | 值 |
|------|-----|
| **地址** | `192.168.3.122:22` |
| **SSH 用户** | `taplo` |
| **认证方式** | SSH 密钥 (`~/.ssh/id_ed25519`) |
| **OS** | Ubuntu 26.04 LTS (Resolute Raccoon) |
| **Kernel** | Linux 7.0.0-15-generic #15-Ubuntu SMP x86_64 |
| **Hostname** | `taplo-VirtualBox` |
| **CPU** | Intel Xeon E5-2680 @ 2.70GHz (8 cores) |
| **Memory** | 7.2 GiB |
| **Disk** | /dev/sda2 99G (53G used, 57%) |
| **Docker** | v29.5.2, Compose v5.1.4 |
| **FFmpeg** | libavcodec59 (仅 Docker 容器内可用) |
| **时区** | Asia/Shanghai |
| **项目路径** | `/home/taplo/getframe` |

### VM (压测服务器, 192.168.3.123)

| 属性 | 值 |
|------|-----|
| **地址** | `192.168.3.123:22` |
| **SSH 用户** | `taplo` |
| **认证方式** | SSH 密钥 (`~/.ssh/id_ed25519`) |
| **OS** | Ubuntu 26.04 LTS (Resolute Raccoon) |
| **Hostname** | `ubuntuserver` |
| **CPU** | 16 核 (Intel, ~2.70GHz) |
| **Memory** | ~15 GiB |
| **Docker** | v29.1.3, Compose v2.32.4 |
| **项目路径** | `/home/taplo/getframe` |
| **sudo 密码** | `rake.t.wang` |
| **网络** | 与 .26 互通（IP 直接可达） |
| **代理** | Docker daemon 代理: `https://192.168.3.208:8787`, SOCKS: `192.168.3.208:8888` |
| **本地保留镜像** | `rust:1.91-slim-bookworm`、`mysql:8.0`、`minio/minio`、`bluenviron/mediamtx`、`linuxserver/ffmpeg`、`confluentinc/cp-kafka`、`debian:bookworm-slim`、`alpine:latest` |

### Docker 构建注意事项

- Cargo / Rust 工具链 **仅 Docker 内可用**，VM 宿主机无 rust/cargo
- `.rs` 文件改动后必须 SCP 同步到 VM 再编译（git 仓库不同步, .31 无 git sync）
- Docker Hub 和 `deb.debian.org` 现在通过 daemon 代理可达，apt 正常
- 编译 release 二进制推荐使用持久容器方式（避免重复 apt install）：
  ```
  # 启动编译容器（仅首次需 apt install）
  docker run -d --name getframe-compile --network host \
    -v /home/taplo/getframe:/app -w /app \
    -e RUSTFLAGS='--cfg tokio_unstable' \
    rust:1.91-slim-bookworm \
    sh -c 'apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      nasm pkg-config libavcodec-dev libavformat-dev libavutil-dev libswscale-dev \
      libavdevice-dev libavfilter-dev cmake make g++ libssl-dev libcurl4-openssl-dev \
      clang libclang-dev && tail -f /dev/null'

  # 重新编译（容器保持运行，跳过 apt）
  docker exec getframe-compile cargo build --release --bin getframe-worker

  # 复制二进制
  docker cp getframe-compile:/app/target/release/getframe-worker \
    /home/taplo/getframe/getframe-worker-new
  ```
- 构建精简 Docker 镜像（从 prebuilt binary）：
  ```
  # 写 Dockerfile.quick 后 scp 到 .31
  docker build -t getframe-worker-tmp:latest /tmp/docker-build/
  ```

### 文件同步工作流

```bash
# 从 Windows 本地同步变更到 VM (192.168.3.122)
scp Cargo.toml Cargo.lock taplo@192.168.3.122:/home/taplo/getframe/
scp -r src/ taplo@192.168.3.122:/home/taplo/getframe/
scp -r migrations/ taplo@192.168.3.122:/home/taplo/getframe/
scp config.docker.yaml docker-compose.yml config.example.yaml taplo@192.168.3.122:/home/taplo/getframe/

# VM 上构建（或使用持久容器加速）
ssh taplo@192.168.3.122 'cd /home/taplo/getframe && docker buildx build --network host -t getframe-worker:latest .'

# 同步到压测服务器 (192.168.3.123)
scp src/pipeline/decode.rs taplo@192.168.3.123:/home/taplo/getframe/src/pipeline/decode.rs

# 压测服务器上执行
ssh taplo@192.168.3.123 'cd /home/taplo/getframe/benchmark && WORKER_IMAGE=getframe-worker-tmp:latest python3 run.py'
```

## 已知 Bug 与修复索引

### Fix 5 — Health 更新频率（2026-05-31）
- **文件**: `src/pipeline/decode.rs:173-181`
- **问题**: health_handle 每 30 帧才更新一次，低帧率(1fps) 下 actual_fps 无法在 5s 采样间隔内正确测量
- **修复**: 改为每帧更新 health（移除 health_counter % 30 的条件判断）
- **效果**: actual_fps 现在在任何帧率下都能准确上报

### Fix 7 — 修正解码耗时仪表化测量点（2026-06-02）
- **文件**: `src/pipeline/decode.rs:81`
- **问题**: `Instant::now()` 的 start 点在 `receive_frame()` 之后才设置，只测量了 `avcodec_receive_frame` 的耗时（~4μs），遗漏了 `avcodec_send_packet` 的实际解码计算时间
- **修复**: 移动 `Instant::now()` 到 `send_packet()` 之前，每次输出帧时重置，确保 measure 整个 decode cycle
- **效果**: 解码真实耗时从上报的 4μs 修正为 ~7,200 μs（包含 send_packet + receive_frame），JPEG 占比从 97% 降至 74%
- **额外**: `STAGE_REPORT_INTERVAL` 从 300 降为 100 帧

### Fix 6 — 迁移至 libjpeg-turbo 编码（2026-05-31）
- **文件**: `src/pipeline/encode.rs`, `Cargo.toml`, `Dockerfile`
- **问题**: `image` crate 的 JPEG 编码器 CPU 效率低，为 5fps 每流贡献 55-78% CPU
- **修复**: 替换为 `turbojpeg` crate（libjpeg-turbo 的 Rust 绑定），结合 yuvutils-rs SIMD YUV→RGB
- **关键**: turbojpeg 1.4.0 移除了 `Image::from_slice()`，改用 struct 直接构造 `Image { pixels, width, pitch, height, format }`
- **效果**: 编码 CPU 开销降低 ~65%（5fps 每流从 55% 降至 19%），5fps 吞吐从 53→86 fps（+62%）

### Fix 8 — YUV Planes 直接编码（2026-06-02）
- **文件**: `src/pipeline/encode.rs`, `Cargo.toml`
- **问题**: turbojpeg 1.3.3 的 RGB 编码路径需要 yuvutils-rs SIMD 做 YUV→RGB 转换（700μs/frame），多一次内存拷贝和 CPU 开销
- **修复**: 
  - 升级 turbojpeg 1.3.3→1.4.0（新增 `compress_yuv_planes` API 和 `YuvPlanesImage` 类型）
  - 移除 yuvutils-rs 的 `yuv_to_rgb` 转换，直接传递 FFmpeg 解码后的 Y/U/V 平面给 turbojpeg
  - 移除平面拼接（原 I420 方案中的 concat 步骤），Y/U/V 保持独立内存区域通过指针传递
- **关键**: turbojpeg 1.4.0 的 `Compressor::compress_yuv_planes()` 接受 `YuvPlanesImage<&[u8]>` 结构体，包含独立的 Y/U/V 平面指针和各自的 stride，直接映射到 FFmpeg 的 `AVFrame` 数据布局
- **效果**: 
  - 消除 ~700μs/frame 的 YUV→RGB 转换（5fps 约 3.5% CPU/流）
  - 消除 ~6MB/frame 的 RGB 临时缓冲区分配（1920×1080×3）
  - yuvutils-rs 依赖已从 Cargo.toml 中移除

### Fix 10 — RTSP connect 30s 超时兜底（2026-06-04）
- **文件**: `src/pipeline/ingest.rs:15-47`
- **问题**: `avformat_open_input`（在 `format::input_with_dictionary` 中）在 RTSP 流未就绪时会阻塞 ~240s。FFmpeg RTSP demuxer 内部会多次重试 DESCRIBE/SETUP/PLAY 序列，而 `stimeout` 仅作用于已建立连接内的 socket 读取，不影响初始握手
- **修复**: 
  - 新增 `open_input_with_timeout()`，使用 `std::sync::mpsc::channel` + `recv_timeout(30s)` 将 `format::input_with_dictionary` 放入独立线程，超时后返回 `Err` 由重连任务重试
  - 仅对 RTSP 源启用（file/HLS 不受影响）
- **效果**: TTFF 从 ~240s 降至 ~15s（1 流），4+ 流降至 0s（即时）
- **连带修复**: `benchmark/run.py:103` 添加 `-g 1` 强制每帧为关键帧，避免连接加速后无关键帧导致的无限等待

### Fix 9 — Cargo.lock UTF-16LE 编码修复（2026-06-03）
- **文件**: `Cargo.lock`
- **问题**: CI 报错 `failed to read file: Cargo.lock` / `stream did not contain valid UTF-8`。本地 Windows 编辑器将 Cargo.lock 保存为 UTF-16LE（`FF FE` BOM），Linux CI 上 Cargo 无法解析
- **修复**: 将 Cargo.lock 从 UTF-16LE 转换为 UTF-8（无 BOM）
- **根因**: `core.autocrlf=true` 的 Windows git 环境在编辑时没有保持原 UTF-8 编码
- **连带修复**: 新增两个 clippy lint 修复（Rust 1.96.0 新 lint）：
  - `src/pipeline/decode.rs:206`: `timing_count % STAGE_REPORT_INTERVAL == 0` → `timing_count.is_multiple_of(STAGE_REPORT_INTERVAL)`（`manual_is_multiple_of`）
  - `src/stream/mod.rs:24`: `spawn_consumer_tasks` 函数添加 `#[allow(clippy::too_many_arguments)]`（`too_many_arguments` 阈值降至 7）

## Benchmark 结果

### 时序流水线优化

**优化内容**（2026-06-01）：将单流的串行 `upload→publish` 拆分为两阶段流水线，通过 bounded `mpsc::channel(32)` + `tokio::sync::Semaphore(4)` 实现上传和发布的并行执行，并消除重连任务中的重复代码。

**结论**：1fps 和 5fps 在所有流数下与基线持平。当前瓶颈不在 I/O（1fps 每流 92% 空闲，5fps 瓶颈在解码/编码），两阶段流水线主要为代码去重和未来准备。



### 5fps 基准（2026-06-04，16核 .31，v0.3.0 完整栈）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 1 | 6.6 | 10.8% | 153 | 10.8% | 0 |
| 2 | 13.2 | 18.9% | 73 | 9.4% | 0 |
| 4 | 26.2 | 37.4% | 131 | 9.4% | 0 |
| 8 | 53.5 | 74.3% | 272 | 9.3% | 0 |
| 12 | 78.6 | 116.5% | 399 | 9.7% | 0 |
| 16 | 97.7 | 141.1% | 488 | 8.8% | 0 |
| 24 | 97.7 | 158.7% | 575 | 6.6% | 0 |
| 32 | 84.8 | 152.8% | 599 | 4.8% | 1 |

> **v0.3.0 5fps 基准完成**: 与 YUV Planes 基线（.31 16核）对比，16 流吞吐从 76.1→97.7 fps（+28%），CPU 从 185%→141%（-24%）。24 流吞吐从 83.1→97.7 fps（+18%），CPU 从 262%→159%（-39%）。32 流首次完成，吞吐峰值约 100fps（16-24 流），之后 I/O 瓶颈显现。32 流时出现少量错误（6 个/180 采样），需关注。

### 流水线阶段耗时分析（1 流 × 5fps，1080p，100 帧平均，固定仪表化）

| 阶段 | 平均 μs | 占比 |
|------|---------|------|
| H.264 Decode (send_packet + receive_frame) | 7,500 μs | 24% |
| scene detect filter | 0 μs | 0% |
| 规则引擎评估 | 0 μs | 0% |
| JPEG 编码 (libjpeg-turbo — YUV planes 直接编码) | 24,000 μs | 76% |
| **每帧合计** | **~31,500 μs** | **100%** |

> **注意**: 此数据使用 Fix 7 修正后的仪表化，正确测量了 `avcodec_send_packet` + `avcodec_receive_frame` 的完整解码周期。旧数据（decode 4μs, JPEG 97%）仅测量了 receive_frame 阶段，严重低估了解码成本。

**核心发现：JPEG 编码占每帧处理时间的 74%，解码占 23%。** 解码不再是可忽略的阶段。7ms 的解码时间部分是因为 testsrc2 生成的帧缺乏帧间预测（每帧都是 I 帧），真实 RTSP 流的解码时间预计 <1ms。优化仍应聚焦 JPEG 编码器。

### 5fps CPU 资源分布估算

| 资源 | 8 流占用 | 16 流占用 | 32 流占用 |
|------|---------|----------|----------|
| 解码流水线 (26ms × fps × 流) | 1.05 核 | 2.1 核 | 4.2 核 |
| S3/Kafka I/O 开销 | 0.45 核 | 1.4 核 | 0.6 核 |
| **总计** | **1.5 核** | **3.5 核** | **4.8 核** |

### 1fps 基准（2026-06-02，16核 .31，RTSP timeout 修复后 — RGB 基线）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 1 | 1.1 | 4.3% | 50 | 4.1% | 0 |
| 2 | 2.4 | 10.3% | 85 | 5.2% | 0 |
| 4 | 4.7 | 22.8% | 156 | 5.7% | 0 |
| 8 | 9.8 | 31.1% | 298 | 3.9% | 0 |
| 12 | 14.8 | 48.7% | 440 | 4.0% | 0 |
| 16 | 18.2 | 64.1% | 550 | 4.0% | 0 |
| 24 | 18.4 | 67.2% | 543 | 2.8% | 0 |
| 32 | 18.8 | 64.9% | 591 | 2.1% | 0 |

> **首次帧等待时间显著改善**（stimeout 5s→30s 修复）: 1 流 245s→32 流 5s（旧版始终 300s+）。TTFF 随流数增加递减，因为更早的存活探测缩短了重试间隔。

### 1fps 基准（2026-06-02，16核 .31，YUV Planes 直接编码）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 1 | 1.2 | 4.5% | 44 | 4.5% | 0 |
| 2 | 2.4 | 8.8% | 74 | 4.4% | 0 |
| 4 | 4.7 | 16.8% | 135 | 4.2% | 0 |
| 8 | 9.3 | 27.4% | 248 | 3.4% | 0 |
| 12 | 14.1 | 37.5% | 376 | 3.1% | 0 |
| 16 | 17.6 | 32.8% | 463 | 2.0% | 0 |
| 24 | 17.6 | 39.0% | 458 | 1.6% | 0 |
| 32 | 17.7 | 35.2% | 510 | 1.1% | 0 |

> **相比 RGB 基线的改进**: CPU/流在高密度下降低 48%（32 流从 2.1%→1.1%），总 CPU 从 64.9%→35.2%（-46%），内存从 591MB→510MB（-14%）。低流数下效果不明显，因为解码+编码空闲 >92%。

### 1fps 基准（2026-05-31，16核 .30，libjpeg-turbo 基线）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 1 | 1.2 | 4.1% | 50 | 4.1% | 0 |
| 2 | 2.4 | 10.3% | 85 | 5.2% | 0 |
| 4 | 4.7 | 22.3% | 156 | 5.6% | 0 |
| 8 | 9.9 | 31.7% | 298 | 4.0% | 0 |
| 12 | 15.1 | 53.5% | 440 | 4.5% | 0 |
| 16 | 18.5 | 67.0% | 551 | 4.2% | 0 |
| 24 | 19.3 | 73.7% | 543 | 3.1% | 0 |
| 32 | 19.7 | 72.1% | 591 | 2.3% | 0 |

### 1fps 基准（2026-06-01，16核 .30，两阶段流水线）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 |
|------|-----------|------|---------|--------|
| 1 | 1.2 | 5.0% | 50 | 5.0% |
| 2 | 2.3 | 7.6% | 85 | 3.8% |
| 4 | 4.7 | 14.9% | 156 | 3.7% |
| 8 | 9.4 | 33.3% | 298 | 4.2% |
| 12 | 14.0 | 45.5% | 442 | 3.8% |
| 16 | 17.8 | 62.2% | 540 | 3.9% |
| 24 | 17.7 | 61.7% | 557 | 2.6% |
| 32 | 17.6 | 67.9% | 609 | 2.1% |

> 与旧版持平 — 1fps 场景下每流 92% 空闲，上传/发布串行化非瓶颈

### 1fps 基准（2026-06-04，16核 .123，v0.3.0 + Fix 10 + claim_batch_size=50）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 32 | 38.9 | 72.2% | 963 | 2.26% | 0 |

### Phase 9: 1fps 大规模扩展基准（2026-06-04，16核 .123，relay mode，v0.3.1 + claim_batch_size=50）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 32 | 38.9 | 72.2% | 963 | 2.26% | 0 |
| 48 | 58.3 | 176.5% | 1,487 | 3.68% | 0 |
| 64 | 79.1 | 216.9% | 1,798 | 3.39% | 0 |
| 96 | 119.0 | 333.7% | 2,542 | 3.47% | 0 |
| 128 | 158.8 | 472.8% | 3,200 | 3.69% | 0 |
| 200¹ | ~154 | 508% | 5,300 | 2.54% | 5-31 |

> **¹200 流**: 前 3 采样帧率正常（152-244 fps），之后出现部分流重连错误（6 采样累计 5→31 错误），内存 5.3GB。128 流以内零错误稳定运行。200 流错误与 MediaMTX 单路径 200+ 订阅者连接限制有关，非 worker 架构瓶颈。
>
> **关键结论**: `claim_batch_size=50` 使 128 流在 15s 内完成抢占（原 5→7 周期 105s），首次帧等待均为即时（0s）。worker 架构支持 200+ 流处理，128 流以下零错误稳定。`docker compose down -v` 修复 MinIO 磁盘满问题（-v 清除匿名卷）。

### 1fps 历史基准（2026-06-02，16核 .31，YUV Planes 直接编码，历史对比用）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 | 错误 |
|------|-----------|------|---------|--------|------|
| 1 | 1.2 | 4.5% | 44 | 4.5% | 0 |
| 2 | 2.4 | 8.8% | 74 | 4.4% | 0 |
| 4 | 4.7 | 16.8% | 135 | 4.2% | 0 |
| 8 | 9.3 | 27.4% | 248 | 3.4% | 0 |
| 12 | 14.1 | 37.5% | 376 | 3.1% | 0 |
| 16 | 17.6 | 32.8% | 463 | 2.0% | 0 |
| 24 | 17.6 | 39.0% | 458 | 1.6% | 0 |
| 32 | 17.7 | 35.2% | 510 | 1.1% | 0 |

### E2E 集成测试套件（2026-06-06）
- **文件**: `tests/e2e/test_full_flow.py`, `tests/e2e/test_auth.py`
- **测试范围（full_flow）**: 14 步端到端测试覆盖完整流水线（含 auth login）：
  1. Compose 堆栈就绪、Worker 健康端点、MinIO 可达、Worker auth login
  2. Kafka topic 存在、启动 ffmpeg RTSP 源 → 通过 API 注册流 → 等待在线和帧解码
  3. 验证帧累积（10s 内增长）、MinIO 中存在帧对象、Kafka 中有元数据消息
  4. 验证 Worker 日志中的流水线定时仪表化
  5. 删除流 → 验证从 API 列表中移除
- **运行**: `cd /home/taplo/getframe && python3 tests/e2e/test_full_flow.py`
- **依赖**: `.123` VM 上运行的 Docker compose（benchmark/compose.yaml），需要 `linuxserver/ffmpeg` 镜像用于 RTSP 源
- **结果**: 14/14 通过（全绿）
- **Auth 测试**: `tests/e2e/test_auth.py` — 10 场景覆盖 JWT 登录/API Key/角色 CRUD/公共端点
- **Auth 运行**: `python3 -m pytest tests/e2e/test_auth.py -v`（需 pip install pytest）
- **Handler 角色检查**: viewer 只读 (GET/LIST)，admin 读写 (POST/PUT/DELETE) — stream/rule/task 全部 handler 覆盖

### Fix 11 — claim_batch_size 从 5→50 用于大规模抢占（2026-06-04）
- **文件**: `src/config.rs:42`, `benchmark/config/config.yaml`, `config.docker.yaml`, `config.example.yaml`, `deploy/helm/getframe/values.yaml`
- **问题**: 默认 `claim_batch_size=5` 导致 128 流需要 7 个心跳周期（~105s）才能完成抢占，200 流需要 40 周期（~600s）
- **修复**: 默认值改为 50（128 流 1 周期 15s，200 流 4 周期 60s）
- **连带修复**: 
  - `benchmark/run.py` 增加 `--scale` 模式，使用单 ffmpeg 源 + MediaMTX 中继，支持 200+ 流基准测试
  - `benchmark/compose.yaml` 增加 ulimit（nofile=65536）、sysctl（net.core.somaxconn=65535）和可选 worker-2
  - `docker compose down -v`（自动清除匿名卷）解决跨迭代 MinIO 磁盘满问题

### Fix 12 — 修复 Helm chart KEDA ScaledObject 模板并完善生产部署（2026-06-04）
- **文件**: `deploy/helm/getframe/templates/keda-scaledobject.yaml`, `deploy/helm/getframe/templates/deployment.yaml`, `deploy/helm/getframe/templates/ingress.yaml`, `deploy/helm/getframe/values.yaml`, `deploy/helm/getframe/Chart.yaml`
- **问题**: 
  - `keda-scaledobject.yaml` 模板结构错误：`kind: ScaledObject` 在条件块外且缺少 `apiVersion`，HPA 分支也缺 `kind`
  - Deployment 不支持环境变量覆盖（无法从 Secret 注入 DB URL 等）
  - 缺少 Ingress 模板用于生产 K8s 暴露 API
- **修复**: 
  - 重写 `keda-scaledobject.yaml`：清晰的 KEDA ↔ HPA 二选一结构，两项均包含完整 apiVersion/kind/metadata/spec
  - `deployment.yaml` 增加 `extraEnv`、`extraVolumes`、`extraVolumeMounts` 支持
  - 新增 `ingress.yaml`（通过 `values.ingress.enabled` 控制）
  - `values.yaml` 新增 `extraEnv/extraVolumes/extraVolumeMounts/ingress` 配置块
  - Chart.yaml 版本同步至 v0.3.1
- **效果**: Helm chart 现在可用于生产部署：`helm install getframe ./deploy/helm/getframe`

### 关键发现

1. **JPEG 编码占 74% 的每帧成本**: 24ms/frame 1080p，解码 7.5ms（23%），YUV 拷贝 0.7ms（2%）
2. **单节点 128 流 1fps 零错误验证通过**: 16 核 .123 上 158.8 fps total，CPU 472.8%（4.7 核），CPU/流 3.69%，内存 3.2GB，错误 0。架构线性扩展至 128 流。
3. **5fps 吞吐峰值 ~100 fps**（16-24 流），CPU 峰值 ~160%（16 核），之后 I/O 瓶颈限制
4. **5fps v0.3.0 相比 YUV Planes 基线**: 16 流 +28% 吞吐（97.7 vs 76.1 fps），-24% CPU，32 流首次完成
5. **瓶颈路径**: Decode (7.5ms) → **JPEG Encode (24ms)** → S3 Upload + Kafka Publish（异步）
6. **内存效率**: 大规模 128 流约 3.2GB（25MB/流），200 流约 5.3GB（26.5MB/流），包含 FFmpeg decoder、libjpeg-turbo encoder、S3 缓冲区
7. **200 流 MediaMTX 瓶颈**: 单 RTSP 路径的 200+ 并发订阅者限制导致部分流重连，非 worker 架构瓶颈
8. **claim_batch_size=50 关键**: 128 流 15s 内完成抢占（vs 原 105s），200 流 60s 内完成
9. **定时仪表化工具**: 每 100 帧输出各阶段平均耗时（`Pipeline timing` 日志），可调 `STAGE_REPORT_INTERVAL`
10. **RTSP 首帧延迟已修复（Fix 10）**: `avformat_open_input` 使用 `std::sync::mpsc::recv_timeout(30s)` 兜底，TTFF 从 ~240s 降至 ~15s（1 流），4+ 流即时
11. **核心绑定退化为负优化（2026-06-05 实测）**: 200 流在 16 核上，`GETFRAME_CPU_CORES=0-15` 使得解码延迟从 104ms→183ms（+76%），CPU 从 600%→975%（+62%）。200 线程数远大于 16 核时，内核调度器的自由调度弹性优于静态核心绑定——线程可以在任意可用核心上运行，避免单核过载。建议当 thread_count ≫ core_count 时不使用核心绑定

### Fix 13 — API 认证（API-06, 2026-06-06）
- **文件**: `src/auth/` (6 files), `src/main.rs`, `src/config.rs`, `Cargo.toml`, `migrations/20260605_000001_api_auth.sql`, `config.*.yaml`, `tests/e2e/test_auth.py`, `tests/e2e/test_full_flow.py`
- **实现**: 
  - JWT Bearer + API Key (gfk_ prefix, SHA-256 hash) 双认证
  - 用户存储在 MySQL: username + argon2 password_hash + role (admin/viewer)
  - `X-API-Key` 头认证（查 api_keys 表）+ `Authorization: Bearer` JWT 认证
  - 7 个 auth handler: 登录、用户 CRUD、API Key CRUD
  - 公共路由白名单：/health, /ready, /metrics, /swagger-ui, /api/v1/auth/login
  - AuthState 中持有 DB pool + JWT secret
  - 初始 admin 用户从 `auth.initial_admin_password` 配置引导
- **Bug 修复**: MySQL TIMESTAMP → DATETIME（sqlx 的 NaiveDateTime 不兼容 TIMESTAMP）
- **测试结果**: Auth 测试 10/10 通过，Full Flow 回归 14/14 通过（含 auth login 步骤）

## 基准测试命令

```bash
# 在 .31 上运行基准测试（1fps，默认）
screen -dmS bench python3 run.py

# 运行 5fps 基准测试
# 先修改 run.py：TARGET_FPS_VALUES = [1] → TARGET_FPS_VALUES = [5]
sed -i 's/TARGET_FPS_VALUES = .*]/TARGET_FPS_VALUES = [5]/' run.py
screen -dmS bench python3 run.py

# .31 上使用自定义镜像运行
screen -dmS bench sh -c 'WORKER_IMAGE=getframe-worker-tmp:latest python3 run.py 2>&1 | tee /tmp/bench-output.log'

# 查看进度
screen -S bench -X hardcopy /tmp/screen.log && tail -20 /tmp/screen.log

# 查看结果
cat /home/taplo/getframe/benchmark/results/benchmark-1fps.csv
cat /home/taplo/getframe/benchmark/results/benchmark-5fps.csv

# 运行大规模扩展基准测试（32-200 流，relay mode）
screen -dmS bench-scale python3 run.py --scale

# 查看结果
cat /home/taplo/getframe/benchmark/results/benchmark-1fps-scale.csv

# 查看 timing 仪表化数据
docker logs getframe-bench-worker 2>&1 | grep 'Pipeline timing'
```

## GSD 工作流

- 使用 `/gsd-plan-phase N` 规划阶段
- 使用 `/gsd-discuss-phase N` 讨论阶段
- 使用 `/gsd-execute-plan` 执行计划
- 使用 `/gsd-transition` 转换阶段
- 文档在 `.planning/` 目录
