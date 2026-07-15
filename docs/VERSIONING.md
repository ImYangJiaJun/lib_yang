# 版本与兼容策略

yang-base 和 yang-db 当前处于 0.x。即使 semver 允许 0.x 更快演进，本仓库仍把 patch 版本用于兼容增强，把有意删除或收紧公共契约的变更集中到 0.2.0。

## 0.1.x 规则

- 允许新增类型、feature、Result-returning 安全入口和结构化错误；已有入口保持可编译。
- 不安全、含糊或无法验证的入口先 deprecated，并提供替代 API、迁移示例和至少一个兼容编译测试。
- feature 默认值在 0.1.x 不做破坏性变化；新可选 feature 默认关闭且不得向关闭构建泄漏依赖。
- 当前发布版本为 yang-db 0.1.4、yang-base 0.1.2。

### 迁移记录 API

旧代码仍可编译，但记录不含 SQL checksum，之后的计划会把它视为不可验证：

```rust,ignore
initializer.record_migration("accounts", "v1").await?; // deprecated
```

新代码应让初始化器执行迁移，或显式提供 checksum/status：

```rust,ignore
initializer.run_migrations(&plugin).await?;
initializer
    .record_migration_with_checksum("accounts", "v2", "0123456789abcdef", "applied")
    .await?;
```

### SQL 标识符与错误

可信固定表达式仍可使用 `field`/`order`；外部列名改用 checked API：

```rust,ignore
let query = db.table("users")
    .field_identifier(user_selected_column)?
    .order_identifier(user_selected_column, true)?;
let sql = query.try_to_sql()?;
```

`where_and_unchecked`、`having_cond_unchecked`、分号切割的 `Database::init` 只保留兼容，不应进入新代码。

## 0.2.0 计划中的 breaking changes

- 删除 deprecated RAW/fail-closed 兼容 renderer 和 unchecked operator 入口，只保留 checked `Result` 路径。
- 删除 MySQL/PostgreSQL `Database::init` 分号切割器；复杂脚本必须使用逐 migration 语句或专用执行器。
- 删除无 checksum 的 `record_migration(module, version)`。
- 完成 identifier 与 trusted expression 的公开语义类型边界；需要表达式的调用点必须显式选择 trusted API。
- 重新评估默认 features；任何默认值变化都在升级说明中列出等价的显式 feature 配置。

0.2.0 实施前，每个条目都必须有 compile-fail/迁移测试，且不能在 0.1.x 提前删除兼容入口。
