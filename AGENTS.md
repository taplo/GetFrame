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
4. **核心绑定调度**: 每核 round-robin 调度 6-7 路流
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

## Benchmark 结果

### 时序流水线优化

**优化内容**（2026-06-01）：将单流的串行 `upload→publish` 拆分为两阶段流水线，通过 bounded `mpsc::channel(32)` + `tokio::sync::Semaphore(4)` 实现上传和发布的并行执行，并消除重连任务中的重复代码。

**结论**：1fps 和 5fps 在所有流数下与基线持平。当前瓶颈不在 I/O（1fps 每流 92% 空闲，5fps 瓶颈在解码/编码），两阶段流水线主要为代码去重和未来准备。

### 5fps 基准（2026-06-01，16核 .30，两阶段流水线 + 定时仪表化）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 |
|------|-----------|------|---------|--------|
| 1 | 12.0 | 47.0% | 81 | 47.0% |
| 2 | 12.2 | 48.3% | 90 | 24.1% |
| 4 | 24.0 | 91.2% | 171 | 22.8% |
| 8 | 48.4 | 150.2% | 319 | 18.8% |
| 12 | 73.0 | 246.1% | 480 | 20.5% |
| 16 | 91.4 | 345.4% | 597 | 21.6% |
| 24 | 87.0 | 441.0% | 738 | 18.4% |
| 32 | 83.2 | 476.9% | 716 | 14.9% |

> 注意: 1-2 流 actual_fps > target (12 > 5) 因为 testsrc2 lavfi 源交付速度快于实时（非 realtime RTSP push）

### 5fps 基准（2026-06-02，16核 .31，YUV Planes 直接编码）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 |
|------|-----------|------|---------|--------|
| 1 | 5.9 | 11.8% | 47 | 11.8% |
| 2 | 11.7 | 21.3% | 78 | 10.6% |
| 4 | 23.5 | 46.6% | 144 | 11.6% |
| 8 | 47.1 | 87.8% | 273 | 11.0% |
| 12 | 70.6 | 150.1% | 406 | 12.5% |
| 16 | 76.1 | 184.7% | 486 | 11.5% |
| 24 | 83.1 | 262.3% | 547 | 10.9% |
| 32 | — | — | — | — |

> 32 流测试因基础设施限制未完成（mediamtx RTSP 源过载）。与旧版 5fps 基线（.30 8核）对比，YUV Planes 版本在 24 流时每帧 CPU 效率提升 ~60%（31.7 fps/core vs 19.7 fps/core），内存降低 ~26%（547MB vs 738MB）。注意机器不同（.30 8核 vs .31 16核），对比仅供参考。

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

### 关键发现

1. **JPEG 编码占 74% 的每帧成本**: 24ms/frame 1080p，解码 7.5ms（23%），YUV 拷贝 0.7ms（2%）
2. **libjpeg-turbo 编码效率提升 ~3 倍**: JPEG 编码 CPU 开销降幅 ~65%（1fps 从 12%→4%，5fps 从 55%→19%）
3. **5fps 每流 CPU 约 19-25%**（16 核 .31），16 流约 3.5 核（22% 总 CPU）
4. **5fps 吞吐峰值 ~91 fps**（16 流），32 流时略降至 83 fps（I/O 争用）
5. **瓶颈路径**: Decode (7.5ms) → **JPEG Encode (24ms)** → S3 Upload + Kafka Publish（异步）
6. **1fps 32 流总 CPU 仅 72%**: 瓶颈固定为 S3/Kafka I/O，计算资源空闲
7. **两阶段流水线**: 与基线持平，主要代码重构价值
8. **定时仪表化工具**: 每 100 帧输出各阶段平均耗时（`Pipeline timing` 日志），可调 `STAGE_REPORT_INTERVAL`（`src/pipeline/decode.rs:12`）
9. **仪表化修复纠正了误判**: 旧版 decode 4μs（只测 receive_frame）→ 新版 7.5ms（测 send_packet + receive_frame），优化优先级不变（JPEG 编码仍是首要瓶颈）
10. **RTSP timeout 修复显著改善 TTFF**: stimeout 5s→30s，首帧等待从 300s+ 降至 5-245s（随流数递增递减）

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

# 查看 timing 仪表化数据
docker logs getframe-bench-worker 2>&1 | grep 'Pipeline timing'
```

## GSD 工作流

- 使用 `/gsd-plan-phase N` 规划阶段
- 使用 `/gsd-discuss-phase N` 讨论阶段
- 使用 `/gsd-execute-plan` 执行计划
- 使用 `/gsd-transition` 转换阶段
- 文档在 `.planning/` 目录
