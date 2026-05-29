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

### VM (VirtualBox, localhost)

| 属性 | 值 |
|------|-----|
| **主机** | VirtualBox VM on Windows 本机 (127.0.0.1:22) |
| **SSH 用户** | `taplo` |
| **认证方式** | SSH 密钥 (`~/.ssh/id_ed25519` 公钥已部署) |
| **OS** | Ubuntu 26.04 LTS (Resolute Raccoon) |
| **Kernel** | Linux 7.0.0-15-generic #15-Ubuntu SMP x86_64 |
| **Hostname** | `taplo-VirtualBox` |
| **CPU** | Intel Xeon E5-2680 @ 2.70GHz (8 cores) |
| **Memory** | 7.2 GiB |
| **Disk** | /dev/sda2 99G (53G used, 57%) |
| **Docker** | v29.5.2, Compose v5.1.4 |
| **FFmpeg** | libavcodec59 (仅 Docker 容器内可用) |
| **网络** | NAT 10.0.2.15/24 (enp0s3), Docker bridges: 172.17.0.1, 172.18.0.1 |
| **时区** | Asia/Shanghai |
| **项目路径** | `/home/taplo/getframe` |

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
# 从 Windows 本地同步变更到 VM
scp Cargo.toml Cargo.lock taplo@127.0.0.1:/home/taplo/getframe/
scp -r src/ taplo@127.0.0.1:/home/taplo/getframe/
scp -r migrations/ taplo@127.0.0.1:/home/taplo/getframe/
scp config.docker.yaml docker-compose.yml config.example.yaml taplo@127.0.0.1:/home/taplo/getframe/

# VM 上构建
ssh taplo@127.0.0.1 'cd /home/taplo/getframe && docker buildx build --network host -t getframe-worker:latest .'
```

## GSD 工作流

- 使用 `/gsd-plan-phase N` 规划阶段
- 使用 `/gsd-discuss-phase N` 讨论阶段
- 使用 `/gsd-execute-plan` 执行计划
- 使用 `/gsd-transition` 转换阶段
- 文档在 `.planning/` 目录
