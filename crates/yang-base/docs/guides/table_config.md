# yang-base schema-first 表定义指南

本指南对应 `yang-base` 0.2.0 的应用侧 API。表、字段、校验、权限、索引、时间戳与关系元数据统一在一份 schema 中声明：

```text
Table + Field
  -> Table::build()
  -> TableDefinition（不可变）
  -> ModuleRouter::table(...).crud()
  -> Record / TableQuery
```

## 1. 最小示例

```rust
use yang_base::table::{col, Field, Table, TableDefinition};

fn users_table() -> Result<TableDefinition, yang_base::BaseError> {
    Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id").label("ID"),
            Field::string("username", 64)
                .label("用户名")
                .required()
                .length(3..=64)
                .unique()
                .filterable()
                .sortable(),
            Field::string("email", 128)
                .label("邮箱")
                .required()
                .email()
                .unique(),
            Field::enumeration("status", ["active", "disabled"])
                .label("状态")
                .default("active"),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("created_at").desc())
        .build()
}
```

`build()` 是集中校验边界。成功后得到可克隆、不可变的 `TableDefinition`；失败时返回带上下文的 `BaseError`。

## 2. Field 构造器

应用代码使用 `Field` 的语义化构造器，不直接拼底层字段元数据。

| 构造器 | 数据类型 | 示例 |
|---|---|---|
| `Field::id(name)` | 64 位自增主键 | `Field::id("id")` |
| `Field::string(name, max)` | 有长度上限的字符串 | `Field::string("name", 80)` |
| `Field::integer(name)` | 32 位整数 | `Field::integer("age")` |
| `Field::bigint(name)` | 64 位整数 | `Field::bigint("user_id")` |
| `Field::float(name)` | 32 位浮点数 | `Field::float("score")` |
| `Field::double(name)` | 64 位浮点数 | `Field::double("amount")` |
| `Field::boolean(name)` | 布尔值 | `Field::boolean("enabled")` |
| `Field::date(name)` | 日期 | `Field::date("birthday")` |
| `Field::datetime(name)` | 日期时间 | `Field::datetime("published_at")` |
| `Field::timestamp(name)` | Unix 时间戳 | `Field::timestamp("expires_at")` |
| `Field::json(name)` | JSON | `Field::json("metadata")` |
| `Field::text(name)` | 长文本 | `Field::text("description")` |
| `Field::enumeration(name, values)` | 枚举 | `Field::enumeration("status", ["draft", "ready"])` |

关联列没有独立存储类型；先按数据库列选择 `bigint` 等构造器，再通过 `relation` 附加关系元数据。

生成列使用专用构造器：

```rust
let generated = [
    Field::id("id"),
    Field::created_at("created_at"),
    Field::updated_at("updated_at"),
    Field::soft_delete("deleted_at"),
];
```

这些构造器同时设置字段类型、写权限和时间戳角色。不要再手动复制这些组合。

## 3. 字段属性与校验

### 3.1 基本属性

```rust
use serde_json::json;
use yang_base::table::Field;

let nickname = Field::string("nickname", 50)
    .label("昵称")
    .nullable()
    .default(json!("anonymous"));

let external_id = Field::bigint("external_id")
    .required()
    .primary_key();
```

- `label`：用户可见名称。
- `required` / `nullable`：是否必填。
- `default`：数据库默认值；`build` 会验证默认值是否匹配字段类型和验证器。
- `primary_key`：声明主键。每张表恰好需要一个主键。
- `auto_increment`：仅允许整数主键使用；`Field::id` 已包含该语义。

### 3.2 内置验证器

```rust
use yang_base::table::Field;

let username = Field::string("username", 64)
    .required()
    .length(3..=64);

let email = Field::string("email", 128).required().email();
let phone = Field::string("phone", 32).phone();
let homepage = Field::string("homepage", 256).url();
let code = Field::string("code", 20).regex(r"^[A-Z0-9_-]+$");
let age = Field::integer("age").min(0.0).max(150.0);
```

| 方法 | 用途 |
|---|---|
| `min_length` / `max_length` / `length` | 字符串长度 |
| `min` / `max` | 数值范围 |
| `email` / `phone` / `url` | 常见格式 |
| `regex` | 自定义正则 |
| `validator(Validator)` | 直接追加一个内置或自定义验证器 |

`Email`、`Phone` 和 `Regex` 的严格校验需要 `validator` feature。

## 4. 字段权限

读取、写入、筛选和排序是四个独立的访问面：

```rust
use yang_base::table::Field;

let salary = Field::double("salary")
    .readable_by(["admin", "finance"])
    .writable_by(["admin"])
    .filterable_by(["admin", "finance"])
    .not_sortable();

let password_hash = Field::string("password_hash", 255)
    .secret()
    .writable_by(["auth-service"]);
```

常用方法：

- 全员允许：`readable`、`writable`、`filterable`、`sortable`
- 全员禁止：`not_readable`、`not_writable`、`not_filterable`、`not_sortable`
- 按角色允许：`readable_by`、`writable_by`、`filterable_by`、`sortable_by`
- 敏感字段预设：`secret`

`TableQuery` 在执行字段选择、筛选、排序或写入前进行权限检查。用户输入的字段名不要绕过这层保护直接进入 SQL。

## 5. 索引与默认排序

### 5.1 单字段索引

```rust
let fields = [
    Field::string("email", 128).unique_named("uk_users_email"),
    Field::bigint("user_id").index(),
    Field::string("order_no", 64).index_named("idx_order_no"),
];
```

### 5.2 复合索引

```rust
use yang_base::table::{col, Field, Table};

let orders = Table::new("orders")
    .fields([
        Field::id("id"),
        Field::bigint("tenant_id").required(),
        Field::string("order_no", 64).required(),
        Field::string("status", 32).required(),
        Field::created_at("created_at"),
    ])
    .unique_named("uk_orders_tenant_order", ["tenant_id", "order_no"])
    .index(["tenant_id", "status"])
    .index_named("idx_status_created", ["status", "created_at"])
    .default_order(col("created_at").desc())
    .then_order(col("id").asc())
    .build()?;
```

索引字段、最终生成的索引名和默认排序字段都在 `build` 时验证。引用不存在的字段、重复索引名或超过 MySQL 64 字符限制的生成名会直接失败；此时使用 `unique_named` / `index_named` 显式给出短名称。

## 6. 关系元数据

字段存储类型与关系元数据正交。普通 `user_id` 列使用 `BigInt` 存储，并通过 `ManyToOne` 描述多条当前表记录指向一条用户记录：

```rust
use yang_base::table::{Field, RelationType};

let user_id = Field::bigint("user_id")
    .relation("users", "id", RelationType::ManyToOne)
    .required()
    .index()
    .relation_display_fields(["username", "email"]);
```

可选类型为 `OneToOne`、`OneToMany`、`ManyToOne` 和 `ManyToMany`。这些值是 schema 元数据，不会改变字段的数据库存储类型，也不会自动生成跨表 JOIN；查询层仍需明确实现业务查询。

## 7. TableDefinition

`Table::build` 返回只读定义。应用代码通过方法读取元数据：

```rust
let users = users_table()?;

assert_eq!(users.name(), "users");
assert_eq!(users.label(), "用户表");
assert_eq!(users.primary_key(), "id");
println!("字段数: {}", users.field_count());

if let Some(field) = users.field("email") {
    println!("{}: {:?}", field.label(), field.field_type());
}

for field in users.fields() {
    println!("{} required={}", field.name(), field.is_required());
}
```

重要能力：

- `input_schema()`：写入侧 JSON Schema，排除数据库生成列和不可写字段。
- `output_schema()`：读取侧 JSON Schema，排除隐藏或不可读字段。
- `soft_delete_field()`：返回软删除字段名。
- `validate_schema(columns)`：比较真实数据库列并生成兼容性报告。
- `bind(pool)`：启用 `mysql` 时生成 `TableHandle`。

定义不可原地修改。需要变更 schema 时，应修改声明代码、重新 `build`，并在启动期让 schema 同步器处理允许的 additive 变更。

## 8. Record

`Record` 是动态行的统一类型，序列化形态就是 JSON object：

```rust
use yang_base::table::Record;

let mut user = Record::new()
    .set("username", "alice")
    .set("email", "alice@example.com");
user.insert("active", true);

let username: String = user.require("username")?;
let nickname: Option<String> = user.optional("nickname")?;

let borrowed = user.as_map();
assert_eq!(borrowed["username"], "alice");
```

- `require::<T>`：字段必须存在且类型可反序列化。
- `optional::<T>`：字段缺失或为 `null` 时返回 `None`。
- `into_map`：转回 `serde_json::Map`。

内置新增、更新、读取和列表查询都使用 `Record`，因此不需要为每张表生成 Rust 实体类型。

## 9. 注册标准 CRUD

```rust
use yang_base::router::{AppRouter, ModuleRouter};

let users = users_table()?;
let sessions = sessions_table()?;

let user_module = ModuleRouter::new("user", "用户管理")
    .table(users)
    .schema(sessions)
    .crud()?;

let app = AppRouter::new().module(user_module)?;
println!("模块数: {}", app.module_names().len());
```

`.table(...)` 绑定内置 Action 使用的主表；`.schema(...)` 添加只参与启动期 schema 汇总的附属表；`.crud()` 注册以下接口：

| Action | HTTP | 相对模块生成的 path | 输入 | 输出 |
|---|---|---|---|---|
| `add` | POST | `/api/{module}` | `Record` | `InsertResult` |
| `put` | PUT | `/api/{module}` | `{ "id": ..., "data": Record }` | `AffectedResult` |
| `del` | DELETE | `/api/{module}` | `{ "id": ... }` | `AffectedResult` |
| `get` | GET | `/api/{module}` | `{ "id": ... }` | `Record` |
| `select` | POST | `/api/{module}/query` | `SelectQuery` | `SelectResult` |
| `table` | GET | `/api/{module}/schema` | `{}` | `TableSchemaResponse` |

调用 `.crud()` 前必须先绑定主表，否则返回 `TableDefinitionNotSet`。

CRUD 授权由模块名确定：写接口 `add` / `put` / `del` 需要 `{module}:write`，读接口 `get` / `select` / `table` 需要 `{module}:read`。权限既在 dispatch 时执行，也写入 `ApiCatalog`。Catalog 的主键、记录、写入字段及查询字段枚举均从当前主表定义生成，因此同名内置 Action 在不同模块中仍保留各自准确的 schema。

## 10. 注册自定义 Api

标准 CRUD 之外的 Action 通过 `Api` 注册：

```rust
use yang_base::action::TypedAction;
use yang_base::router::{Api, ModuleRouter};

fn build_import_module(
    action: impl TypedAction,
) -> Result<ModuleRouter, yang_base::BaseError> {
    ModuleRouter::new("user", "用户管理").api(
        Api::post("/api/user/import", action)
            .operation_id("user.import")
            .created()
            .tag("user"),
    )
}
```

`Api` 将 Action 和 method/path/operation id/status/tags 一起交给 `ModuleRouter` 校验。多个异构 Action 使用 `apis([Api::get(...), Api::post(...)])` 批量注册。

路径模板遵循 Axum 0.8：单段参数使用 `{id}`，尾部通配使用 `{*path}`。旧式 `:id` / `*path`、非法模板和模块内匹配冲突会在 `api` / `apis` 调用时失败；跨模块冲突由 `AppRouter::catalog()` 在 transport 构建前统一拒绝。

## 11. SelectQuery JSON 形态

列表查询支持分页、where 布尔树、排序和可选 count：

```json
{
  "page": 1,
  "page_size": 20,
  "where": {
    "type": "and",
    "conditions": [
      { "type": "eq", "field": "status", "value": "active" },
      { "type": "gte", "field": "age", "value": 18 }
    ]
  },
  "order_by": [
    { "field": "created_at", "direction": "desc" }
  ],
  "count_total": true
}
```

where 树中的字段存在性、筛选权限、嵌套深度、操作符和值类型会在查询层验证，`IN` / `BETWEEN` 的每个值也逐项检查。`Eq` / `Ne` 的值为 JSON `null` 时分别生成 `IS NULL` / `IS NOT NULL`，也可以直接使用 `IsNull` / `IsNotNull`；不会把 `NULL` 当成普通绑定值生成错误比较。排序字段同样受字段权限保护。

## 12. 完整订单表示例

```rust
use yang_base::table::{col, Field, RelationType, Table, TableDefinition};

fn orders_table() -> Result<TableDefinition, yang_base::BaseError> {
    Table::new("orders")
        .label("订单表")
        .fields([
            Field::id("id"),
            Field::string("order_no", 64)
                .label("订单号")
                .required(),
            Field::bigint("user_id")
                .relation("users", "id", RelationType::ManyToOne)
                .label("用户")
                .required()
                .index()
                .relation_display_fields(["username"]),
            Field::double("amount")
                .label("金额")
                .required()
                .min(0.0),
            Field::enumeration(
                "status",
                ["pending", "paid", "shipped", "completed", "cancelled"],
            )
            .label("状态")
            .required()
            .default("pending")
            .filterable()
            .sortable(),
            Field::json("details").label("订单明细").required(),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .unique(["order_no"])
        .index(["status", "created_at"])
        .default_order(col("created_at").desc())
        .build()
}
```

## 13. 最佳实践

1. 用 `snake_case` 命名数据库表和字段，展示文字放在 `label`。
2. 每张表只保留一个明确主键；常规自增主键优先 `Field::id("id")`。
3. 一次性把字段集合传给 `Table::fields`，让 `build` 统一验证。
4. 用户可控的筛选、排序和写入始终走 `TableQuery`。
5. 只对真实查询模式建索引；复合索引按最常用前缀排列。
6. 敏感字段使用 `secret`，再显式开放最小必要的写角色。
7. 时间戳和软删除使用专用构造器，避免手动声明出现语义漂移。
8. 模块主表用 `table`，附属 schema 用 `schema`；不要混淆运行期 CRUD 主表。
9. 标准表接口用 `.crud()`；自定义端点用 `Api`，保持 Action 与传输元数据原子一致。

## 14. 常见问题

### 如何动态添加字段？

`TableDefinition` 是不可变快照。修改 `Table` 声明并重新构建应用；启动期 schema 同步只自动执行允许的 additive 变更，危险差异会 fail-fast。

### 为什么 `build()` 提示必须有主键？

内置 get/put/del 需要稳定主键。使用 `Field::id("id")`，或对一个必填整数/字符串字段调用 `primary_key()`。

### 为什么 `.crud()` 返回 `TableDefinitionNotSet`？

先调用 `.table(definition)` 绑定模块主表，再调用 `.crud()`。

### 如何返回强类型业务 DTO？

自定义 Action 的 Input/Output 继续使用普通 serde + schemars 类型；只有真正动态的表行才使用 `Record`。

### 如何做跨表 JOIN？

关系声明是元数据，不自动执行 JOIN。跨表查询应放在自定义 `TypedHandler` 中，并显式处理授权、事务与查询边界。
