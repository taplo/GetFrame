# GetFrame — 高性能视频抽帧平台

## What This Is

GetFrame 是一个高性能的视频抽帧软件平台，能够接入多种来源（RTSP/RTMP/HLS/本地文件）的视频流，通过自定义规则引擎控制抽帧策略，将抽取的图片帧推送至 Kafka。平台运行在 Kubernetes 上，纯 CPU 处理（无硬件加速），面向需要大规模、高稳定性的工程化视频抽帧场景。

## Core Value

在纯 CPU 环境下，以最小资源消耗稳定处理数百路并发视频流，可靠地将指定帧投递到 Kafka。

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **STREAM-01**: 平台支持多种视频源接入（RTSP、RTMP、HLS、本地文件上传）
- [ ] **STREAM-02**: 单节点支持 200+ 路 1080P H.264 视频流并发处理
- [ ] **STREAM-03**: 整个集群支持 1000+ 路并发流，支持水平扩展
- [ ] **RULE-01**: 提供自定义规则引擎，支持时间间隔抽帧（如每秒1帧、每X秒1帧）
- [ ] **RULE-02**: 规则引擎支持场景变化检测抽帧（画面变化超过阈值时抽取）
- [ ] **RULE-03**: 规则引擎支持复合规则（时间+场景的结合策略）
- [ ] **RULE-04**: 规则引擎支持用户自定义条件组合，可通过配置文件/API动态管理
- [ ] **KAFKA-01**: 抽帧图片可靠推送至 Kafka
- [ ] **PLAT-01**: 提供 Web 管理界面，用于视频源管理、抽帧任务配置和运行监控
- [ ] **PLAT-02**: 提供 RESTful API，支持任务的 CRUD 和状态查询
- [ ] **PLAT-03**: 支持 Kubernetes 部署，容器化运行
- [ ] **OPS-01**: 内置 Metrics 和健康检查，与 Prometheus/Grafana 集成
- [ ] **OPS-02**: 优雅处理视频流断开/重连，自动恢复
- [ ] **OPS-03**: 支持日志分级和结构化日志输出

### Out of Scope

- GPU 硬件加速 — 明确指定纯 CPU 方案
- AI 视觉分析（如目标检测、人脸识别）— 这是 Kafka 下游消费者的职责
- 视频转码/重新编码 — 不改变视频编码格式
- 视频存储/归档 — 不提供长期视频存储功能

## Context

- 视频源以 1080P H.264 为主，可能有多种来源混合
- 网络环境可能不稳定，需要健壮的重连和容错机制
- 抽帧是计算密集型任务，纯 CPU 环境下需要极致优化（SIMD、零拷贝、内存池等）
- Kafka 消息结构待设计，需平衡吞吐量与灵活性
- 这是全新项目，从零构建

## Constraints

- **硬件**: 无 GPU/NPU 加速，完全依赖 CPU 解码和处理
- **性能**: 单节点需稳定承载 200+ 路 1080P 流，集群承载 1000+ 路
- **部署**: 运行在 Kubernetes 集群中，需设计为云原生架构
- **可靠**: 抽帧数据必须可靠投递，不能丢帧
- **语言**: 技术选型待调研确定，优先考虑性能与工程化的平衡

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 技术栈选型 | 需调研 Rust vs Go vs C/C++ 的性能与工程化平衡 | — Pending |
| FFmpeg vs 原生解码器 | FFmpeg 生态成熟但可能成为瓶颈，需评估 | — Pending |
| 规则引擎设计 | 时间+场景+自定义组合，需调研有无可复用方案 | — Pending |
| Kafka 消息格式 | 单帧 vs 批量 vs 按流聚合，待架构设计阶段确定 | — Pending |
| 任务调度模型 | 每个流独立 goroutine/task vs 线程池模型 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-24 after initialization*
