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
