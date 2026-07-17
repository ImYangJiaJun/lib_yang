# YANG 核心运行时 quick baseline

**日期**：2026-07-17

**目的**：在删除旧 Router/GlobalTools/字符串 QueryBuilder 之前固定同机比较基线。该结果不是最终性能验收；最终阶段必须使用相同命令重复运行，并补充真实数据库、分配、锁竞争、序列化次数与 p50/p95 报告。

## 环境

- CPU：12th Gen Intel(R) Core(TM) i5-12400F，6C/12T
- OS：Microsoft Windows 11 专业版 10.0.28000，x86_64
- rustc/cargo：1.94.1，MSVC
- 工作区声明 MSRV：Rust 1.80
- Criterion：0.6.0（该系列 MSRV 为 Rust 1.80）
- 模式：Criterion `--quick`，release profile，无网络/数据库往返

## 运行命令

```powershell
cargo bench -p yang-base --bench runtime_baseline -- --quick
```

## 首次结果

| 路径 | Criterion 估计区间 |
|---|---:|
| typed Action dispatch（含 body 反序列化与响应序列化） | 651.02–655.58 ns |
| body Params 提取 | 361.70–363.87 ns |
| Tools 高频直接字段访问 | 272.16–279.52 ps |
| Tools 字符串 TypeMap 访问 | 32.565–33.539 ns |
| TableQuery plan 构建 | 740.32–773.93 ns |
| `table -> where_and -> order -> SQL` | 1.3478–1.3967 µs |
| 构建期 ActionRef -> Registry slot 读取 | 11.011–11.239 ns |
| 最小 App 定义构建与交叉校验 | 1.6467–1.6610 µs |

受控 `TableRef/FieldRef/CompareOp/SortOrder` 入口加入后，以同一进程和同一次 quick 运行直接对照：

| 路径 | Criterion 估计区间 |
|---|---:|
| 旧字符串查询入口 | 1.5333–1.5421 µs |
| 新受控引用查询入口 | 1.0117–1.0508 µs |

旧入口相对首次 baseline 的变化在本次 quick 采样中 `p = 0.08`，不具统计显著性；不能据此声称旧路径退化。新旧同次运行显示受控入口没有因为体验连续性增加字符串操作符解析成本，但最终仍需完整采样和调用方全部迁移后再验收 3% 门槛。

## 解释边界

- `Tools` 直接字段访问的亚纳秒结果主要用于检测是否意外增加锁、字符串查找或 TypeMap，不代表端到端业务延迟。
- 当前 Action dispatch 仍包含旧 `ActionContext` 和 `ApiResponse`；后续新运行时必须使用同一 workload 比较。
- 当前 TableQuery plan 仍会读取全局数据库/可观测配置；删除全局路径后预期不慢于该基线。
- 当前查询构建仍接收字符串字段和操作符；受控引用版本不得在请求期查询 Catalog 或解析字符串。
- Criterion quick 模式提供快速统计估计，不等价于设计要求的完整 p50/p95、吞吐和显著性门槛。

## 尚缺基准

- 强类型内部 Action 调用（零 JSON）与 BR JsonValue 往返对照；
- 普通 CRUD 与真实 MySQL 性能测量（功能往返已由 Docker 集成测试覆盖）；
- `table_list` 与 Table/Radio 关系批量加载的 SQL 次数；
- Tools/RequestContext 每请求分配次数和字节数；
- 锁竞争与序列化次数；
- 显式事务内多语句性能测量（提交/回滚功能已由 Docker 集成测试覆盖）；
- 启动时间和峰值内存。

## 重构后复测

完成唯一原生 Registry、`Tools` 显式所有权、`ParamInput`、受控 QueryBuilder、`ActionLink` 与 `Tables` 后，在同一台机器运行了全采样 Criterion。基准源码也已删除旧 API workload；当前命令会直接编译唯一原生链路。

| 路径 | 全采样估计区间 | 与首次基线比较 |
|---|---:|---|
| HTTP/JSON Action dispatch | 699.43–723.44 ns | 本次跨运行比较 `p = 0.15`，无统计显著回退；quick 复测为 668.59–678.01 ns |
| `params!` body 解码 | 142.07–150.70 ns | 相比 361.70–363.87 ns 明显减少 |
| Tools 直接访问 | 266.69–272.17 ps | 与首次基线同量级 |
| Tools 类型扩展访问 | 11.674–11.822 ns | 相比旧字符串 TypeMap 32.565–33.539 ns 明显减少 |
| RequestContext 类型化访问 | 19.126–19.406 ns | 新增独立基线；无锁、无字符串 clone |
| TableQuery plan 构建 | 594.96–605.42 ns | 相比 740.32–773.93 ns 显著提升（`p < 0.05`） |
| 受控 `table → where_and → order → SQL` | 1.0441–1.0585 µs | 与首次受控入口 1.0117–1.0508 µs 同量级；相对旧字符串入口显著提升 |
| Registry slot 读取 | 10.712–10.839 ns | 与 11.011–11.239 ns 同量级 |
| 强类型内部 Action 调用 | 649.17–707.36 ns | 新增完整采样 |
| HTTP/JSON 边界对照 | 686.92–696.85 ns | 强类型内部调用均值更低；结构上少一次输入解码和输出序列化 |

测量过程中发现原生 `Action -> TypedHandler` 的 `async-trait` 适配会二次装箱 future；修复为直接把原生 Handler future 交给擦除边界后，quick 基准中 dispatch 降低 13.8%，内部调用降低 14.0%。完整采样仍受 Windows 调度抖动影响，因此按设计规则以统计显著性判断：当前没有热路径出现统计显著回退。

最小 App 构建约 4.85–5.11 µs，高于旧 1.65 µs；增加的成本来自启动期名称、依赖、路由、关系、ActionLink、Schema 和 View 交叉校验。该路径不在请求热路径，符合“启动期承担校验和元数据生成”的设计边界。

关系加载单元测试固定验证 1000 行、10 个关系 key 只调用一次批量执行器，查询次数与结果行数无关。真实 MySQL 功能边界已通过 Docker 集成测试：CRUD 12/12、分页 8/8、事务 10/10，另有 typed Action 1/1 与 Schema sync 1/1；这些结果证明执行语义，不作为延迟或吞吐基准。真实 MySQL 性能、Redis 往返、分配字节数、锁竞争和进程峰值内存仍需要独立的稳定性能环境；本报告不把未测量项目写成性能验收。
