# 批量声明表字段

本文对应 `yang-base` 0.2.0。应用通过 `Table` 和 `Field` 声明 schema，`Table::build()` 生成不可变的 `TableDefinition`。

## 使用数组声明固定字段

字段集合固定时，直接把数组传给 `Table::fields`：

```rust
use yang_base::table::{Field, Table, TableDefinition};
use yang_base::BaseError;

fn users_table() -> Result<TableDefinition, BaseError> {
    Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id").label("ID"),
            Field::string("username", 64)
                .label("用户名")
                .required()
                .length(3..=64)
                .unique(),
            Field::string("email", 128)
                .label("邮箱")
                .required()
                .email()
                .unique(),
            Field::boolean("active").default(true),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
        ])
        .build()
}
```

`build()` 是集中校验边界，会检查字段名、主键、重复字段、默认值、索引引用、权限和生成列组合。

## 使用 Vec 组装字段

字段来自配置或功能开关时，先构造 `Vec<Field>`，再一次性交给 `fields`：

```rust
use yang_base::table::{Field, Table, TableDefinition};
use yang_base::BaseError;

fn audit_table(include_payload: bool) -> Result<TableDefinition, BaseError> {
    let mut fields = vec![
        Field::id("id"),
        Field::bigint("user_id").required().index(),
        Field::string("event", 64).required(),
        Field::created_at("created_at"),
    ];

    if include_payload {
        fields.push(Field::json("payload"));
    }

    Table::new("audit_logs")
        .label("审计日志")
        .fields(fields)
        .build()
}
```

## 从迭代器生成重复字段

`fields` 接受任何产出 `Field` 的迭代器，因此可以先用迭代器生成字段：

```rust
use yang_base::table::{Field, Table, TableDefinition};
use yang_base::BaseError;

fn metrics_table(names: &[&str]) -> Result<TableDefinition, BaseError> {
    let mut fields = vec![Field::id("id")];
    fields.extend(
        names
            .iter()
            .map(|name| Field::double(*name).default(0.0)),
    );

    Table::new("metrics").fields(fields).build()
}
```

不要绕过 `build()` 保存可变的中间字段配置。运行时、内置 CRUD、schema 同步和 JSON Schema 都应共享同一个 `TableDefinition`。

## 注册到模块

启用 `mysql` 后，把定义绑定到模块并原子注册标准 CRUD：

```rust
use yang_base::router::ModuleRouter;
use yang_base::BaseError;

fn user_module() -> Result<ModuleRouter, BaseError> {
    ModuleRouter::new("user", "用户管理")
        .table(users_table()?)
        .crud()
}
```

进一步配置字段关系、权限、索引和默认排序，参见[表定义指南](../guides/table_config.md)。
