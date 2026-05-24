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
- **数据库**: PostgreSQL + SQLx
- **对象存储**: MinIO / S3（Claim-Check 模式）
- **容器**: Docker 多阶段构建 + distroless
- **K8s**: Deployments + KEDA

## 关键架构决策

1. **混合并发模型**: OS 线程处理 FFmpeg 解码 + tokio async 处理网络 I/O
2. **有界通道反压**: 流水线各阶段间使用 bounded channel 形成反压链
3. **Claim-Check 模式**: 图片存 MinIO/S3，Kafka 只传元数据+S3 链接
4. **核心绑定调度**: 每核 round-robin 调度 6-7 路流
5. **Guaranteed QoS**: `limits.cpu = requests.cpu` + CPU Manager static policy

## GSD 工作流

- 使用 `/gsd-plan-phase N` 规划阶段
- 使用 `/gsd-discuss-phase N` 讨论阶段
- 使用 `/gsd-execute-plan` 执行计划
- 使用 `/gsd-transition` 转换阶段
- 文档在 `.planning/` 目录
