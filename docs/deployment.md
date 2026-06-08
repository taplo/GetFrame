# GetFrame 部署文档

**版本**: v0.4.1

**代码仓库**: [https://github.com/taplo/GetFrame](https://github.com/taplo/GetFrame)
**Docker 镜像**: [`taplo/getframe-worker`](https://hub.docker.com/r/taplo/getframe-worker) (Docker Hub) / `ghcr.io/taplo/getframe-worker` (GitHub Container Registry)

---

## 1. 部署前准备

### 1.1 系统要求

| 组件 | 最低配置 | 推荐配置 |
|------|---------|---------|
| **CPU** | x86_64, 4 核 | 16 核+ |
| **内存** | 4GB | 32GB+ |
| **磁盘** | 20GB | 100GB+ SSD |
| **内核** | Linux 5.x+ (推荐 Ubuntu 24.04+) | — |
| **Docker** | 24.0+ | 29.0+ |
| **FFmpeg** | 59.x (libavcodec59) | 容器内已包含 |

### 1.2 依赖服务

| 服务 | 版本 | 说明 |
|------|------|------|
| MySQL | 8.0 | 状态持久化 + Worker 协调 |
| Kafka | 3.x (KRaft) | 消息队列 + Schema Registry（可选） |
| MinIO | latest | 兼容 S3 API 的对象存储 |
| Prometheus | v2.53+ | （可选）指标采集 |
| Grafana | 11.x | （可选）可视化仪表盘 |

---

## 2. Docker Compose 部署（推荐）

### 2.1 快速启动

```bash
# 克隆项目
git clone https://github.com/taplo/GetFrame.git getframe
cd getframe

# 复制配置
cp config.docker.yaml config.yaml

# 启动全部服务
docker compose up -d

# 查看状态
docker compose ps

# 查看日志
docker compose logs -f worker
```

### 2.2 服务拓扑

```
                          ┌──────────┐
                          │  Worker  │  :8080 (API + Web UI)
                          └────┬─────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
        ┌─────▼─────┐   ┌─────▼──────┐   ┌─────▼──────┐
        │  MinIO    │   │   Kafka    │   │   MySQL    │
        │ :9000/9001│   │   :9092    │   │   :3306    │
        └───────────┘   └────────────┘   └────────────┘

         ┌──────────┐    ┌──────────┐
         │Prometheus│    │ Grafana  │
         │ :9090    │    │ :3000    │
         └──────────┘    └──────────┘
```

### 2.3 服务容器说明

| 容器 | 镜像 | 端口 | 健康检查 | 说明 |
|------|------|------|---------|------|
| `mysql` | mysql:8.0 | 3306 | mysqladmin ping | 状态数据库 |
| `kafka` | confluentinc/cp-kafka:latest | 9092 | kafka-topics --list | 消息队列（KRaft 模式） |
| `minio` | minio/minio | 9000/9001 | curl /minio/health/live | 帧存储 |
| `init-bucket` | minio/mc | — | — | 初始化存储桶（一次性） |
| `init-kafka-topics` | confluentinc/cp-kafka:latest | — | — | 初始化 Kafka 主题（一次性）|
| `worker` | taplo/getframe-worker:latest | 8080 | /health | 核心服务 |
| `prometheus` | prom/prometheus:v2.53.0 | 9090 | — | 指标采集（可选）|
| `grafana` | grafana/grafana:11.1.0 | 3000 | — | 仪表盘（可选）|

### 2.4 启动顺序

```
mysql:healthy → init: ──────┐
kafka:healthy → init: ──────┤──→ worker:healthy
minio:healthy → init: ──────┘
```

### 2.5 初始化操作

存储桶和 Kafka 主题自动初始化（通过 init 容器）：

```bash
# 手动验证
docker compose logs init-bucket
docker compose logs init-kafka-topics

# 预期输出：
# Bucket ready: getframe-frames
# Topic ready: getframe-frames
```

### 2.6 验证部署

```bash
# 健康检查
curl http://localhost:8080/health
# {"status":"ok","active_streams":0,"version":"0.4.1"}

# 就绪检查
curl http://localhost:8080/ready
# {"ready":true}

# Web UI
# http://localhost:8080/

# Swagger UI
# http://localhost:8080/swagger-ui/

# Prometheus 指标
curl http://localhost:8080/metrics
```

---

## 3. 配置指南

### 3.1 配置文件结构

```yaml
# config.yaml

# 预加载流（声明模式前预注册）
preload_streams:
  - name: "camera-1"
    source_url: "rtsp://192.168.1.100:554/stream1"
    source_type: "rtsp"
    extract_interval_seconds: 5.0       # 抽帧间隔
    jpeg_quality: 85                    # JPEG 质量 (1-100)
    ffmpeg_threads: 1                   # 解码线程数
    rtsp_transport: "tcp"               # RTSP 传输协议

# 对象存储
storage:
  bucket: "getframe-frames"
  endpoint_url: "http://minio:9000"     # S3 兼容端点
  region: "us-east-1"
  access_key_id: "getframe"
  secret_access_key: "getframe123"
  retention_days: 7                     # 0=不清理

# Kafka 消息队列
kafka:
  brokers: "kafka:9092"
  topic: "getframe-frames"
  acks: "all"                           # 生产者确认级别
  compression: "snappy"                 # zstd/snappy/gzip
  schema_registry_url: ""               # Confluent Schema Registry（可选）
  consumer_group: "getframe-workers"

# MySQL 数据库
database:
  url: "mysql://getframe:getframe@mysql:3306/getframe"
  max_connections: 20

# 认证
auth:
  jwt_secret: ""                        # 留空自动生成
  jwt_expiry_seconds: 86400             # JWT 24h 过期
  initial_admin_password: "changeme123"  # 首次启动创建 admin 用户

# Worker 模式
worker:
  id: ""                                # 留空使用 HOSTNAME
  heartbeat_interval_secs: 15
  claim_batch_size: 50
  claim_timeout_secs: 60                # 声明超时（其他 Worker 可抢占）

# HTTP 服务
http:
  bind_address: "0.0.0.0"
  bind_port: 8080

# 日志
logging:
  level: "info"                         # debug/info/warn/error
  json: true                            # JSON 格式（容器部署推荐）
```

### 3.2 配置说明

**worker 模式（默认启用）**:
- `database` 配置存在 + `claim_batch_size > 0` 时自动启用 Worker 模式
- Worker 通过 MySQL 声明流，支持水平扩展
- `claim_timeout_secs` 控制声明超时（其他 Worker 可抢占失败 Worker 的流）

**非 Worker 模式**:
- 移除 `database` 和 `worker` 配置
- 所有 `preload_streams` 直接在当前进程启动

---

## 4. 多 Worker 部署

### 4.1 架构

```
                   ┌─────────┐
                   │  MySQL  │  (共享状态)
                   └────┬────┘
                        │
          ┌─────────────┼─────────────┐
          │             │             │
    ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼─────┐
    │ Worker 1  │ │ Worker 2  │ │ Worker N  │
    │ claim 50  │ │ claim 50  │ │ claim 50  │
    └───────────┘ └───────────┘ └───────────┘
```

### 4.2 使用 Docker Compose

```bash
# 启动多 Worker（需先构建镜像）
docker compose --profile multi-worker up -d

# 验证两个 Worker 运行
docker compose ps
# getframe-bench-worker    Up
# getframe-bench-worker-2  Up
```

### 4.3 使用独立容器

```bash
# Worker 1
docker run -d --name getframe-worker-1 \
  -v /path/to/config.yaml:/etc/getframe/config.yaml \
  taplo/getframe-worker:latest

# Worker 2（不同 ID）
docker run -d --name getframe-worker-2 \
  -v /path/to/config-worker2.yaml:/etc/getframe/config.yaml \
  taplo/getframe-worker:latest
```

### 4.4 Worker 生命周期

```
启动 → 注册 workers 表 → 心跳循环（~15s）
  ├── heartbeat: UPDATE workers SET last_heartbeat_at=NOW()
  ├── claim: UPDATE streams SET claimed_by=? WHERE timeout
  ├── start pipelines for new claims
  └── cleanup: stop pipelines for lost claims

关闭 → 释放声明 → STOP pipelines → 删除 workers 行
```

---

## 5. Kubernetes 部署（Helm）

### 5.1 安装

```bash
# 安装 Chart
helm install getframe ./deploy/helm/getframe

# 使用自定义 values
helm install getframe ./deploy/helm/getframe -f my-values.yaml
```

### 5.2 配置

values.yaml 关键配置：

```yaml
replicaCount: 3

image:
  repository: taplo/getframe-worker  # Docker Hub 镜像
  # repository: ghcr.io/taplo/getframe-worker  # GitHub Container Registry 镜像
  tag: v0.4.1

storage:
  endpoint_url: "http://minio:9000"
  bucket: "getframe-frames"
  access_key_id: "getframe"
  secret_access_key: "getframe123"

kafka:
  brokers: "kafka:9092"
  topic: "getframe-frames"

database:
  url: "mysql://getframe:getframe@mysql:3306/getframe"

worker:
  heartbeat_interval_secs: 15
  claim_batch_size: 50
  claim_timeout_secs: 60
```

### 5.3 自动扩缩容

**KEDA（Kafka Lag 驱动）**:

```yaml
autoscaling:
  enabled: true
  keda:
    enabled: true
    pollingInterval: 30
    cooldownPeriod: 300
    kafkaLagThreshold: "100"
```

**HPA（CPU 驱动）**:

```yaml
autoscaling:
  enabled: true
  keda:
    enabled: false
  minReplicas: 1
  maxReplicas: 10
  cpuTargetAverageUtilization: 80
```

### 5.4 Ingress

```yaml
ingress:
  enabled: true
  hosts:
    - host: getframe.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - hosts:
        - getframe.example.com
      secretName: getframe-tls
```

### 5.5 环境变量注入

```yaml
extraEnv:
  - name: RUST_LOG
    value: "debug"
  - name: DATABASE_URL
    valueFrom:
      secretKeyRef:
        name: getframe-db
        key: url
```

---

## 6. 认证系统引导

### 6.1 初始设置

1. 在配置文件中设置 `auth.initial_admin_password`
2. 启动 Worker — 自动创建 `admin` 用户
3. 登录获取 JWT：

```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"changeme123"}'

# 响应示例
{"token":"eyJhbGciOiJIUzI1NiJ9...","username":"admin","role":"admin"}
```

### 6.2 创建 API Key

```bash
# 需要 JWT 认证
curl -X POST http://localhost:8080/api/v1/auth/api-keys \
  -H "Authorization: Bearer <jwt-token>" \
  -H "Content-Type: application/json" \
  -d '{"name":"benchmark-key"}'

# 响应示例（原始 Key 仅返回一次）
{"id":"...","name":"benchmark-key","key":"gfk_<48hex>","created_at":"..."}
```

### 6.3 使用 API Key

```bash
curl -H "X-API-Key: gfk_<48hex>" http://localhost:8080/api/v1/streams
```

---

## 7. 构建指南

### 7.1 标准 Docker 构建

```bash
# 完整构建（含 Web UI）
docker build -t taplo/getframe-worker:latest .

# 注：需要 FFmpeg 开发库和 Node.js
# Docker 多阶段构建会包含全部依赖
```

### 7.2 快速构建（预编译二进制）

```bash
# 步骤 1 — VM 上编译二进制
docker run -d --name getframe-compile --network host \
  -v /home/user/getframe:/app -w /app \
  -e RUSTFLAGS='--cfg tokio_unstable' \
  rust:1.91-slim-bookworm \
  sh -c 'apt-get update -qq && apt-get install -y -qq \
    nasm pkg-config libavcodec-dev libavformat-dev libavutil-dev \
    libswscale-dev libavdevice-dev libavfilter-dev cmake make g++ \
    libssl-dev libcurl4-openssl-dev clang libclang-dev && tail -f /dev/null'

docker exec getframe-compile cargo build --release --bin getframe-worker
docker cp getframe-compile:/app/target/release/getframe-worker ./getframe-worker

# 步骤 2 — 构建最小运行时镜像
docker build -t taplo/getframe-worker:latest -f Dockerfile.quick .
```

### 7.3 交叉编译（Windows 本地）

> 注意：Rust 工具链仅 Docker 内容可用，VM 宿主机无 rust/cargo

```bash
# 同步源文件到 VM
scp Cargo.toml Cargo.lock taplo@192.168.3.122:/home/taplo/getframe/
scp -r src/ taplo@192.168.3.122:/home/taplo/getframe/
scp -r migrations/ taplo@192.168.3.122:/home/taplo/getframe/
scp config.docker.yaml docker-compose.yml taplo@192.168.3.122:/home/taplo/getframe/
```

---

## 8. 监控与运维

### 8.1 Grafana 仪表盘

预置仪表盘 `deploy/grafana/getframe-dashboard.json` 包含：

- 活跃流数统计
- 每流解码/提取帧率
- 错误率（存储 + Kafka）
- CPU/内存使用
- 帧处理速率
- Kafka 消费者滞后
- 流水线阶段耗时

### 8.2 日志

```bash
# 查看实时日志
docker compose logs -f worker

# 过滤流水线仪表化数据
docker compose logs worker 2>&1 | grep 'Pipeline timing'
# 示例输出:
# Pipeline timing: decode=7.5ms copy=0.7ms scdet=0.0ms rule=0.0ms jpeg=24.0ms

# 查看错误
docker compose logs worker 2>&1 | grep 'error'
```

### 8.3 关键运维操作

```bash
# 查看当前声明流数
curl -s http://localhost:8080/metrics | grep getframe_claimed_streams

# 查看活跃流数
curl -s http://localhost:8080/metrics | grep getframe_streams_active

# 查看 Kafka 滞后
curl -s http://localhost:8080/metrics | grep getframe_kafka_lag

# 查看任务列表
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/v1/tasks

# 清理 MinIO 过期帧（自动每小时执行，retention_days=7）
```

---

## 9. 基准测试

### 9.1 运行测试

```bash
cd benchmark

# 1fps 基准测试（默认）
python3 run.py

# 5fps 基准测试
sed -i 's/TARGET_FPS_VALUES = .*]/TARGET_FPS_VALUES = [5]/' run.py
python3 run.py

# 大规模扩展测试（32-200流，relay模式）
python3 run.py --scale
```

### 9.2 测试方法

对每个流数（1/2/4/8/12/16/24/32 等）：
1. 启动 Docker compose 堆栈
2. 启动 N 个 ffmpeg RTSP 源 → MediaMTX 中继
3. 通过 API 注册流
4. 等待首帧（最长 300s）
5. 稳定 30s → 采集 6 个样本（每 5s）
6. 测量: actual_fps, CPU%, MEM, errors → CSV
7. `docker compose down -v` 清理

### 9.3 结果查看

```bash
cat results/benchmark-1fps.csv
cat results/benchmark-1fps-scale.csv
cat results/benchmark-5fps.csv
```

---

## 10. 故障排除

### 10.1 常见问题

| 问题 | 原因 | 解决 |
|------|------|------|
| Worker 无法连接到 MySQL | 配置错误或 MySQL 未就绪 | 检查 `database.url`，确保 MySQL healthcheck 通过 |
| RTSP 流连接超时 | 源不可用或网络问题 | 检查 `stimeout` 配置，使用 `test-url` 端点探测 |
| MinIO 磁盘满 | 未设置 retention 或 retention 不足 | `docker compose down -v` 清除卷，设置 `retention_days` |
| Kafka 消息未收到 | offset 提交或 topic 问题 | 检查 `consumer_group`，通过 kafka CLI 验证 |
| Worker 不声明流 | Worker 配置或 DB 问题 | 检查 `claim_batch_size` 和 `claim_timeout_secs` |
| 核心绑定导致延迟增加 | thread_count ≫ core_count 时绑定负优化 | 移除 `cpu_cores` 配置 |

### 10.2 清理所有数据

```bash
# 停止并删除所有容器 + 卷
docker compose down -v

# 重新构建
docker compose build --no-cache worker
docker compose up -d
```

### 10.3 查看流水线阶段耗时

```bash
# 持续监控
docker logs -f getframe-bench-worker 2>&1 | grep 'Pipeline timing'

# 示例输出
# Pipeline timing: decode=7210μs copy=680μs scdet=0μs rule=0μs jpeg=23800μs
# Pipeline timing: decode=7340μs copy=695μs scdet=0μs rule=0μs jpeg=24100μs
```
