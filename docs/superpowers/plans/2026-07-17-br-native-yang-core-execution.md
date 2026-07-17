# BR 体验连续的 YANG 原生内核执行计划

**上位设计**：`docs/superpowers/specs/2026-07-17-br-experience-compatible-yang-core-design.md`

**执行原则**：只构建一条 YANG 原生链路；阶段性旧实现仅作为待删除代码存在，不新增兼容 Adapter、compat feature、动态旁路或第二套可发布 Interface。

## 当前基线（2026-07-17）

| 范围 | 命令 | 结果 |
|---|---|---|
| `yang-base` | `cargo test --lib -p yang-base` | 470 passed，8 ignored |
| `yang-db` | `cargo test --lib -p yang-db` | 397 passed，1 ignored |
| `yang-system` | `cargo test --all-targets` | 11 passed |

当前仓库没有 Action dispatch、Params、Tools、Tables、事务和查询构建的可重复性能基准；阶段 0 必须补齐，不能把“测试通过”当成性能验收。

基准入口：

```powershell
cargo bench -p yang-base --bench runtime_baseline -- --quick
```

当前无网络基准覆盖 typed dispatch、body Params 提取、Tools 直接/TypeMap 访问、TableQuery plan、yang-db 查询 SQL 构建、定义 build 与 Registry slot 读取。真实 CRUD、分页、事务已通过 Docker 功能测试；RelationLoader 查询次数由批量执行器单元测试约束。数据库延迟/吞吐仍需独立性能环境测量。

## 三个 BR 迁移契约样板

### 1. 简单 CRUD：`org.post`

- Module/fields：`br/scs-api/src/addon/org/post/mod.rs`
- Params/Tools/query：`br/scs-api/src/addon/org/post/add.rs`
- `params_table + table_list + Plugins::action`：`br/scs-api/src/addon/org/post/table.rs`
- `params_table_select + table_select`：`br/scs-api/src/addon/org/post/select.rs`

必须保留的业务顺序：`Module -> fields/actions`、`params -> index`、`ctx.tools().db().table -> where_and -> select`、`params_table -> table_list/table_select`。

### 2. 关系/权限/选择器：`org.user`

- Module/Fields/关系：`br/scs-api/src/addon/org/user/mod.rs`
- Table/Radio/权限按钮：`br/scs-api/src/addon/org/user/table.rs`
- 选择器：`br/scs-api/src/addon/org/user/select.rs`

必须升级：Table/Action/Field 字符串改为受控引用；按钮引用启动期解析；关系显示批量加载；租户键默认 fail-closed。

### 3. 事务/Tools/内部调用：`org.user.register`

- `br/scs-api/src/addon/org/user/register.rs`
- 辅助对照：`br/scs-api/src/addon/admin/user/add.rs`

必须保留的业务顺序：开始事务、调用账户 Action、查询/写入组织用户、提交或回滚。必须升级为显式异步 Transaction 与强类型内部调用，禁止 thread-local 事务、Request/JsonValue 往返和字符串 `Plugins::api_run`。

## 纵向实施与证据

| 阶段 | 唯一目标 | 完成证据 |
|---|---|---|
| 0 | 冻结样板、功能快照和重构前性能基线 | 样板快照、criterion 基准、可重复运行说明 |
| 1 | 名称/引用/Spec/AppBuilder/BuiltApp/Registry/Catalog | 重名、依赖、引用、route 冲突构建失败；注册顺序确定性测试 |
| 2 | `yang-db` 受控 TableRef/FieldRef/CompareOp 查询链 | 无任意字符串公开查询入口；绑定参数与 fail-closed 测试 |
| 3 | `ToolsBuilder -> Tools` 显式所有权 | 运行路径无 GlobalDatabase/GlobalRedis/GlobalTools；多 App 并行测试 |
| 4 | Fields/Params Builder 与宏 | 一次声明生成 Schema/校验/OpenAPI/UI/查询策略；trybuild 测试 |
| 5 | Addon/Module/Action/Plugins 单 Registry | 路由/Action/定义原子注册；内部调用零 JSON |
| 6 | Tables 深模块 | TableQueryPlan/CompiledView/RelationLoader；查询次数与行数无关 |
| 7 | 预编译热路径 | Route/Action/Field/View 全部 handle 化；基准无稳定超过 3% 回退 |
| 8 | 重写 `yang-system` | 直接展示全部原生 Interface，不含兼容类型或动态旁路 |
| 9 | 删除旧 API、codemod 与最终审计 | 单 Registry/Fields/Query/Tools；迁移映射、诊断、性能报告 |

## 当前进度

- [x] 读取上位设计与 BR 参考系统。
- [x] 选择三个迁移契约样板。
- [x] 记录三个项目的功能测试基线。
- [x] 建立阶段 0 无网络性能基准与样板产物快照，并完成同机 quick/完整 Criterion 复测。
- [x] 完成阶段 1 名称、引用、Spec、确定性 Catalog、Registry slot 与构建期交叉校验。
- [x] 将 typed Action handler 与 Tools 接入同一个 AppBuilder/Registry，替换旧 Router 注册链。
- [x] 建立阶段 2 的跨方言受控 `TableRef/FieldRef/CompareOp/SortOrder` 入口与绑定参数测试。
- [x] 迁移全部查询调用方到受控入口并删除字符串 QueryBuilder 入口。
- [x] 完成阶段 2-9 的源码实现、迁移工具、参考应用与离线验收。

MySQL 外部功能边界已完成 Docker 验收：CRUD 12/12、分页 8/8、事务 10/10、typed Action 1/1、Schema sync 1/1。仍单独保留的性能/外部环境项是 Redis 往返、真实数据库延迟与吞吐、分配字节数、锁竞争和进程峰值内存；对应边界和现有 Criterion 数据记录在 `docs/superpowers/baselines/2026-07-17-runtime-quick-baseline.md`。
