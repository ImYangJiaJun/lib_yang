# YANG 核心运行时性能 Shadow 基线

> 状态：B-01，仅采集，不阻断
> 固定配置：`benchmarks/runtime-shadow.toml`
> 采集器：`scripts/run_performance_shadow.py`

## 目标

本阶段把已有 Criterion workload 变成可重复、可留存、可机器消费的性能证据。它解决“测量方式不一致”和“原始数据不可追溯”，但不在共享 runner 尚未校准时声称性能通过或失败。

固定契约包括：

- 同一份 `runtime_baseline` 源码和 `Cargo.lock`；
- 3 次外层独立重复；
- 每个重复内 50 个 Criterion 样本、1 秒预热、3 秒测量；
- 12 个具名指标，区分请求热路径、低频请求路径、Serde 对照与启动期路径；
- 每次 canonical 估计值、置信区间、归一化批次样本 p50/p95；
- 跨重复中位数、吞吐、离散范围和变异系数；
- Git、Rust 工具链、CPU/OS 以及配置、源码、锁文件指纹；
- Criterion 原始 JSON 和完整运行日志。

## 运行

在仓库根目录执行：

```powershell
python scripts/run_performance_shadow.py --self-test
python scripts/run_performance_shadow.py
```

默认输出到带 UTC 时间戳的 `target/performance-shadow/` 子目录。也可以指定一个尚不存在或为空的目录：

```powershell
python scripts/run_performance_shadow.py --output target/performance-shadow/manual-01
```

采集器拒绝覆盖非空目录，避免为了得到更好数字而无意抹掉历史原始数据。

## 输出契约

`summary.json` 的 `schema_version` 当前为 `1`，核心字段如下：

```text
environment
  git / toolchain / host / fingerprints
workload_contract
  repetitions / sample_size / warm_up_seconds / measurement_seconds / metrics
statistics
  canonical / normalized_batch_sample / percentile_method
runs[]
  command / duration_seconds / metrics[]
summary[]
  median_run_canonical_ns
  median_run_normalized_batch_sample_p50_ns
  median_run_normalized_batch_sample_p95_ns
  operations_per_second / outer_spread_percent / outer_cv_percent
policy
  blocking=false / regression_threshold_percent=null / conclusion=not_evaluated
```

每轮 canonical 值使用 Criterion `slope.point_estimate`，没有 slope 时退回 `mean.point_estimate`；跨重复的主值是三轮 canonical 值的中位数。`normalized_batch_sample_p50/p95` 来自 Criterion 原始批次总耗时 `times / iters`，每轮使用 nearest-rank 后再取三轮相应分位数的中位数。它们是“批次平均耗时样本”的分布描述，不是单请求延迟百分位，也不能用作线上 SLO。

每轮使用独立 `CRITERION_HOME`。原始 `benchmark.json`、`estimates.json`、`sample.json`、`tukey.json` 与合并的 stdout/stderr 日志随 CI artifact 一并保留；汇总文件不是唯一证据。采集器还会校验每轮每项恰好 50 个样本，并在采集或解析失败时写出 `failure.json`，让非阻断 job 的失败原因仍可追溯。

## Shadow 边界

- 共享 GitHub runner 的绝对值不能和开发机或不同 runner 直接比较。
- 不同 CPU 指纹的数据不能聚合；CI 固定 `ubuntu-24.04` 与 Rust `1.97.1` 只是缩小漂移面，不代表共享硬件稳定。
- `outer_spread_percent` 和 `outer_cv_percent` 用于校准噪声，不是通过阈值。
- 当前 job 标记为 non-blocking；执行失败和异常波动都应调查，但不会阻断合并。
- `hot_path`、`request_path`、`control`、`startup` 是不同政策域；`definition/app_build` 不能与请求热路径使用同一回归政策。
- 当前 workload 无真实网络或数据库往返；数据库延迟、连接池竞争和多副本容量需独立稳定环境。
- 本阶段只完成 wall-time shadow v1，不覆盖分配次数、锁等待、CPU profile 或端到端 UI 渲染；这些仍属于后续性能治理。
- 不允许通过“更新基线”消除红灯。B-07 只有在同类 runner 的历史噪声可控后，才会引入相对主分支的 3% 稳定热路径门槛。
