//! 敏感 Action 的短期 step-up challenge/proof。
//!
//! Step-up 使用独立 HMAC 密钥域，证明绑定已认证主体、Action 全限定引用和资源指纹。
//! 前端确认框不能替代本模块；业务必须通过 [`CredentialVerifier`] 重新校验凭据后，
//! 才能调用 [`StepUpManager::complete_challenge`]。

use super::auth::{CredentialVerifier, LoginInput};
use super::{ActionContext, ApiResponse};
use crate::definition::ActionRef;
use crate::error::BaseError;
use crate::router::{Middleware, Next};
use async_trait::async_trait;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// 默认 challenge 有效期：2 分钟。
pub const DEFAULT_STEP_UP_CHALLENGE_TTL: Duration = Duration::from_secs(120);
/// 默认 proof 有效期：5 分钟。
pub const DEFAULT_STEP_UP_PROOF_TTL: Duration = Duration::from_secs(300);
const MAX_STEP_UP_CHALLENGE_TTL: Duration = Duration::from_secs(300);
const MAX_STEP_UP_PROOF_TTL: Duration = Duration::from_secs(600);
const MIN_STEP_UP_SECRET_BYTES: usize = 32;

/// 敏感 Action 提交 step-up proof 的固定请求头。
pub const STEP_UP_PROOF_HEADER: &str = "x-step-up-proof";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StepUpTokenKind {
    Challenge,
    Proof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StepUpClaims {
    iss: String,
    sub: String,
    aud: String,
    exp: u64,
    iat: u64,
    jti: String,
    kind: StepUpTokenKind,
    action: String,
    resource_hash: String,
    challenge_jti: Option<String>,
}

/// 等待业务重新校验凭据的短期 challenge。
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct StepUpChallenge {
    /// 必须原样提交给重认证 Action 的签名 challenge。
    pub challenge: String,
    /// 从签发时刻起的有效秒数。
    pub expires_in: u64,
}

impl fmt::Debug for StepUpChallenge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StepUpChallenge")
            .field(
                "challenge",
                &format_args!("[REDACTED:{}]", self.challenge.len()),
            )
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// 凭据重认证成功后签发的短期 proof。
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct StepUpProof {
    /// 提交给敏感 Action 的签名 proof。
    pub proof: String,
    /// 从重认证成功时刻起的有效秒数。
    pub expires_in: u64,
}

impl fmt::Debug for StepUpProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StepUpProof")
            .field("proof", &format_args!("[REDACTED:{}]", self.proof.len()))
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// 已验证 proof 的服务端信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepUpVerification {
    /// 绑定的认证主体。
    pub subject: String,
    /// 绑定的全限定 Action 引用。
    pub action: String,
    /// 资源标识的 SHA-256 指纹；原始资源不会写入 Token。
    pub resource_hash: String,
    /// 完成重认证的 Unix 时间戳。
    pub authenticated_at: u64,
    /// proof 过期 Unix 时间戳。
    pub expires_at: u64,
    /// proof 的唯一 ID，供审计关联。
    pub proof_id: String,
}

/// 从当前可信请求状态解析 proof 必须绑定的稳定资源标识。
///
/// 实现者可以读取路径参数等客户端候选，但必须结合服务端事实完成规范化与授权相关
/// 校验，返回例如 `org_user:42` 的稳定标识。不得把客户端自报的“已验证资源”直接
/// 原样返回。解析失败必须 fail-closed。
#[async_trait]
pub trait StepUpResourceResolver: Send + Sync + 'static {
    /// 解析当前请求实际操作的稳定资源标识。
    async fn resolve(&self, context: &ActionContext) -> Result<String, BaseError>;
}

/// 为一个确定 Action 强制执行短期重认证的中间件。
///
/// 中间件必须注册在身份认证中间件之后。它只拦截构造时绑定的 Action；当前实际
/// Action 身份由 `Registry` 在派发边界覆盖注入，不能由客户端声明。缺少 proof 时
/// 返回携带短期 challenge 的 [`BaseError::StepUpRequired`]；存在但无效的 proof
/// 直接拒绝，不降级为新 challenge。
pub struct StepUpMiddleware<R> {
    manager: Arc<StepUpManager>,
    action: ActionRef,
    resolver: R,
}

impl<R> StepUpMiddleware<R>
where
    R: StepUpResourceResolver,
{
    /// 绑定管理器、目标 Action 与服务端资源解析器。
    pub fn new(manager: Arc<StepUpManager>, action: ActionRef, resolver: R) -> Self {
        Self {
            manager,
            action,
            resolver,
        }
    }
}

#[async_trait]
impl<R> Middleware for StepUpMiddleware<R>
where
    R: StepUpResourceResolver,
{
    fn target_action(&self) -> Option<&ActionRef> {
        Some(&self.action)
    }

    async fn handle(
        &self,
        context: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        let (module, action) = context
            .dispatch_target()
            .ok_or_else(|| BaseError::ConfigError("step-up 中间件缺少可信派发目标".to_string()))?;
        if module != self.action.module().as_str() || action != self.action.action().as_str() {
            return next.run(context).await;
        }

        let subject = context
            .authenticated_user()
            .ok_or_else(|| BaseError::Unauthorized("step-up 需要已认证用户".to_string()))?
            .id
            .to_string();
        let resource = self.resolver.resolve(&context).await?;
        if resource.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "step-up 资源解析器返回了空标识".to_string(),
            ));
        }

        match context.request.get_header(STEP_UP_PROOF_HEADER) {
            Some(proof) => {
                self.manager
                    .verify_proof(proof, &subject, &self.action, &resource)?;
                next.run(context).await
            }
            None => Err(BaseError::StepUpRequired(self.manager.issue_challenge(
                subject,
                &self.action,
                &resource,
            )?)),
        }
    }
}

/// 独立签名域的 step-up challenge/proof 管理器。
pub struct StepUpManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    header: Header,
    validation: Validation,
    issuer: String,
    audience: String,
    challenge_ttl: u64,
    proof_ttl: u64,
}

impl StepUpManager {
    /// 使用 HS256 和安全默认短时限创建管理器。
    ///
    /// `secret` 必须至少 32 字节，且应与 access/refresh token 使用不同密钥。
    pub fn new(
        secret: impl AsRef<[u8]>,
        issuer: impl Into<String>,
        audience: impl Into<String>,
    ) -> Result<Self, BaseError> {
        let secret = secret.as_ref();
        if secret.len() < MIN_STEP_UP_SECRET_BYTES {
            return Err(BaseError::ConfigError(
                "step-up HMAC 密钥至少需要 32 字节".to_string(),
            ));
        }
        let issuer = issuer.into();
        let audience = audience.into();
        if issuer.trim().is_empty() || audience.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "step-up issuer/audience 不能为空".to_string(),
            ));
        }
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("step-up+jwt".to_string());
        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            header,
            validation: step_up_validation(&issuer, &audience),
            issuer,
            audience,
            challenge_ttl: DEFAULT_STEP_UP_CHALLENGE_TTL.as_secs(),
            proof_ttl: DEFAULT_STEP_UP_PROOF_TTL.as_secs(),
        })
    }

    /// 覆盖 challenge/proof 时限；分别硬限制为 5/10 分钟。
    pub fn with_ttls(
        mut self,
        challenge_ttl: Duration,
        proof_ttl: Duration,
    ) -> Result<Self, BaseError> {
        validate_ttl("challenge", challenge_ttl, MAX_STEP_UP_CHALLENGE_TTL)?;
        validate_ttl("proof", proof_ttl, MAX_STEP_UP_PROOF_TTL)?;
        self.challenge_ttl = challenge_ttl.as_secs();
        self.proof_ttl = proof_ttl.as_secs();
        Ok(self)
    }

    /// 为已认证主体和确定的 Action/资源签发 challenge。
    pub fn issue_challenge(
        &self,
        subject: impl Into<String>,
        action: &ActionRef,
        resource: &str,
    ) -> Result<StepUpChallenge, BaseError> {
        self.issue_challenge_at(
            subject.into(),
            action.to_string(),
            resource,
            unix_timestamp()?,
        )
    }

    /// 重新校验凭据并把 challenge 升级为 proof。
    ///
    /// `CredentialVerifier` 返回的 subject 必须与 challenge 完全一致；先解析 challenge，
    /// 再调用业务 verifier，避免用无效 Token 触发昂贵的密码哈希。
    pub async fn complete_challenge<V>(
        &self,
        context: &ActionContext,
        verifier: &V,
        credentials: &LoginInput,
        challenge: &str,
    ) -> Result<StepUpProof, BaseError>
    where
        V: CredentialVerifier,
    {
        let challenge = self.decode_kind(challenge, StepUpTokenKind::Challenge)?;
        let verified = verifier.verify(context, credentials).await?;
        if verified.subject != challenge.sub {
            return Err(invalid_step_up("重认证主体与 challenge 不一致"));
        }
        self.issue_proof_at(challenge, unix_timestamp()?)
    }

    /// 校验 proof 与当前主体、Action 和资源是否完全一致。
    pub fn verify_proof(
        &self,
        proof: &str,
        subject: &str,
        action: &ActionRef,
        resource: &str,
    ) -> Result<StepUpVerification, BaseError> {
        let claims = self.decode_kind(proof, StepUpTokenKind::Proof)?;
        let expected_action = action.to_string();
        let expected_resource = resource_fingerprint(resource);
        if claims.sub != subject
            || claims.action != expected_action
            || claims.resource_hash != expected_resource
            || claims.challenge_jti.is_none()
        {
            return Err(invalid_step_up("proof 绑定目标不一致"));
        }
        Ok(StepUpVerification {
            subject: claims.sub,
            action: claims.action,
            resource_hash: claims.resource_hash,
            authenticated_at: claims.iat,
            expires_at: claims.exp,
            proof_id: claims.jti,
        })
    }

    fn issue_challenge_at(
        &self,
        subject: String,
        action: String,
        resource: &str,
        now: u64,
    ) -> Result<StepUpChallenge, BaseError> {
        if subject.trim().is_empty() || action.trim().is_empty() || resource.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "step-up subject/action/resource 不能为空".to_string(),
            ));
        }
        let claims = StepUpClaims {
            iss: self.issuer.clone(),
            sub: subject,
            aud: self.audience.clone(),
            exp: now.checked_add(self.challenge_ttl).ok_or_else(|| {
                BaseError::ConfigError("step-up challenge 过期时间溢出".to_string())
            })?,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            kind: StepUpTokenKind::Challenge,
            action,
            resource_hash: resource_fingerprint(resource),
            challenge_jti: None,
        };
        Ok(StepUpChallenge {
            challenge: self.encode(&claims)?,
            expires_in: self.challenge_ttl,
        })
    }

    fn issue_proof_at(&self, challenge: StepUpClaims, now: u64) -> Result<StepUpProof, BaseError> {
        let claims = StepUpClaims {
            iss: self.issuer.clone(),
            sub: challenge.sub,
            aud: self.audience.clone(),
            exp: now
                .checked_add(self.proof_ttl)
                .ok_or_else(|| BaseError::ConfigError("step-up proof 过期时间溢出".to_string()))?,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            kind: StepUpTokenKind::Proof,
            action: challenge.action,
            resource_hash: challenge.resource_hash,
            challenge_jti: Some(challenge.jti),
        };
        Ok(StepUpProof {
            proof: self.encode(&claims)?,
            expires_in: self.proof_ttl,
        })
    }

    fn encode(&self, claims: &StepUpClaims) -> Result<String, BaseError> {
        encode(&self.header, claims, &self.encoding_key)
            .map_err(|_| BaseError::ConfigError("step-up Token 签发失败".to_string()))
    }

    fn decode_kind(
        &self,
        token: &str,
        expected: StepUpTokenKind,
    ) -> Result<StepUpClaims, BaseError> {
        let claims = decode::<StepUpClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|_| invalid_step_up("Token 无效或已过期"))?
            .claims;
        if claims.kind != expected {
            return Err(invalid_step_up("Token 类型不匹配"));
        }
        Ok(claims)
    }
}

fn step_up_validation(issuer: &str, audience: &str) -> Validation {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.algorithms = vec![Algorithm::HS256];
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.required_spec_claims = ["exp", "iss", "aud", "sub"]
        .into_iter()
        .map(str::to_string)
        .collect::<HashSet<_>>();
    validation.leeway = 0;
    validation
}

fn validate_ttl(name: &str, value: Duration, maximum: Duration) -> Result<(), BaseError> {
    if value.is_zero() || value > maximum {
        return Err(BaseError::ConfigError(format!(
            "step-up {name} TTL 必须在 1..={} 秒",
            maximum.as_secs()
        )));
    }
    Ok(())
}

fn unix_timestamp() -> Result<u64, BaseError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| BaseError::ConfigError(format!("系统时钟异常: {error}")))
}

fn resource_fingerprint(resource: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(resource.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn invalid_step_up(reason: &str) -> BaseError {
    BaseError::Unauthorized(format!("step-up 验证失败: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::super::auth::VerifiedSubject;
    use super::*;
    use crate::action::{Request, TypedHandler, User};
    use crate::definition::{
        ActionName, ActionSpec, AddonName, AddonSpec, AppBuilder, HttpMethod, ModuleName,
        ModuleSpec, RouteSpec,
    };
    use crate::tools::ToolsBuilder;
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use yang_base_derive::Action;

    const SECRET: &str = "step-up-test-secret-must-be-at-least-32-bytes";

    struct TestVerifier;

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct EmptyInput {}

    #[derive(Debug, Serialize, JsonSchema)]
    struct ProbeOutput {
        operation: &'static str,
    }

    #[derive(Action)]
    #[action(name = "delete", display_name = "删除用户")]
    struct DeleteProbe;

    #[async_trait]
    impl TypedHandler for DeleteProbe {
        type Input = EmptyInput;
        type Output = ProbeOutput;

        async fn handle(
            &self,
            _context: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(ProbeOutput {
                operation: "delete",
            })
        }
    }

    #[derive(Action)]
    #[action(name = "read", display_name = "读取用户")]
    struct ReadProbe;

    #[async_trait]
    impl TypedHandler for ReadProbe {
        type Input = EmptyInput;
        type Output = ProbeOutput;

        async fn handle(
            &self,
            _context: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(ProbeOutput { operation: "read" })
        }
    }

    struct PathResourceResolver;

    #[async_trait]
    impl StepUpResourceResolver for PathResourceResolver {
        async fn resolve(&self, context: &ActionContext) -> Result<String, BaseError> {
            let raw = context
                .request
                .get_path_param("id")
                .ok_or_else(|| BaseError::ParamMissing("id".to_string()))?;
            let id = raw.parse::<u64>().map_err(|_| {
                BaseError::ParamInvalid("id".to_string(), "必须是正整数".to_string())
            })?;
            if id == 0 {
                return Err(BaseError::ParamInvalid(
                    "id".to_string(),
                    "必须是正整数".to_string(),
                ));
            }
            Ok(format!("org_user:{id}"))
        }
    }

    #[async_trait]
    impl CredentialVerifier for TestVerifier {
        async fn verify(
            &self,
            _context: &ActionContext,
            input: &LoginInput,
        ) -> Result<VerifiedSubject, BaseError> {
            if input.password == "correct-password" {
                Ok(VerifiedSubject::new(input.username.clone()))
            } else {
                Err(BaseError::Unauthorized("凭据错误".to_string()))
            }
        }
    }

    fn manager() -> StepUpManager {
        StepUpManager::new(SECRET, "yang-test", "yang-sensitive-actions")
            .expect("测试 step-up manager 应构建成功")
    }

    fn action_ref(name: &str) -> ActionRef {
        ActionRef::new(
            ModuleName::new("org.user").expect("测试 Module 名称应有效"),
            ActionName::new(name).expect("测试 Action 名称应有效"),
        )
    }

    fn context() -> ActionContext {
        ActionContext::new(
            Request::new(serde_json::json!({})),
            Arc::new(ToolsBuilder::new().build().expect("测试 Tools 应构建成功")),
        )
    }

    fn test_app(manager: Arc<StepUpManager>) -> crate::definition::BuiltApp {
        test_app_with_target(manager, action_ref("delete")).expect("step-up 测试应用应构建成功")
    }

    fn test_app_with_target(
        manager: Arc<StepUpManager>,
        target: ActionRef,
    ) -> Result<crate::definition::BuiltApp, crate::definition::BuildError> {
        let module_name = ModuleName::new("org.user").expect("测试 Module 名称应有效");
        let module = ModuleSpec::new(module_name)
            .middleware(StepUpMiddleware::new(manager, target, PathResourceResolver))
            .action(
                ActionSpec::new(
                    ActionName::new("delete").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Delete,
                        "/api/v1/org/users/{id}",
                        "org.user.delete",
                    ),
                ),
                DeleteProbe,
            )
            .action(
                ActionSpec::new(
                    ActionName::new("read").expect("测试 Action 名称应有效"),
                    RouteSpec::new(HttpMethod::Get, "/api/v1/org/users/{id}", "org.user.read"),
                ),
                ReadProbe,
            );
        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(Arc::new(
                ToolsBuilder::new().build().expect("测试 Tools 应构建成功"),
            ))
    }

    async fn dispatch_as(
        app: &crate::definition::BuiltApp,
        action: &str,
        request: Request,
        user: Option<User>,
    ) -> Result<ApiResponse, BaseError> {
        let handle = app
            .registry()
            .resolve(&action_ref(action))
            .expect("测试 Action 应已注册");
        let mut context = app.context(request).with_module("client.forged");
        if let Some(user) = user {
            context = context.with_user(user);
        }
        app.dispatch_context(handle, context).await
    }

    fn credentials(subject: &str, password: &str) -> LoginInput {
        LoginInput {
            username: subject.to_string(),
            password: password.to_string(),
            extra: serde_json::Value::Null,
        }
    }

    #[tokio::test]
    async fn challenge_requires_reauthentication_and_proof_binds_every_dimension() {
        let manager = manager();
        let action = action_ref("delete");
        let resource = "org_user:42";
        let challenge = manager
            .issue_challenge("7", &action, resource)
            .expect("challenge 应签发成功");
        assert_eq!(
            challenge.expires_in,
            DEFAULT_STEP_UP_CHALLENGE_TTL.as_secs()
        );
        assert!(
            !challenge.challenge.contains(resource),
            "原始资源标识不得写入 Token"
        );

        let bad_credentials = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "wrong-password"),
                &challenge.challenge,
            )
            .await;
        assert!(matches!(bad_credentials, Err(BaseError::Unauthorized(_))));

        let proof = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .expect("正确凭据应完成 challenge");
        assert_eq!(proof.expires_in, DEFAULT_STEP_UP_PROOF_TTL.as_secs());
        let verified = manager
            .verify_proof(&proof.proof, "7", &action, resource)
            .expect("proof 全部绑定维度一致时应通过");
        assert_eq!(verified.subject, "7");
        assert_eq!(verified.action, "org.user.delete");
        assert_eq!(verified.resource_hash.len(), 64);
        assert!(verified.authenticated_at < verified.expires_at);

        for mismatch in [
            manager.verify_proof(&proof.proof, "8", &action, resource),
            manager.verify_proof(&proof.proof, "7", &action_ref("disable"), resource),
            manager.verify_proof(&proof.proof, "7", &action, "org_user:43"),
        ] {
            assert!(
                matches!(mismatch, Err(BaseError::Unauthorized(_))),
                "跨主体/Action/资源复用必须失败"
            );
        }
    }

    #[tokio::test]
    async fn middleware_challenges_then_accepts_only_exact_subject_action_and_resource() {
        let manager = Arc::new(manager());
        let app = test_app(Arc::clone(&manager));
        let request = || Request::new(serde_json::json!({})).path_param("id", "42");
        let user = User::new(7, "member");

        let missing = dispatch_as(&app, "delete", request(), Some(user.clone())).await;
        let challenge = match missing {
            Err(BaseError::StepUpRequired(challenge)) => challenge,
            other => panic!("缺少 proof 应返回 step-up challenge，实际为: {other:?}"),
        };
        assert_eq!(BaseError::StepUpRequired(challenge.clone()).code(), 700010);

        let proof = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .expect("正确凭据应换取 proof");
        let response = dispatch_as(
            &app,
            "delete",
            request().header(STEP_UP_PROOF_HEADER, &proof.proof),
            Some(user),
        )
        .await
        .expect("完全绑定的 proof 应允许敏感 Action");
        assert_eq!(response.data.expect("应返回 data")["operation"], "delete");
    }

    #[tokio::test]
    async fn middleware_fails_closed_without_downgrading_invalid_proofs() {
        let manager = Arc::new(manager());
        let app = test_app(Arc::clone(&manager));
        let user = User::new(7, "member");
        let challenge = manager
            .issue_challenge("7", &action_ref("delete"), "org_user:42")
            .expect("challenge 应签发成功");
        let proof = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .expect("proof 应签发成功");

        for request in [
            Request::new(serde_json::json!({}))
                .path_param("id", "42")
                .header(STEP_UP_PROOF_HEADER, ""),
            Request::new(serde_json::json!({}))
                .path_param("id", "43")
                .header(STEP_UP_PROOF_HEADER, &proof.proof),
        ] {
            let result = dispatch_as(&app, "delete", request, Some(user.clone())).await;
            assert!(
                matches!(result, Err(BaseError::Unauthorized(_))),
                "空 proof 与跨资源 proof 必须直接拒绝，不能降级为 challenge: {result:?}"
            );
        }

        let unauthenticated = dispatch_as(
            &app,
            "delete",
            Request::new(serde_json::json!({})).path_param("id", "42"),
            None,
        )
        .await;
        assert!(matches!(unauthenticated, Err(BaseError::Unauthorized(_))));

        for invalid_id in [None, Some("0"), Some("not-a-number")] {
            let mut request = Request::new(serde_json::json!({}));
            if let Some(id) = invalid_id {
                request = request.path_param("id", id);
            }
            let result = dispatch_as(&app, "delete", request, Some(user.clone())).await;
            assert!(
                matches!(
                    result,
                    Err(BaseError::ParamMissing(_)) | Err(BaseError::ParamInvalid(_, _))
                ),
                "资源解析必须 fail-closed: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn middleware_uses_registry_target_and_skips_unrelated_actions() {
        let manager = Arc::new(manager());
        let app = test_app(manager);
        let user = User::new(7, "member");

        let response = dispatch_as(
            &app,
            "read",
            Request::new(serde_json::json!({})).path_param("id", "42"),
            Some(user),
        )
        .await
        .expect("非目标 Action 不应被 step-up 拦截");
        assert_eq!(response.data.expect("应返回 data")["operation"], "read");
    }

    #[test]
    fn app_rejects_missing_or_cross_module_step_up_targets() {
        let manager = Arc::new(manager());
        let missing = test_app_with_target(Arc::clone(&manager), action_ref("destroy"));
        assert!(matches!(
            missing,
            Err(crate::definition::BuildError::InvalidReference {
                kind: "Middleware Action",
                reference,
            }) if reference == "org.user.destroy"
        ));

        let cross_module = ActionRef::new(
            ModuleName::new("other.user").expect("测试 Module 名称应有效"),
            ActionName::new("delete").expect("测试 Action 名称应有效"),
        );
        let cross = test_app_with_target(manager, cross_module);
        assert!(matches!(
            cross,
            Err(crate::definition::BuildError::InvalidReference {
                kind: "Middleware Action",
                reference,
            }) if reference == "other.user.delete"
        ));
    }

    #[tokio::test]
    async fn challenge_cannot_be_completed_as_another_subject_or_used_as_proof() {
        let manager = manager();
        let action = action_ref("delete");
        let challenge = manager
            .issue_challenge("7", &action, "org_user:42")
            .expect("challenge 应签发成功");

        let cross_subject = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("8", "correct-password"),
                &challenge.challenge,
            )
            .await;
        assert!(matches!(cross_subject, Err(BaseError::Unauthorized(_))));
        assert!(matches!(
            manager.verify_proof(&challenge.challenge, "7", &action, "org_user:42"),
            Err(BaseError::Unauthorized(_))
        ));
    }

    #[tokio::test]
    async fn tampered_expired_and_wrong_kind_tokens_are_rejected() {
        let manager = manager();
        let action = action_ref("delete");
        let resource = "org_user:42";
        let challenge = manager
            .issue_challenge("7", &action, resource)
            .expect("challenge 应签发成功");
        let proof = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .expect("proof 应签发成功");

        let tampered = tamper_signature(&proof.proof);
        assert!(matches!(
            manager.verify_proof(&tampered, "7", &action, resource),
            Err(BaseError::Unauthorized(_))
        ));
        let valid_claims = manager
            .decode_kind(&proof.proof, StepUpTokenKind::Proof)
            .expect("测试 proof 应可解析");
        let wrong_algorithm = jsonwebtoken::encode(
            &Header::new(Algorithm::HS512),
            &valid_claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .expect("错误算法测试 Token 应可签发");
        assert!(matches!(
            manager.verify_proof(&wrong_algorithm, "7", &action, resource),
            Err(BaseError::Unauthorized(_))
        ));
        let mut wrong_domain_claims = valid_claims;
        wrong_domain_claims.aud = "another-audience".to_string();
        let wrong_domain = manager
            .encode(&wrong_domain_claims)
            .expect("错误域测试 Token 应可签发");
        assert!(matches!(
            manager.verify_proof(&wrong_domain, "7", &action, resource),
            Err(BaseError::Unauthorized(_))
        ));
        let proof_as_challenge = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &proof.proof,
            )
            .await;
        assert!(matches!(
            proof_as_challenge,
            Err(BaseError::Unauthorized(_))
        ));

        let expired_challenge = manager
            .encode(&claims(
                &manager,
                StepUpTokenKind::Challenge,
                1,
                None,
                &action,
                resource,
            ))
            .expect("过期 challenge 测试 Token 应编码成功");
        let expired = manager
            .complete_challenge(
                &context(),
                &TestVerifier,
                &credentials("7", "correct-password"),
                &expired_challenge,
            )
            .await;
        assert!(matches!(expired, Err(BaseError::Unauthorized(_))));

        let expired_proof = manager
            .encode(&claims(
                &manager,
                StepUpTokenKind::Proof,
                1,
                Some("challenge-id".to_string()),
                &action,
                resource,
            ))
            .expect("过期 proof 测试 Token 应编码成功");
        assert!(matches!(
            manager.verify_proof(&expired_proof, "7", &action, resource),
            Err(BaseError::Unauthorized(_))
        ));
    }

    #[test]
    fn manager_rejects_weak_secrets_empty_domains_and_long_ttls() {
        assert!(matches!(
            StepUpManager::new("short", "issuer", "audience"),
            Err(BaseError::ConfigError(_))
        ));
        assert!(matches!(
            StepUpManager::new(SECRET, "", "audience"),
            Err(BaseError::ConfigError(_))
        ));
        assert!(matches!(
            manager().with_ttls(Duration::ZERO, DEFAULT_STEP_UP_PROOF_TTL),
            Err(BaseError::ConfigError(_))
        ));
        assert!(matches!(
            manager().with_ttls(
                DEFAULT_STEP_UP_CHALLENGE_TTL,
                MAX_STEP_UP_PROOF_TTL + Duration::from_secs(1),
            ),
            Err(BaseError::ConfigError(_))
        ));
        assert!(matches!(
            manager().issue_challenge("7", &action_ref("delete"), ""),
            Err(BaseError::ConfigError(_))
        ));
    }

    fn claims(
        manager: &StepUpManager,
        kind: StepUpTokenKind,
        exp: u64,
        challenge_jti: Option<String>,
        action: &ActionRef,
        resource: &str,
    ) -> StepUpClaims {
        StepUpClaims {
            iss: manager.issuer.clone(),
            sub: "7".to_string(),
            aud: manager.audience.clone(),
            exp,
            iat: 0,
            jti: "expired-token-id".to_string(),
            kind,
            action: action.to_string(),
            resource_hash: resource_fingerprint(resource),
            challenge_jti,
        }
    }

    fn tamper_signature(token: &str) -> String {
        let mut bytes = token.as_bytes().to_vec();
        let signature = bytes
            .iter()
            .rposition(|byte| *byte == b'.')
            .map(|index| index + 1)
            .expect("JWT 应包含签名段");
        bytes[signature] = if bytes[signature] == b'A' { b'B' } else { b'A' };
        String::from_utf8(bytes).expect("JWT 应是 UTF-8 ASCII")
    }
}
