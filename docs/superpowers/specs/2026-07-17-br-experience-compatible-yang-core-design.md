# YANG 底层升级与 BR 开发体验兼容架构设计

**日期**：2026-07-17

**状态**：已批准，采用 5.2 方案 B

**适用范围**：`yang-system`、`yang-base`、`yang-db`

**最终决策**：保留 BR 的开发心智、配置逻辑和高频操作方式，但不提供兼容 Adapter、兼容投影或第二条运行链路

**核心目标**：让熟悉 `scs-api + br-addon + br-fields + br-db` 的开发者，可以顺滑地转向 YANG，而不是重新学习一套完全不同的应用开发方式

---

## 1. 一句话决策

YANG 不应成为一套与 BR 对立的新框架，而应成为 BR 开发模型的安全、类型化、异步化和高性能升级：

- 永久保留 `Addon / Module / Action / Fields / Params / Tables / Tools / Plugins` 这些高频业务概念；
- 保留数据库表配置、HTTP 接口配置、Module 配置、链式查询和内部 Action 调用的业务操作逻辑与使用顺序；
- 用强类型 Catalog、显式资源所有权、参数绑定 SQL、异步事务和启动期校验替换 BR 的动态 JSON、字符串分派、进程全局可变状态和线程局部事务；
- 所有业务配置直接构建 YANG 原生定义，所有请求直接执行 YANG 原生运行时，不存在兼容 Adapter、动态旁路或新旧双轨；
- Schema、OpenAPI 和后台元数据可以从同一份原生定义生成，但这些投影只生成产物，不承担 BR 兼容，也不参与请求热路径；
- 开发体验连续性不能以增加请求期转换、动态分派、重复序列化、额外锁或额外分配为代价。

目标不是“兼容 BR 的每一个方法签名或让旧代码不修改即可运行”，而是“使用一条新的原生链路，保留开发者完成业务任务时已经形成的认知路径”。

---

## 2. 背景与事实依据

### 2.1 BR 生态真正提供的价值

`scs-api` 的生产力并不主要来自某一个 trait，而来自一套稳定的业务开发流程：

1. 创建一个 Addon；
2. 在 Addon 下创建 Module；
3. 在 Module 中通过 `fields()` 描述表字段；
4. 为 Module 增加 Action；
5. 在 Action 中通过 `params()` 描述输入；
6. 通过 `self.tools()` 获取数据库、缓存、邮件等能力；
7. 使用 `where_and`、`table_list`、`table_select` 完成查询和后台页面数据；
8. 使用 `Plugins::api_run` 或 `Plugins::action` 调用其他 Action 或生成按钮；
9. 字段和参数元数据继续投影为数据库结构、校验、Swagger 和后台 UI 描述。

这套流程已经形成了成熟的开发心智模型。即使底层存在明显技术债，开发者仍能快速复制一个 Module 并完成业务功能。

### 2.2 高频使用路径

对 `D:\code\scs-api\src` 的只读统计显示：

| 使用模式 | 出现次数 | 涉及文件数 | 设计含义 |
|---|---:|---:|---|
| `where_and(...)` | 4,563 | 1,276 | 链式动态查询是最核心的日常体验 |
| `self.tools()` | 3,882 | 1,148 | 统一工具入口是高频依赖获取方式 |
| `fn params()` | 1,756 | 1,756 | Action 参数声明是固定开发仪式 |
| `Plugins::action` | 849 | 189 | Action 元数据和按钮引用被大量复用 |
| `Plugins::api_run` | 461 | 247 | Module 间调用依赖统一分派入口 |
| `Tables::params_table` | 417 | 417 | 通用列表参数具有稳定心智模型 |
| `table_select(...)` | 357 | 185 | 关系选择器是后台业务高频能力 |
| `fn fields()` | 217 | 217 | 每个 Module 都围绕一份字段声明组织 |

因此，如果 YANG 删除这些概念并要求开发者转向完全不同的 Repository、Controller、Extractor 或 ORM 模型，即使内部设计更“纯”，迁移体验也会失败。

### 2.3 当前 YANG 已具备的升级基础

当前实现已经解决了 BR 底层的许多关键问题：

- `yang-db` 使用 SQLx、参数绑定和结构化错误；
- `Condition`、`SqlValue` 和 checked identifier 收窄了 SQL 注入面；
- 事务具有显式异步生命周期，不依赖线程 ID；
- `TableDefinition` 是校验后不可变定义；
- `TableQuery` 能集中处理字段权限、查询条件、软删除和分页限制；
- `TypedHandler`、`ActionMeta` 和 `ApiCatalog` 已经具有强类型接口契约；
- 启动期 Schema 同步是 additive、可规划、fail-fast 的；
- OpenAPI 和可选后台元数据已经存在投影基础。

问题不在于底层能力不足，而在于这些能力尚未被组织成一套与 BR 同样顺手的开发 Interface。

---

## 3. 第一性原理

### 3.1 开发体验不是方法名，而是决策路径

开发者感受到的“顺滑”来自以下问题是否容易回答：

- 我在哪里声明表字段？
- 我在哪里声明接口参数？
- 我怎样拿到数据库、Redis、邮件等工具？
- 我怎样写一条带条件的查询？
- 我怎样生成标准列表和选择器？
- 我怎样调用另一个 Action？
- 我怎样知道字段声明会影响数据库、校验、文档和 UI？

如果这些问题在 YANG 中仍然对应 `fields / params / tools / tables / plugins`，迁移者可以复用原有经验。底层类型和执行模型可以彻底改变，而不必迫使业务开发者重建全部心智模型。

### 3.2 保留本质复杂度，删除偶然复杂度

应保留的本质复杂度：

- Addon、Module、Action 的业务层级；
- 字段、参数、表格视图、权限和关系选择器；
- Module 间调用；
- 多租户、用户上下文、事务和外部工具；
- 不同业务产品通过 Cargo feature 组合。

应删除的偶然复杂度：

- 用 `JsonValue` 表达所有类型；
- 通过固定的 Rust 类型路径下标推导名称；
- 手写巨型字符串 `match` 注册 Action；
- 每次调用重新创建 Addon、Module 和 Action；
- 线程局部变量承载请求上下文和事务；
- 进程全局 `Mutex<HashMap<...>>` 保存数据库和工具；
- SQL 值和操作符直接拼接；
- 参数错误通过 panic、空 JSON、`false` 或字符串成功值表达；
- 启动时扫描源码文件发现接口。

### 3.3 一个业务事实只声明一次，但不同关注点不能强行合并

字段的“名称、语义类型、长度、是否必填”应只声明一次，并可投影到：

- 数据库 Schema；
- 输入校验；
- OpenAPI；
- 后台表格和表单；
- 查询权限和排序能力。

但是数据库字段、HTTP 输入和某一个后台页面不是同一个模型：

- `password_hash` 存在于数据库，但不应出现在普通输出；
- 注册参数包含 `password_confirm`，但它不是数据库列；
- 同一 users 表可以有平台用户列表、组织成员列表和用户选择器；
- 不同 Action 对同一字段可以有不同的必填和可写规则。

因此采用“共享语义核心 + 独立投影/覆盖”，不采用一个巨型 Field 对象直接拥有全部 SQL、校验、Swagger 和 UI 实现。

### 3.4 错误必须左移，但不能把复杂度推给业务开发者

错误发现顺序应为：

```text
编译期
  └─ 输入输出类型、Handler 类型不匹配

启动期定义构建
  └─ 重复名称、无效字段引用、无效 Action 引用、路由冲突

启动期 Schema 规划
  └─ 数据库结构不兼容

请求解析期
  └─ 参数缺失、类型错误、值校验错误

业务执行期
  └─ 业务不变量、数据库和外部工具错误
```

业务开发者不应为了获得这些保证而手写四层 trait 或重复声明同一元数据。宏和 Builder 的职责是把错误左移，同时隐藏实现复杂度。

### 3.5 熟悉的 Interface 必须对应更深的 Module

本设计中的永久 Module 必须通过删除测试：

- 删除 `Fields`，DDL、校验、文档和 UI 字段逻辑会重新散落到每个调用者，因此 `Fields` 有 Depth；
- 删除 `Tables`，分页、过滤、关系批量加载、权限和响应结构会重新散落，因此 `Tables` 有 Depth；
- 删除 `Tools`，资源初始化、健康检查、关闭和依赖获取会重新散落，因此 `Tools` 有 Depth；
- 删除 `Plugins`，Action 解析、权限、内部调用、元数据获取会重新散落，因此 `Plugins` 有 Depth。

BR 风格 Interface 必须直接属于这些深 Module；禁止再包一层只转发调用的 Shallow 兼容 Module。

---

## 4. 开发体验连续性的边界

本设计不再把“兼容”理解为旧代码或旧数据结构可以继续运行。最终只有一套 YANG 原生 Interface 和一条执行链路。

| 类别 | 内容 | 决策 |
|---|---|---|
| 永久保留 | Addon、Module、Action、Fields、Params、Tables、Tools、Plugins 的业务心智 | 保留 |
| 永久保留 | `fields()`、`params()`、`tools()`、`where_and()`、`table_list()`、`api_run()` 的操作逻辑和调用顺序 | 保留 |
| 原生升级 | 强类型字段/参数/Action 引用、`Result`、async/await、显式上下文 | 统一采用 |
| 不提供 | BR JsonValue、BR Request/Response、动态字符串 Action 旁路 | 拒绝 |
| 不提供 | 兼容 Adapter、兼容投影、兼容 feature、双 Registry、双 QueryBuilder | 拒绝 |
| 不提供 | 全局可变状态、线程局部事务、字符串 SQL、panic/false 错误 | 拒绝 |

因此，“顺滑切换”意味着业务配置步骤熟悉、代码结构可机械改写，而不是保留一条旧语义运行路径。

### 4.1 允许存在的必要差异

以下变化会直接出现在业务代码中，但它们是底层升级不可避免且值得承担的成本：

- 数据库和内部 Action 调用变成异步，需要 `.await`；
- 可能失败的 Builder 操作返回 `Result`，需要 `?`；
- 业务代码从 `self.tools()` 转为 `ctx.tools()`，使请求和资源所有权显式；
- 表、字段、Action 和 View 使用构建期生成的受控引用，不接受请求热路径中的任意字符串；
- 查询操作符统一使用受限 `CompareOp`；
- Action 统一接收强类型 Input，不再从任意 JSON 中反复取值；
- BR 业务代码需要一次性迁移到原生 YANG 写法，不能停留在中间兼容形态。

---

## 5. 方案比较

### 5.1 方案 A：完整复制 BR Interface，内部转发到 YANG

做法：复刻 `br-addon`、`br-fields`、`br-db` 的公开方法，尽量让旧业务代码只替换 import。

优点：

- 初次迁移改动最少；
- 学习成本最低。

缺点：

- 动态 JSON、字符串名称和模糊错误会成为永久 Interface；
- 兼容外壳会接近底层实现复杂度，形成 Shallow Module；
- 新内核被迫为旧缺陷提供长期语义支持；
- 难以真正发挥 Rust 类型系统、异步事务和 Schema Catalog 的 Leverage。

结论：不采用。

### 5.2 方案 B：BR 开发体验连续的类型化 YANG 内核（最终选定）

做法：永久保留 BR 的业务概念、配置逻辑、术语和高频调用顺序；重新定义其类型、错误、所有权和执行语义；业务代码一次性迁移到唯一的 YANG 原生链路，不提供迁移 Adapter 或运行时兼容旁路。

优点：

- 熟悉 BR 的开发者能快速定位能力；
- 高频业务代码保持相似形态；
- 新代码能获得编译期类型、启动期引用校验、异步安全和参数绑定；
- 定义、Registry、查询和执行从一开始就只有一套；
- 不存在兼容转换、重复序列化、动态分派旁路或额外锁；
- 可以在保持 BR 操作逻辑的同时达到或超过当前 YANG 原生链路性能。

缺点：

- 旧业务代码必须进行一次明确的源码迁移，不能只替换 import；
- 永久 Interface 必须同时兼顾熟悉度、类型安全和热路径性能；
- `fields!`、`params!` 等宏需要较高质量的错误信息。

结论：最终采用，后续设计和实现以本方案为唯一方向。

### 5.3 方案 C：完全重新设计 YANG 领域模型

做法：放弃 Addon/Module/Action 等 BR 术语，改用新的 Entity、Endpoint、Repository、Resource 等模型，再编写迁移手册。

优点：

- 理论上的设计自由度最高；
- 不受旧术语约束。

缺点：

- 开发者必须同时迁移代码和心智模型；
- 大量现有业务模式无法机械映射；
- 新抽象需要重新经过大规模业务验证；
- 很可能得到“底层更好，但开发速度明显下降”的结果。

结论：不采用。

---

## 6. 目标架构

```text
┌──────────────────────────────────────────────────────────────┐
│ 唯一业务开发 Interface                                       │
│ Addon / Module / Action / Fields / Params / Tables / Tools   │
│ Plugins / where_and / table_list / table_select              │
└───────────────────────────┬──────────────────────────────────┘
                            │
                 熟悉术语 + 强类型 Builder/Macro
                            │
┌───────────────────────────▼──────────────────────────────────┐
│ YANG 定义内核                                                 │
│ AppBuilder / Registry / DefinitionCatalog                    │
│ FieldSpec / ParamSpec / TableSpec / ViewSpec / ActionSpec     │
└───────────────┬───────────────────────────┬──────────────────┘
                │                           │
        构建运行时 Registry             构建只读 Catalog
                │                           │
┌───────────────▼──────────────┐   ┌────────▼──────────────────┐
│ AppRuntime                    │   │ 投影 Module                │
│ Dispatcher + Tools            │   │ Schema / OpenAPI / Admin   │
│ Auth + RequestContext         │   │ QueryPolicy / Docs         │
└───────────────┬──────────────┘   └────────┬──────────────────┘
                │                           │
                └──────────────┬────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────┐
│ yang-db                                                       │
│ checked identifier + bound parameters + async transaction    │
│ MySQL/PostgreSQL/Redis Adapter                                │
└──────────────────────────────────────────────────────────────┘
```

从业务配置到数据库执行只有这一条链路。不存在 BR JsonValue 转换层、兼容 Registry、兼容 QueryBuilder 或兼容 Action Dispatcher。

### 6.1 为什么同时存在 Registry 和 Catalog

Catalog 是只读定义快照，不直接参与请求分派；Registry 保存已经构建完成的 Handler 和中间件，用于运行时执行。二者都由同一个 `AppBuilder` 构建，因此不存在两份业务事实：

```rust
let app = AppBuilder::new()
    .addon(OrgAddon::new())
    .addon(UserAddon::new())
    .build(tools)?;

let runtime = app.runtime();
let catalog = app.catalog();
```

这样既保持现有 `ApiCatalog`“只读快照、不参与 dispatch”的安全性质，又避免 Module、路由、表和 Action 分别维护独立注册表。Catalog 的 DDL/OpenAPI/UI 投影只发生在构建期、启动期或离线工具中，不进入请求执行热路径。

---

## 7. 永久保留的 BR 风格业务 Interface

本节代码用于固定目标形态和语义；宏的标点细节可以在实现计划中优化，但不得改变本节描述的开发流程。

### 7.1 Addon：继续表达产品能力组合

```rust,ignore
pub struct OrgAddon;

impl Addon for OrgAddon {
    const NAME: AddonName = addon!("org");

    fn modules(&self) -> Modules {
        modules![OrgOrg::new(), OrgUser::new(), OrgDept::new()]
    }
}
```

保留点：

- Addon 仍是 Cargo feature 和产品能力组合的主要粒度；
- 开发者仍从 `addon/<name>/` 定位业务；
- Addon 仍负责列出自己的 Module。

升级点：

- 名称由显式 `AddonName` 提供，不从 `type_name::<Self>()` 路径下标推导；
- 不再手写字符串 `match`；
- 构建后 Module 集合冻结；
- 重名和依赖缺失在启动时失败。

### 7.2 Module：继续围绕 fields 和 actions 组织

```rust,ignore
pub struct OrgUser;

impl Module for OrgUser {
    const NAME: ModuleName = module!("org.user");
    const TABLE: TableName = table!("org_user");

    fn fields(&self) -> Fields {
        fields! {
            id => Key::new()
                .title("ID"),

            username => Str::new()
                .title("用户名")
                .require(true)
                .max_length(64)
                .unique(true)
                .searchable(true),

            org_org => Table::new(table!("org_org"))
                .title("组织")
                .display([field!("name")])
                .select(action!("org.org.select"))
                .require(true),

            status => Radio::<i8>::new()
                .title("状态")
                .options([(1, "启用"), (0, "禁用")])
                .default(1),

            password_hash => Str::new()
                .title("密码摘要")
                .secret(true)
                .readable(false)
                .writable_by(System),
        }
    }

    fn actions(&self) -> Actions {
        actions![TableAction, SelectAction, Add, Edit, Delete]
    }
}
```

保留点：

- `fields()` 仍是 Module 表语义的主要入口；
- `Key / Str / Table / Radio / Timestamp` 等熟悉字段类型继续存在；
- `title / require / default / select / display` 等高频 Builder 名称尽量保留；
- `actions()` 代替手写 `action(name)` 字符串匹配，但仍在 Module 内集中查看全部 Action。

升级点：

- `Fields` 是强类型集合，不是 `JsonValue`；
- 字段引用使用 `FieldRef`，Action 引用使用 `ActionRef`；
- `fields!` 在构建期拒绝重复字段；
- 关系目标、选择 Action、索引、默认值和权限在启动期交叉校验；
- 字段的数据库值始终通过参数绑定传递。

### 7.3 Fields 的共享语义核心与展示覆盖

每个字段内部拆为：

```text
FieldSpec
├─ StorageSpec       数据库类型、长度、空值、默认值、索引
├─ ValidationSpec    长度、范围、格式、枚举和自定义规则
├─ AccessSpec        可读、可写、secret、角色限制
└─ PresentationSpec  标题、说明、默认控件和展示提示
```

为了保持 BR 的简单体验，Module 默认从 `fields()` 自动生成一个默认表格/表单描述。只有同一张表存在多个使用场景时，才显式增加 View：

```rust,ignore
fn views(&self) -> Views {
    views![
        TableView::new(view!("org.user.list"))
            .columns([USERNAME, ORG_ORG, STATUS, CREATED_AT])
            .search([USERNAME])
            .filter([ORG_ORG, STATUS])
            .actions([EDIT, DELETE]),

        SelectView::new(view!("org.user.select"))
            .value(ID)
            .label([USERNAME])
            .search([USERNAME])
    ]
}
```

这使简单 Module 继续保持“一份 fields 就能工作”，复杂 Module 又不必把所有页面差异塞进数据库字段。

### 7.4 Params：保留参数声明概念，升级为强类型输入

永久 Interface 只提供一种原生写法：使用 `params!` 一次声明参数，同时生成强类型 Input 和静态 `ParamSet`。不存在动态 Params、兼容 Params 或运行时 Schema 转换。

```rust,ignore
params! {
    #[deny_unknown_fields]
    pub AddInput {
        username: Str::from(USERNAME)
            .require(true),

        password: Password::new()
            .title("登录密码")
            .min_length(10)
            .require(true),

        org_org: Table::from(ORG_ORG)
            .require(true),
    }
}
```

宏在编译期生成：

- `AddInput` 强类型结构；
- `impl Params for AddInput`；
- 静态字段访问和反序列化代码；
- JSON Schema/OpenAPI 所需定义；
- 参数来源、必填性和校验规则。

`Str::from(USERNAME)` 复用字段的标题、类型、基础校验和展示提示；Action 可以覆盖必填性、标题或额外规则，但不能把 String 字段改成 Integer。

```rust,ignore
impl Action for Add {
    type Input = AddInput;
    type Output = AddResult;

    async fn index(
        &self,
        ctx: ActionContext,
        input: AddInput,
    ) -> Result<AddResult, ActionError> {
        // 业务实现
    }
}
```

`Action::params()` 由 `type Input` 自动返回 `AddInput` 的静态 `ParamSet`，开发者和工具仍可按 BR 心智查看 params，但不会重复配置，也没有第二种执行方式。请求只进行一次 `bytes → AddInput` 反序列化。

### 7.5 Action：保留 params + index 的阅读顺序

```rust,ignore
#[derive(Action)]
#[action(
    name = "add",
    title = "新增用户",
    method = POST,
    path = "/org/users",
    permissions = ["org.user.add"]
)]
pub struct Add;

impl Action for Add {
    type Input = AddInput;
    type Output = AddResult;

    async fn index(
        &self,
        ctx: ActionContext,
        input: AddInput,
    ) -> Result<AddResult, ActionError> {
        let id = ctx
            .tools()
            .db()
            .table(ORG_USER)?
            .insert(input.into_record())
            .await?;

        Ok(AddResult { id })
    }
}
```

保留点：

- 业务概念仍叫 Action；
- 参数仍可通过 `params()` 查看；
- 业务入口仍叫 `index()`；
- title、权限、method、path 等元数据与 Action 放在一起；
- Action 仍由 Module 注册。

升级点：

- Input/Output 是强类型；
- 路由和 Action 原子注册，不再维护独立字符串映射；
- 内部通过 `DynAction` 擦除存储，不要求业务开发者理解；
- 输入只反序列化一次，输出只序列化一次；
- 所有错误进入结构化 `ActionError`。

### 7.6 Tables：保留列表和选择器的高 Leverage Interface

`Tables` 名称和核心操作继续保留，但内部不再是同时持有数据库查询、字段 JSON、UI 状态和返回 JSON 的巨型可变对象。

永久 Interface：

```rust,ignore
let result = Tables::new(&ctx, ORG_USER)
    .view(ORG_USER_LIST)
    .where_from(&input.where_and)?
    .search(input.search.as_deref())?
    .order(input.order)?
    .page(input.page, input.limit)?
    .table_list()
    .await?;
```

选择器：

```rust,ignore
let result = Tables::new(&ctx, ORG_USER)
    .view(ORG_USER_SELECT)
    .search(input.search.as_deref())?
    .table_select()
    .await?;
```

内部拆为三个有 Locality 的实现：

- `TableQueryPlan`：过滤、搜索、排序、分页和权限；
- `CompiledTableView`：启动期已经编译完成的列、筛选器、按钮和展示元数据；
- `RelationLoader`：批量加载 Table/Tree/Radio 显示值，避免逐行 N+1 查询。

业务开发者只接触一个熟悉的 `Tables` Interface。请求执行时直接读取 `CompiledTableView`，不在热路径做 Catalog 投影或兼容转换。

### 7.7 查询链：保留方法名和顺序，替换执行语义

原生写法：

```rust,ignore
let rows = ctx
    .tools()
    .db()
    .table(ORG_USER)?
    .where_and(STATUS, CompareOp::Eq, 1)?
    .where_and(ORG_ORG, CompareOp::Eq, org_id)?
    .order(CREATED_AT, SortOrder::Desc)?
    .select::<UserRow>()
    .await?;
```

这也是唯一执行写法：

- `ORG_USER / STATUS / ORG_ORG` 在构建期解析为受控引用；
- `CompareOp` 和 `SortOrder` 是封闭枚举，不在请求期解析字符串；
- 值始终进入 SQL 驱动绑定参数；
- UPDATE/DELETE 没有 WHERE 时 fail-closed；
- 查询错误保留结构化原因；
- 事务通过显式 `Transaction` 传递，不与线程绑定。

开发者需要适应的主要差异只有受控引用、`?` 和 `.await`，链式思维保持不变。由于不存在字符串校验旁路，QueryBuilder 也不需要在每次请求中查询 Catalog。

### 7.8 Tools：保留统一入口，删除全局所有权

名称 `Tools` 永久保留：

```rust,ignore
let db = ctx.tools().db();
let cache = ctx.tools().cache()?;
let email = ctx.tools().extension::<EmailClient>()?;
let settings = ctx.tools().config::<ServerSettings>()?;
```

底层结构：

```text
Tools
├─ Database
├─ Option<RedisClient>
├─ Option<TokenManager>
├─ immutable TypeMap extensions
├─ immutable typed config
└─ lifecycle/health registry
```

关键约束：

- `ToolsBuilder` 只在启动期可变，`build()` 后冻结；
- ActionContext 持有 `Arc<Tools>`；
- 不存在进程级 `OnceLock<GlobalTools>`；
- 不存在字符串名称的可变 `HashMap<String, Arc<dyn Any>>`；
- 生命周期、健康检查和关闭由 Tools 内部统一编排；
- 只有真正存在两个 Adapter 时才抽取公共 Seam，不为每个工具预先制造 trait。

`self.tools()` 到 `ctx.tools()` 是有意保留的一处可见迁移：它让“当前请求使用哪个应用实例的资源”变得明确，同时仍保留熟悉的 `tools().db()` 调用节奏。

### 7.9 Plugins：保留统一发现和调用入口，取消静态全局分派

唯一写法：

```rust,ignore
let value = ctx
    .plugins()
    .api_run(USER_TOKEN_VERIFY, VerifyInput { token })
    .await?;

let button = ctx
    .plugins()
    .action(ORG_USER_EDIT)?
    .button();
```

`USER_TOKEN_VERIFY` 在 App 构建期解析成稳定的 Action handle。HTTP Router 和内部调用都直接持有该 handle；请求期不解析字符串、不遍历 Addon/Module、不重新创建 Action。内部强类型调用直接传递 `VerifyInput` 并获得类型化 Output，不经过 JSON 序列化。

### 7.10 请求级“全局变量”：保留便利性，改成类型化上下文

BR 中的 `org_org`、当前用户、request id 等数据，本质上不是进程全局变量，而是请求上下文。

新模型：

```rust,ignore
pub const TENANT_ID: ContextKey<TenantId> = context_key!("org_org");

ctx.request_context().insert(TENANT_ID, tenant_id);
let tenant_id = ctx.request_context().require(TENANT_ID)?;
```

常用值提供直接方法：

```rust,ignore
ctx.tenant().id()
ctx.actor().user_id()
ctx.request_id()
```

请求上下文只接受 `ContextKey<T>`，底层存入请求拥有的 TypeMap，不使用 `thread_local!`。异步任务切换线程不会丢失或串请求。

租户表声明：

```rust,ignore
fields! {
    org_org => Table::new(ORG_ORG).tenant_key(true),
}
```

`TableQuery` 自动：

- 为读取、更新、删除添加租户条件；
- 为新增记录写入租户 ID；
- 拒绝业务请求覆盖租户字段；
- 仅允许显式 system context 绕过租户范围。

---

## 8. 内部定义内核

BR 风格名称就是 YANG 原生业务 Interface，不是兼容外壳。Builder 和宏直接生成更精确的不可变定义：

| 业务 Interface | 内部定义 | 作用 |
|---|---|---|
| `Addon` | `AddonSpec` | 产品能力、依赖、feature 信息 |
| `Module` | `ModuleSpec` | 表、Action、View 的聚合 |
| `Fields` | `FieldSet + TableSpec` | 字段语义和数据库结构 |
| `Params` | `ParamSet` | HTTP 输入来源、校验和展示 |
| `Action` | `ActionSpec + DynAction` | 元数据与运行时执行 |
| `Tables` | `TableQueryPlan + ViewSpec` | 查询和列表/选择器投影 |
| `Tools` | `AppResources` | 显式资源所有权 |
| `Plugins` | `RegistryHandle` | Action 发现、调用和元数据读取 |

业务开发者不需要直接学习这些内部类型；它们为产物生成、校验和测试提供 Locality。这里不存在 BR 对象到 YANG 对象的请求期转换：宏在编译期生成代码，Builder 在启动期一次性构建定义，运行期只使用编译完成的数据。

### 8.1 构建过程

```text
业务 Module Builder/Macro
        │
        ▼
AppBuilder（可变，仅启动期）
        │
        ├─ 名称、引用、路由、权限、关系校验
        ├─ 构建 Registry
        ├─ 构建 DefinitionCatalog
        └─ 冻结 Tools
        │
        ▼
BuiltApp（运行期不可变）
```

### 8.2 产物生成（不是兼容投影）

同一个 `DefinitionCatalog` 产生：

- 数据库 `TableSchema` 和 additive `SchemaPlan`；
- HTTP Router 描述；
- OpenAPI 3.1；
- 后台表格、表单、筛选器、选择器和按钮元数据；
- `TableQueryPolicy`；
- CLI/诊断输出；
- 稳定、可快照测试的定义 JSON。

这些生成器不能修改 Registry、权限或数据库；后台元数据继续保持可选 feature，并且只描述展示，不承担审核流、状态机等业务行为。

“一个定义生成多个产物”与“兼容投影”必须明确区分：

- 前者是原生 YANG 定义的编译结果，保留；
- 后者是把 BR 动态对象转换成 YANG 对象的第二条路径，禁止；
- HTTP 请求不会经过 OpenAPI、后台元数据或 Schema 生成器；
- 生成后的 `CompiledTableView`、路由 handle 和查询策略在运行期直接读取。

---

## 9. 单一链路与性能模型

### 9.1 唯一执行链

```text
编译期
fields! / params! / action! / module!
        │ 生成强类型定义和静态引用
        ▼
启动期
AppBuilder
        │ 一次性校验并解析所有引用
        ├─ CompiledRegistry
        ├─ DefinitionCatalog
        ├─ CompiledTableView
        └─ HTTP Route → ActionHandle
        ▼
请求期
Route 直接命中 ActionHandle
        │ 一次反序列化为强类型 Input
        ▼
Action::index
        │ FieldRef + CompareOp + 绑定值
        ▼
yang-db QueryBuilder / Transaction
        │ 参数绑定
        ▼
SQLx Driver
```

禁止出现：

- BR JsonValue → YANG ParamSpec 的请求期转换；
- 字符串路径 → Action 的请求期多级解析；
- 兼容 QueryBuilder → 原生 QueryBuilder 的二次构建；
- 内部 Action 调用的 JSON 序列化/反序列化；
- Catalog → UI/View 的请求期重复生成；
- 为兼容旧全局调用而增加的锁、thread-local 或隐式 Registry。

### 9.2 热路径约束

1. HTTP 输入只反序列化一次，输出只序列化一次；
2. Router 在启动期绑定 `ActionHandle`，HTTP 请求不按字符串遍历 Addon/Module/Action；
3. 内部 Action 调用使用强类型 Input/Output 和预解析 handle，不经过 JSON；
4. `Tools` 的 Database、Redis、Token 等高频资源使用直接字段访问，不经过字符串或 TypeMap；
5. 可选低频扩展才允许使用 TypeId，并可在 Module 构建期解析后缓存 handle；
6. FieldRef、TableRef、ActionRef 和 ViewRef 在启动期解析，运行期不查 Catalog 验证名称；
7. Params、Fields、ActionMeta 和 CompiledTableView 使用静态引用或共享不可变数据，不按请求 clone 大对象；
8. Registry 和 Catalog 构建后只读，请求路径不获取写锁；
9. `Tables` 对关系显示执行批量加载，不允许逐行 N+1；
10. QueryBuilder 直接写入受限标识符和绑定参数，不构建兼容中间 AST；
11. 标准 CRUD 和自定义 Action 进入同一 Dispatcher，不存在慢速兼容分派；
12. 仅在 HTTP 动态路由必须擦除类型的位置使用一次 `DynAction` 调用，内部调用优先保持静态类型。

### 9.3 性能验收门槛

重构前先建立当前 YANG 原生路径基线，至少覆盖：

- Action dispatch；
- Params 解析；
- 内部 Action 调用；
- `table → where_and → select` 查询构建；
- 普通 CRUD；
- `table_list`；
- 带 Table/Radio 关系的列表；
- Tools 高频访问；
- RequestContext 高频访问；
- 事务内多语句执行。

每个基准同时记录：

- 吞吐量；
- p50/p95 延迟；
- 每请求分配次数和字节数；
- 锁竞争；
- SQL 查询次数；
- 内部序列化次数。

验收规则：

- 等价业务路径不得出现统计显著的吞吐下降或延迟上升；
- 任一稳定超过 3% 的热路径回退视为阻塞问题，必须优化或提交明确证据证明测量误差；
- 每请求分配、锁获取和序列化次数不得因为“BR 体验连续性”增加；
- 内部 Action 调用必须比 BR 的 JsonValue 往返更少分配，并且不得比当前 YANG 强类型调用更慢；
- `table_list` 的数据库查询次数必须与结果行数无关，关系加载按关系种类批量执行；
- 启动期允许承担名称校验、引用解析和元数据生成成本，但应单独设置启动时间和峰值内存基线；
- 任何为熟悉语法增加的宏必须展开为原生调用，宏本身不能引入运行时成本。

### 9.4 性能优先级

当“更像 BR”与“更高性能”发生冲突时，按以下顺序决策：

1. 保留业务操作逻辑和阅读顺序；
2. 调整具体语法，例如使用受控引用代替字符串；
3. 保留强类型、单链路和零兼容转换；
4. 不为源码少改几行而接受请求期性能回退。

---

## 10. 修改路径

破坏性修改被允许，但采用纵向可验证路径。阶段划分只用于实施顺序，不允许产生最终可用的兼容 Interface 或第二条运行链路。

### 阶段 0：冻结 BR 开发体验契约与性能基线

选择三个代表性样板：

1. 简单 CRUD Module；
2. `org.user` 一类包含 Table/Radio/权限/选择器的 Module；
3. 包含事务、Tools 和内部 Action 调用的复杂 Action。

为每个样板保存：

- BR 原始代码；
- 迁移后目标代码；
- 字段、参数、列表、OpenAPI 和 SQL 行为快照；
- 迁移差异说明。
- 当前 YANG Action dispatch、查询构建、Tools、Tables 和事务基准。

验收：本设计列出的所有高频模式都有唯一原生映射；性能基线可重复；不允许在实现中临时发明另一套业务术语或兼容路径。

### 阶段 1：建立名称、引用和定义内核

在 `yang-base` 中建立：

- `AddonName / ModuleName / ActionName / TableName`；
- `FieldRef / ActionRef / ViewRef`；
- `FieldSpec / ParamSpec / TableSpec / ViewSpec / ActionSpec`；
- `AppBuilder / BuiltApp / DefinitionCatalog / Registry`。

先用手写 Builder 跑通一个最小 Module，不立即开发复杂宏。Builder 直接生成最终定义，不经过中间兼容对象。

验收：重复名称、无效关系、无效 Action 引用和路由冲突都在 `build()` 失败。

### 阶段 2：把 yang-db 塑造成 BR 熟悉的安全查询 Interface

保留或补齐高频方法名：

- `table`
- `where_and / where_or`
- `field`
- `join`
- `order`
- `page / limit`
- `select / find / count`
- `insert / update / delete`
- `transaction / commit / rollback`

同时强制：

- 受控 TableRef/FieldRef；
- 受限操作符；
- 值参数绑定；
- 结构化 `DbError`；
- 显式异步事务；
- UPDATE/DELETE 无条件 fail-closed。

不提供任意字符串表名、字段名和操作符的并行入口。字符串字面量只允许出现在 `table! / field!` 等定义宏中，并在编译期或启动期解析为受控引用。

验收：代表性 BR 查询保持 `table → where_and → order/page → select` 的顺序；生成 SQL 不包含用户值插值；QueryBuilder 不在请求期查询 Catalog 或解析操作符。

### 阶段 3：用 Tools 统一资源所有权

新增冻结的 `ToolsBuilder -> Tools`，接入 Database、Redis、Token 和扩展工具；修改 `ActionContext` 显式持有 `Arc<Tools>`。

同时改造：

- 健康检查；
- 生命周期关闭；
- 配置读取；
- token revocation；
- 数据库和 Redis 访问；
- 测试资源构建。

验收：`GlobalDatabase`、`GlobalRedis`、全局 `GlobalTools` 不再出现在运行路径；常用调用仍保持 `ctx.tools().db()/cache()`。

### 阶段 4：实现 Fields 与 Params 的原生 BR 风格 DSL

先实现类型 Builder：

- `Key / Str / Text / Int / Decimal / Switch / Radio / Table / Tree / Timestamp`；
- `Fields / Params`；
- Storage、Validation、Access、Presentation 四类内部语义；
- 字段复用与 Action 参数覆盖规则。

Builder 稳定后再实现：

- `fields!`；
- `params!`，同时生成强类型 Input 和静态 ParamSet；
- Field → Param 的静态复用；
- 清晰的编译错误和 trybuild 测试。

验收：一个字段声明可稳定生成数据库 Schema、输入/输出 Schema、OpenAPI、默认后台元数据和查询策略；请求只反序列化一次；没有动态 Params 转换。

### 阶段 5：重建 Addon / Module / Action / Plugins

调整 `yang-base` 公开 Interface，使其保留 BR 业务术语：

- Addon 通过 `modules()` 注册；
- Module 通过 `fields()/actions()/views()` 注册；
- Action 使用 `params()/index()` 心智顺序；
- Registry 内部使用 `DynAction`；
- Plugins 只支持构建期解析的 ActionRef；
- 路由、Action 和输入输出定义原子注册。

验收：不再存在巨型字符串 Action match；内部调用不重新创建 Addon/Module/Action、不序列化 JSON；Catalog 与 Registry 来自同一构建过程。

### 阶段 6：重建 Tables 深 Module

实现：

- `TableQueryPlan`；
- 通用列表参数；
- 默认 TableView；
- 显式多 View；
- `table_list / table_select / table_tree`；
- 权限按钮定义生成；
- RelationLoader 批量解析；
- 稳定响应 Schema。

验收：BR 常见的 `params_table + table_list/table_select` 可以机械改写为原生 YANG；CompiledTableView 在启动期生成；关系显示不产生逐行 N+1 查询。

### 阶段 7：编译运行时并优化热路径

完成从定义到运行时的预编译：

- Route → ActionHandle；
- ActionRef → Registry slot；
- FieldRef/TableRef → checked identifier；
- ViewSpec → CompiledTableView；
- QueryPolicy → 只读运行时策略；
- 高频 Tools 直接字段访问；
- 常用 RequestContext 直接字段访问。

运行第 9.3 节基准，消除多余 clone、分配、锁、字符串查找和内部序列化。

验收：单一链路性能门槛全部满足；不存在兼容 Adapter、请求期定义转换或 Catalog 名称查找。

### 阶段 8：重写 yang-system 参考应用

`yang-system` 直接使用唯一原生 Interface：

```text
project/yang-system/src/
├─ bootstrap.rs
├─ app.rs
├─ addon/
│  ├─ mod.rs
│  ├─ user/
│  └─ org/
├─ transport/http/
└─ config.rs
```

参考应用至少展示：

- Addon/Module/Action 注册；
- fields 与 params；
- CRUD、table_list、table_select；
- Tools；
- Plugins 内部调用；
- RequestContext 和租户；
- Schema 同步、OpenAPI 和后台元数据。

验收：熟悉 scs-api 的开发者可以通过目录和名称直接找到对应能力。

### 阶段 9：删除旧 YANG Interface 并完成源码迁移工具

删除：

- `GlobalDatabase / GlobalRedis / GlobalTools`；
- pool 绑定与全局取 pool 双轨；
- 与新定义内核重复的 Router/Catalog/Table 注册；
- 旧 Action 参数提取路径；
- 动态字符串查询和 Action 调用入口；
- 实施期间产生的所有临时代码。

提供：

- BR → YANG import 映射表；
- 常用方法迁移清单；
- 只生成 YANG 原生代码的 codemod；
- 对动态 JSON 和无法自动改写位置的源码诊断；
- 性能基线与基准运行说明。

验收：三个 crate 只有一套 Registry、一套 Fields/Params 定义、一套查询执行和一套资源所有权模型；仓库中不存在 compat crate、compat feature 或 compat 类型。

---

## 11. 单个 BR Module 的迁移路径

每个 Module 一次性迁移到原生 YANG，不允许以“暂时兼容”为由把动态路径带入主干。

### 步骤 1：迁移 Module 配置

- 保留原 Addon/Module 目录和业务名称；
- `json::object! fields` 改为 `fields!`；
- `action(name)` 字符串 match 改为 `actions![...]`；
- Table/Tree 关系和 select Action 改为受控引用；
- Cargo feature 继续控制 Addon 是否编译。

### 步骤 2：迁移 HTTP 接口配置

- 每个 `params()` 改为唯一的 `params!` 定义；
- `params!` 生成 Action 的强类型 Input；
- method/path/title/permissions 与 Action 原子配置；
- Handler 从动态 Request 读取改为直接接收 Input；
- Output 改为强类型结果。

### 步骤 3：迁移查询和 Tools

- `self.tools()` 改为 `ctx.tools()`；
- 表、字段和操作符字符串改为 TableRef/FieldRef/CompareOp；
- 保留 `table → where_and → order/page → select` 链式顺序；
- 增加必要的 `?` 和 `.await`；
- 事务改为显式 `Transaction`；
- Redis、邮件、OSS 等通过 Tools typed accessor 获取。

### 步骤 4：迁移 Module 间调用和请求上下文

- `Plugins::api_run("...")` 改为 `ctx.plugins().api_run(ACTION_REF, input)`；
- 内部调用直接传递强类型 Input/Output；
- `global_data` 改为 TenantContext、ActorContext 或 ContextKey<T>；
- 按钮、权限和选择器引用统一使用 ActionRef/ViewRef。

### 步骤 5：验证后替换

- 对比字段、参数、列表、OpenAPI 和数据库 Schema 快照；
- 运行业务行为和权限测试；
- 运行 SQL 查询次数和性能基准；
- 确认 Module 中没有 JsonValue 参数、动态 Action 路径、字符串字段查询和全局资源调用；
- 原子替换旧 Module，不保留双实现。

结果：源码结构和业务操作逻辑延续 BR，但 Module 从提交到运行都只经过 YANG 原生链路。

---

## 12. BR → YANG 概念映射

| BR 操作 | YANG 唯一原生 Interface | 保留的开发逻辑 | 必须接受的升级 |
|---|---|---|---|
| `Addon` | `Addon` | 按产品能力组合 Module | 显式名称、启动期冻结 |
| `Module` | `Module` | 在 Module 中查看 fields/actions/views | 无字符串 match、定义交叉校验 |
| `Action` | `Action` | params + index 阅读顺序 | 强类型 Input/Output、异步 Result |
| `fields()` | `fields() -> Fields` | 用字段类型 Builder 配置表 | `fields!` 直接生成 FieldSpec |
| `params()` | `params!` / `Action::params()` | 用字段类型配置接口参数 | 同时生成强类型 Input，无动态 Params |
| `Tables` | `Tables` | table_list/table_select 链式操作 | QueryPlan/CompiledView/批量 RelationLoader |
| `Tools` | `ctx.tools()` | 单一工具入口 | 显式 Arc、冻结资源、统一生命周期 |
| `Plugins::api_run` | `ctx.plugins().api_run` | 统一 Module 间调用入口 | 预解析 ActionRef、强类型直调 |
| `global_data` | `RequestContext` | 请求内共享上下文 | `ContextKey<T>`、无 thread_local |
| `Db::table` | `Tools::db().table` | 从表开始构建查询 | TableRef、参数绑定 |
| `where_and` | `where_and` | 条件链顺序不变 | FieldRef、CompareOp、无运行时字符串解析 |
| `transaction` | `Transaction` | begin/commit/rollback 心智不变 | 显式异步所有权、无线程映射 |
| `JsonValue` 响应 | 强类型 Output | Action 返回业务结果 | 仅 HTTP 出口序列化一次 |
| 源码扫描接口 | AppBuilder 注册 | 接口归属 Addon/Module/Action | 编译/启动期确定性 Registry/Catalog |

---

## 13. 错误模型

统一错误层次：

```text
BuildError
├─ DuplicateName
├─ InvalidReference
├─ RouteConflict
├─ InvalidFieldDefinition
└─ DependencyMissing

InputError
├─ Missing
├─ TypeMismatch
├─ UnknownField
├─ ValidationFailed
└─ SourceMismatch

ActionError
├─ Unauthorized
├─ PermissionDenied
├─ NotFound
├─ Conflict
├─ BusinessRule
├─ Database(DbError)
├─ Tool(ToolError)
└─ Internal
```

所有 YANG 原生 Module 直接返回上述结构化错误，不能把 `Err` 转换成空 JSON、`false` 或成功响应。

HTTP Adapter 统一把结构化错误转换成稳定响应，并保留 request id 和错误路径；业务 Action 不手写状态码映射。

---

## 14. 测试策略

### 14.1 开发体验契约测试

对代表性 BR 样板建立编译和差异测试：

- Addon/Module/Action 目录和注册方式；
- fields/params 声明；
- tools/db 查询链；
- Tables 列表和选择器；
- Plugins 内部调用；
- 请求上下文和事务。

测试目标不是逐字符源码相同，而是业务步骤和概念映射稳定。

### 14.2 定义内核测试

- 重复名称和无效引用失败；
- FieldSpec/ParamSpec 覆盖规则；
- Catalog 稳定排序；
- Registry 与 Catalog 一致；
- 不同注册顺序产生相同定义快照。

### 14.3 产物快照

- 数据库 TableSchema；
- OpenAPI；
- 默认后台表格/表单；
- 显式 View；
- Action 参数；
- 权限和按钮引用。

### 14.4 数据库安全测试

- 所有值使用绑定参数；
- 非法表名、字段名和操作符失败；
- 无 WHERE 的更新和删除失败；
- Database 与 Transaction 对同一 QueryPlan 语义一致；
- MySQL/PostgreSQL 能力差异显式暴露。

### 14.5 异步和上下文测试

- 请求跨线程调度时 Tenant/User/RequestId 不丢失；
- 并发请求上下文不串数据；
- 事务提交、回滚和 drop 行为确定；
- 同一进程可以构造多个独立测试 App；
- Tools 关闭后健康检查和错误稳定。

### 14.6 单链路与性能测试

- 架构测试禁止出现 compat crate、compat feature、Compat 类型和第二套 Dispatcher/QueryBuilder；
- HTTP Route 必须在启动期绑定 ActionHandle；
- 内部 Action 调用不得经过 serde_json；
- FieldRef/ActionRef/ViewRef 在请求期不得进行字符串解析；
- 核心 Tools accessor 不得使用 TypeMap；
- 运行第 9.3 节全部基准并与重构前 YANG 基线比较；
- 性能报告必须与功能测试一起成为阶段 7 和最终验收产物。

---

## 15. 验收标准

### 15.1 开发体验

- 熟悉 BR 的开发者仍使用 Addon、Module、Action、Fields、Params、Tables、Tools、Plugins 组织业务；
- 常见查询仍按 `table → where_and → order/page → select` 编写；
- 常见列表仍按 `params_table → table_list/table_select` 的心智模型完成；
- 一个简单 Module 只声明 fields 和 actions 即可工作；
- 自定义 Action 只需要 Input、Output、元数据和 `index()`；
- 标准 CRUD、列表和选择器不要求一 Action 一文件；
- 业务目录可以从 scs-api 的 Addon/Module 结构直接映射。

### 15.2 底层升级

- 核心运行路径中不存在 `GlobalDatabase`、`GlobalRedis` 和全局 `GlobalTools`；
- 不存在基于线程 ID 的事务；
- SQL 值全部参数绑定；
- 请求运行路径不存在任意字段、操作符和 Action 字符串入口；
- 业务 Module 不接触 SQLx pool；
- Registry 和 Catalog 只构建一次且运行期不可变；
- body/query/path/header 可以形成强类型 ParamSpec；
- 字段声明可以生成 Schema、校验、OpenAPI、后台元数据和查询策略；
- 租户查询默认 fail-closed；
- HTTP 和内部 Action 调用分别只发生必要的一次和零次 JSON 往返；
- 第 9.3 节性能门槛全部满足。

### 15.3 迁移可控性

- 至少三个代表性 scs-api 样板一次性迁移到原生 YANG；
- 每个 BR 高频调用都有明确替换表；
- 能自动迁移的 import、配置结构和方法名提供只生成原生代码的 codemod；
- 不能自动迁移的动态 JSON 给出字段级源码诊断；
- `yang-system` 中不存在兼容依赖、兼容类型或动态旁路；
- 每个迁移完成的 Module 都通过功能、Schema 和性能验证后原子替换旧实现。

---

## 16. 风险与控制

### 16.1 兼容目标把 BR 缺陷带入永久 Interface

控制：只保留业务心智、配置逻辑和安全的调用顺序；动态请求、字符串引用、全局状态和字符串 SQL 全部拒绝，不提供过渡运行方式。

### 16.2 熟悉语法意外形成第二条实现

控制：`fields! / params! / action! / module!` 必须直接生成 YANG 原生定义；架构测试禁止 Compat 类型、兼容 feature、第二套 Registry、第二套 QueryBuilder 和请求期定义转换。

### 16.3 宏过重，错误难以理解

控制：先完成手写 Builder，再增加宏；每个宏提供 trybuild 错误测试；宏只生成定义，不包含数据库或业务执行代码。

### 16.4 Fields 再次耦合数据库与 UI

控制：Fields 只包含可共享的默认 PresentationSpec；复杂页面使用独立 ViewSpec 覆盖；Admin 产物生成是可选 feature，不影响数据库和 dispatch。

### 16.5 为了隐藏 async 而恢复阻塞调用

控制：不提供同步执行语义。`.await` 是允许且必须出现的升级成本，源码迁移工具负责机械补充可判断的位置。

### 16.6 为每个工具预先定义 trait

控制：遵循“两个 Adapter 才形成真实 Seam”。只有确实存在多个实现或测试替代需求时才抽取 Interface；否则 Tools 持有具体客户端。

### 16.7 为保持 BR 体验牺牲性能

控制：业务熟悉度只影响名称、配置方式和调用顺序，不允许增加热路径转换、字符串解析、序列化、分配或锁；第 9.3 节基准是合并阻塞门槛。

---

## 17. 与现有设计文档的关系

本设计保留以下既有决定：

- `ApiCatalog` 是只读快照，不直接参与 dispatch；
- OpenAPI 是 Catalog 的可选投影；
- 后台元数据是可选展示描述，不改变 Action dispatch、TableQuery 和权限；
- Schema 同步保持 additive、可规划和 fail-fast，不自动删除生产表、列或索引；
- `yang-db` 保持 checked identifier、参数绑定和结构化错误；
- Typed Action 的 Input/Output Schema 继续由 Rust 类型产生。

本设计修正或扩展以下旧方向：

- Action 输入不再局限于单一 body，ParamSpec 可以描述 body/query/path/header；
- `GlobalTools`、`GlobalDatabase`、`GlobalRedis` 被明确删除，不提供兼容目标；
- Table 与 Action 的类型化不以牺牲 BR 的 fields/params 开发心智为代价；
- 后台 UI 元数据从同一个定义核心生成，但仍与数据库执行和 dispatch 解耦；
- 公开业务 Interface 优先采用 BR 熟悉术语，内部实现继续使用精确类型。

本设计已经批准采用 5.2 方案 B，应作为 `yang-base 0.3` 架构和 BR → YANG 迁移工作的上位设计；与其冲突的历史计划需要在实施计划中显式标记为已被替代。

---

## 18. 最终结果

完成后，YANG 给开发者的感觉应当是：

> 我仍然在写熟悉的 Addon、Module、Action、fields、params、Tables 和 Tools；只是 JSON 变成了类型，字符串错误提前到了构建期，数据库调用需要 await，事务不再依赖线程，全局变量变成了明确的请求上下文，而且同一份声明真正稳定地产生数据库、校验、文档和后台元数据。

完成后，YANG 给维护者的结构应当是：

```text
熟悉且稳定的业务 Interface
            │
            ▼
少量具有 Depth 的定义、运行时和查询 Module
            │
            ▼
可测试、可替换、可观测的 Adapter
```

这既保留 BR 生态经过大量业务验证的生产力，也删除其底层偶然复杂度。整个系统从配置到请求执行只有一条 YANG 原生链路，并以不低于重构前 YANG 的性能为硬门槛。这是“升级 BR”，而不是“重新发明一个与 BR 无关的框架”，也不是“给 BR 套一层兼容外壳”。
