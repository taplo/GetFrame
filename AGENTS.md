# GetFrame 项目指令

## 项目简介

高性能视频抽帧平台，纯 CPU 处理 200-1000+ 路并发 1080P H.264 视频流，支持多源接入（RTSP/RTMP/HLS/文件），通过规则引擎抽帧，帧存 MinIO/S3，元数据推送 Kafka。

## 技术栈

- **语言**: Rust (Edition 2024)
- **视频解码**: FFmpeg libavcodec via ffmpeg-next（库模式，非 CLI 进程）
- **SIMD YUV→RGB**: yuvutils-rs
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

### VM (开发/编译, 192.168.3.26)

| 属性 | 值 |
|------|-----|
| **地址** | `192.168.3.26:22` |
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

### VM (压测服务器, 192.168.3.29)

| 属性 | 值 |
|------|-----|
| **地址** | `192.168.3.29:22` |
| **SSH 用户** | `taplo` |
| **认证方式** | SSH 密钥 (`~/.ssh/id_ed25519`) |
| **OS** | Ubuntu 26.04 LTS (Resolute Raccoon) |
| **Hostname** | `ubuntuserver` |
| **CPU** | 16 核 |
| **Memory** | ~15 GiB |
| **Docker** | 已安装（apt docker.io） |
| **项目路径** | `/home/taplo/getframe` |
| **sudo 密码** | `rake.t.wang` |
| **网络** | 与 .26 互通（IP 直接可达） |

### Docker 构建注意事项

- Cargo / Rust 工具链 **仅 Docker 内可用**，VM 宿主机无 rust/cargo
- 首次构建极慢（apt 包 238MB + 全部 crate 下载 + 编译），约 40-60 分钟
- `.rs` 文件改动后必须 SCP 同步到 VM 再 `docker build`（git 仓库不同步）
- VM 本地 `docker build` 用 `buildx` 驱动，不支持 `docker buildx build .` 的构建输出
- VM 处于 VirtualBox NAT 下无法直接访问 GitHub（`utoipa-swagger-ui` build script 下载失败）：
  - 方案一（当前环境适用）：通过 `--build-arg` 传入代理（仅 HTTPS，否则 HTTP 如 apt/deb.debian.org 被拦截）
    ```
    docker buildx build --network host \
      --build-arg https_proxy=http://192.168.3.200:8787 \
      -t getframe-worker:latest .
    ```
  - 方案二（CI/无代理环境）：预下载 zip 并通过 HTTP server 提供，参考对话记录

### 文件同步工作流

```bash
# 从 Windows 本地同步变更到 VM (192.168.3.26)
scp Cargo.toml Cargo.lock taplo@192.168.3.26:/home/taplo/getframe/
scp -r src/ taplo@192.168.3.26:/home/taplo/getframe/
scp -r migrations/ taplo@192.168.3.26:/home/taplo/getframe/
scp config.docker.yaml docker-compose.yml config.example.yaml taplo@192.168.3.26:/home/taplo/getframe/

# VM 上构建
ssh taplo@192.168.3.26 'cd /home/taplo/getframe && docker buildx build --network host -t getframe-worker:latest .'

# 同步压测文件到压测服务器 (192.168.3.29)
scp -r benchmark/ taplo@192.168.3.29:/home/taplo/getframe/benchmark/

# 压测服务器上执行
ssh taplo@192.168.3.29 'cd /home/taplo/getframe/benchmark && python3 run.py'
```

## 已知 Bug 与修复索引

### Fix 5 — Health 更新频率（2026-05-31）
- **文件**: `src/pipeline/decode.rs:173-181`
- **问题**: health_handle 每 30 帧才更新一次，低帧率(1fps) 下 actual_fps 无法在 5s 采样间隔内正确测量
- **修复**: 改为每帧更新 health（移除 health_counter % 30 的条件判断）
- **效果**: actual_fps 现在在任何帧率下都能准确上报

### Fix 6 — 迁移至 libjpeg-turbo 编码（2026-05-31）
- **文件**: `src/pipeline/encode.rs`, `Cargo.toml`, `Dockerfile`
- **问题**: `image` crate 的 JPEG 编码器 CPU 效率低，为 5fps 每流贡献 55-78% CPU
- **修复**: 替换为 `turbojpeg` crate（libjpeg-turbo 的 Rust 绑定），结合 yuvutils-rs SIMD YUV→RGB
- **关键**: turbojpeg 1.4.0 移除了 `Image::from_slice()`，改用 struct 直接构造 `Image { pixels, width, pitch, height, format }`
- **效果**: 编码 CPU 开销降低 ~65%（5fps 每流从 55% 降至 19%），5fps 吞吐从 53→86 fps（+62%）

## Benchmark 结果

### 5fps 基准（2026-05-31，16核 .29，libjpeg-turbo）
| 流数 | actual_fps | CPU% | MEM(MB) | CPU/流 |
|------|-----------|------|---------|--------|
| 1 | 5.9 | 19% | 51 | 19% |
| 2 | 11.8 | 37% | 80 | 19% |
| 4 | 23.5 | 73% | 161 | 18% |
| 8 | 47.2 | 179% | 312 | 22% |
| 12 | 67.2 | 282% | 430 | 24% |
| 16 | 86.4 | 415% | 551 | 26% |
| 24 | 84.6 | 467% | 722 | 20% |
| 32 | 76.6 | 462% | 778 | 15% |

> 旧版 image crate 对比: 8 流旧 CPU/流 78% → 新 22%（-72%），16 流旧吞吐 53fps → 新 86fps（+63%），32 流旧吞吐 37fps → 新 77fps（+105%）

### 1fps 基准（2026-05-31，16核 .29，libjpeg-turbo）
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

### 关键发现
1. **libjpeg-turbo 编码效率提升 ~3 倍**: JPEG 编码 CPU 开销降幅 ~65%（1fps 从 12%→4%，5fps 从 55%→19%）
2. **5fps 最大吞吐提升 62%**: 从旧版 53fps（16 流）到新版 86fps，瓶颈从编码转移回解码
3. **CPU 饱和点推迟**: 5fps 场景从 12 流（970% CPU）推迟到 ~16 流（415% CPU）
4. **32 流共 462% CPU**: 5fps 场景释放的 CPU 被解码流水线重新利用，吞吐翻倍（37→77 fps）
5. **1fps 32 流总 CPU 仅 72%**: 瓶颈固定为 S3/Kafka I/O

## 基准测试命令

```bash
# 在 .29 上运行基准测试（1fps，默认）
screen -dmS bench python3 run.py

# 运行 5fps 基准测试
# 先修改 run.py：TARGET_FPS_VALUES = [1] → TARGET_FPS_VALUES = [5]
sed -i 's/TARGET_FPS_VALUES = .*]/TARGET_FPS_VALUES = [5]/' run.py
screen -dmS bench python3 run.py

# 查看进度
screen -S bench -X hardcopy /tmp/screen.log && tail -20 /tmp/screen.log

# 查看结果
cat /home/taplo/getframe/benchmark/results/benchmark-1fps.csv
cat /home/taplo/getframe/benchmark/results/benchmark-5fps-libturbojpeg.csv
```

## GSD 工作流

- 使用 `/gsd-plan-phase N` 规划阶段
- 使用 `/gsd-discuss-phase N` 讨论阶段
- 使用 `/gsd-execute-plan` 执行计划
- 使用 `/gsd-transition` 转换阶段
- 文档在 `.planning/` 目录
