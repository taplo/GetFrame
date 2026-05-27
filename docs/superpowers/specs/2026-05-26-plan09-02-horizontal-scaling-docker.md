# Plan 09-02: Worker Horizontal Scaling + Docker Production Build

**Date:** 2026-05-26
**Status:** Draft
**Phase:** 9 (Worker Scaling — 200+/1000+ Streams)

## Goals

1. **Docker 多阶段生产构建** — 静态链接 FFmpeg + musl，多阶段构建（前端 + Rust），最终镜像 < 50MB
2. **DB 级 Stream 抢占** — 多个 worker 实例通过 PostgreSQL 自分配 stream，支持水平扩缩容
3. **KEDA 可观测指标** — 暴露 worker 粒度指标，供 KEDA Prometheus scaler 消费

## 架构决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 部署模型 | 单一二进制（API + 管道同进程） | 简化运维，减少组件数 |
| 流分配 | PostgreSQL UPDATE 抢占 + 心跳 | 无需额外控制面服务，利用已有 DB |
| Docker 构建 | Alpine + musl 静态链接 | 最小镜像，无 FFmpeg 运行时依赖 |
| SQL 客户端 | sqlx 纯 Rust 协议（无需 libpq） | sqlx 0.8 postgres feature 已内置 |

## 1. Docker 多阶段构建

### 构建架构

```dockerfile
# Stage 1: Web UI
FROM node:20-alpine AS web-builder
WORKDIR /web
COPY web/package.json web/ ./
RUN npm ci && npm run build

# Stage 2: Rust binary (musl, static FFmpeg)
FROM rust:1.85-alpine3.20 AS rust-builder
RUN apk add --no-cache \
    ffmpeg-dev musl-dev pkgconfig cmake make g++ \
    openssl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY src/ src/
COPY migrations/ migrations/
# Static link FFmpeg via pkg-config
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV FFMPEG_STATIC=1
RUN cargo build --release --bin getframe-worker

# Stage 3: Minimal runtime
FROM alpine:3.20
RUN apk add --no-cache ca-certificates
COPY --from=rust-builder /app/target/release/getframe-worker /getframe-worker
COPY --from=web-builder /web/dist /web/dist
COPY config.example.yaml /etc/getframe/config.yaml
EXPOSE 8080
ENTRYPOINT ["/getframe-worker"]
```

### 关键依赖

- `ffmpeg-dev` + `ffmpeg-libs` (Alpine) = 头文件 + musl 兼容的共享库，ffmpeg-sys-next 可通过 pkg-config 找到
- `cmake` + `g++` = rdkafka cmake-build 从源码编译 librdkafka
- `openssl-dev` = rdkafka 和 reqwest 需要 TLS
- **无需 libpq** — sqlx 使用纯 Rust PostgreSQL 客户端

## 2. DB 级 Stream 抢占机制

### 新增表结构

```sql
CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY,
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE streams ADD COLUMN IF NOT EXISTS claimed_by TEXT REFERENCES workers(id);
ALTER TABLE streams ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_streams_claimed ON streams(claimed_by, claimed_at);
```

### Worker 生命周期

#### 启动 (worker::WorkerManager::run)

```
1. worker_id = config.worker.id 或 std::env::var("HOSTNAME") 或 Uuid::new_v4()
2. DB: upsert INTO workers (id, heartbeat_at) VALUES ($1, NOW())
3. LOOP:
   a. claim: UPDATE streams SET claimed_by=$1, claimed_at=NOW()
      WHERE (claimed_by IS NULL OR claimed_at < NOW() - $timeout)
      AND deleted_at IS NULL
      LIMIT $batch_size
      RETURNING id, config
   b. For each returned id:
      - 检查是否已在本地运行 → 否则 start_pipeline()
   c. 释放: SELECT id FROM streams WHERE claimed_by=$1 AND deleted_at IS NOT NULL
      → stop_pipeline() for each
   d. heartbeat: UPDATE workers SET heartbeat_at=NOW() WHERE id=$1
   e. 等待 15s
4. ON SHUTDOWN:
   a. UPDATE streams SET claimed_by=NULL WHERE claimed_by=$1
   b. DELETE FROM workers WHERE id=$1
   c. stop_pipeline() for all local streams
```

#### 冲突处理

- 两个 worker 同时抢占同一 stream → PostgreSQL `UPDATE ... WHERE` 的原子性保证只有一个成功
- Worker 崩溃 → 30s timeout 后，其他 worker 自动接管
- 网络分区 → 心跳超时后双写风险极小（旧 worker 的 pipeline 在断开网络后已失败）

### 配置增强

```yaml
worker:
  id: ""                       # 默认取 hostname
  heartbeat_interval_secs: 15  # 心跳间隔
  claim_batch_size: 5          # 每次抢占数量
  claim_timeout_secs: 30       # claim 过期时间
```

### 新增模块

- `src/worker/mod.rs` — `WorkerManager` struct
  - `run()` — 抢占循环（tokio::spawn background task）
  - `claim_streams()` — DB 抢占逻辑
  - `release_streams()` — 释放不再属于自己的 stream
  - `heartbeat()` — 定期心跳

## 3. KEDA 指标

### 新增 Prometheus 指标

| 指标名 | 类型 | 标签 | 说明 |
|--------|------|------|------|
| `getframe_streams_claimed_total` | Gauge | `worker_id` | 当前实例声称的 stream 数 |
| `getframe_claim_errors_total` | Counter | `worker_id` | 抢占失败次数 |

现有指标 `getframe_streams_active` 保持（通过 worker_id 标签区分实例）。

### KEDA ScaledObject 参考

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: getframe-worker
spec:
  scaleTargetRef:
    name: getframe-worker
  minReplicaCount: 1
  maxReplicaCount: 10
  triggers:
    - type: prometheus
      metadata:
        serverAddress: http://prometheus:9090
        metricName: getframe_streams_claimed
        query: |
          sum(getframe_streams_claimed) / count(getframe_streams_claimed)
        threshold: "150"  # 平均每个 worker 超 150 条流时扩容
```

## 4. 兼容性

- **向后兼容**: `worker:` 配置块为可选的，不配置时保持当前行为（不抢占，本地管理全部 stream）
- **DB migration**: 新增 migration 文件 `20260527_000001_horizontal_scaling.sql`
- **开发环境**: 不启用 worker 抢占时，一切如常

## 实现边界

### 不在此计划中的内容

- Grafana dashboard（Phase 10）
- Helm chart（Phase 10）
- 实际 KEDA ScaledObject YAML（Phase 10，仅提供参考配置）
- Core pinning / CPU manager static policy（Phase 9-03 或生产调优）
