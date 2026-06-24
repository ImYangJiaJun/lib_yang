# yang-base + yang-db 生产就绪度全面审计报告

**审计日期**: 2026-06-24
**审计范围**: `crates/yang-db/` + `crates/yang-base/`（含 `yang-base-derive`）
**审计维度**: 安全(67项) + 性能(25项) + 逻辑正确性(26项) + API/架构(32项) + 文档一致性(42项)
**总计发现**: 192 项

---

## 总体评估

### 各维度评分

| 维度 | 评分 | 定级 | 说明 |
|------|------|------|------|
| **安全性** | 4.0/10 | 不足 | 裸 SQL 注入面宽（8 CRITICAL + 11 新增注入面），公共 API 全是裸字符串接口；标识符侧无编译期强制转义；聚合方法/ORDER BY/GROUP BY/JOIN/PG RETURNING 均裸拼。Token 撤销存在绕过；parse_token_unsafe 公开无验证 API。值侧参数化正确。 |
| **性能** | 5.0/10 | 中等偏下 | Redis recycle=5s 是功能性 bug（连接池形同虚设）。大量 unnecessary clone、SQL 生成+绑定双重 clone、HashMap clone、base64 编码膨胀。SqlGenerator 预分配合理。 |
| **逻辑正确性** | 4.5/10 | 中等偏下 | Transaction 缺少 Drop 回滚日志；PluginManager TOCTOU 竞态；RedisTransaction 重试复用死连接；graceful_shutdown 被 mysql feature 绑死。Token 撤销两次 Redis 查询非原子。3 个 ErrorCategory 映射错误。 |
| **API/架构** | 5.0/10 | 中等 | MySQL/PG 完全平行复制无共享抽象；query_builder.rs 5484 行最大瓶颈；RedisConfig 位置参数 vs DatabaseConfig Builder 不一致。 |
| **文档** | 3.5/10 | 严重不足 | AGENTS.md 多处描述已删除的旧 Action trait；BACKLOG M-1 严重低报（"20+" vs 实际 ~870+）；yang-base.md 漏掉撤销/中间件/熔断器。 |
| **综合加权** | **4.5/10** | **不建议直接生产部署** | 约 20 个 CRITICAL/HIGH 需先关闭。预估 7 个工作日可达到可部署水平。 |

### 生产就绪度判断

**结论：不建议当前状态直接部署到生产环境。** 主要阻塞项集中在安全维度（裸 SQL API 面宽 + Token 撤销绕过 + 聚合方法注入 + parse_token_unsafe）和文档维度（开发者无法仅凭文档正确使用）。Phase 0+1 约 55h 投入可关闭所有阻塞项，将评分从 4.5 提升至约 7.5。

---

# 第一部分：安全审计

## CRITICAL (8项)

### S-C1. Database::execute() 接受裸 SQL 字符串，无参数化保护
- **文件**: `crates/yang-db/src/mysql/database.rs:223`
- **证据**: `pub async fn execute(&self, sql: &str) -> Result<u64, DbError> { ... let result = sqlx::query(sql).execute(&self.pool).await?; }` — SQL 字符串完全由调用方拼接，无占位符/参数绑定。文档示例甚至展示了字符串内联值的危险写法。
- **修复**: 标记 `#[deprecated]` 并引导调用方使用 `execute_with_params` 或 QueryBuilder；在方法文档中增加醒目的安全警告；长期将 visibility 降为 `pub(crate)`。

### S-C2. Database::query() 接受裸 SQL 字符串，无参数化保护
- **文件**: `crates/yang-db/src/mysql/database.rs:209`
- **证据**: `pub async fn query<T>(&self, sql: &str) -> Result<Vec<T>, DbError> { ... sqlx::query_as::<_, T>(sql).fetch_all(&self.pool).await?; }` — 与 execute() 相同问题。
- **修复**: 同 S-C1，标记 deprecated 并引导使用 `query_with_params`。

### S-C3. Transaction::execute() 接受裸 SQL 字符串，绕过所有保护
- **文件**: `crates/yang-db/src/mysql/transaction.rs:49`
- **证据**: `pub async fn execute(&mut self, sql: &str) -> Result<u64, DbError> { ... sqlx::query(sql).execute(&mut **tx).await?; }` — 事务内裸 SQL 执行，无参数化。
- **修复**: 标记 deprecated，引导调用方使用 TransactionQueryBuilder 或 `execute_with_params`。

### S-C4. Transaction::executor() 提供原始 DB 连接，可完全绕过所有安全层
- **文件**: `crates/yang-db/src/mysql/transaction.rs:210`
- **证据**: `pub fn executor(&mut self) -> Option<&mut sqlx::MySqlConnection>` — 返回底层 sqlx 连接，调用方可执行任意裸 SQL。虽标 `#[doc(hidden)]` 但同 workspace 内可调用。
- **修复**: 保持 `#[doc(hidden)]` 并增加安全注释说明调用方责任；审计所有 workspace 内调用点；考虑改为 `unsafe` 函数。

### S-C5. [PG] Database::query() 接受裸 SQL 字符串
- **文件**: `crates/yang-db/src/postgres/database.rs:199`
- **证据**: 与 MySQL 端相同问题，PG 方言同样暴露。
- **修复**: 同 S-C2。

### S-C6. [PG] Database::execute() 接受裸 SQL 字符串
- **文件**: `crates/yang-db/src/postgres/database.rs:213`
- **证据**: 与 MySQL 端相同问题。
- **修复**: 同 S-C1。

### S-C7. GlobalDatabase::execute() 全局暴露裸 SQL 执行
- **文件**: `crates/yang-base/src/database/global.rs:269`
- **证据**: `pub async fn execute(sql: &str) -> Result<u64, BaseError> { Self::get()?.execute(sql).await ... }` — 全局单例直接透传裸 SQL，文档示例含危险模式 `VALUES ('Alice', 'alice@example.com')`。
- **修复**: 标记 deprecated，引导使用 `execute_with_params`；示例代码替换为参数化写法；增加安全警告注释。

### S-C8. GlobalDatabase::query() 全局暴露裸 SQL 查询
- **文件**: `crates/yang-base/src/database/global.rs:201`
- **证据**: 与 S-C7 相同问题。
- **修复**: 同 S-C7，标记 deprecated 并引导使用 `query_with_params`。

## HIGH (11项)

### S-H1. build_select() 中表名和字段名裸拼入 SQL，未做标识符转义
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:129-150`
- **证据**: `self.append(&builder.fields.join(", "))` 和 `self.append(&builder.table)` — 字段名和表名直接 join 后拼入 SQL，无 backtick 引用。`build_select` 是 `pub(crate)` 内部方法，无编译期保证调用方已验证。
- **修复**: 对 `builder.table` 调用 `quote_identifier` 强制转义表名；字段列表引入两个独立方法：`select_raw_expression`（保留当前行为但改名）和 `select_identifier`（内部调用 `quote_identifier`）。

### S-H2. QueryBuilder::field() 接受任意字符串作为 SQL 表达式，无校验
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:950`
- **证据**: `pub fn field(mut self, field: &str) -> Self { self.fields.push(field.to_string()); self }` — 方法签名接受任意 `&str`。文档声明"属可信输入"但 API 为 pub，无编译期防护。
- **修复**: 拆分为 `field_identifier(name)` (转义) 和 `field_expression(expr)` (保留当前行为但改名体现风险)；或将当前 `field()` 降为 `pub(crate)`。

### S-H3. condition_to_sql_owned() 中字段名裸拼入 SQL，未做标识符转义
- **文件**: `crates/yang-db/src/mysql/condition.rs:164`
- **证据**: `Condition::Eq(field, value) => { params.push(value); format!("{} = ?", field) }` — 所有 12 种条件变体中的 field 均以 `{}` 直接拼入 SQL（LIKE、IN、BETWEEN、IS NULL 等），不经 `quote_identifier`。值侧参数化正确，但标识符侧无保护。yang-base 的 `render_condition` 已正确转义，yang-db 端漏了。
- **修复**: 在 `condition_to_sql_owned` 中对 field 调用 `quote_identifier`（MySQL 反引号）/ `quote_qualified`（处理 `a.b` 限定名）。

### S-H4. build_order_by() 排序字段裸拼入 SQL
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:249-266`
- **证据**: `self.sql.push_str(&order.field)` — `OrderClause.field` 直接 push_str 到 SQL，不校验不转义。
- **修复**: 对 `order.field` 调用 `quote_identifier` 或 `quote_qualified` 转义。

### S-H5. build_group_by() 分组字段裸拼入 SQL
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:276-282`
- **证据**: `self.append(&groups.join(", "))` — group_by 字段列表直接 join 拼入，不转义。
- **修复**: 对每个 group 字段调用 `quote_identifier` 转义后再 join。

### S-H6. build_joins() 中 JOIN 表名和 ON 条件裸拼入 SQL
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:232-245`
- **证据**: `self.append(&join.table)` 和 `self.append(&join.on)` — JOIN 子句的表名和 ON 条件直接拼入，完全不校验。ON 条件是完整 SQL 表达式，注入面最大。
- **修复**: 对 `join.table` 调用 `quote_identifier`；`join.on` 如为表达式性质则保留，但需在 `JoinClause` 构造处明确风险。

### S-H7. refresh_access_token 绕过黑名单检测
- **文件**: `crates/yang-base/src/token/manager.rs:495-501`
- **证据**: `refresh_access_token` 调用 `self.verify_token(refresh_token)?` 只做了签名+过期校验，完全没有检查 Redis 黑名单。对比同文件的 `rotate_refresh_token` 正确使用了 `verify_token_checked`。已撤销的 Refresh Token 仍可换取新 Access Token，撤销机制被绕过。
- **修复**: 将该方法内部改为调用 `verify_token_checked`，或标记 `#[deprecated]` 引导到 `rotate_refresh_token`。

### S-H8. RefreshAction 未实现 Refresh Token Rotation
- **文件**: `crates/yang-base/src/action/auth.rs:440-456`
- **证据**: `RefreshAction::handle` 仅调用 `verify_token_checked` 验证旧 refresh token 后直接 `generate_access_token` 签发新 access token，既没有调用 `revoke_claims` 拉黑旧 refresh token，也没有签发新 refresh token pair。被盗 Refresh Token 可无限刷新。
- **修复**: 使 `RefreshAction` 内部调用 `rotate_refresh_token`，将响应类型从 `AccessTokenResponse` 改为 `TokenPairResponse`（breaking change）；或新增 `RotateAction` 并在文档中说明 `RefreshAction` 的安全权衡。

### S-H9. 无自动化依赖漏洞扫描
- **文件**: `CLAUDE.md`（全局）
- **证据**: CLAUDE.md 声明 "CI 缺失是有意的现状"。存在已知漏洞的 `rsa` crate (RUSTSEC-2023-0071, Marvin Attack) 未被检测。缺少 `cargo-audit` 或 `cargo-deny` 集成。
- **修复**: 至少添加 `cargo audit` 到本地开发流程（pre-commit hook）；建议创建定期扫描脚本。

### S-H10. rsa crate Marvin Attack 定时侧信道漏洞
- **文件**: `Cargo.lock` — `rsa v0.9.10` 通过 `sqlx-mysql → rsa` 路径引入
- **证据**: RUSTSEC-2023-0071 (CVE-2023-49092)，影响所有 rsa 0.9.x 版本，至今无 patched version。sqlx-mysql 在 `caching_sha2_password` 认证握手期间使用 RSA。若 MySQL 部署在不可信网络（非 localhost），存在凭据泄露风险。
- **修复**: 确保 MySQL 连接仅发生在受信任网络（localhost/Docker 内部网络），或使用 TLS 加密的 MySQL 连接。关注 sqlx 和 RustCrypto 社区进展。

### S-H11. reqwest 版本重复，攻击面翻倍
- **文件**: `crates/yang-base/Cargo.toml` — jsonschema 0.29.1 传递依赖 reqwest 0.12.28，与 yang-base 的 0.13.2 共存
- **证据**: `cargo tree -d` 确认两套 HTTP 客户端被同时编译进最终二进制。
- **修复**: 升级 jsonschema 到最新版本（> 0.32），或通过 cargo-deny 强制统一 reqwest 版本。

## MEDIUM (17项)

### S-M1. map_comparison_condition 中字段名仍裸传
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:884`
- **修复**: 对 field 调用 `is_valid_identifier` 或 `quote_identifier`。

### S-M2. `_unchecked` 方法在非法操作符时 panic
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:1063`
- **修复**: 这些方法已标记 deprecated，建议下个大版本移除。

### S-M3. to_sql() 降级路径裸拼表名
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:1387`
- **修复**: 对 `self.table` 调用 `quote_identifier` 后再拼入。

### S-M4. Database::table() 表名不经校验直接传入 QueryBuilder
- **文件**: `crates/yang-db/src/mysql/database.rs:150`
- **修复**: 在 `Database::table()` 中对 table_name 做 `is_valid_identifier` 校验。

### S-M5. LoginAction 无内置速率限制
- **文件**: `crates/yang-base/src/action/auth.rs:278-330`
- **证据**: 暴力破解防护完全依赖 `CredentialVerifier` 业务实现。`CredentialVerifier` trait 没有要求返回剩余尝试次数或锁定状态。
- **修复**: 在 `CredentialVerifier` 文档上明确注明「实现者有责任加入速率限制/账户锁定」；考虑提供 `RateLimitedVerifier` wrapper。

### S-M6. TokenAuthMiddleware 在 is_public 检查前执行
- **文件**: `crates/yang-base/src/action/auth.rs:634-659`
- **证据**: 公开 Action（login/refresh/logout）若与 TokenAuthMiddleware 注册在同一个 ModuleRouter，会在到达 `is_public` 检查前就被中间件拒绝，形成"需要先登录才能登录"的死锁。
- **修复**: 文档警告：鉴权中间件应仅注册在需要保护的路由器上，公开 Action 放在不含 TokenAuthMiddleware 的独立路由器上。可选增强：TokenAuthMiddleware 内检查 Action meta 的 `is_public` 字段。

### S-M7. DefaultRefreshClaims 刷新后静默丢弃所有自定义声明
- **文件**: `crates/yang-base/src/action/auth.rs:362-373`
- **证据**: `DefaultRefreshClaims::resolve` 直接返回 `Value::Null`。原 access token 中的角色/权限等自定义声明在刷新后全部丢失。
- **修复**: 在文档中显式标注；考虑让 RefreshAction 的 resolver 参数去掉默认值以强制业务方显式选择。

### S-M8. LoginInput 派生 Debug 导致密码明文可能泄露到日志
- **文件**: `crates/yang-base/src/action/auth.rs:55`
- **修复**: 为 LoginInput 手动实现 Debug，password 字段输出为 `"***"`。

### S-M9. TokenPairResponse 派生 Debug 暴露 Access Token 和 Refresh Token 原文
- **文件**: `crates/yang-base/src/action/auth.rs:67`
- **修复**: 手动实现 Debug，token 字段输出指纹而非原文。

### S-M10. AccessTokenResponse 派生 Debug 暴露 Access Token 原文
- **文件**: `crates/yang-base/src/action/auth.rs:84`
- **修复**: 同 S-M9。

### S-M11. LogoutInput 和 RefreshInput 派生 Debug 暴露 Token 原文
- **文件**: `crates/yang-base/src/action/auth.rs:94`
- **修复**: 手动实现 Debug，token 字段输出指纹。

### S-M12. TokenClaims 派生 Debug 暴露用户标识和自定义声明
- **文件**: `crates/yang-base/src/token/mod.rs:85`
- **修复**: 手动实现 Debug，jti 输出指纹、custom 输出 `<custom claims>`。

### S-M13. validator feature 关闭时 Email/Phone 静默安全降级，Regex 变为硬错误
- **文件**: `crates/yang-base/src/table/validator.rs:204-363`
- **证据**: Email 降级为仅检查 `contains('@')`，Phone 降级为仅检查字符集。Regex 直接返回硬错误。Cargo.toml 中 validator 是 default feature，若下游误关，所有严格校验静默失效。
- **修复**: Email/Phone 降级时打 `log::warn!`；Regex 降级不应是硬错误（打 warn 后跳过或提供 `compile_error!`）。

### S-M14. tokio features=["full"] 在 workspace 级别，库 crate 不应强制运行时选择
- **文件**: `Cargo.toml:16`
- **修复**: 改为最小特性集（`sync, time, net, io-util, macros`），让各 crate 按需声明。

### S-M15. default features 启用全部可选依赖，feature gate 默认失效
- **文件**: `crates/yang-base/Cargo.toml:25`
- **修复**: 未来主版本将 default 改为最小集；短期在 README 中建议消费者使用 `default-features = false`。

### S-M16. redis crate 使用默认 features，可能包含不需要的网络功能
- **文件**: `crates/yang-db/Cargo.toml:38`
- **修复**: 分析实际使用模式，显式声明最小 features 集。

### S-M17. Cargo.lock 双加密后端共存（aws-lc-rs + ring）
- **文件**: `Cargo.lock`
- **修复**: 定期执行 cargo audit 自动跟踪两套加密库的公告。

## LOW (12项)

### S-L1. is_valid_identifier 仅允许 ASCII 字母数字下划线
- **文件**: `crates/yang-db/src/mysql/identifier.rs:24`
- **修复**: 未来非 ASCII 标识符需放宽。

### S-L2. leeway=0 无时钟偏差容忍
- **文件**: `crates/yang-base/src/token/manager.rs:54`
- **修复**: 将 leeway 设为可配置参数（默认 30-60 秒）。

### S-L3. revoke_token 拒绝已过期 Token
- **文件**: `crates/yang-base/src/token/revocation.rs:63-66`
- **修复**: 区分「过期」与「签名无效」：过期时直接返回 Ok(())，签名无效时返回 Err。

### S-L4. required_spec_claims 未包含 sub
- **文件**: `crates/yang-base/src/token/manager.rs:49-53`
- **修复**: 将 sub 加入 required_spec_claims 作为纵深防御。

### S-L5. LogoutAction 的 refresh_token 字段为可选
- **文件**: `crates/yang-base/src/action/auth.rs:96-104`
- **修复**: 强化文档警告；考虑在审计日志中区分「仅撤销 access」与「完整撤销」。

### S-L6. Redis 连接成功日志可能泄露密码
- **文件**: `crates/yang-db/src/redis/client.rs:106`
- **修复**: 日志中遮蔽 URL 的密码部分。

### S-L7. yang-base 的 unsafe_code 仅为 warn 级别
- **文件**: `crates/yang-base/Cargo.toml:20`
- **修复**: 提升为 deny，与 yang-db 保持一致。

### S-L8. schemars 为非可选硬依赖
- **文件**: `crates/yang-base/Cargo.toml:97`
- **修复**: 将 schemars 标记为 optional 并放入独立的 feature。

### S-L9. dev-dependencies 未纳入 workspace 统一管理
- **文件**: `Cargo.toml:20`
- **修复**: 将 testcontainers 和 proptest 提升到 workspace.dependencies。

### S-L10. rand crate 三个主版本共存
- **文件**: `Cargo.lock`
- **修复**: 关注 sqlx 升级以统一版本。

### S-L11. Cargo.lock 中存在孤立条目
- **文件**: `Cargo.lock`
- **修复**: 运行 `cargo update` 清理。

### S-L12. TokenManager Debug 实现正确遮蔽密钥（正面发现）
- **文件**: `crates/yang-base/src/token/manager.rs:566`
- **证据**: `finish_non_exhaustive()` 明确不输出 encoding_key/decoding_key。可作为其他 Debug 遮蔽实现的参考模板。

## 补充发现：SQL 注入面（完整性审查新增，审计初版遗漏）

> **说明**：以下 11 项在原始审计中未被覆盖，由第二轮完整性审查发现。其中 NEW-SQL-1~4 为完全未提及的新攻击面（聚合方法 + value() + PG RETURNING），NEW-SQL-5~11 为审计仅列出 MySQL 路径但 PG 侧存在镜像问题的对称遗漏。

### S-NEW-SQL-1. [CRITICAL] sum/avg/min/max 聚合方法 field 参数裸拼入 SQL — 审计完全未覆盖
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:1754,1818,1898,1978`
- **证据**: `format!("CAST(SUM({}) AS DOUBLE)", field)` / `format!("CAST(AVG({}) AS DOUBLE)", field)` / `format!("MIN({})", field)` / `format!("MAX({})", field)` — `field: &str` 是公开 API 参数，不经 `is_valid_identifier` 或 `quote_identifier` 直接拼入 SQL。调用方可传入任意字符串。
- **修复**: 对 field 调用 `quote_identifier` 转义（需同时支持表达式场景，考虑拆分为 `sum_identifier` / `sum_expression`）。

### S-NEW-SQL-2. [CRITICAL] PG 侧聚合方法同样裸拼
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:1403,1416,1430,1444`
- **证据**: 与 MySQL 端完全相同的模式，`sum/avg/min/max` 中 field 裸拼入 SQL 表达式。
- **修复**: 同 S-NEW-SQL-1。

### S-NEW-SQL-3. [CRITICAL] value() 方法 field 参数裸拼入 SQL
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:1648` + `crates/yang-db/src/postgres/query_builder.rs:1377`
- **证据**: `value(field)` 将 field 传入 `fetch_scalar()` → 放入 `self.fields` → `build_select()` 裸拼。路径与聚合方法相同，注入面一致。
- **修复**: 同 S-NEW-SQL-1，对 field 调用 `quote_identifier`。

### S-NEW-SQL-4. [CRITICAL] PG insert() 的 RETURNING 列名用户可控且直接 format!() 拼入 SQL
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:1475-1479`
- **证据**: `format!("{} RETURNING CAST({} AS BIGINT)", generator.get_sql(), self.returning)` — `self.returning` 通过 `QueryBuilder::returning(column)`（第 1024 行）由调用方设置，默认为 `"id"`。调用方可传入任意列名直接拼入 SQL。MySQL 用 `last_insert_id()` 无此问题——这是 PG 独有注入点。
- **修复**: 对 `self.returning` 调用 `quote_identifier` 转义。

### S-NEW-SQL-5. [HIGH] PG condition_to_sql_owned 全部变体 field 裸拼（审计 S-H3 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/condition.rs:177-255`
- **证据**: PG 版本与 MySQL 完全相同的 12 个条件变体，field 以 `format!("{} = {}", field, ph)` 裸拼，无引号转义。审计 S-H3 仅列出 MySQL 文件路径。
- **修复**: 在 PG 版的 `condition_to_sql_owned` 中对 field 调用 `quote_identifier`。

### S-NEW-SQL-6. [HIGH] PG build_select 字段和表名裸拼（审计 S-H1 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:160,165`
- **证据**: `builder.fields.join(", ")` 和 `builder.table` 直接 `append()` 入 SQL，无 `quote_identifier`。审计 S-H1 仅列出 MySQL 路径。
- **修复**: 与 MySQL 端统一修复策略。

### S-NEW-SQL-7. [HIGH] PG build_order_by 排序字段裸拼（审计 S-H4 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:284`

### S-NEW-SQL-8. [HIGH] PG build_group_by 分组字段裸拼（审计 S-H5 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:300`

### S-NEW-SQL-9. [HIGH] PG build_joins 表名和 ON 条件裸拼（审计 S-H6 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:262-264`

### S-NEW-SQL-10. [MEDIUM] PG to_sql() 降级路径表名裸拼（审计 S-M3 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/query_builder.rs:1275`
- **修复**: 对 `self.table` 调用 `quote_identifier` 后再拼入。

### S-NEW-SQL-11. [MEDIUM] PG Database::table() 无表名校验（审计 S-M4 仅列 MySQL）
- **文件**: `crates/yang-db/src/postgres/database.rs:145-146`
- **修复**: 在 PG `Database::table()` 中对 table_name 做 `is_valid_identifier` 校验。

## 补充发现：认证/授权问题（完整性审查新增）

### S-NEW-AUTH-1. [HIGH] parse_token_unsafe 是公开 API，零加密验证即可解码任意 JWT
- **文件**: `crates/yang-base/src/token/manager.rs:431`
- **证据**: 该方法使用 `jsonwebtoken::dangerous::insecure_decode`，跳过所有安全检查（签名、过期、issuer/audience 等）。尽管有文档警告，仍是 `pub` 方法，任何调用方误用即造成认证完全绕过。审计 S-H7 讨论了 `verify_token` vs `verify_token_checked` 的区别，但完全未提及这个更危险的 API。
- **修复**: 标记 `#[deprecated]` 并加严重警告；考虑改为 `pub(crate)` 或 feature-gate 到 `debug-utils`。

### S-NEW-AUTH-2. [MEDIUM] 无编译期或启动期强制要求注册认证中间件
- **文件**: `crates/yang-base/src/router/module_router.rs:239`
- **证据**: `ModuleRouter` 以空中间件链启动。没有任何机制（编译期、启动断言、lint）验证含非公开 Action 的路由器至少注册了一个认证中间件。`authorize_and_dispatch` 虽会返回 `Unauthorized`（fail-closed），但配置错误会导致所有非公开请求静默被拒，无诊断信息。
- **修复**: 在 debug 模式添加断言或启动日志警告；考虑 `ModuleRouter::build() -> Result<Self>` 验证配置。

### S-NEW-AUTH-3. [MEDIUM] ActionContext::with_user() 可注入任意用户绕过 Token 验证
- **文件**: `crates/yang-base/src/action/context.rs:273`
- **证据**: `ActionContext` 有 `pub user: Option<User>` 和公开链式方法 `with_user()`。任何构造 `ActionContext` 并调用 `router.dispatch()` 的代码可注入任意 User（含任意角色/权限）。`authorize_and_dispatch` 仅检查 `ctx.user.is_some()` 然后就该 User 检查权限——不验证 User 是否由受信任的中间件设置。
- **修复**: 文档标明 `with_user()` 仅供中间件/内部使用；考虑将 `user` 字段改为 `pub(crate)` 或增加 `UserSource` 标记。

### S-NEW-AUTH-4. [MEDIUM] GetAction/SelectAction 中 unwrap_or(&anon) 静默降级为匿名用户
- **文件**: `crates/yang-base/src/action/builtin/get.rs:51-52`、`select.rs:155-156`
- **证据**: 两个处理器使用 `ctx.user.as_ref().unwrap_or(&anon)`（`anon = User::new(0, "")`）。虽 `authorize_and_dispatch` 确保非公开 Action 执行时 `ctx.user` 为 `Some`，但若中间件链有 bug 导致 `ctx.user` 为 `None`，处理器不会返回 `Unauthorized` 而是以零权限匿名用户静默继续。
- **修复**: 替换为 `ctx.user.as_ref().ok_or_else(|| BaseError::Unauthorized(...))?`。

### S-NEW-AUTH-5. [MEDIUM] GlobalDatabase::execute/query 可从任何 Action 调用，完全绕过 TableQuery 权限层
- **文件**: `crates/yang-base/src/database/global.rs:201,269`
- **证据**: 审计 S-C7/S-C8 将这些方法标记为 SQL 注入漏洞，但遗漏了另一个维度：任何自定义 Action 处理器可直接调用 `GlobalDatabase::execute("DELETE FROM users")`，完全绕过 `TableQuery` 的整个权限系统（字段级读写检查、行级 WHERE 强制、软删除、字段验证）。通过 Action 级权限检查的已认证用户即可通过编写不安全的自定义 Action 执行任意 SQL。
- **修复**: 除标记 deprecated 外，在 AGENTS.md 中增加安全说明，明确警告自定义 Action 作者使用 `ctx.table_query()?` 而非 `GlobalDatabase::execute/query`。

### S-NEW-AUTH-6. [LOW] #[action(public)] 无护栏——可被应用于任何数据访问 Action
- **文件**: `crates/yang-base-derive/src/action.rs:20,95`
- **证据**: `#[derive(Action)]` 宏接受 `#[action(public)]` 使 `is_public = true`。公开 Action 在 `authorize_and_dispatch` 中跳过所有认证和授权检查。开发者可意外将 `AddAction<User>` 或 `SelectAction<Orders>` 标记为 public，创建完全不设防的端点。审计 S-M6 讨论了 TokenAuthMiddleware 死锁但未标记此更广泛的风险。
- **修复**: 添加 clippy lint 或构建期检查，对标记为 public 的非认证 Action（login/refresh/logout 豁免）发出警告；考虑将属性改名为 `#[action(skip_auth)]` 以更明确表达危险。

### S-NEW-AUTH-7. [LOW] 无 JWT 签名密钥轮换机制
- **文件**: `crates/yang-base/src/token/manager.rs:38-164`
- **证据**: `TokenManager` 使用单一硬编码 `EncodingKey`/`DecodingKey` 对。不支持 JWT `kid`（Key ID）头、JWK 集或任何密钥轮换机制。审计 S-H8 讨论了 Refresh Token Rotation 但未涉及签名密钥轮换——这是 JWT 基础设施的独立关切。
- **修复**: 增加可选 `key_id` 字段和多解码密钥支持（按 `kid` 索引）；至少文档说明此限制。

### S-NEW-AUTH-8. [LOW] TableAction 返回完整实体 schema，不做角色级字段过滤
- **文件**: `crates/yang-base/src/action/builtin/table.rs:56-76`
- **证据**: `TableAction` 返回实体的完整 JSON Schema（`schemars::schema_for!(T)`），含所有字段名、类型、描述、验证器。虽非公开 Action（需要认证），但无论用户字段级读权限如何都返回相同完整 schema。只能读取部分字段的用户仍可通过此端点发现所有字段名和类型。
- **修复**: 用 TableConfig 的字段权限过滤输出 schema，仅包含当前用户有 `can_read` 权限的字段。

---

# 第二部分：性能审计

## HIGH (6项)

### P-H1. Redis 连接池 recycle 超时参数错误，导致连接池形同虚设
- **文件**: `crates/yang-db/src/redis/client.rs:83`
- **证据**: `Timeouts::recycle` 被设为 `config.connect_timeout_duration()`（默认 5 秒）。deadpool 语义：归还时连接创建时间超过 recycle 即丢弃。5 秒意味着几乎所有连接在首次借出归还后被立即销毁。
- **修复**: recycle 应使用独立的 `idle_timeout`/`max_lifetime` 参数（默认至少 300 秒），或设为 `None`。不得复用 `connect_timeout`。

### P-H2. QueryBuilder 缺少索引提示支持
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:898`
- **证据**: 无任何 `index_hint` 或类似字段。全文搜索 `FORCE INDEX`、`USE INDEX`、`STRAIGHT_JOIN` 均为零命中。
- **修复**: 在 QueryBuilder 中添加 `index_hints` 字段，提供 `force_index()` / `use_index()` 链式 API。

### P-H3. build_update_sql_impl 无条件 clone 整个 data HashMap
- **文件**: `crates/yang-base/src/table/table_query.rs:2403`
- **修复**: 将 clone 推迟到确认需要注入 `updated_at` 之后。

### P-H4. bind_param 对 String 参数产生额外 clone
- **文件**: `crates/yang-base/src/table/table_query.rs:1538`
- **证据**: `SqlParam::String(s) => query.bind(s.clone())` — 函数签名为 `param: &SqlParam`，`s` 类型为 `&String`，`s.clone()` 从借用克隆一次产生新的 owned String。SqlParam 内部已持有 owned String 但 bind 时仍需 clone。
- **修复**: 将 `bind_param` 改为接受 `SqlParam`（owned）而非 `&SqlParam`，使 `String` 可通过解构移出而无需 clone；或 SqlParam::from_json 对 Value::String 改用 take（消费 Value）。

### P-H5. BLOB 列逐行 base64 编码产生额外分配
- **文件**: `crates/yang-base/src/table/dynamic_row.rs:218`
- **修复**: 预分配 base64 输出缓冲区容量 `4*ceil(len/3)`；可选 lazy/zero-copy 方式。

### P-H6. FieldPermissions 使用 Vec<String> + contains() 做 O(n) 权限查找
- **文件**: `crates/yang-base/src/table/field_config.rs:490`
- **证据**: 对每个 user role 在 readable_roles Vec 上线性查找。N=字段数, M=用户角色数, K=权限角色数 → O(N×M×K)。
- **修复**: 改为 `HashSet<String>`，使 contains 从 O(n) 降为 O(1)。

## MEDIUM (10项)

### P-M1. RedisConfig 缺少连接池自愈参数，与 DatabaseConfig 不对等
- **文件**: `crates/yang-db/src/redis/config.rs:7`
- **修复**: 增加 `min_connections`、`max_lifetime`、`idle_timeout`、`test_before_acquire` 字段。

### P-M2. EngineConfig::from_env 缺少 Redis 连接池环境变量覆盖
- **文件**: `crates/yang-base/src/config.rs:102`
- **修复**: 增加 `YANG_REDIS_MAX_CONNECTIONS` 等环境变量解析。

### P-M3. 分页查询执行两次独立 SQL，WHERE 子句重复构建
- **文件**: `crates/yang-base/src/table/table_query.rs:1199`
- **修复**: 将 WHERE 子句构建与 SQL 拼接解耦——先一次性构建 params 向量，然后复用同一份 params 分别拼 COUNT 和 SELECT SQL。

### P-M4. MySQL Condition 枚举缺少 NotIn 变体
- **文件**: `crates/yang-db/src/mysql/condition.rs:20`
- **修复**: 添加 `NotIn(String, Vec<SqlValue>)` 变体，填补 yang-db 与 yang-base 之间的能力缺口。

### P-M5. TableQuery 缺少关联查询/eager loading 机制，存在 N+1 风险
- **文件**: `crates/yang-base/src/table/table_query.rs:94`
- **修复**: 添加 `joins: Vec<JoinClause>` 字段和相关链式 API。

### P-M6. RedisValue::as_string() 总是 clone 返回 Owned String
- **文件**: `crates/yang-db/src/redis/value.rs:41`
- **修复**: 改为返回 `Option<&str>`。让调用方仅在需要所有权时才 clone。

### P-M7. collect_string_array 每个元素都 clone
- **文件**: `crates/yang-db/src/redis/client.rs:1944`
- **修复**: 如果 `as_string` 改为返回 `Option<&str>`，用 `filter_map(|v| v.as_str().map(String::from))`；如果能消费 RedisValue，用 `into_iter` 取走所有权。

### P-M8. redis::Value 未知类型用 format!("{:?}") 做 Debug 格式化分配
- **文件**: `crates/yang-db/src/redis/value.rs:180`
- **修复**: 对不支持的 Redis 类型返回错误或使用固定字符串标记，而非 Debug 格式化整个 value 树。

### P-M9. build_insert_sql 和 build_update_sql_impl 中 Vec 未预分配
- **文件**: `crates/yang-base/src/table/table_query.rs:2132, 2413`
- **修复**: 使用 `Vec::with_capacity(data.len())` 预分配。

### P-M10. PluginManager::register 存在 TOCTOU 竞态窗口（已知 I11）
- **文件**: `crates/yang-base/src/plugin/mod.rs:243-268`
- **修复**: 按 I11 计划：将 read-check→write-insert 合并为单次 write 锁内的 check-and-insert。

## LOW (6项)

### P-L1. GlobalRedis::close() 同步而 GlobalDatabase::close() 异步，API 不对称
- **文件**: `crates/yang-base/src/database/global_redis.rs:166`

### P-L2. pool_status().waiting 硬编码为 0 无法反映真实等待数
- **文件**: `crates/yang-db/src/mysql/database.rs:177`

### P-L3. build_where 对多条件路径做不必要的 Vec 克隆
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:203`

### P-L4. insert_batch_with_size 每批序列化为 JSON 产生中间分配
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:2207`

### P-L5. Condition 枚举的字段名每次分配 String
- **文件**: `crates/yang-db/src/mysql/condition.rs:20`
- **建议**: 改为 `Cow<'static, str>` 或 `Arc<str>`（有侵入性，性能驱动确认后再做）。

### P-L6. 无 spawn_blocking：SQL 生成与 JSON 序列化均在异步运行时线程上执行
- **文件**: `crates/yang-db/src/mysql/query_builder.rs`

## 补充发现：性能问题（完整性审查新增）

### P-NEW-1. [HIGH] SQL 生成+绑定阶段 String/Bytes/JSON 参数系统性双重 clone
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:870`、`crates/yang-db/src/mysql/transaction.rs:536`、`crates/yang-db/src/postgres/query_builder.rs:886`、`crates/yang-base/src/table/table_query.rs:2822,1538`
- **证据**: 每条字符串/字节/JSON 参数被 clone 两次：第一次在 SQL 生成时（`Value::String(s) => Ok(SqlValue::String(s.clone()))`），第二次在绑定时（`SqlValue::String(s) => query.bind(s.clone())`）。中间 `SqlValue::String(String)` 存储了 owned String，执行时本可通过 `Vec::drain` 或 `into_iter` 移出，避免第二次堆分配和拷贝。JSON 值还额外调用 `j.to_string()` 重新序列化整个 JSON 载荷。
- **影响**: 每条查询的每个字符串/字节列多一次完整堆分配+拷贝。批量和多列场景下是整个代码库最显著的分配热点。
- **修复**: 执行时将 params 消费（`into_iter`），从 `SqlValue` 中移出 String 而非 clone；或重构为两步（先生成 SQL 后直接绑定，无中间 `SqlValue` 持有）。

### P-NEW-2. [MEDIUM] build_having 使用低效借用路径，与 build_where 不一致
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:286-304`
- **证据**: `build_having` 调用 `condition_to_sql`（借用版），内部执行 `condition.clone()` 克隆整个 Condition 树。而 `build_where` 正确使用了 `condition_to_sql_owned` 直接消费 Condition。多条件 HAVING 子句中每个条件被单独 clone——与 `build_where` 的优化路径不一致。
- **修复**: 将 `build_having` 改为使用 `condition_to_sql_owned`，与 `build_where` 保持一致。

### P-NEW-3. [MEDIUM] 多处 Vec::new() 未使用 with_capacity（已知最终大小）
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:337-338,505`、`crates/yang-db/src/postgres/query_builder.rs:360-361`、`crates/yang-base/src/table/table_query.rs:2132-2134,2413-2414`
- **证据**: `build_insert` 中 `fields` 和 `placeholders` 已知 `obj.len()`，`build_update` 中 `set_clauses` 已知 `obj.len()`，但均使用 `Vec::new()` 从零容量开始，导致每次 push 触发若干次 reallocation（5-20 字段场景约 3-4 次重新分配）。
- **修复**: 使用 `Vec::with_capacity(data.len())` 预分配。

---

# 第三部分：逻辑与正确性审计

## CRITICAL (2项)

### L-C1. Transaction 缺少 Drop 实现，未提交事务被静默丢弃
- **文件**: `crates/yang-db/src/mysql/transaction.rs`
- **证据**: Transaction 仅有 commit() 和 rollback() 两个显式方法，无 `impl Drop`。当因 `?` 提前返回或 panic 被丢弃时，底层 sqlx 隐式 rollback 无日志、无错误上报。PG 端同样缺失。
- **修复**: 为 Transaction 实现 Drop（或 RAII 风格的 TransactionGuard），至少记录 warn 日志。

### L-C2. GlobalDatabase::init 和 GlobalRedis 所有方法将 DbError 转为字符串，丢弃错误链
- **文件**: `crates/yang-base/src/database/global.rs:92`、`crates/yang-base/src/database/global_redis.rs:100`
- **证据**: `.map_err(|e| BaseError::DatabaseConnectionFailed(e.to_string()))` — DbError 被转为字符串，丢失了 source() 链和 DbError::code()。而 BaseError 已有 `DatabaseConnectionDbError(#[source] yang_db::DbError)` 变体。
- **修复**: 改用 `BaseError::DatabaseConnectionDbError` 和 `BaseError::RedisOperationDbError` 保留 source 链。

## HIGH (5项)

### L-H1. DatabaseTransactionFailed 被错误分类为 Transient（可重试）
- **文件**: `crates/yang-base/src/error/mod.rs:668`
- **修复**: 将 DatabaseTransactionFailed 从 Transient 移至 Server 类别。

### L-H2. HttpResponseParseFailed 和 HttpCircuitBreakerOpen 被错误分类为 Transient
- **文件**: `crates/yang-base/src/error/mod.rs:687-688`
- **修复**: 分别移至 Client 和 Server 类别。

### L-H3. Plugin trait 回调返回 Box<dyn Error>，错误类型被擦除
- **文件**: `crates/yang-base/src/plugin/mod.rs:138-161`
- **修复**: 改为返回 `Result<(), BaseError>`，或至少保留 source() 链用于日志。

### L-H4. 非事务模式下插件初始化失败会留下半初始化状态
- **文件**: `crates/yang-base/src/database/initializer.rs:280-307`
- **证据**: 非事务模式第 N 个插件失败时前 N-1 个插件的 SQL 已提交（无事务包裹）；而事务模式失败时事务会被 sqlx 的 Drop 回滚。两者行为不对称。
- **修复**: 文档明确声明非事务模式不保证原子性，推荐生产环境始终使用事务模式。

### L-H5. action ↔ router 模块循环依赖
- **文件**: `crates/yang-base/src/action/auth.rs:40` ↔ `crates/yang-base/src/router/module_router.rs:34`
- **证据**: `action/auth.rs` 导入 `crate::router::{Middleware, Next}`，而 `router/` 导入 `crate::action::*`。双向 import 形成架构环。
- **修复**: 将 Middleware/Next 提取到独立的基础模块（如 `router::middleware_types`）。

## MEDIUM (9项)

### L-M1. From<serde_json::Error> 的分类逻辑不可靠
- **文件**: `crates/yang-base/src/error/mod.rs:380-388`
- **修复**: 废弃此 From 实现，改为在调用处显式 map_err。

### L-M2. 生产代码中的 expect() 可能导致 panic
- **文件**: `crates/yang-base/src/table/table_query.rs:999`
- **修复**: 将 expect 替换为 `ok_or_else(|| BaseError::Unknown(...))?`。

### L-M3. RedisValue 类型转换方法静默返回 None
- **文件**: `crates/yang-db/src/redis/value.rs:39-56`
- **修复**: 提供 `as_string_checked() -> Result<Option<String>, DbError>` 系列方法。

### L-M4. DatabaseBundle 半初始化无回滚机制
- **文件**: `crates/yang-base/src/database/bundle.rs:63-73`
- **修复**: 在 init 失败时若 MySQL 已初始化，设置「bundle 初始化未完成」标志。

### L-M5. topological_sort 中 unwrap_or(usize::MAX) 静默掩盖排序失败
- **文件**: `crates/yang-base/src/plugin/mod.rs:437`
- **修复**: 改为记录 warn 或返回 Result。

### L-M6. verify_token_checked() 两次 Redis 查询非原子，存在 TOCTOU 窗口
- **文件**: `crates/yang-base/src/token/revocation.rs:175-188`
- **修复**: 使用 Lua 脚本（EVAL）在 Redis 端原子执行两个检查。

### L-M7. 中间件链与 Action 派发无 panic 防护
- **文件**: `crates/yang-base/src/router/middleware.rs:90-102`
- **修复**: 用 `catch_unwind` 包裹中间件调用，将 panic 转为 `BaseError::InternalError`。

### L-M8. RedisTransaction::exec() 非可重试错误路径未显式 DISCARD/UNWATCH
- **文件**: `crates/yang-db/src/redis/transaction.rs:313-369`
- **修复**: 在非可重试错误返回前显式发送 UNWATCH 或 DISCARD 命令清理连接状态。

### L-M9. QueryBuilder 与 TransactionQueryBuilder 的 where_and 错误处理策略分裂
- **文件**: `crates/yang-db/src/mysql/transaction.rs:284`
- **修复**: 统一为 `Result<Self, DbError>` 模式。

## LOW (8项)

### L-L1. DbError::QueryError 覆盖范围过宽
- **文件**: `crates/yang-db/src/error.rs:158-206`
- **修复**: 细化 sqlx::Error 的映射（Protocol→ConnectionError，RowNotFound→单独变体等）。

### L-L2. 缺少 From<tokio_postgres::Error> 实现
- **文件**: `crates/yang-db/src/error.rs`

### L-L3. QueryParams 字段公开可绕过 page_size=0 校验
- **文件**: `crates/yang-base/src/table/query_params.rs`

### L-L4. TTL 的 u64→i64 转换在极端值下会溢出为负数
- **文件**: `crates/yang-base/src/token/revocation.rs:83, 132`
- **修复**: 改为 `ttl.min(i64::MAX as u64) as i64` 做饱和截断。

### L-L5. TokenManager 接受 TTL=0 无校验
- **文件**: `crates/yang-base/src/token/manager.rs:153-233`
- **修复**: 在构造器中增加 expiry > 0 的校验。

### L-L6. CircuitBreaker HalfOpen 允许多个并发探测请求同时放行
- **文件**: `crates/yang-base/src/http/circuit_breaker.rs:108`
- **修复**: 如需限流探测数量，可在 HalfOpen 分支中加入 max_concurrent_probes 计数器。

### L-L7. exec() 在 100 次重试耗尽后无 backoff
- **文件**: `crates/yang-db/src/redis/transaction.rs:301-369`
- **修复**: 在重试循环中加入指数退避。

### L-L8. u64 to SqlValue 超大值降级为 String，调用方无感知
- **文件**: `crates/yang-db/src/mysql/condition.rs:62`
- **修复**: 文档显式标注此行为。

## 补充发现：逻辑正确性问题（完整性审查新增）

### L-NEW-1. [MEDIUM] RedisTransaction::exec() 重试循环在循环外获取连接，死连接导致 100 次重试全部立即失败
- **文件**: `crates/yang-db/src/redis/transaction.rs:302-311`
- **证据**: 连接在重试循环**外部**获取（`let mut conn = self.client.pool().get().await...`），之后 100 次重试全部使用同一个连接。若 Redis 服务重启或连接在重试中途断开，所有重试将立即失败——重试机制完全不提供故障恢复能力。审计 L-L7 指出了缺少指数退避，但未覆盖连接复用这一独立的正确性缺陷。
- **修复**: 将 `pool.get()` 移入重试循环内部，确保每次重试获取新连接。

### L-NEW-2. [MEDIUM] graceful_shutdown 完全被 #[cfg(feature = "mysql")] 绑死，纯 Redis 用户无统一停机入口
- **文件**: `crates/yang-base/src/lifecycle.rs:69`
- **证据**: 整个 `graceful_shutdown` 函数被 `#[cfg(feature = "mysql")]` 门控。仅使用 Redis（`default-features = false, features = ["redis"]`）的下游用户完全没有任何统一停机入口，必须手动调用 `GlobalRedis::close()`，且没有 `wait_for_shutdown_signal()` → `graceful_shutdown()` 流水线可用。审计 A-M8 将此框定为 API 不一致，但低估了正确性风险：遗漏停机意味着悬挂连接和不干净的进程退出。
- **修复**: 将 Redis 关闭逻辑提取为独立函数，移除 mysql feature gate 依赖；`graceful_shutdown` 始终可用。

---

# 第四部分：API 与架构审计

## CRITICAL (3项)

### A-C1. query_builder.rs 5484 行为全仓库最大文件，严重瓶颈
- **文件**: `crates/yang-db/src/mysql/query_builder.rs`
- **修复**: 按 SQL 子句拆分：where_builder.rs、select_builder.rs、insert_builder.rs、update_builder.rs、join_builder.rs、batch_builder.rs。此为高风险操作，必须逐步进行并保持全部测试通过。

### A-C2. close() 异步不一致：Redis 同步 vs MySQL 异步
- **文件**: `crates/yang-db/src/redis/client.rs:124`
- **修复**: 统一为 `async fn close(&self)`。

### A-C3. 测试文件无条件依赖 token feature，禁用时编译失败（3 个文件）
- **文件**: `crates/yang-base/src/action/__tests__/context_test.rs:4`、`typed_test.rs:7`、`crates/yang-base/src/router/__tests__/module_router_tests.rs:2`
- **修复**: 添加 `#[cfg(feature = "token")]` gate。

## HIGH (10项)

### A-H1. health_check() 返回类型不一致：MySQL 返回 () vs Redis 返回 bool
- **文件**: `crates/yang-db/src/mysql/database.rs:189`
- **修复**: 将 MySQL/PG 的 health_check() 也改为返回 `Result<bool, DbError>`。

### A-H2. 公共枚举 Condition、SqlValue、FieldType 等缺少 #[non_exhaustive]
- **文件**: `crates/yang-db/src/mysql/condition.rs:6` 等 6 个枚举
- **修复**: 为 Condition、SqlValue、FieldType（mysql + postgres）、JoinType、JoinClause、OrderClause 添加 `#[non_exhaustive]`。

### A-H3. RedisConfig 使用位置参数构造函数，与 DatabaseConfig 的 Builder 模式不一致
- **文件**: `crates/yang-db/src/redis/config.rs:53`
- **修复**: 添加 `with_max_connections` / `with_connect_timeout` / `with_wait_timeout` / `with_enable_logging` 链式 setter，标记 `new()` 为 deprecated。

### A-H4. TransactionQueryBuilder 完全复制 QueryBuilder 的字段类型方法，无共享 trait
- **文件**: `crates/yang-db/src/mysql/transaction.rs:242-278`
- **修复**: 提取 FieldTypeMarker trait 或使用宏消除重复。

### A-H5. MySQL/PostgreSQL 模块完全平行复制，无共享抽象
- **文件**: `crates/yang-db/src/mysql/condition.rs` vs `crates/yang-db/src/postgres/condition.rs`
- **修复**: 引入 common 模块提取 SqlValue/Condition/FieldType/JoinClause/OrderClause 等方言无关类型。

### A-H6. table_query.rs 2829 行，权限校验 + SQL 渲染混杂
- **文件**: `crates/yang-base/src/table/table_query.rs`
- **修复**: 拆分为 query_builder.rs（查询构造）、permission_checker.rs（权限校验）、where_renderer.rs（WHERE 条件渲染）。

### A-H7. derive crate 硬编码 ~20 个 ::yang_base:: 路径引用，无契约层
- **文件**: `crates/yang-base-derive/src/table_entity.rs:119`
- **修复**: 在 yang-base 中建立 `pub mod derive_contract` 模块集中 re-export derive 所需类型。

### A-H8. RedisClient 2151 行，命令方法过多（50+ 方法）
- **文件**: `crates/yang-db/src/redis/client.rs`
- **修复**: 按命令族拆分：commands/string.rs、hash.rs、list.rs、set.rs、sorted_set.rs。

### A-H9. validator 测试的行为断言依赖 strict regex，禁用 feature 时失败
- **文件**: `crates/yang-base/src/table/__tests__/validator_test.rs:214`
- **修复**: 为依赖 strict regex 行为的测试项添加 `#[cfg(feature = "validator")]` gate。

### A-H10. metrics feature 缺少运行期 exporter 需求的文档说明
- **文件**: `crates/yang-base/Cargo.toml:43`
- **修复**: 在 lib.rs 的 Feature Gates 文档节中补充说明。

## MEDIUM (9项)

### A-M1. query_with_params 使用 Vec<serde_json::Value> 做参数化（String typing）
- **文件**: `crates/yang-db/src/mysql/database.rs:327`
- **修复**: 提供类型安全替代 `query_with_typed_params`。

### A-M2. 标识符工具函数未在 crate root 重导出
- **文件**: `crates/yang-db/src/lib.rs:23`
- **修复**: 添加 `is_valid_identifier`、`quote_identifier`、`quote_qualified` 到 root re-export。

### A-M3. MigrationConfig 公开但未在任何层级重导出
- **文件**: `crates/yang-db/src/mysql/init.rs:7`

### A-M4. GlobalDatabase::get() 与 GlobalRedis::client() 命名不对称
- **文件**: `crates/yang-base/src/database/global.rs:120`
- **修复**: 统一为 `get()` 或 `instance()`。

### A-M5. bind_execute_param 在 query_builder.rs 和 transaction.rs 中重复定义
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:2696`
- **修复**: 提取到 mysql 模块的共享位置。

### A-M6. plugin/mod.rs 1386 行单一文件
- **文件**: `crates/yang-base/src/plugin/mod.rs`

### A-M7. 模块 gate 风格不一致——声明级 vs 文件级 cfg
- **文件**: `crates/yang-base/src/action/mod.rs:60`
- **修复**: 统一为声明级 `#[cfg(feature = "xxx")] pub mod xxx;` 模式。

### A-M8. graceful_shutdown 被 mysql feature gate 绑定，Redis 仅场景无统一停机入口
- **文件**: `crates/yang-base/src/lifecycle.rs:69`
- **修复**: 将 Redis 关闭逻辑提取为独立函数。

### A-M9. Database::pool() 直接暴露 sqlx::MySqlPool 内部实现
- **文件**: `crates/yang-db/src/mysql/database.rs:159`
- **修复**: 改为 `pub(crate)` 或用 `doc(hidden)` 标注。

## LOW (10项)

### A-L1. 大量 pub 泄露内部实现 — yang-db 91 pub vs 2 pub(crate)
- **文件**: `crates/yang-db/src/lib.rs:17`
- **修复**: 收窄子模块 visibility 为 `pub(crate)`。

### A-L2. u64 to SqlValue 超大值降级为 String
- **文件**: `crates/yang-db/src/mysql/condition.rs:62`

### A-L3. where_or 每次调用都会加深 Condition 嵌套层级
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:1113`

### A-L4. RedisValue 对多种 Redis 类型退化为 Debug 字符串
- **文件**: `crates/yang-db/src/redis/value.rs:178`

### A-L5. SqlValue::Null 绑定为 Option<i32>::None 可能导致非整数列类型不匹配
- **文件**: `crates/yang-db/src/mysql/query_builder.rs:22`

### A-L6. token feature 在 Windows 上编译需 NASM
- **文件**: `crates/yang-base/Cargo.toml:28`

### A-L7. 快速开始文档示例使用全部 feature 但未标注需求
- **文件**: `crates/yang-base/src/lib.rs:25`

### A-L8. token 模块直接依赖 database 模块（GlobalRedis）
- **文件**: `crates/yang-base/src/token/revocation.rs:17`
- **注**: 依赖方向正确（上层依赖下层），但通过 trait 抽象可提高可测试性。

### A-L9. auth.rs action 模块依赖 router 模块引发架构矛盾
- **文件**: `crates/yang-base/src/action/auth.rs:38-41`
- **修复**: 与 A-H5（循环依赖）联合修复。

### A-L10. error/mod.rs 1137 行过大，BaseError 变体过多（25+ 变体）
- **文件**: `crates/yang-base/src/error/mod.rs:37`

---

# 第五部分：文档一致性审计

## HIGH (8项)

### D-H1. action/AGENTS.md "ACTION TRAIT" 节描述的是已删除的旧 Action trait
- **文件**: `crates/yang-base/src/action/AGENTS.md:32-42`
- **修复**: 重写为 "TYPED ACTION SYSTEM"：TypedHandler → TypedAction → DynAction 三层体系。

### D-H2. action/AGENTS.md WHERE TO LOOK 表格指向错误的文件和方法
- **文件**: `crates/yang-base/src/action/AGENTS.md:23`
- **修复**: 改为 "implement TypedHandler::handle；用 #[derive(Action)] 派生元信息层"。

### D-H3. action/AGENTS.md BUILTIN ACTIONS 表描述的是 pre-H-1 的 serde_json::Value 输入输出
- **文件**: `crates/yang-base/src/action/AGENTS.md:44-52`
- **修复**: 改为类型化版本（AddAction Input=T, GetAction Output=T 等）。

### D-H4. yang-base/AGENTS.md CODE MAP 中 Action 符号指向已过时
- **文件**: `crates/yang-base/AGENTS.md:48`
- **修复**: 拆为 TypedHandler、TypedAction、DynAction、Permission 四行。

### D-H5. yang-base/AGENTS.md ANTI-PATTERNS 声称 Token 系统无撤销/黑名单
- **文件**: `crates/yang-base/AGENTS.md:80`
- **修复**: 删除该 anti-pattern，替换为正确的使用指引。

### D-H6. table/AGENTS.md STRUCTURE 和 CODE MAP 遗漏 entity.rs 及整个类型化实体体系
- **文件**: `crates/yang-base/src/table/AGENTS.md:9-44`
- **修复**: 补上 TableEntity、AsColumnName、WhereOp、Filter 等符号。

### D-H7. yang-db/AGENTS.md STRUCTURE 和 CODE MAP 完全遗漏 postgres/ 模块
- **文件**: `crates/yang-db/AGENTS.md:9-59`
- **修复**: 补全 postgres/ 子树和 isolation.rs。

### D-H8. docs/yang-base.md Action 模块仍描述已删除的旧 Action trait
- **文件**: `docs/yang-base.md:782-957`
- **修复**: 全节重写为类型化 Action 系统。

## MEDIUM (12项)

### D-M1. docs/yang-base.md ModuleRouter 使用不存在的 register_builtin_actions()
- **文件**: `docs/yang-base.md:973`
- **修复**: 替换为 `table_typed::<T>()?`；新增中间件小节。

### D-M2. docs/yang-base.md Token 模块完全遗漏撤销/黑名单机制
- **文件**: `docs/yang-base.md:327-414`
- **修复**: 新增撤销机制小节。

### D-M3. docs/yang-base.md HTTP 模块遗漏熔断器和重试配置
- **文件**: `docs/yang-base.md:417-503`
- **修复**: 新增韧性小节。

### D-M4. docs/yang-base.md Database 模块遗漏 DatabaseBundle 统一初始化入口
- **文件**: `docs/yang-base.md:187-323`
- **修复**: 新增 DatabaseBundle 小节。

### D-M5. docs/yang-base.md 错误码表遗漏 15+ 个新增错误码
- **文件**: `docs/yang-base.md:57-68`
- **修复**: 从 error/mod.rs 的 code() 方法中提取完整列表。

### D-M6. action/AGENTS.md STRUCTURE 树缺少 5 个关键文件
- **文件**: `crates/yang-base/src/action/AGENTS.md:10-17`
- **修复**: 补全 auth.rs、meta.rs、request_id.rs、sql_bridge.rs、typed.rs。

### D-M7. action/AGENTS.md 第 12/16 行注释仍称 action_trait.rs 包含 Action trait
- **文件**: `crates/yang-base/src/action/AGENTS.md:12,16`
- **修复**: 更新注释。

### D-M8. yang-base/AGENTS.md STRUCTURE 声称 "8 public modules" 与实际 11 个不符
- **文件**: `crates/yang-base/AGENTS.md:12`
- **修复**: 更新为 11 个 public modules。

### D-M9. yang-base/AGENTS.md ANTI-PATTERNS 声称 builtin actions 仍主要使用 serde_json::Value
- **文件**: `crates/yang-base/AGENTS.md:76`
- **修复**: 更新为准确描述。

### D-M10. yang-db/AGENTS.md mysql/ STRUCTURE 缺少 identifier.rs，根级缺少 isolation.rs
- **文件**: `crates/yang-db/AGENTS.md:13-29`

### D-M11. BACKLOG.md M-1 测试 unwrap 数量严重低报
- **文件**: `docs/BACKLOG.md`
- **修复**: 标题改为 "~870+ unwrap/expect"，更新文件列表。

### D-M12. BACKLOG.md 最近更新日期为 2026-05-31，缺少近期修复项
- **文件**: `docs/BACKLOG.md`
- **修复**: 更新日期至当前；补充 DB/NEW/NG/I 系列修复。

## LOW (22项)

### D-L1. docs/yang-base.md Feature 列表遗漏 metrics feature
### D-L2. docs/yang-base.md 模块依赖图遗漏 middleware 子模块
### D-L3. yang-db/AGENTS.md HOTSPOTS 文件行数过时（4.8k→5.5k, 2k→2.2k）
### D-L4. yang-db/AGENTS.md WHERE TO LOOK 缺少 Redis PoolStatus
### D-L5. yang-base/AGENTS.md plugin/mod.rs 行数略有过时（1.1k→1.4k）
### D-L6. BACKLOG.md [C-1] — 状态准确，无需修改
### D-L7. BACKLOG.md [C-2] — 状态准确，无需修改
### D-L8. BACKLOG.md [H-1] — 状态准确，无需修改
### D-L9. BACKLOG.md [H-2] — 建议改为「✅ 已完成（核查确认始终合规）」
### D-L10. BACKLOG.md [H-3] — 状态准确，无需修改
### D-L11. BACKLOG.md [H-4] — 状态准确，无需修改
### D-L12. BACKLOG.md [H-5] — 核心准确，遗留端到端中间件单测仍缺失
### D-L13. BACKLOG.md [H-6] — 状态准确，无需修改
### D-L14. BACKLOG.md [M-2] — 状态准确，无需修改
### D-L15. BACKLOG.md [M-3] — 状态准确，无需修改
### D-L16. BACKLOG.md [M-4] — 状态准确，无需修改
### D-L17. BACKLOG.md [L-1] — 状态准确，无需修改
### D-L18. BACKLOG.md [L-2] — 状态准确，无需修改
### D-L19. BACKLOG.md [L-3] — 状态准确，无需修改
### D-L20. BACKLOG.md [L-4] — 状态准确，无需修改
### D-L21. BACKLOG.md [L-5] — 状态准确，无需修改
### D-L22. BACKLOG.md [汇总表] — 与详细条目一致，修正 M-1 后同步更新

---

# 第六部分：行动计划

## Phase 0: 阻断修复（第 1-2 天）—— 生产部署前必须完成

| 编号 | 任务 | 关联发现 | 预估 |
|------|------|---------|------|
| P0-1 | **裸 SQL API 废弃标记**：对 8 个方法标记 `#[deprecated]`，增加安全警告注释 | S-C1~C8 | 2h |
| P0-2 | **标识符转义修复**：`condition_to_sql_owned()` 中所有 field 参数调用 `quote_identifier`；`build_select()` 表名强制 quote | S-H1~H6 | 4h |
| P0-3 | **Token 撤销绕过修复**：`refresh_access_token` 改用 `verify_token_checked` | S-H7 | 1h |
| P0-4 | **RefreshAction Token Rotation**：使之调用 `rotate_refresh_token` 或文档警告 | S-H8 | 3h |
| P0-5 | **Redis recycle 修复**：增加 `idle_timeout`/`max_lifetime` 字段，recycle 设为独立参数 | P-H1 | 3h |
| P0-6 | **ErrorCategory 修正**：修正 3 个错误分类 | L-H1, L-H2 | 1h |
| P0-7 | **Transaction Drop 日志**：添加 Drop 实现记录 warn 日志 | L-C1 | 2h |
| P0-8 | **测试 feature gate 修复**：为 3 个测试文件添加 cfg gate | A-C3 | 1h |
| P0-9 | **聚合方法/value()/RETURNING 标识符转义**：sum/avg/min/max/value 中 field 调用 quote_identifier；PG RETURNING 列名转义 | S-NEW-SQL-1~4 | 4h |
| P0-10 | **parse_token_unsafe 废弃标记**：标记 deprecated，加严重安全警告，考虑降为 pub(crate) | S-NEW-AUTH-1 | 1h |
| **P0 合计** | | | **~22h** |

## Phase 1: 重要改进（第 3-5 天）

| 编号 | 任务 | 关联发现 | 预估 |
|------|------|---------|------|
| P1-1 | **RedisConfig Builder 模式**：添加 with_* 链式 setter | A-H3 | 2h |
| P1-2 | **Redis 自愈参数补充**：增加 min_connections/max_lifetime/idle_timeout/test_before_acquire | P-M1 | 3h |
| P1-3 | **#[non_exhaustive] 补充**：6 个枚举 | A-H2 | 1h |
| P1-4 | **错误链保留**：GlobalDatabase::init 和 GlobalRedis 改用带 source 的错误变体 | L-C2 | 2h |
| P1-5 | **paginate() LIMIT 修复**：调用 select() 前写回 page/page_size | S-M11 (旧) | 1h |
| P1-6 | **page_size 上限**：增加上限校验（默认 1000） | S-M12 (旧) | 0.5h |
| P1-7 | **敏感信息 Debug 遮蔽**：LoginInput/TokenResponse/TokenClaims 手动实现 Debug | S-M8~M12 | 2h |
| P1-8 | **validator 降级日志**：Email/Phone 降级打 warn；Regex 不硬错误 | S-M13 | 1h |
| P1-9 | **Redis close() 异步化**：统一为 async fn | A-C2 | 1h |
| P1-10 | **health_check() 返回类型统一**：改为返回 Result\<bool\> | A-H1 | 1h |
| P1-11 | **FieldPermissions HashSet 化**：Vec→HashSet 降 O(n) 为 O(1) | P-H6 | 1h |
| P1-12 | **Vec 预分配**：build_insert_sql/build_update_sql_impl 使用 with_capacity | P-M9 | 0.5h |
| P1-13 | **HashMap clone 推迟**：build_update_sql_impl 条件判断前不 clone | P-H3 | 0.5h |
| P1-14 | **expect 消除**：hash_condition_renderer table_query expect 用 ok_or_else 替代 | L-M2 | 0.5h |
| P1-15 | **循环依赖修复**：提取 Middleware/Next 到独立基础模块 | L-H5 | 2h |
| P1-16 | **文档修复 P0**：action/AGENTS.md + yang-base/AGENTS.md + yang-base.md Action 章节重写 | D-H1~H8 | 6h |
| P1-17 | **PG 镜像标识符转义修复**：PG condition_to_sql_owned/build_select/build_order_by/build_group_by/build_joins 对标 MySQL 修复 | S-NEW-SQL-5~11 | 3h |
| P1-18 | **认证中间件启动期检查**：debug 断言或启动日志警告缺少认证中间件的路由器 | S-NEW-AUTH-2 | 1h |
| P1-19 | **with_user() 权限边界文档**：标明仅供中间件/内部使用；考虑 user 字段改为 pub(crate) | S-NEW-AUTH-3 | 0.5h |
| P1-20 | **GetAction/SelectAction unwrap_or 替换**：用 ok_or_else 返回 Unauthorized 而非静默降级 | S-NEW-AUTH-4 | 0.5h |
| P1-21 | **GlobalDatabase 绕过 TableQuery 安全文档**：AGENTS.md 增加安全说明，警告自定义 Action 作者 | S-NEW-AUTH-5 | 0.5h |
| P1-22 | **SQL 绑定双重 clone 消除**：执行时消费 params（into_iter）从 SqlValue 移出 String 而非 clone | P-NEW-1 | 4h |
| P1-23 | **build_having 改用 owned 路径**：与 build_where 一致使用 condition_to_sql_owned | P-NEW-2 | 1h |
| P1-24 | **Vec 预分配补全**：build_insert/build_update 及 PG 侧使用 with_capacity | P-NEW-3, P-M9 | 1h |
| P1-25 | **RedisTransaction 连接获取移入重试循环**：每次重试获取新连接以支持故障恢复 | L-NEW-1 | 1h |
| P1-26 | **graceful_shutdown 解绑 mysql feature**：提取 Redis 关闭逻辑为独立函数，统一停机入口始终可用 | L-NEW-2 | 1.5h |
| **P1 合计** | | | **~38.5h** |

## Phase 2: 文档与生态（第 6-7 天）

| 编号 | 任务 | 关联发现 | 预估 |
|------|------|---------|------|
| P2-1 | **docs/yang-base.md 全面更新**：补全撤销/中间件/熔断器/DatabaseBundle/错误码表/Feature 表 | D-H8, D-M1~M5 | 4h |
| P2-2 | **yang-db/AGENTS.md 更新**：补充 postgres 模块、isolation.rs、identifier.rs；更新行数 | D-H7, D-M10, D-L3 | 1h |
| P2-3 | **BACKLOG.md 更新**：修正 M-1 计数、补充近期修复项、新增审计发现条目 | D-M11, D-M12 | 2h |
| P2-4 | **cargo-audit 集成**：添加 pre-commit hook 或开发脚本 | S-H9 | 2h |
| P2-5 | **tokio features 瘦身**：workspace 级别改为最小特性集 | S-M14 | 1h |
| P2-6 | **标识符工具函数重导出** | A-M2 | 0.5h |
| **P2 合计** | | | **~10.5h** |

## Phase 3: 架构优化（可持续迭代，不阻塞生产）

| 编号 | 任务 | 关联发现 | 预估 |
|------|------|---------|------|
| P3-1 | **query_builder.rs 拆分**：按 SQL 子句拆分 | A-C1 | 16h+ |
| P3-2 | **RedisClient 按命令族拆分** | A-H8 | 8h |
| P3-3 | **table_query.rs 拆分**：权限校验/SQL 渲染分离 | A-H6 | 8h |
| P3-4 | **MySQL/PG 共享抽象**：提取 common 模块 | A-H5 | 8h |
| P3-5 | **derive_contract 模块**：集中 re-export | A-H7 | 2h |
| P3-6 | **中间件 panic 防护**：catch_unwind 包裹 | L-M7 | 2h |
| P3-7 | **Lua 脚本原子撤销检查** | L-M6 | 3h |
| P3-8 | **Plugin trait 错误类型改进**：Box\<dyn Error\>→BaseError | L-H3 | 2h |
| P3-9 | **#[action(public)] 护栏**：clippy lint 或构建期检查，对非认证 Action 标 public 发出警告 | S-NEW-AUTH-6 | 2h |
| P3-10 | **JWT 密钥轮换机制**：增加可选 kid 字段和多解码密钥支持 | S-NEW-AUTH-7 | 3h |
| P3-11 | **TableAction 角色过滤**：按用户字段读权限过滤输出 schema | S-NEW-AUTH-8 | 2h |
| **P3 合计** | | | **~56h** |

## 工时汇总

| Phase | 总工时 | 说明 |
|-------|--------|------|
| Phase 0（阻断） | ~22h | 约 3 个工作日，全部串行 |
| Phase 1（重要） | ~38.5h | 约 5 个工作日，可部分并行 |
| Phase 2（文档） | ~10.5h | 约 1.5 个工作日 |
| Phase 3（架构） | ~56h | 可分批迭代，不阻塞生产 |
| **总计** | **~127h** | Phase 0+1 = 8 个工作日可关闭所有 blocking |

---

# 附录：正向发现

审计中发现的正面实践值得保持和扩展：

1. **TokenManager Debug 实现** (`manager.rs:566`) — 正确遮蔽了 encoding_key/decoding_key，使用 `finish_non_exhaustive()`，可作为其他 Debug 遮蔽实现的参考模板
2. **HttpClientConfig 默认安全** (`http/client.rs:63`) — `accept_invalid_certs` 默认 false，文档已充分警告
3. **SqlGenerator 预分配** (`query_builder.rs:67`) — `String::with_capacity(256)` + `Vec::with_capacity(8)` 合理预分配
4. **CircuitBreaker 锁使用正确** (`circuit_breaker.rs`) — `std::sync::Mutex` 在同步块内获取释放，永不跨 `.await`
5. **PluginRegistry 无锁设计** (`plugin/mod.rs`) — 构建阶段有锁，运行阶段零锁，教科书级分阶段设计
6. **GlobalTools 锁中毒恢复** (`action/context.rs`) — `unwrap_or_else(|p| p.into_inner())` 正确处理锁中毒
7. **值侧参数化始终正确** — 整个代码库的值绑定（`?` 占位符）未发现注入漏洞
8. **TableEntity/WhereOp/Filter 类型化体系** — H-1 重构成果完整落地，端到端类型安全覆盖 CRUD 路径

---

# 附录：BACKLOG 新增条目建议

| ID | 标题 | 优先级 | 简述 |
|----|------|--------|------|
| NEW-1 | Database::execute/query 裸 SQL 废弃 | 🔴 CRITICAL | 8 个方法需标记 deprecated 并引导到参数化替代 |
| NEW-2 | condition_to_sql_owned 标识符转义缺失 | 🟠 HIGH | 12 个变体中的 field 需调 quote_identifier |
| NEW-3 | Transaction Drop 回滚日志 | 🔴 CRITICAL | 未提交事务被静默丢弃无日志 |
| NEW-4 | refresh_access_token 绕过黑名单 | 🟠 HIGH | 改用 verify_token_checked 或标记 deprecated |
| NEW-5 | RefreshAction 无 Token Rotation | 🟠 HIGH | 改为调用 rotate_refresh_token |
| NEW-6 | Redis recycle 参数错误 | 🟠 HIGH | recycle 不应复用 connect_timeout |
| NEW-7 | RedisConfig 缺少连接池自愈参数 | 🟡 MEDIUM | 增加 min_connections/max_lifetime/idle_timeout/test_before_acquire |
| NEW-8 | ErrorCategory 分类错误（3 个） | 🟠 HIGH | 修正 TransactionFailed/ParseFailed/CircuitBreakerOpen |
| NEW-9 | cargo-audit 集成 | 🟠 HIGH | 至少本地开发流程集成 |
| NEW-10 | 测试 feature gate 编译失败 | 🔴 CRITICAL | 3 个测试文件需 cfg gate |
| NEW-11 | Action↔Router 循环依赖 | 🟠 HIGH | 提取 Middleware trait 到独立模块 |
| NEW-12 | #[non_exhaustive] 缺失（6 个枚举） | 🟠 HIGH | Condition/SqlValue/FieldType 等 |
| NEW-13 | QueryBuilder 缺少索引提示支持 | 🟠 HIGH | force_index/use_index API |
| NEW-14 | 敏感信息 Debug 泄露（5 个结构体） | 🟡 MEDIUM | LoginInput/TokenPairResponse 等手动实现 Debug |
| NEW-15 | validator feature 关闭时静默降级无日志 | 🟡 MEDIUM | Email/Phone 降级打 warn |
| NEW-16 | RedisConfig 无 Builder 模式 | 🟠 HIGH | 添加 with_* 链式 setter |
| NEW-17 | paginate() SQL 层未应用 LIMIT | 🟡 MEDIUM | 调用 select() 前写回 page/page_size |
| NEW-18 | page_size 无上限校验 | 🟡 MEDIUM | 增加上限（默认 1000） |
| NEW-19 | FieldPermissions Vec→HashSet | 🟡 MEDIUM | O(n) → O(1) |
| NEW-20 | sum/avg/min/max/value 聚合方法 SQL 注入 | 🔴 CRITICAL | field 参数裸拼 SQL 无转义（MySQL+PG 两侧） |
| NEW-21 | PG RETURNING 子句列名注入 | 🔴 CRITICAL | self.returning 用户可控且直接 format!() 拼入 SQL |
| NEW-22 | parse_token_unsafe 公开无验证 API | 🟠 HIGH | 标记 deprecated 或降为 pub(crate) |
| NEW-23 | PG 镜像标识符转义缺失（6 项） | 🟠 HIGH | PG condition/build_select/order_by/group_by/joins 与 MySQL 同等修复 |
| NEW-24 | 认证中间件启动期检查缺失 | 🟡 MEDIUM | 无编译期/启动期强制要求注册认证中间件 |
| NEW-25 | ActionContext::with_user() 注入绕过 | 🟡 MEDIUM | 文档标明仅供内部使用；考虑 pub(crate) |
| NEW-26 | GetAction/SelectAction unwrap_or 静默降级 | 🟡 MEDIUM | 替换为 ok_or_else 返回 Unauthorized |
| NEW-27 | GlobalDatabase 绕过 TableQuery 权限层 | 🟡 MEDIUM | AGENTS.md 增加安全说明 |
| NEW-28 | SQL 绑定 String/Bytes/JSON 双重 clone | 🟠 HIGH | 系统性性能热点，每条查询每列多一次堆分配 |
| NEW-29 | build_having 低效借用路径 | 🟡 MEDIUM | 改用 condition_to_sql_owned 与 build_where 一致 |
| NEW-30 | RedisTransaction 重试复用死连接 | 🟡 MEDIUM | pool.get() 移入重试循环 |
| NEW-31 | graceful_shutdown 被 mysql feature 绑死 | 🟡 MEDIUM | 提取 Redis 关闭逻辑，移除 feature gate 依赖 |
| NEW-32 | #[action(public)] 无护栏 | 🟢 LOW | clippy lint 对非认证 Action 标 public 发出警告 |
| NEW-33 | JWT 无密钥轮换机制 | 🟢 LOW | 增加 kid 支持和多密钥索引 |
| NEW-34 | TableAction 无角色级 schema 过滤 | 🟢 LOW | 按 can_read 权限过滤输出字段 |
