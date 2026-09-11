# ImYangJiaJun/lib_yang 与 yang-system 代码成熟度与安全审查

审查基线：2026-09-10
仓库：
- https://github.com/ImYangJiaJun/lib_yang
- https://github.com/ImYangJiaJun/yang-system

## 结论摘要

- `lib_yang`：高级个人基础设施/框架雏形，安全与工程设计明显超过一般个人 Rust 项目；但仍不应当被视为“成熟通用生产框架”。最大问题不是缺少功能，而是安全边界存在少量可绕过的公共 escape hatch，尤其是 `yang-db` 原生 SQL、部分裸标识符/表达式 API、公开的 `parse_token_unsafe`，以及发布/依赖审计策略存在不一致。
- `yang-system`：已经具备“安全基线参考应用”的骨架：认证、MFA、Refresh Rotation、授权版本、Outbox、Step-up、审计、配置 fail-closed、非 root 容器、集成测试和契约门禁都已落地；但业务域仍很窄，部署/CI/运营成熟度还没达到大型商业系统标准。

## 综合评分

| 维度 | lib_yang | yang-system |
|---|---:|---:|
| 架构设计 | 8.0/10 | 8.2/10 |
| 安全模型 | 7.2/10 | 8.0/10 |
| 测试与验证 | 8.0/10 | 7.8/10 |
| API 安全性 | 6.5/10 | 8.0/10 |
| 可运维性 | 7.3/10 | 8.0/10 |
| 发布工程 | 6.8/10 | 7.2/10 |
| 产品/业务成熟度 | 5.5/10 | 5.5/10 |
| 总体成熟度 | **7.2/10** | **7.8/10** |

评分不是“能不能上线”的二元结论，而是对“设计、验证、边界、可维护性”的综合判断。

## 关键优点

### lib_yang

1. 工作区已经按职责拆成 `yang-db`、`yang-base`、`yang-base-derive`、`yang-runtime`、`yang-pcg`，并把 `yang-system` 独立为参考应用。
2. `Cargo.toml` 已统一依赖版本、MSRV、lint 和许可证元数据；CI 有 fmt、clippy、feature matrix、MSRV、真实 MySQL/PostgreSQL/Redis 集成测试。
3. `cargo-deny` 已纳入 CI，并对漏洞、许可证、未知 registry/git source 设置了政策门禁。
4. JWT 验证默认白名单算法，同时验证 issuer/audience/exp；keyring 具备稳定 `kid`、密钥数量限制和最小 HMAC 密钥长度。
5. TableQuery 已形成比较完整的 server-side 安全边界：字段权限、排序/筛选权限、条件树递归深度、LIKE 长度、IN 列表数量、字段类型和值类型校验、空逻辑组拒绝。
6. `ActionContext` 将认证用户和系统 capability 收口在内部路径，普通租户与系统租户用互斥类型表达，明显优于到处传 `Option<User>` / `bool is_admin` 的常见设计。

### yang-system

1. 认证体系已经覆盖密码、刷新令牌、会话撤销、Refresh Rotation、Step-up、TOTP、恢复码、邮箱备用第二因子。
2. 登录路径有等时密码校验和规范化限流，显式防止用户名枚举与多维度限流绕过。
3. 浏览器 Refresh Token 使用 Host-only + HttpOnly + SameSite=Strict Cookie，并对变更类请求增加 Same-Origin 检查。
4. `config` 使用 `deny_unknown_fields`、secret 单独来源、Debug 脱敏、敏感配置启动校验；容器默认非 root。
5. 生产启动顺序已经考虑 Schema 预检、readiness、Outbox worker、Telemetry flush、共享关闭预算，这说明已经从“能运行”进入“可运维”思维。

## 主要安全问题

### P0：收紧数据库层 public escape hatch

`yang-db` 仍有公开的裸 SQL `query(&str)` / `execute(&str)`，虽然已经 deprecated；同时历史复核确认 `create_table`/`init(sql_script)` 一类原生 SQL 能力、部分 `order_by/group_by/join/value` 和条件标识符回退仍存在不同程度的原始字符串入口。

这类 API 不一定意味着框架存在直接 SQL 注入漏洞，因为值绑定和 `yang-base` 类型层已有较强约束；真正的问题是：任何业务开发者只要走错一条 API 路径，就可以绕过这些安全保证。

建议：
- 新版本把裸 SQL API 降到 `pub(crate)` 或独立 `unsafe/raw-sql` feature。
- 将“标识符”“表达式”“已验证片段”设计成不同类型，而不是都用 `&str`。
- `JOIN ON` 之类表达式 API 要求显式 raw 标记，默认 API 不能接受任意字符串。
- `create_table`、脚本执行等初始化接口与 runtime query API 分离。

### P0：审计/发布策略统一

仓库的 `cargo-deny` 配置是 `ignore = []`，CI 运行 advisory 检查；而发布候选文档又记录了 `cargo audit --ignore` 对 `rsa 0.9.10` 和 `time 0.3.36` 的显式豁免。这意味着“CI 的依赖安全定义”和“release audit 的安全定义”不是同一份机器可验证契约。

建议：
- 统一为一个 `security/advisories.toml` 或同类单一事实源。
- 每一个豁免都记录：CVE/RUSTSEC、受影响路径、可达性分析、补偿控制、复审日期、退出条件。
- CI 的 cargo-deny / cargo-audit 都从同一来源读取。
- 对“不可修复但不可达”的漏洞使用独立隔离说明，禁止只写一句“安全”。

### P1：移除或隔离 `parse_token_unsafe`

它已经 deprecated 并带警告，但仍是 `pub` API，而且实际关闭了签名、exp、nbf、aud 校验。对基础库来说，这是典型的高危 footgun。

建议在下一个 breaking release：
- 移到 `debug` feature；
- 或完全改名为带明显危险语义的 `decode_unverified_for_debug`；
- 最好只暴露内部测试/调试模块，而不是主 API。

### P1：认证 API 需要继续做“类型化约束”

当前安全模型大量依赖代码审查与使用规范，例如 `ActionContext::with_user` 是 crate-private，这是正确方向；但一些“可信上下文”依然通过普通字符串和普通结构传递。

后续建议把以下概念进一步类型化：
- `AuthenticatedUser`
- `TenantCapability`
- `SystemTenantCapability`
- `VerifiedToken`
- `TrustedClientIp`
- `VerifiedOrigin`

目标不是增加类型数量，而是让“不安全值”无法自然流入安全决策代码。

### P1：GitHub Actions 供应链仍可继续加固

当前 workflow 已明确 `permissions: contents: read`，这是好的；但仍有 `actions/checkout@v4`、`dtolnay/rust-toolchain@master`、`Swatinem/rust-cache@v2` 等可变引用。GitHub 官方建议对第三方 action 使用完整 commit SHA 进行不可变固定。

建议：
- 所有第三方 action 改成 full-length SHA + 注释版本号。
- 对 workflow 建立 Dependabot/Renovate 依赖更新。
- 为 action allowlist / required pinned SHAs 建立 repository policy。

### P1：配置 secret 覆盖面做一次闭环检查

当前 secret provider 的设计已经存在，但新增的 `email.change`、`email.mfa`、TOTP 等配置域应确认是否全部进入“目录型 secret”安全路径，而不是只有环境变量/配置文件方式。

对于生产部署，建议所有真正的秘密都能走：
`secret file / external secret manager -> startup validation -> immutable Settings`
而不是要求人工把高价值 secret 写进普通 TOML。

### P2：yang-system 的“best effort”路径要做故障语义分级

例如登录成功后会话记录、成功事件记录、新设备邮件通知当前有 best-effort 行为。这个选择本身合理，但应明确分类：
- 安全事实：失败必须阻断；
- 业务增强：失败允许继续；
- 审计事实：失败应进入 durable retry/outbox，而不是只 warn。

尤其是安全事件/审计记录不能长时间只靠日志。

## 当前成熟度判断

### lib_yang

适合定位为：
> “个人长期维护的 Rust backend/security infrastructure，已进入 production-oriented engineering 阶段，但仍在进行 API 安全收口。”

不适合现在对外定位为：
> “成熟通用 Web Framework / 企业级基础框架”。

原因不是功能少，而是公共 API 仍允许调用方绕开部分安全策略。框架成熟度最终取决于“错误使用是否容易发生”。

### yang-system

适合定位为：
> “以 `lib_yang` 为基础的安全参考应用 / 内部平台基线。”

它已经足够作为你以后做账号中心、管理后台、插件系统等项目的母体，但还没有形成真正的多租户、插件生态、业务域模型和完整运营体系。

## 下一阶段建议

### Phase 0：先做安全收口，而不是继续堆功能

1. 关闭/隔离 `yang-db` raw SQL escape hatch。
2. 给所有标识符/表达式 API 做类型化边界。
3. 移除 public `parse_token_unsafe`。
4. 统一 cargo-deny/cargo-audit advisory policy。
5. 所有 GitHub Actions 改成 SHA pin。
6. 给安全豁免增加自动复审日期和退出条件。

### Phase 1：把安全设计变成自动验证

建立 adversarial integration suite，至少覆盖：
- SQL 注入变体；
- 越权查询/排序/筛选；
- 租户边界穿透；
- Refresh Token replay；
- revoked token race；
- Step-up proof replay；
- MFA brute force / bypass；
- Host/Origin/Forwarded spoofing；
- schema migration race；
- shutdown/readiness race。

测试不是“正常用例更多”，而是专门证明安全不变量不会被绕过。

### Phase 2：做真正的 release engineering

建议形成以下 release contract：
- stable + MSRV；
- all feature matrix；
- unit/doc/integration；
- dependency audit；
- cargo package verify；
- SBOM；
- Docker image digest；
- provenance/attestation；
- changelog + migration notes；
- breaking API review。

### Phase 3：再做平台化

等安全边界冻结后，再继续做：
- multi-tenant；
- addon/plugin SDK；
- OIDC/SSO；
- RBAC/ABAC；
- background jobs；
- object storage；
- rate-limit/queue abstraction；
- admin/audit console。

否则基础库会长期处于“边加功能边改安全边界”的高成本状态。

## 最重要的一条判断

你现在不缺“再实现一个功能”。真正需要做的是把 `lib_yang` 从“有很多安全设计”升级成“即使普通开发者用错 API，也很难破坏安全边界”。

这是从个人高级项目进入成熟工程最关键的一步。
