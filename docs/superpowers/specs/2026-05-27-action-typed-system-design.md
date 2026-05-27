# Action 系统类型化重构设计（H-1）

**生成日期**：2026-05-27
**对应 Backlog 条目**：H-1（builtin Action 使用 serde_json::Value 而非具体类型）
**适用 Crate**：`yang-base`，新增 `yang-base-derive`
**相容性**：破坏性变更（基础库尚未投产，破坏性变更被允许）
**优化目标**：性能/安全性最优

---

## 1. 背景与目标

### 1.1 现状问题

`yang-base` 的 Action 系统当前的契约：

```rust
#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError>;
    fn name(&self) -> &str;
    // ...
}
```

`ApiResponse::data` 类型是 `Option<serde_json::Value>`，且参数提取通过 `ctx.param("key")` 字符串查表。这导致：

- **输出无类型契约**：handler 返回 `Value`，调用方无法在编译期校验字段
- **输入运行时化**：`ctx.param("typo")` 类型/字段名错误只在运行时报错
- **builtin 把强类型扔回 Value**：`SelectAction` 用 `fetch_optional::<DynamicRow>` 从 sqlx 拿到的强类型行被立刻序列化为 `Value` 塞进 `ApiResponse`，再由调用方反序列化——一次往返浪费且失去类型保证
- **API 文档无机器可读契约**：无法生成 OpenAPI/JSON Schema

### 1.2 设计目标

按优先级排序：

1. **编译期端到端类型安全**：handler 的输入和输出类型由 trait 关联类型固定，字段名错误必须编译失败
2. **零成本运行时**：相对当前实现不引入额外序列化/反序列化往返
3. **Schema 自动派生**：`Input`/`Output` 的 JSON Schema 由 `schemars` 从类型自动生成，永不漂移
4. **开发体验**：用户写一个 `#[derive(TableEntity)]` 即可获得全套强类型 CRUD；写一个自定义 Action 只需 `#[derive(Action)] + impl TypedHandler`
5. **派发性能不退化**：`ModuleRouter` 仍以 `HashMap<String, Arc<dyn Action>>` 派发；类型化通过 blanket impl 桥接，dyn 调用次数与现状相同

### 1.3 非目标

- **不引入 axum 风格的多 extractor**（`Path` + `Query` + `Body` 多参提取）。当前所有请求体一次性进入 `request.body`，单 `Input` 结构足够
- **不在第一轮支持 OR/嵌套 where 表达式**。`Vec<WhereCond>` AND 连接覆盖 95% 用例；OR 推迟到下一轮迭代
- **不重写 `TableQuery`/`QueryBuilder`**。底层 SQL 构建器保持原样，只在 builtin Action 与 `TableQuery` 之间增加类型化桥接
- **不动权限模型**。`Permission` 类型与 `ModuleRouter` 的权限检查流程不变

---

## 2. 总体架构

### 2.1 三层 trait

```
┌─────────────────────────────────────────────────────────┐
│  TypedHandler          ← 用户唯一手写的 trait            │
│    type Input          (DeserializeOwned + JsonSchema)  │
│    type Output         (Serialize + JsonSchema)         │
│    fn handle(ctx, input) -> Result<Output>              │
└─────────────────────────────────────────────────────────┘
              ▲
              │ 由 #[derive(Action)] 派生
              │
┌─────────────────────────────────────────────────────────┐
│  TypedAction : TypedHandler                              │
│    fn name() -> &str                                    │
│    fn permissions() -> &[Permission]                    │
│    fn is_public() -> bool                               │
│    fn input_schema() -> &RootSchema                     │
│    fn output_schema() -> &RootSchema                    │
│    ...                                                  │
└─────────────────────────────────────────────────────────┘
              ▲
              │ 由 yang-base 提供 blanket impl
              │
┌─────────────────────────────────────────────────────────┐
│  Action  (object-safe，存进 router 的 dyn trait)         │
│    async fn dispatch(ctx) -> Result<ApiResponse>        │
│    fn meta() -> &ActionMeta                             │
└─────────────────────────────────────────────────────────┘
```

每一层的职责：

- **`TypedHandler`** — 业务逻辑层，只处理 `Input → Output`
- **`TypedAction`** — 元信息层，由 `#[derive(Action)]` 从属性派生，用户不手写
- **`Action`** — dyn 擦除层，让 `ModuleRouter` 能以 `Arc<dyn Action>` 统一存储派发；blanket `impl<T: TypedAction> Action for T` 桥接

### 2.2 数据流

```
HTTP Request
    │
    ▼
ActionContext { request, user, tools, table_config }
    │
    ▼
ModuleRouter::dispatch(name, ctx)
    │  - 查 actions HashMap
    │  - 检查 is_public / 权限
    │  - 调用 Arc<dyn Action>::dispatch(ctx)
    ▼
Action::dispatch(ctx)              [blanket 实现]
    │  - ctx.extract_input::<T::Input>()      ← 一次反序列化
    │  - self.handle(ctx, input).await        ← 用户业务逻辑
    │  - ApiResponse::success(output, "成功") ← 一次序列化
    ▼
ApiResponse { code, message, data }
```

序列化次数：**入口 1 次反序列化 + 出口 1 次序列化**，与现状相同。中间不再绕道 `Value`。

### 2.3 单元划分

每个单元有清晰边界、可独立测试：

| 单元 | 职责 | 依赖 |
|---|---|---|
| `action::trait` | `TypedHandler` / `TypedAction` / `Action` 三 trait + blanket impl | `ApiResponse`, `BaseError`, `ActionContext` |
| `action::context` | `ActionContext` + `extract_input` | `Request`, `User`, `GlobalTools` |
| `action::meta` | `ActionMeta`（name/permissions/schemas 的运行时聚合） | `Permission`, `schemars` |
| `action::builtin::*` | 六个泛型 builtin Action | `TableEntity`, `TypedHandler` |
| `table::entity` | `TableEntity` trait + `WhereOp<V>` + `SortOrder` | `TableConfig`, `sqlx::FromRow` |
| `yang-base-derive` | `#[derive(TableEntity)]` 与 `#[derive(Action)]` | `syn`, `quote`, `darling`, `proc-macro-error` |

---

## 3. 核心类型定义

### 3.1 `TypedHandler`（用户手写）

```rust
#[async_trait::async_trait]
pub trait TypedHandler: Send + Sync + 'static {
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send;
    type Output: serde::Serialize + schemars::JsonSchema + Send;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError>;
}
```

设计要点：

- `Input: DeserializeOwned`（不要求 `Default`）— 缺字段必须显式标 `Option<T>` 或 `#[serde(default)]`
- `Output: Serialize` — 通常是用户自定义 struct、`TableEntity`、`Vec<TableEntity>`、或 `()`
- `'static` bound — Action 通常以 `Arc<dyn Action>` 长生命周期持有，所有引用必须 owned
- `Send` — 派发可能跨线程

### 3.2 `TypedAction`（派生生成，用户不手写）

```rust
pub trait TypedAction: TypedHandler {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str { self.name() }
    fn description(&self) -> &'static str { "" }
    fn permissions(&self) -> &'static [Permission] { &[] }
    fn is_public(&self) -> bool { false }
    fn input_schema(&self) -> &'static schemars::schema::RootSchema;
    fn output_schema(&self) -> &'static schemars::schema::RootSchema;
}
```

`input_schema()`/`output_schema()` 内部用 `OnceLock` 惰性生成，全程序只生成一次。

### 3.3 `Action`（擦除层）

```rust
#[async_trait::async_trait]
pub trait Action: Send + Sync + 'static {
    async fn dispatch(
        &self,
        ctx: ActionContext,
    ) -> Result<ApiResponse, BaseError>;

    fn meta(&self) -> &ActionMeta;
}

pub struct ActionMeta {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub permissions: &'static [Permission],
    pub is_public: bool,
    pub input_schema: &'static schemars::schema::RootSchema,
    pub output_schema: &'static schemars::schema::RootSchema,
}
```

### 3.4 Blanket 桥接

```rust
#[async_trait::async_trait]
impl<T: TypedAction> Action for T {
    async fn dispatch(
        &self,
        ctx: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        let input: <Self as TypedHandler>::Input = ctx.extract_input()?;
        let output = self.handle(ctx, input).await?;
        ApiResponse::success(output, "成功")
    }

    fn meta(&self) -> &ActionMeta {
        // 由派生宏在 TypedAction 实现块内额外生成一个 OnceLock<ActionMeta>
        Self::__meta_static()
    }
}
```

`__meta_static()` 是派生宏额外生成的关联函数（双下划线表示框架内部），把 `name/display_name/.../schemas` 一次性聚合成 `&'static ActionMeta`。

### 3.5 `ActionContext` 改造

```rust
#[derive(Debug)]
pub struct ActionContext {
    pub request: Request,
    pub user: Option<User>,
    pub tools: Arc<GlobalTools>,
    pub table_config: Option<Arc<TableConfig>>,
}

impl ActionContext {
    pub fn extract_input<I: serde::de::DeserializeOwned>(&self) -> Result<I, BaseError> {
        let value = serde_json::Value::Object(self.request.body.clone());
        serde_json::from_value(value).map_err(|e| {
            BaseError::ParamInvalid("body".to_string(), e.to_string())
        })
    }

    // 保留：user/tools/table_config 的 getter 与链式构造方法
    // 移除：param / param_optional / param_or_default / query_param
}
```

`request.body` 现状是 `serde_json::Map<String, Value>`，先封装回 `Value::Object` 再 `from_value` 是零拷贝（`Map` move 进 `Value`）。`query_param` 类似改为 `extract_query<Q: DeserializeOwned>()`，从 `request.query`（`HashMap<String, String>`）通过 `serde_qs` 或自实现的 `from_url_values` 反序列化；本轮先只实现 `extract_input`，`extract_query` 留待有 builtin 需要时再加（YAGNI）。

---

## 4. TableEntity 与表实体类型化

### 4.1 `TableEntity` trait

```rust
pub trait TableEntity:
    serde::de::DeserializeOwned
    + serde::Serialize
    + schemars::JsonSchema
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
    + Send + Sync + Unpin + 'static
{
    /// 主键 Rust 类型（i64 / String / Uuid 等）
    type Pk: serde::de::DeserializeOwned
        + serde::Serialize
        + schemars::JsonSchema
        + Send + Sync + 'static;

    /// 字段名枚举（派生生成）。所有合法列名的封闭集合。
    /// `Eq + Hash` bound 让 `T::Field` 可用作 `HashSet`/`HashMap` 的 key，方便用户
    /// 在自定义 Action 中做字段权限检查或去重等场景。
    type Field: AsColumnName + serde::de::DeserializeOwned + serde::Serialize
        + schemars::JsonSchema + Copy + Eq + std::hash::Hash + Send + Sync + 'static;

    /// where 条件枚举（派生生成）。每个变体绑定字段类型。
    type WhereCond: IntoSqlCondition + serde::de::DeserializeOwned
        + schemars::JsonSchema + Send + Sync + 'static;

    const TABLE_NAME: &'static str;
    const PK_FIELD: &'static str;

    /// 运行时表配置（用于 SQL 构建）。OnceLock 缓存，全程序生成一次。
    fn table_config() -> &'static TableConfig;
}

pub trait AsColumnName {
    fn column_name(&self) -> &'static str;
}

pub trait IntoSqlCondition {
    /// 将类型化条件转为底层 SQL 片段 + 参数。
    /// 返回的 column_name 是 &'static str，由枚举判别式映射，杜绝列名拼接攻击。
    fn into_sql_condition(self) -> SqlCondition;
}

pub struct SqlCondition {
    pub column: &'static str,
    pub op: SqlOp,
    pub params: Vec<serde_json::Value>,
}

pub enum SqlOp {
    Eq, Ne, Lt, Lte, Gt, Gte,
    In,           // params 长度 = N，渲染为 (?, ?, ..., ?)
    Between,      // params 长度 = 2
    Like,         // params 长度 = 1，pattern 由用户提供
    IsNull,
    IsNotNull,
}
```

### 4.2 `WhereOp<V>` 通用操作符

```rust
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum WhereOp<V> {
    Eq(V),
    Ne(V),
    Lt(V),
    Lte(V),
    Gt(V),
    Gte(V),
    In(Vec<V>),
    Between(V, V),
    IsNull,
    IsNotNull,
    // Like 仅对 String 字段开放——不放进通用 WhereOp，避免对数值字段编译通过却语义错误。
    // 派生宏在 String 字段对应的 WhereCond variant 中单独生成 Like(String) 变体。
}
```

JSON 表示（`#[serde(tag = "op", content = "value")]` 决定）：

```json
{ "op": "eq", "value": 42 }
{ "op": "in", "value": [1, 2, 3] }
{ "op": "between", "value": [10, 20] }
{ "op": "is_null" }
```

### 4.3 派生宏生成示例

用户写：

```rust
#[derive(Deserialize, Serialize, JsonSchema, FromRow, TableEntity)]
#[table(name = "users")]
pub struct User {
    #[entity(primary_key)]
    pub id: i64,

    #[entity(max_length = 50, unique)]
    pub username: String,

    pub email: String,

    pub status: UserStatus,
}
```

`#[derive(TableEntity)]` 展开生成（伪代码，实际由 `quote!` 拼装）：

```rust
// 1. 字段枚举
#[derive(Deserialize, Serialize, JsonSchema, Copy, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum UserField { Id, Username, Email, Status }

impl AsColumnName for UserField {
    fn column_name(&self) -> &'static str {
        match self {
            UserField::Id       => "id",
            UserField::Username => "username",
            UserField::Email    => "email",
            UserField::Status   => "status",
        }
    }
}

// 2. where 条件枚举（每字段一个变体，字段类型作为 WhereOp 的 V）
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "field", content = "cond", rename_all = "snake_case")]
pub enum UserWhere {
    Id(WhereOp<i64>),
    Username(UserWhereUsername),    // String 字段使用扩展枚举（含 Like）
    Email(UserWhereEmail),
    Status(WhereOp<UserStatus>),
}

// String 字段的扩展枚举：WhereOp 的所有变体 + Like
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum UserWhereUsername {
    Eq(String), Ne(String),
    Lt(String), Lte(String), Gt(String), Gte(String),
    In(Vec<String>),
    Between(String, String),
    Like(String),    // 仅 String 字段有
    IsNull, IsNotNull,
}
// UserWhereEmail 同理

impl IntoSqlCondition for UserWhere {
    fn into_sql_condition(self) -> SqlCondition {
        match self {
            UserWhere::Id(op)       => sql_from_where_op("id",       op),
            UserWhere::Username(op) => sql_from_string_where("username", op),
            UserWhere::Email(op)    => sql_from_string_where("email",    op),
            UserWhere::Status(op)   => sql_from_where_op("status",   op),
        }
    }
}

// 3. TableEntity 实现
impl TableEntity for User {
    type Pk = i64;
    type Field = UserField;
    type WhereCond = UserWhere;
    const TABLE_NAME: &'static str = "users";
    const PK_FIELD: &'static str = "id";

    fn table_config() -> &'static TableConfig {
        static CONFIG: OnceLock<TableConfig> = OnceLock::new();
        CONFIG.get_or_init(|| {
            TableConfig::new("users")
                .primary_key("id")
                .field(FieldConfig::new("id", FieldType::BigInt))
                .field(FieldConfig::new("username", FieldType::String { max_length: 50 }))
                .field(FieldConfig::new("email", FieldType::String { max_length: 255 }))
                .field(FieldConfig::new("status", FieldType::Enum { values: vec![...] }))
                // 唯一索引
                .add_unique_index(IndexConfig::new(vec!["username"]))
        })
    }
}
```

### 4.4 派生宏属性

`#[table(...)]` 在 struct 上：

| 属性 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `name` | `&str` | 是 | 数据库表名 |
| `display_name` | `&str` | 否 | 默认 = `name` |
| `soft_delete` | `&str` | 否 | 软删除字段名 |

`#[entity(...)]` 在字段上：

| 属性 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `primary_key` | flag | — | 标记主键。整个 struct 必须恰好一个 |
| `max_length` | `usize` | 255 | 字符串字段最大长度 |
| `unique` | flag | false | 字段唯一索引 |
| `required` | flag | true（除 `Option`） | 必填；`Option<T>` 自动 false |
| `default` | str expr | — | SQL 默认值 |
| `column` | `&str` | 字段名 | 自定义列名（默认与 Rust 字段同名） |
| `skip` | flag | false | 跳过此字段（不参与 SQL/Schema） |

字段类型到 `FieldType` 的映射规则：

| Rust 类型 | `FieldType` |
|---|---|
| `i32` / `u32` | `Integer` |
| `i64` / `u64` | `BigInt` |
| `f32` | `Float` |
| `f64` | `Double` |
| `bool` | `Boolean` |
| `String` / `&str` | `String { max_length }` |
| `chrono::NaiveDate` | `Date` |
| `chrono::NaiveDateTime` | `DateTime` |
| `chrono::DateTime<Utc>` | `Timestamp` |
| `serde_json::Value` | `Json` |
| `Vec<u8>` | `Blob` |
| 用户 enum 派生 `JsonSchema` | `Enum { values }`（编译期从枚举判别式提取） |
| `Option<T>` | 同 `T`，但 `required=false` |

不识别的类型（如自定义 newtype）报编译错误，引导用户加 `#[entity(skip)]` 或自实现。

### 4.5 `#[derive(Action)]` 派生宏

用户写：

```rust
#[derive(Action)]
#[action(
    name = "login",
    public,
    display_name = "用户登录",
    description = "通过用户名密码换取访问令牌"
)]
pub struct LoginAction;

#[async_trait]
impl TypedHandler for LoginAction {
    type Input = LoginInput;
    type Output = LoginOutput;
    async fn handle(&self, ctx, input) -> Result<LoginOutput, BaseError> { ... }
}
```

`#[derive(Action)]` 生成：

```rust
impl TypedAction for LoginAction {
    fn name(&self) -> &'static str { "login" }
    fn display_name(&self) -> &'static str { "用户登录" }
    fn description(&self) -> &'static str { "通过用户名密码换取访问令牌" }
    fn permissions(&self) -> &'static [Permission] { &[] }
    fn is_public(&self) -> bool { true }
    fn input_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(LoginInput))
    }
    fn output_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(LoginOutput))
    }
}

impl LoginAction {
    fn __meta_static() -> &'static ActionMeta {
        static M: OnceLock<ActionMeta> = OnceLock::new();
        M.get_or_init(|| ActionMeta {
            name: "login",
            display_name: "用户登录",
            description: "通过用户名密码换取访问令牌",
            permissions: &[],
            is_public: true,
            input_schema: schemars::schema_for!(LoginInput),  // 简化，实际取 OnceLock
            output_schema: schemars::schema_for!(LoginOutput),
        })
    }
}
```

`#[action(...)]` 属性：

| 属性 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `name` | `&str` | 必填 | Action 唯一标识 |
| `display_name` | `&str` | = name | 展示名 |
| `description` | `&str` | "" | 描述 |
| `public` | flag | false | 公开（跳过权限检查） |
| `permissions` | `[&str]` | `[]` | 静态权限列表，e.g. `permissions("user:read", "audit:log")` |


---

## 5. 内置 Action 重写

六个 builtin 全部泛型化为 `Builtin<T: TableEntity>`。

### 5.1 GetAction

```rust
#[derive(Deserialize, JsonSchema)]
pub struct GetByPk<PK> {
    pub id: PK,
}

pub struct GetAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> GetAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for GetAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = T;

    async fn handle(&self, ctx: ActionContext, input: GetByPk<T::Pk>)
        -> Result<T, BaseError>
    {
        let query = ctx.table_query()?
            .where_eq(T::PK_FIELD, serde_json::to_value(&input.id)?)?;
        query.fetch_optional::<T>().await?.ok_or_else(||
            BaseError::RecordNotFound(format!(
                "{} 中主键 {} 的记录不存在", T::TABLE_NAME, T::PK_FIELD
            ))
        )
    }
}

impl<T: TableEntity> TypedAction for GetAction<T> {
    fn name(&self) -> &'static str { "get" }
    // ... 其他元信息默认实现
}
```

注意 `GetAction<T>::name()` 必须返回 `&'static str`，所有 `T` 共用 `"get"`——这没问题，因为同一 `ModuleRouter` 里只会注册一��� `GetAction<User>`，路由命名空间天然按 module 划分。

### 5.2 AddAction

```rust
pub struct AddAction<T: TableEntity> { _phantom: PhantomData<T> }

#[derive(Serialize, JsonSchema)]
pub struct AffectedResult {
    pub affected: u64,
}

#[async_trait]
impl<T: TableEntity> TypedHandler for AddAction<T> {
    type Input = T;            // 整个实体作为输入（Pk 字段可为 Option，由用户决定）
    type Output = AffectedResult;

    async fn handle(&self, ctx: ActionContext, input: T) -> Result<AffectedResult, BaseError> {
        let value = serde_json::to_value(&input)?;
        let map = match value {
            Value::Object(m) => m.into_iter().collect::<HashMap<_, _>>(),
            _ => return Err(BaseError::ParamInvalid("body".into(), "must be object".into())),
        };
        let affected = ctx.table_query()?.insert(map).await?;
        Ok(AffectedResult { affected })
    }
}
```

如果用户想插入后返回插入的实体（含自增 PK），需写自定义 Action（不在 builtin 范围）。

### 5.3 PutAction

```rust
#[derive(Deserialize, JsonSchema)]
pub struct PutInput<T: TableEntity> {
    pub id: T::Pk,
    /// 字段更新对：每个元素是 (字段, 新值)。用 Vec 而非 HashMap，因为 JSON object key
    /// 必须是字符串，而 `T::Field` 在 JSON 中表示为单元变体字符串（如 "username"），
    /// 用 Vec<(T::Field, Value)> 直接映射 JSON 数组形态最干净。
    ///
    /// JSON 形态：
    /// ```json
    /// { "id": 1, "data": [["username", "alice"], ["email", "a@b.com"]] }
    /// ```
    pub data: Vec<(T::Field, serde_json::Value)>,
}

#[async_trait]
impl<T: TableEntity> TypedHandler for PutAction<T> {
    type Input = PutInput<T>;
    type Output = AffectedResult;

    async fn handle(&self, ctx: ActionContext, input: PutInput<T>) -> Result<AffectedResult, BaseError> {
        if input.data.is_empty() {
            return Err(BaseError::ParamInvalid("data".into(), "至少一个字段".into()));
        }
        let pk_value = serde_json::to_value(&input.id)?;
        let data: HashMap<String, Value> = input.data.into_iter()
            .map(|(field, value)| (field.column_name().to_string(), value))
            .collect();
        let affected = ctx.table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .update(data)
            .await?;
        Ok(AffectedResult { affected })
    }
}
```

值类型校验：`data` 的 value 仍是 `Value`（因为更新可以是任意字段子集，不能用整个 `T`）。在 `update()` 调用前，由 `TableConfig::get_field(field).field_type.validate(value)` 做运行时类型校验——这部分校验现已存在，沿用即可。字段名通过 `T::Field` 枚举保证编译期/反序列化期合法。

### 5.4 DelAction

```rust
pub struct DelAction<T: TableEntity> { _phantom: PhantomData<T> }

#[async_trait]
impl<T: TableEntity> TypedHandler for DelAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = AffectedResult;
    async fn handle(&self, ctx, input) -> Result<AffectedResult, BaseError> {
        let pk_value = serde_json::to_value(&input.id)?;
        let affected = ctx.table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .delete()
            .await?;
        Ok(AffectedResult { affected })
    }
}
```

软删除：如果 `T::table_config().soft_delete_field` 存在，`delete()` 内部已经走 UPDATE 而非物理删除（现有逻辑不变）。

### 5.5 SelectAction

```rust
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectQuery<T: TableEntity> {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub fields: Option<Vec<T::Field>>,
    /// 多条件 AND 连接；空 Vec 表示无 where。JSON key 为 `"where"`（rust 字段加下划线避开关键字）
    #[serde(default, rename = "where")]
    pub where_clause: Vec<T::WhereCond>,
    #[serde(default)]
    pub order_by: Vec<OrderByItem<T>>,
    /// 是否返回总数（额外一次 COUNT 查询，默认 false 以省略开销）
    #[serde(default)]
    pub count_total: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrderByItem<T: TableEntity> {
    pub field: T::Field,
    #[serde(default = "default_sort_order")]
    pub direction: SortOrder,
}

fn default_sort_order() -> SortOrder { SortOrder::Asc }

fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 10 }

#[derive(Serialize, JsonSchema)]
pub struct SelectResult<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    /// 总数。仅当请求的 `count_total = true` 时返回。
    pub total: Option<u64>,
}

#[async_trait]
impl<T: TableEntity> TypedHandler for SelectAction<T> {
    type Input = SelectQuery<T>;
    type Output = SelectResult<T>;

    async fn handle(&self, ctx, input) -> Result<SelectResult<T>, BaseError> {
        // 范围校验
        if input.page == 0 || input.page_size == 0 || input.page_size > 100 {
            return Err(BaseError::ParamInvalid("page/page_size".into(),
                "page>=1, 1<=page_size<=100".into()));
        }

        // 把 where_clause 转成 SqlCondition 序列（一次消耗）
        let conditions: Vec<SqlCondition> = input.where_clause.into_iter()
            .map(|c| c.into_sql_condition())
            .collect();

        // count 查询（如需要）：与 select 共享 where 条件
        let total = if input.count_total {
            Some(count_with_conditions::<T>(&ctx, &conditions).await?)
        } else {
            None
        };

        // 构建主查询
        let mut q = ctx.table_query()?;
        if let Some(fields) = input.fields {
            let names: Vec<&str> = fields.iter().map(|f| f.column_name()).collect();
            q = q.select_fields(&names)?;
        }
        for cond in &conditions {
            q = apply_sql_condition(q, cond)?;
        }
        for OrderByItem { field, direction } in input.order_by {
            q = q.order_by(field.column_name(), direction)?;
        }
        let items: Vec<T> = q.paginate(input.page, input.page_size).fetch_all().await?;
        Ok(SelectResult { items, page: input.page, page_size: input.page_size, total })
    }
}
```

`apply_sql_condition` 与 `count_with_conditions` 是 yang-base 内部 helper：

```rust
/// 把 SqlCondition 翻译为 TableQuery 上的对应链式调用
pub(crate) fn apply_sql_condition<'a>(
    q: TableQuery<'a>,
    cond: &SqlCondition,
) -> Result<TableQuery<'a>, BaseError> {
    match cond.op {
        SqlOp::Eq      => q.where_eq(cond.column, cond.params[0].clone()),
        SqlOp::Ne      => q.where_ne(cond.column, cond.params[0].clone()),
        SqlOp::Lt      => q.where_lt(cond.column, cond.params[0].clone()),
        SqlOp::Lte     => q.where_lte(cond.column, cond.params[0].clone()),
        SqlOp::Gt      => q.where_gt(cond.column, cond.params[0].clone()),
        SqlOp::Gte     => q.where_gte(cond.column, cond.params[0].clone()),
        SqlOp::In      => q.where_in(cond.column, cond.params.clone()),
        SqlOp::Between => q.where_between(cond.column,
                            cond.params[0].clone(), cond.params[1].clone()),
        SqlOp::Like    => q.where_like(cond.column, cond.params[0]
                            .as_str().unwrap_or("").to_string()),
        SqlOp::IsNull  => Ok(q.where_null(cond.column)),
        SqlOp::IsNotNull => Ok(q.where_not_null(cond.column)),
    }
}

/// 给定相同的 where 条件，跑一次 SELECT COUNT(*) 计算总数
pub(crate) async fn count_with_conditions<T: TableEntity>(
    ctx: &ActionContext,
    conditions: &[SqlCondition],
) -> Result<u64, BaseError> {
    let mut q = ctx.table_query()?;
    for cond in conditions {
        q = apply_sql_condition(q, cond)?;
    }
    q.count().await
}
```

`column` 已是 `&'static str`，绝对安全。`TableQuery` 现状已有 `where_eq` / `where_in` / `where_between` 等方法；如缺 `where_lt` / `where_gt` 等需要补齐——这部分作为步骤 5（builtin 重写）的前置子任务列入实施清单。

### 5.6 TableAction

```rust
pub struct TableAction<T: TableEntity> { _phantom: PhantomData<T> }

#[derive(Serialize, JsonSchema)]
pub struct TableSchemaResponse {
    pub table_name: &'static str,
    pub primary_key: &'static str,
    pub fields: Vec<FieldSchema>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}

#[async_trait]
impl<T: TableEntity> TypedHandler for TableAction<T> {
    type Input = ();
    type Output = TableSchemaResponse;
    async fn handle(&self, _ctx, _input: ()) -> Result<TableSchemaResponse, BaseError> {
        Ok(TableSchemaResponse {
            table_name: T::TABLE_NAME,
            primary_key: T::PK_FIELD,
            fields: T::table_config().fields.values().map(field_schema_from).collect(),
            input_schema: serde_json::to_value(schemars::schema_for!(T))?,
            output_schema: serde_json::to_value(schemars::schema_for!(T))?,
        })
    }
}
```

`Input = ()` 反序列化要求请求体是空对象 `{}`、空数组 `[]` 或 `null`——通过 `serde(default)` 容忍空 body。


---

## 6. ModuleRouter 集成

### 6.1 一行注册全套 CRUD

```rust
impl ModuleRouter {
    pub fn table_typed<T: TableEntity>(mut self) -> Self {
        // 内部记录 T::table_config() 到 self.table_config
        self.table_config = Some(Arc::new(T::table_config().clone()));
        // 注册六个 builtin（顺序无关）
        self.register(Arc::new(GetAction::<T>::new()))
            .register(Arc::new(AddAction::<T>::new()))
            .register(Arc::new(PutAction::<T>::new()))
            .register(Arc::new(DelAction::<T>::new()))
            .register(Arc::new(SelectAction::<T>::new()))
            .register(Arc::new(TableAction::<T>::new()))
    }

    pub fn register<A: Action>(mut self, action: Arc<A>) -> Self {
        self.actions.insert(action.meta().name.to_string(), action as Arc<dyn Action>);
        self
    }
}
```

### 6.2 dispatch 不变

```rust
pub async fn dispatch(
    &self,
    action_name: &str,
    mut context: ActionContext,
) -> Result<ApiResponse, BaseError> {
    let action = self.actions.get(action_name)
        .ok_or_else(|| BaseError::ActionNotFound(action_name.to_string()))?;

    if let Some(table_config) = &self.table_config {
        context = context.with_table_config(table_config.clone());
    }

    let meta = action.meta();
    if !meta.is_public {
        let user = context.user.as_ref()
            .ok_or_else(|| BaseError::Unauthorized("需要登录".into()))?;
        if !meta.permissions.is_empty()
            && !meta.permissions.iter().all(|p| user.has_permission(p.name())) {
            return Err(BaseError::PermissionDenied(format!(
                "缺少权限: {:?}", meta.permissions
            )));
        }
    }

    action.dispatch(context).await   // 这里走 blanket impl 的 extract → handle → wrap
}
```

权限检查从遍历 trait 方法（`is_public()`、`permissions()`）改为从 `ActionMeta` 读静态字段——单个间接读取，性能略优于现状。

### 6.3 Schema 暴露

```rust
impl ModuleRouter {
    /// 返回模块下所有 Action 的元信息（用于 /docs 端点）
    pub fn list_actions(&self) -> Vec<&ActionMeta> {
        self.actions.values().map(|a| a.meta()).collect()
    }
}
```

调用方可以把 `Vec<&ActionMeta>` 序列化为 JSON 暴露 `/api/<module>/_schema`，前端据此 codegen TypeScript 类型。

---

## 7. 错误处理

| 错误情形 | 错误类型 | 出错点 |
|---|---|---|
| 请求体反序列化为 `Input` 失败（字段缺失/类型不匹配/未知字段） | `BaseError::ParamInvalid("body", serde 错误信息)` | `ActionContext::extract_input` |
| 非法字段名（不在 `T::Field` 枚举内） | `BaseError::ParamInvalid("body", "unknown variant ...")` | 同上（serde 反序列化阶段） |
| 主键记录不存在 | `BaseError::RecordNotFound` | `GetAction::handle` |
| 无权限 | `BaseError::PermissionDenied` | `ModuleRouter::dispatch` 权限检查 |
| 未登录 | `BaseError::Unauthorized` | 同上 |
| 数据库错误 | `BaseError::DatabaseQueryFailed(DbError)` | `TableQuery::*` |
| Action 不存在 | `BaseError::ActionNotFound` | router 查表 |

`#[serde(deny_unknown_fields)]` 应用于所有 builtin 的 `Input` struct，前端传来的多余字段直接报错——避免静默吞参数（提高契约严格度，符合"安全最优"目标）。

---

## 8. 性能特性

- **dyn dispatch 次数**：1 次（`Action::dispatch`）。与现状相同
- **序列化次数**：入口 1 次反序列化（请求体 → `Input`），出口 1 次序列化（`Output` → JSON）。**比现状少一次**——现状 builtin 内部把 `DynamicRow` → `Value` → `ApiResponse.data` 中间多一次 `to_value`
- **Schema 生成**：`OnceLock` 全程序生成一次，运行时零成本
- **Field/WhereCond 枚举**：派生为 `#[repr(...)]` Rust 默认枚举，匹配判别式 = 单条 jump 指令；`column_name()` 内联为返回静态字符串
- **column_name 来源**：完全是 `&'static str`，无动态分配
- **Arc 引用计数**：`Arc<dyn Action>` 在 router 中引用计数 = 1 + 派发期间临时 +1，与现状相同

---

## 9. 安全特性

- **SQL 注入面**：列名空间从"任意 `&str`"收窄到"`T::Field` 枚举判别式"。整个 builtin 路径中，列名永不来自用户输入字符串
- **未声明字段拒收**：`#[serde(deny_unknown_fields)]` 强制契约严格——前端误传字段直接报错
- **类型不匹配早失败**：`{"where": [{"field": "id", "cond": {"op": "eq", "value": "abc"}}]}`（id 是 i64，传字符串）在反序列化阶段就失败，不会到 SQL 层
- **`Like` 仅限 String 字段**：派生宏只在 String 字段对应的 WhereCond variant 中生成 Like 变体，从类型系统层面禁止 `LIKE` 用于数值字段
- **`Like` pattern 转义**：本设计**不**做自动转义。`%`/`_`/`\` 等元字符的语义按用户提供的 pattern 原样传给 MySQL（与 sqlx 参数化查询一致）。文档需明确警示：用户输入直接拼到 pattern 中存在通配符注入风险，需要用户在 handler 层 `escape_like_pattern()`
- **`Between` 边界**：双闭区间（与 SQL `BETWEEN` 语义一致），文档说明
- **JsonSchema 与 chrono**：依赖 `schemars` 的 `chrono` feature；chrono 类型派生为 ISO 8601 字符串

---

## 10. 测试策略

### 10.1 单元测试

| 模块 | 测试要点 |
|---|---|
| `action::trait` | blanket impl 路径：mock 一个 `TypedHandler`，调用 `Action::dispatch`，断言 input 反序列化、output 序列化路径走通 |
| `action::context` | `extract_input` 各失败模式（缺字段、类型错、未知字段） |
| `action::meta` | `ActionMeta` 静态聚合一致性 |
| `table::entity::WhereOp` | JSON 反序列化各 variant；非法 `op` 报错 |
| `table::entity::IntoSqlCondition` | 各 `WhereOp` → `SqlCondition` 映射正确 |

### 10.2 派生宏测试

`yang-base-derive/tests/`：

- 正常派生：`User` 例子全套字段类型，断言生成的 `UserField`/`UserWhere` 反序列化合法值通过、非法字段名失败
- `#[entity(primary_key)]` 缺失或多个：编译错误（用 `compile_fail` doc test）
- `#[entity(skip)]` 字段不出现在 `Field` 枚举
- `Option<T>` 字段：`required = false`、`Field` 枚举仍包含、`WhereCond` 仍包含

### 10.3 集成测试

`crates/yang-base/tests/typed_action_integration.rs`：

```rust
#[derive(Deserialize, Serialize, JsonSchema, FromRow, TableEntity)]
#[table(name = "test_users")]
struct TestUser {
    #[entity(primary_key)]
    id: i64,
    #[entity(max_length = 50)]
    username: String,
    age: i32,
}

#[tokio::test]
async fn full_crud_cycle() {
    let router = ModuleRouter::new("user", "用户").table_typed::<TestUser>().build();

    // add → get → put → select → del → table，每步断言强类型 ApiResponse.data 反序列化为
    // 预期类型（AffectedResult / TestUser / SelectResult<TestUser> / TableSchemaResponse）

    // 关键断言：select where 用 TestUserWhere::Username(WhereOp::Like("%a%")) 的 JSON
    // 形态被正确反序列化、生成正确 SQL、返回正确结果
}
```

### 10.4 trybuild 编译失败测试

`yang-base/tests/compile_fail/`，每个文件配 `.stderr` 期望：

| 测试用例 | 期望失败原因 |
|---|---|
| `where_invalid_field.rs` | `TestUserWhere::NoSuchField(...)` 编译失败：variant 不存在 |
| `where_type_mismatch.rs` | `TestUserWhere::Id(WhereOp::Eq("string"))`（id 是 i64） |
| `like_on_int.rs` | `TestUserWhere::Id(WhereOp::Like(...))`：`WhereOp` 无 `Like` 变体 |
| `add_missing_field.rs` | `AddAction<TestUser>` 输入缺 required 字段——这部分仍是运行时检查（serde），故 trybuild 不覆盖 |

### 10.5 Schema 快照测试

`crates/yang-base/tests/schema_snapshots/`：

- `TestUser` 的 input schema 序列化为 JSON，与快照文件比对
- `SelectQuery<TestUser>` 的 input schema 同
- 改动派生宏后快照变化由人工确认

使用 `insta` crate 做快照管理。

---

## 11. 实施顺序

每步可独立编译/合并，建议每步一个 PR：

### 步骤 1：基础 trait + ActionContext

- 新增 `TypedHandler`、`TypedAction`、`Action`（重命名旧 `Action` 为 `LegacyAction` 或直接破坏式替换——后者）
- 新增 `ActionMeta`、`ActionContext::extract_input`
- 移除 `ActionContext::param`、`param_optional`、`param_or_default`、`query_param`
- 单元测试：mock TypedHandler 走通 dispatch

**验收**：`cargo test --lib -p yang-base action::` 通过

### 步骤 2：手写 TableEntity 接口

- 新增 `crates/yang-base/src/table/entity.rs`：`TableEntity` trait + `WhereOp<V>` + `AsColumnName` + `IntoSqlCondition` + `SqlCondition` + `SqlOp`
- 不动派生宏；先用纯手写让一个 entity 跑通（证明设计自洽）
- 测试：手写 `TestUser`、`TestUserField`、`TestUserWhere`，集成测试调用 `GetAction<TestUser>`

**验收**：手写实现的集成测试通过

### 步骤 3：`yang-base-derive` crate

- 新建 `crates/yang-base-derive/`，`proc-macro = true`
- 实现 `#[derive(TableEntity)]`：解析 `#[table(...)]` 与 `#[entity(...)]`，生成 `Field` / `WhereCond` 枚举 + `TableEntity` 实现
- 在 `yang-base` 中 `pub use yang_base_derive::TableEntity`
- 派生宏单元测试 + 编译失败测试

**验收**：步骤 2 的手写代码用 `#[derive(TableEntity)]` 替换后所有测试仍通过

### 步骤 4：`#[derive(Action)]`

- 在 `yang-base-derive` 实现 `#[derive(Action)]`：解析 `#[action(...)]`，生成 `TypedAction` impl + `__meta_static`
- 测试：用 `#[derive(Action)]` 写 LoginAction，断言 meta 字段正确

**验收**：自定义 Action 的元信息派生正确

### 步骤 5：六个 builtin Action 重写

- 按 5.1–5.6 顺序实现：Get → Add → Del → Put → Select → Table
- 每个 builtin 一个集成测试 + 一个 `compile_fail` 测试

**验收**：`cargo test -p yang-base` 全套通过

### 步骤 6：ModuleRouter 集成

- 新增 `table_typed::<T>()`、`register<A: Action>()`、`list_actions()`
- 重写 `dispatch` 用 `ActionMeta`
- 删除旧 builtin 注册逻辑

**验收**：完整 CRUD 集成测试（10.3）通过

### 步骤 7：trybuild + schema 快照

- 添加 `trybuild` dev-dep，编写所有 `compile_fail` 测试
- 添加 `insta`，写 schema 快照测试

**验收**：trybuild 全绿、快照已建立

### 步骤 8：文档与示例更新

- 更新 `crates/yang-base/AGENTS.md`、`docs/yang-base.md`
- 更新所有现有示例代码到新 API
- 更新 `docs/BACKLOG.md`：H-1 状态改为 ✅ 已完成

**验收**：文档与代码一致；`cargo doc --no-deps` 无警告

---

## 12. 破坏性变更清单

迁移指南会作为单独文档（`docs/migrations/2026-05-27-typed-action.md`），本 spec 仅列出破坏点：

1. **`Action` trait 全部签名变化**：`execute → dispatch`，参数 `(ctx)` 不再带 `Input`
2. **用户实现 Action 的方式变化**：从 `impl Action for X` 改为 `#[derive(Action)] + impl TypedHandler for X`
3. **`ActionContext::param*` 方法移除**：所有参数访问改为 `Input` struct 字段
4. **`TableConfig` 不再手写**：改用 `#[derive(TableEntity)]`；手写 `TableConfig` 仍合法但必须配套手写 `TableEntity` 实现
5. **`ModuleRouter::table_config(...)` 删除**：改为 `table_typed::<T>()`
6. **`ApiResponse::data` 类型不变**（仍为 `Option<Value>`），但内部已是从 `Output: Serialize` 序列化而来——无观测变化
7. **`SelectAction` 输入 JSON 形态变化**：`where`、`order_by` 的 JSON shape 与现状不兼容（见 4.2、5.5）；前端需按新 schema 调整

---

## 13. 待 spec 后续迭代的项

- **OR / 嵌套 where**：第二轮加 `WhereCond::Or(Vec<WhereCond>)` / `WhereCond::And(...)` / `WhereCond::Not(Box<WhereCond>)`
- **`extract_query` for URL 查询参数**：等到有 builtin 需要时再加
- **路径参数 extractor**：当路由层引入路径模式时再做（axum 风格）
- **JSON Schema 端点暴露规范**：`/api/_schema`、`/api/<module>/_schema`、`/api/<module>/<action>/_schema` 路由格式

---

## 14. 验收清单

实现完成必须满足：

- [ ] `cargo test --workspace` 全绿（含 trybuild、insta）
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [ ] `cargo doc --workspace --no-deps` 无警告
- [ ] 一个完整集成测试：`add → get → put → select → del → table` 全部走 typed 路径
- [ ] 至少 4 个 trybuild `compile_fail` 用例覆盖字段名错误、类型错误、Like 限制、`primary_key` 缺失
- [ ] Schema 快照已建立并 commit
- [ ] `docs/yang-base.md` 与 `crates/yang-base/AGENTS.md` 已更新到新 API
- [ ] `docs/BACKLOG.md` H-1 状态 → ✅
