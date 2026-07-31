//! 敏感 Action 的短期 step-up challenge/proof。
//!
//! Step-up 使用独立 HMAC 密钥域，证明绑定已认证主体、Action 全限定引用和资源指纹。
//! 前端确认框不能替代本模块；业务必须通过 [`CredentialVerifier`] 重新校验凭据后，
//! 才能调用 [`StepUpManager::complete_challenge`]。

use super::auth::{CredentialVerifier, LoginInput};
use super::{ActionContext, ApiResponse};
use crate::definition::ActionRef;
use crate::error::BaseError;
use crate::router::{Middleware, MiddlewareRole, Next};
use async_trait::async_trait;
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use yang_base_derive::Action;

/// 默认 challenge 有效期：2 分钟。
pub const DEFAULT_STEP_UP_CHALLENGE_TTL: Duration = Duration::from_secs(120);
/// 默认 proof 有效期：5 分钟。
pub const DEFAULT_STEP_UP_PROOF_TTL: Duration = Duration::from_secs(300);
const MAX_STEP_UP_CHALLENGE_TTL: Duration = Duration::from_secs(300);
const MAX_STEP_UP_PROOF_TTL: Duration = Duration::from_secs(600);
const MIN_STEP_UP_SECRET_BYTES: usize = 32;
const MAX_STEP_UP_RETIRING_KEYS: usize = 8;

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

/// 完成 step-up challenge 的输入。
#[derive(Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepUpCompleteInput {
    /// 敏感 Action 返回的签名 challenge。
    pub challenge: String,
    /// 要重新校验的业务凭据。
    pub credentials: LoginInput,
}

/// 内置 step-up 重认证 Action。
///
/// Action 只负责编排签名 challenge 与业务 [`CredentialVerifier`]；限流、失败计数和
/// 锁定仍必须由 verifier 使用共享存储实现，避免框架猜测账号体系。
#[derive(Action)]
#[action(
    name = "step_up_complete",
    display_name = "完成敏感操作重认证",
    description = "重新校验凭据并把 step-up challenge 升级为一次性 proof",
    method = "POST",
    path = "/step-up/complete",
    public
)]
pub struct StepUpCompleteAction<V: CredentialVerifier> {
    manager: Arc<StepUpManager>,
    verifier: V,
}

impl<V: CredentialVerifier> StepUpCompleteAction<V> {
    /// 使用共享 step-up 管理器和业务凭据校验器创建 Action。
    pub fn new(manager: Arc<StepUpManager>, verifier: V) -> Self {
        Self { manager, verifier }
    }
}

#[async_trait]
impl<V: CredentialVerifier> super::TypedHandler for StepUpCompleteAction<V> {
    type Input = StepUpCompleteInput;
    type Output = StepUpProof;

    async fn handle(
        &self,
        context: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        self.manager
            .complete_challenge(
                &context,
                &self.verifier,
                &input.credentials,
                &input.challenge,
            )
            .await
    }
}

/// step-up proof 的一次性消费存储。
///
/// 实现必须以 proof ID 为键执行原子“首次写入成功”语义，并至少保留到 proof
/// 过期。返回 `false` 表示该 proof 已被消费。
#[async_trait]
pub trait StepUpProofStore: Send + Sync + 'static {
    /// 尝试消费一个已完成签名与绑定校验的 proof。
    async fn consume(&self, proof: &StepUpVerification) -> Result<bool, BaseError>;
}

/// 单进程一次性 proof 存储。
///
/// 适用于单实例服务和测试；多实例部署必须改用 [`RedisStepUpProofStore`]，否则
/// 不同进程无法共享已消费状态。
#[derive(Debug, Default)]
pub struct InMemoryStepUpProofStore {
    consumed: Mutex<HashMap<String, u64>>,
}

#[async_trait]
impl StepUpProofStore for InMemoryStepUpProofStore {
    async fn consume(&self, proof: &StepUpVerification) -> Result<bool, BaseError> {
        let now = unix_timestamp()?;
        let mut consumed = self.consumed.lock().map_err(|_| {
            BaseError::ConfigError("step-up proof 内存消费存储锁已损坏".to_string())
        })?;
        consumed.retain(|_, expires_at| *expires_at > now);
        if consumed.contains_key(&proof.proof_id) {
            return Ok(false);
        }
        consumed.insert(proof.proof_id.clone(), proof.expires_at);
        Ok(true)
    }
}

/// 基于 Redis `SET NX EX` 的多实例一次性 proof 存储。
#[derive(Clone)]
pub struct RedisStepUpProofStore {
    cache: yang_db::RedisClient,
    key_prefix: String,
}

impl RedisStepUpProofStore {
    /// 创建 Redis proof 存储。
    pub fn new(cache: yang_db::RedisClient) -> Self {
        Self {
            cache,
            key_prefix: "yang:step-up:proof-used:".to_string(),
        }
    }

    /// 覆盖 Redis key 前缀，便于应用隔离命名空间。
    pub fn with_key_prefix(mut self, key_prefix: impl Into<String>) -> Result<Self, BaseError> {
        let key_prefix = key_prefix.into();
        if key_prefix.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "step-up proof Redis key 前缀不能为空".to_string(),
            ));
        }
        self.key_prefix = key_prefix;
        Ok(self)
    }
}

#[async_trait]
impl StepUpProofStore for RedisStepUpProofStore {
    async fn consume(&self, proof: &StepUpVerification) -> Result<bool, BaseError> {
        let now = unix_timestamp()?;
        let ttl = proof.expires_at.saturating_sub(now).max(1);
        let ttl = i64::try_from(ttl)
            .map_err(|_| BaseError::ConfigError("step-up proof Redis TTL 超出范围".to_string()))?;
        self.cache
            .set_nx_ex(format!("{}{}", self.key_prefix, proof.proof_id), "1", ttl)
            .await
            .map_err(BaseError::from)
    }
}

/// 从当前可信请求状态解析 proof 必须绑定的稳定资源标识。
///
/// 实现者可以读取路径参数等客户端候选，但必须结合服务端事实完成规范化与授权相关
/// 校验，返回例如 `org_user:42` 的稳定标识。不得把客户端自报的“已验证资源”直接
/// 原样返回。解析失败必须 fail-closed。
///
/// # 收窄 proof 重放窗口
///
/// proof 在 TTL 内对同一资源标识可重放。资源标识越粗，重放面越大：仅绑定
/// `transfer:account:42` 意味着重认证一次即可在该账户上重复发起任意金额的转账。
/// 资源标识应把**操作参数指纹**纳入标识，例如金额、目标账户等关键参数的 hash
/// （`transfer:account:42:sha256(amount=100,to=account:7)`），使 proof 只覆盖一次
/// 确定的操作语义，任何参数变化都会使 proof 失效并触发新的 challenge。
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
///
/// # 约束范围（重要）
///
/// step-up 仅约束 `Registry::dispatch` 路径（HTTP 传输与显式 dispatch 均经此路径）。
/// `Registry::call` / `Plugins::api_run` 等内部调用**不经过中间件链**，敏感 Action
/// 被内部调用时不会触发 step-up（仅保留权限校验）。这是有意的语义划分：内部调用方
/// 是受信代码，如需重认证必须自行编排 `StepUpManager`。该语义由
/// `internal_call_bypasses_step_up_middleware_by_design` 测试锁定，改动前请先评审。
pub struct StepUpMiddleware<R> {
    manager: Arc<StepUpManager>,
    action: ActionRef,
    resolver: R,
    proof_store: Arc<dyn StepUpProofStore>,
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
            proof_store: Arc::new(InMemoryStepUpProofStore::default()),
        }
    }

    /// 覆盖一次性 proof 存储。
    ///
    /// 多实例部署应传入 [`RedisStepUpProofStore`]，保证所有实例共享消费状态。
    #[must_use]
    pub fn with_proof_store<S>(mut self, proof_store: S) -> Self
    where
        S: StepUpProofStore,
    {
        self.proof_store = Arc::new(proof_store);
        self
    }
}

#[async_trait]
impl<R> Middleware for StepUpMiddleware<R>
where
    R: StepUpResourceResolver,
{
    fn role(&self) -> MiddlewareRole {
        MiddlewareRole::StepUpProtection
    }

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
                let verification =
                    self.manager
                        .verify_proof(proof, &subject, &self.action, &resource)?;
                if !self.proof_store.consume(&verification).await? {
                    tracing::warn!(
                        proof_id = %verification.proof_id,
                        subject = %verification.subject,
                        action = %verification.action,
                        resource_hash = %verification.resource_hash,
                        "step-up proof 重放被拒绝"
                    );
                    return Err(invalid_step_up("proof 已被消费"));
                }
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
    decoding_keys: HashMap<String, DecodingKey>,
    legacy_decoding_key: Option<DecodingKey>,
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
            decoding_keys: HashMap::new(),
            legacy_decoding_key: Some(DecodingKey::from_secret(secret)),
            header,
            validation: step_up_validation(&issuer, &audience),
            issuer,
            audience,
            challenge_ttl: DEFAULT_STEP_UP_CHALLENGE_TTL.as_secs(),
            proof_ttl: DEFAULT_STEP_UP_PROOF_TTL.as_secs(),
        })
    }

    /// 使用 active/retiring HS256 keyring 创建可无感轮换的管理器。
    ///
    /// 新 challenge/proof 始终使用 active key 并写入 `kid`；校验只接受 active 与
    /// 显式 retiring keys。删除 retiring key 后，由该 key 签发且仍在 TTL 内的 Token
    /// 会立即失效。
    pub fn new_with_keyring<I, K, S>(
        active_kid: impl Into<String>,
        active_secret: impl AsRef<[u8]>,
        retiring_keys: I,
        issuer: impl Into<String>,
        audience: impl Into<String>,
    ) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = (K, S)>,
        K: Into<String>,
        S: AsRef<[u8]>,
    {
        let active_kid = active_kid.into();
        validate_step_up_kid(&active_kid)?;
        let active_secret = active_secret.as_ref();
        validate_step_up_secret(active_secret)?;

        let retiring_keys = retiring_keys.into_iter().collect::<Vec<_>>();
        if retiring_keys.len() > MAX_STEP_UP_RETIRING_KEYS {
            return Err(BaseError::ConfigError(format!(
                "step-up retiring keys 最多允许 {MAX_STEP_UP_RETIRING_KEYS} 把"
            )));
        }
        let mut decoding_keys = HashMap::with_capacity(retiring_keys.len() + 1);
        decoding_keys.insert(active_kid.clone(), DecodingKey::from_secret(active_secret));
        let mut seen_secrets = HashSet::with_capacity(retiring_keys.len() + 1);
        seen_secrets.insert(active_secret.to_vec());
        for (kid, secret) in retiring_keys {
            let kid = kid.into();
            validate_step_up_kid(&kid)?;
            let secret = secret.as_ref();
            validate_step_up_secret(secret)?;
            if decoding_keys.contains_key(&kid) {
                return Err(BaseError::ConfigError(format!(
                    "step-up key id 重复: {kid}"
                )));
            }
            if !seen_secrets.insert(secret.to_vec()) {
                return Err(BaseError::ConfigError(
                    "step-up active/retiring keys 不得复用同一密钥".to_string(),
                ));
            }
            decoding_keys.insert(kid, DecodingKey::from_secret(secret));
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
        header.kid = Some(active_kid);
        Ok(Self {
            encoding_key: EncodingKey::from_secret(active_secret),
            decoding_keys,
            legacy_decoding_key: None,
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
    ///
    /// # 限流（实现方必须）
    ///
    /// 本方法是凭据猜测的在线入口，**实现方必须**为完成 challenge 的端点配置速率
    /// 限制与失败计数，本管理器自身不做限流。接线要点：
    ///
    /// - 在调用方（重认证 Action 或 [`CredentialVerifier`] 实现内）按
    ///   `subject + 客户端标识`（如来源 IP、设备指纹）计数连续失败；
    /// - 超过阈值后按指数退避或直接锁定，并返回 `Unauthorized`，不得泄露锁定原因；
    /// - 计数与锁定状态应经 `ctx.tools()` 中的共享存储（如 Redis）实现，
    ///   多实例部署下才有一致的限流视图；
    /// - `CredentialVerifier` 返回的失败与 challenge 解析失败都应计入失败次数。
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
    ///
    /// 成功与失败都会发出审计事件（均含 subject/action/resource_hash，不记录
    /// proof Token 原文）。成功事件含 proof_id；失败事件按可解码性分两类：
    /// 签名可解码、因绑定维度不匹配被拒时，额外记录签名背书的
    /// `claimed_proof_id`/`claimed_resource_hash`（重放取证的关键字段）；
    /// 伪造或过期的 Token 无法解码，没有可信 proof_id，失败事件不含
    /// `claimed_*` 字段。
    pub fn verify_proof(
        &self,
        proof: &str,
        subject: &str,
        action: &ActionRef,
        resource: &str,
    ) -> Result<StepUpVerification, BaseError> {
        let expected_action = action.to_string();
        let expected_resource = resource_fingerprint(resource);
        let claims = match self.decode_kind(proof, StepUpTokenKind::Proof) {
            Ok(claims) => claims,
            Err(error) => {
                tracing::warn!(
                    subject = %subject,
                    action = %expected_action,
                    resource_hash = %expected_resource,
                    reason = %error,
                    "step-up proof 验证失败"
                );
                return Err(error);
            }
        };
        if claims.sub != subject
            || claims.action != expected_action
            || claims.resource_hash != expected_resource
            || claims.challenge_jti.is_none()
        {
            let error = invalid_step_up("proof 绑定目标不一致");
            tracing::warn!(
                claimed_proof_id = %claims.jti,
                claimed_resource_hash = %claims.resource_hash,
                subject = %subject,
                action = %expected_action,
                resource_hash = %expected_resource,
                reason = %error,
                "step-up proof 验证失败"
            );
            return Err(error);
        }
        let verification = StepUpVerification {
            subject: claims.sub,
            action: claims.action,
            resource_hash: claims.resource_hash,
            authenticated_at: claims.iat,
            expires_at: claims.exp,
            proof_id: claims.jti,
        };
        tracing::info!(
            proof_id = %verification.proof_id,
            subject = %verification.subject,
            action = %verification.action,
            resource_hash = %verification.resource_hash,
            "step-up proof 验证成功"
        );
        Ok(verification)
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
        let challenge = self.encode(&claims)?;
        tracing::info!(
            challenge_id = %claims.jti,
            subject = %claims.sub,
            action = %claims.action,
            resource_hash = %claims.resource_hash,
            "step-up challenge 已签发"
        );
        Ok(StepUpChallenge {
            challenge,
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
        let proof = self.encode(&claims)?;
        tracing::info!(
            proof_id = %claims.jti,
            challenge_id = %claims.challenge_jti.as_deref().unwrap_or_default(),
            subject = %claims.sub,
            action = %claims.action,
            resource_hash = %claims.resource_hash,
            "step-up challenge 已完成"
        );
        Ok(StepUpProof {
            proof,
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
        let header = decode_header(token).map_err(|_| invalid_step_up("Token header 无效"))?;
        if header.alg != Algorithm::HS256 || header.typ.as_deref() != Some("step-up+jwt") {
            return Err(invalid_step_up("Token header 不匹配"));
        }
        let decoding_key = match header.kid.as_deref() {
            Some(kid) => self
                .decoding_keys
                .get(kid)
                .ok_or_else(|| invalid_step_up("Token kid 未知"))?,
            None => self
                .legacy_decoding_key
                .as_ref()
                .ok_or_else(|| invalid_step_up("Token 缺少 kid"))?,
        };
        let claims = decode::<StepUpClaims>(token, decoding_key, &self.validation)
            .map_err(|_| invalid_step_up("Token 无效或已过期"))?
            .claims;
        if claims.kind != expected {
            return Err(invalid_step_up("Token 类型不匹配"));
        }
        Ok(claims)
    }
}

fn validate_step_up_secret(secret: &[u8]) -> Result<(), BaseError> {
    if secret.len() < MIN_STEP_UP_SECRET_BYTES {
        return Err(BaseError::ConfigError(
            "step-up HMAC 密钥至少需要 32 字节".to_string(),
        ));
    }
    Ok(())
}

fn validate_step_up_kid(kid: &str) -> Result<(), BaseError> {
    if kid.is_empty()
        || kid.len() > 64
        || !kid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(BaseError::ConfigError(
            "step-up key id 必须是 1..=64 位 ASCII 字母、数字、- 或 _".to_string(),
        ));
    }
    Ok(())
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
        let proof_request = || request().header(STEP_UP_PROOF_HEADER, &proof.proof);
        let response = dispatch_as(&app, "delete", proof_request(), Some(user.clone()))
            .await
            .expect("完全绑定的 proof 应允许敏感 Action");
        assert_eq!(response.data.expect("应返回 data")["operation"], "delete");

        let replay = dispatch_as(&app, "delete", proof_request(), Some(user)).await;
        assert!(
            matches!(replay, Err(BaseError::Unauthorized(ref message)) if message.contains("已被消费")),
            "同一 proof 第二次提交必须被原子消费边界拒绝: {replay:?}"
        );
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

    /// 锁定语义：step-up 仅约束 dispatch 路径；`Registry::call`/`Plugins::api_run`
    /// 内部调用不经过中间件链，敏感 Action 无需 proof 即可执行。
    /// 若日后调整该语义，必须同步修改 `StepUpMiddleware` 与 `Registry::call` 的文档。
    #[tokio::test]
    async fn internal_call_bypasses_step_up_middleware_by_design() {
        let manager = Arc::new(manager());
        let app = test_app(manager);
        let handle = app
            .registry()
            .resolve_typed::<EmptyInput, ProbeOutput>(&action_ref("delete"))
            .expect("敏感 Action 应可解析为强类型句柄");

        // Registry::call：无 proof 直接成功（policy 仍要求已认证用户）
        let context = app
            .context(Request::new(serde_json::json!({})))
            .with_user(User::new(7, "member"));
        let output = app
            .registry()
            .call(handle, context, EmptyInput {})
            .await
            .expect("内部 call 不经过 step-up 中间件，应无需 proof 直接成功");
        assert_eq!(output.operation, "delete");

        // Plugins::api_run：同一内部路径，同样不经过 step-up 中间件
        let context = app
            .context(Request::new(serde_json::json!({})))
            .with_user(User::new(7, "member"));
        let output = context
            .plugins()
            .expect("测试上下文应已绑定 Registry")
            .api_run(handle, EmptyInput {})
            .await
            .expect("api_run 不经过 step-up 中间件，应无需 proof 直接成功");
        assert_eq!(output.operation, "delete");
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

    /// 审计事件：challenge 签发、challenge 完成、proof 验证成功/失败四处
    /// 必须发出 tracing 事件，携带 proof_id/subject/action/resource_hash，
    /// 且不得泄露 challenge/proof Token 与资源原文。
    #[tokio::test]
    async fn audit_events_cover_the_full_sensitive_lifecycle() {
        use std::io::Write;
        use std::sync::Mutex;

        // 自定义 MakeWriter：将 tracing fmt 输出捕获到内存缓冲区（同 transport_axum 的 warn 捕获模式）
        struct BufWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }
        impl Write for BufWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().expect("缓冲区锁不应中毒").write(buf)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.buf.lock().expect("缓冲区锁不应中毒").flush()
            }
        }
        struct BufMakeWriter {
            buf: Arc<Mutex<Vec<u8>>>,
        }
        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufMakeWriter {
            type Writer = BufWriter;
            fn make_writer(&'a self) -> Self::Writer {
                BufWriter {
                    buf: self.buf.clone(),
                }
            }
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(BufMakeWriter { buf: buf.clone() })
            .with_ansi(false)
            .finish();
        // 必须用全局默认订阅器，而非线程局部 `set_default`：
        // 并行测试会在无订阅器的线程上命中同一 callsite，把兴趣缓存为 never，
        // 线程局部订阅器无法再重建这些缓存（tracing 的 JustOne 快路径），
        // 导致事件被静默跳过、断言 flaky。全局默认在注册时重建全部 callsite，
        // 且作为所有线程的回退默认参与后续 callsite 注册，语义确定。
        // 代价：进程内其他测试的事件也会进入缓冲区，故断言一律用 contains。
        tracing::subscriber::set_global_default(subscriber)
            .expect("step-up 审计测试要求全局默认订阅器未被占用");

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
        let verified = manager
            .verify_proof(&proof.proof, "7", &action, resource)
            .expect("proof 应验证成功");
        let rejected = manager.verify_proof(&proof.proof, "7", &action, "org_user:43");
        assert!(matches!(rejected, Err(BaseError::Unauthorized(_))));
        let tampered = tamper_signature(&proof.proof);
        assert!(matches!(
            manager.verify_proof(&tampered, "7", &action, resource),
            Err(BaseError::Unauthorized(_))
        ));

        let output = String::from_utf8(buf.lock().expect("缓冲区锁不应中毒").clone())
            .expect("审计输出应为 UTF-8");
        for event in [
            "step-up challenge 已签发",
            "step-up challenge 已完成",
            "step-up proof 验证成功",
            "step-up proof 验证失败",
        ] {
            assert!(output.contains(event), "缺少审计事件 {event}: {output}");
        }
        assert!(
            output.contains(&verified.proof_id),
            "审计事件应携带 proof_id: {output}"
        );
        assert!(
            output.contains("subject=7"),
            "审计事件应携带 subject: {output}"
        );
        assert!(
            output.contains("org.user.delete"),
            "审计事件应携带 action: {output}"
        );
        assert!(
            output.contains(&verified.resource_hash),
            "审计事件应携带 resource_hash: {output}"
        );
        // 绑定不匹配（重放攻击面）：失败事件必须记录签名背书的真实声明，供取证关联
        let failure_lines: Vec<&str> = output
            .lines()
            .filter(|line| line.contains("step-up proof 验证失败"))
            .collect();
        assert!(
            failure_lines
                .iter()
                .any(|line| line.contains(&format!("claimed_proof_id={}", verified.proof_id))),
            "绑定不匹配失败事件应记录 claimed_proof_id: {output}"
        );
        assert!(
            failure_lines
                .iter()
                .any(|line| line
                    .contains(&format!("claimed_resource_hash={}", verified.resource_hash))),
            "绑定不匹配失败事件应记录 claimed_resource_hash: {output}"
        );
        // 伪造 Token 无法解码：没有可信 proof_id，失败事件不得虚构 claimed_* 字段
        assert!(
            failure_lines
                .iter()
                .any(|line| line.contains("无效或已过期") && !line.contains("claimed_proof_id")),
            "不可解码的失败事件不应携带 claimed_proof_id: {output}"
        );
        assert!(
            !output.contains(resource),
            "审计事件不得携带资源原文: {output}"
        );
        assert!(
            !output.contains(&proof.proof),
            "审计事件不得携带 proof Token 原文"
        );
        assert!(
            !output.contains(&challenge.challenge),
            "审计事件不得携带 challenge Token 原文"
        );
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
        assert!(matches!(
            StepUpManager::new_with_keyring(
                "invalid kid",
                SECRET,
                std::iter::empty::<(&str, &str)>(),
                "issuer",
                "audience"
            ),
            Err(BaseError::ConfigError(_))
        ));
        assert!(matches!(
            StepUpManager::new_with_keyring(
                "active",
                SECRET,
                [("retiring", SECRET)],
                "issuer",
                "audience"
            ),
            Err(BaseError::ConfigError(_))
        ));
    }

    #[tokio::test]
    async fn keyring_rotates_without_breaking_retiring_challenges() {
        const OLD_SECRET: &str = "old-step-up-secret-must-be-at-least-32-bytes";
        const NEW_SECRET: &str = "new-step-up-secret-must-be-at-least-32-bytes";
        let old = StepUpManager::new_with_keyring(
            "old-key",
            OLD_SECRET,
            std::iter::empty::<(&str, &str)>(),
            "yang-test",
            "yang-sensitive-actions",
        )
        .expect("旧 keyring 应有效");
        let action = action_ref("delete");
        let challenge = old
            .issue_challenge("7", &action, "org_user:42")
            .expect("旧 key 应可签发 challenge");
        assert_eq!(
            decode_header(&challenge.challenge)
                .expect("challenge header 应可解码")
                .kid
                .as_deref(),
            Some("old-key")
        );

        let rotated = StepUpManager::new_with_keyring(
            "new-key",
            NEW_SECRET,
            [("old-key", OLD_SECRET)],
            "yang-test",
            "yang-sensitive-actions",
        )
        .expect("轮换 keyring 应有效");
        let context = context();
        let proof = rotated
            .complete_challenge(
                &context,
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .expect("retiring key 的未过期 challenge 应可完成");
        assert_eq!(
            decode_header(&proof.proof)
                .expect("proof header 应可解码")
                .kid
                .as_deref(),
            Some("new-key")
        );
        rotated
            .verify_proof(&proof.proof, "7", &action, "org_user:42")
            .expect("新 proof 应由 active key 验证");

        let retired = StepUpManager::new_with_keyring(
            "new-key",
            NEW_SECRET,
            std::iter::empty::<(&str, &str)>(),
            "yang-test",
            "yang-sensitive-actions",
        )
        .expect("移除旧 key 后 keyring 仍应有效");
        assert!(retired
            .complete_challenge(
                &context,
                &TestVerifier,
                &credentials("7", "correct-password"),
                &challenge.challenge,
            )
            .await
            .is_err());
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
