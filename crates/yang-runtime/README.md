# yang-runtime

YANG 服务进程运行时，提供与具体业务无关的启动期配置、生命周期和可观测性机制。

## 能力范围

- **配置源合成**：文件、环境变量与目录型 secret provider 按固定优先级合并，调用方声明可覆盖字段。
- **多阶段关闭预算**：共享绝对截止时间，阶段失败与超时可区分；`OwnedTask` 保证后台任务不泄漏。
- **可观测性**：Prometheus 管理面、OpenTelemetry trace 导出、readiness gate 与请求日志中间件。

本 crate 是 `yang-system` 的进程组装层，公开 API 不承诺长期稳定；可复用的业务能力在 `yang-base`。

## 使用

```toml
[dependencies]
yang-runtime = "0.1.0"
```

`yang-runtime` 固定工作区依赖版本（`yang-base` 0.4.x、`yang-db` 0.2.x）与 OpenTelemetry 0.30 栈，独立使用者应同时核对 MSRV 1.80。

## 相关 crate

- [`yang-base`](https://crates.io/crates/yang-base)：定义内核、Action、传输适配
- [`yang-db`](https://crates.io/crates/yang-db)：MySQL/PostgreSQL 查询构建与 Redis 客户端
