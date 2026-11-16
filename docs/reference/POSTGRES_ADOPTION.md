# 存量 PostgreSQL 项目接入指南

本文面向已有 PostgreSQL schema、希望直接复用现有数据库接入 lib_yang / yang-system 的项目。
框架不要求把数据迁到 MySQL，也不生成版本化迁移脚本；启动时按声明式 `TableDefinition`
只做附加演进。

## 1. 注册具名数据源

框架侧使用 `ToolsBuilder`（yang-system 使用 `config.toml` 的 `[postgres]`）：

```rust
use yang_base::datasource::DatasourceId;
use yang_base::tools::ToolsBuilder;

let pg_source = DatasourceId::new("pg_main")?;
let tools = ToolsBuilder::new()
    .mysql(mysql_database) // 默认源仍为 MySQL
    .postgres_named(pg_source, pg_database)
    .default_datasource(DatasourceId::default_source())
    .build()?;
```

一个逻辑源对应一个物理库；事务、外键、JOIN 都只在单源内生效。跨源一致性用
outbox/saga，不要在表定义里声明跨源外键。

## 2. 绑定模块与表

```rust
ModuleSpec::new(ModuleName::new("orders.items")?)
    .datasource(pg_source.clone())
    .table(
        TableSpec::new(TableName::new("orders")?)
            .datasource(pg_source.clone())
            .fields(fields! {
                id => Key::new(),
                status => Radio::<String>::new()
                    .require(true)
                    .native_enum("order_status")
                    .options([("pending", "待处理"), ("paid", "已支付")]),
            }),
    );
```

表未声明数据源时继承模块源，模块也未声明时落到默认源。

## 3. 声明存量结构（只声明要用的列）

同步器**不会删除**数据库中未声明的表、列、索引或约束。因此：

- 存量历史列、自定义类型列（如 `inet`、`citext`、domain）可以保留在库中但不声明；
  框架不会改写或读取它们。
- 需要参与查询/写入的列必须声明；无法映射为通用字段类型的列用
  `Table::opaque_column("addr", "inet")`（DSL 为 `TableSpec::opaque_column`）登记：
  同步器只校验该列已存在且 `format_type` 匹配，不改写、不读取、不写入，也不投影到
  通用 CRUD/UI。opaque 列不会自动创建，表必须已存在。
- 主键支持单列、复合主键与无主键；复合主键按 `Key` 字段声明顺序生成。
- 原生枚举用 `Radio::native_enum("type_name")` 复用既有 PG enum。类型名必须与表
  位于同一 schema；同步器会补齐缺失取值（要求 PostgreSQL 12+），但不会删除既有
  额外取值。
- 部分索引/表达式索引用 `TableSpec::partial_index`、`partial_unique_named`、
  `expression_index` 声明；谓词/表达式按 PostgreSQL 反射文本做空白与外层括号归一化
  后比较，语义相同即视为已满足。

## 4. 先 dry-run，再启动

先生成只读接入报告，辅助编写定义：

```text
cargo run -p yang-base --no-default-features --features postgres \
  --example pg_adoption_report -- postgres://user:pass@host:5432/db public
```

报告列出每张表的列、`format_type`、主键、索引，以及可直接参考的
`Field::...` / `native_enum(...)` 声明；无法安全映射的列会给出
`opaque_column("列名", "类型")` 建议。

```rust
let report = initializer.plan_table_definitions(&definitions).await?;
assert!(report.is_noop(), "仍有待执行变更: {:?}", report.statements);
```

`plan_table_definitions` / `preflight_table_definitions` 只读，不创建
`_yang_schema_binding`、不写任何 DDL。确认计划只包含预期的附加变更后，再执行
`sync_table_definitions`。

## 5. 所有权登记

首次 `sync_table_definitions` 会在每个数据源的默认 schema 创建
`_yang_schema_binding`：

| 模式 | 含义 |
|---|---|
| `managed` | 框架创建或已实际演进过的表，声明式定义是唯一事实源 |
| `adopted` | 首次接管、且本轮没有任何结构变更的存量表 |

`SchemaSyncReport.bindings` 会报告本轮新增/变更的绑定。`adopted` 表在下一次被框架
实际演进后自动升级为 `managed`。该表名为框架保留，模块不得声明。

## 6. 典型失败与处理

| 现象 | 原因 / 处理 |
|---|---|
| `表 X 存在不可自动修改的 schema 差异` | 列类型不兼容（如 integer → enum）。同步器拒绝破坏性变更；先人工迁移列或调整声明。 |
| `PostgreSQL 禁止提交前使用新取值` | 同一次同步既要给既有 enum 追加取值、又要改列/默认值。先人工 `ALTER TYPE ... ADD VALUE`，再重启。 |
| `枚举类型 X 缺失取值且新表默认值引用这些取值` | 同上，先补类型取值。 |
| 启动反复检测到 CHECK 不存在 | 从早期 `VARCHAR + CHECK` 切到 `native_enum` 时，旧的隐式 CHECK 不会被自动删除；人工 `ALTER TABLE ... DROP CONSTRAINT` 后再启动。 |
| `缺少声明中的 opaque 列` / `opaque 列类型不匹配` | 确认列名与 `format_type` 输出；opaque 列必须已存在且类型一致，框架不会创建或改写。 |
| `表 X 尚不存在，无法自动创建 opaque 列` | 含 opaque 列的表必须先由人工迁移建好，再交给框架接管。 |
| 同名表出现在多个数据源 | 表名只在单个源内唯一；UI `TableViewSchema.datasource` 会随视图下发，前端据此区分。 |
