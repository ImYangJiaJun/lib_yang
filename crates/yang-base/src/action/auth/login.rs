//! 登录 Action 与业务凭证校验契约。

use super::audit::{AuthAuditEvent, AuthAuditHook, TracingAuditHook};
use super::dto::{LoginInput, TokenPairResponse, VerifiedSubject};
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use yang_base_derive::Action;

// ──────────────────────────────────────────────────────────────────────────────
// CredentialVerifier：业务实现的凭证校验
// ──────────────────────────────────────────────────────────────────────────────

/// 凭证校验 trait，由业务方实现并注入 [`LoginAction`]。
///
/// # 限流（实现方必须）
///
/// 本 trait 同时服务登录与 step-up 重认证（`StepUpManager::complete_challenge`），
/// 两者都是凭据猜测的在线入口。实现方**必须**在 `verify` 内（或调用它的端点上）
/// 做速率限制与失败计数：按 `subject + 客户端标识`（来源 IP、设备指纹等）计数
/// 连续失败，超阈值后指数退避或锁定并返回 `Unauthorized`；计数状态应经
/// `ctx.tools()` 的共享存储（如 Redis）实现，保证多实例下一致。
#[async_trait]
pub trait CredentialVerifier: Send + Sync + 'static {
    /// 校验登录凭证。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（可借此访问数据库等）
    /// - `input`: 登录输入
    ///
    /// # 返回
    ///
    /// - `Ok(VerifiedSubject)`: 校验通过，携带签发 Token 所需的主体信息
    /// - `Err(BaseError)`: 校验失败（如 `Unauthorized`）
    async fn verify(
        &self,
        ctx: &ActionContext,
        input: &LoginInput,
    ) -> Result<VerifiedSubject, BaseError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// LoginAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登录 Action：校验凭证后签发 Token 对。公开（无需登录即可访问）。
///
/// 泛型 `A` 为审计钩子（默认 [`TracingAuditHook`]）：登录成功/失败均发审计事件，
/// 事件只含 request_id/subject/错误码，**不含凭据明文**。
#[derive(Action)]
#[action(
    name = "login",
    display_name = "登录",
    description = "校验凭证并签发 Token 对",
    public
)]
pub struct LoginAction<V: CredentialVerifier, A: AuthAuditHook = TracingAuditHook> {
    verifier: V,
    audit: A,
}

impl<V: CredentialVerifier> LoginAction<V, TracingAuditHook> {
    /// 用业务凭证校验器创建登录 Action（默认 tracing 审计钩子）。
    pub fn new(verifier: V) -> Self {
        Self {
            verifier,
            audit: TracingAuditHook,
        }
    }
}

impl<V: CredentialVerifier, A: AuthAuditHook> LoginAction<V, A> {
    /// 用业务凭证校验器 + 自定义审计钩子创建登录 Action。
    pub fn with_audit(verifier: V, audit: A) -> Self {
        Self { verifier, audit }
    }
}

#[async_trait]
impl<V: CredentialVerifier, A: AuthAuditHook> TypedHandler for LoginAction<V, A> {
    type Input = LoginInput;
    type Output = TokenPairResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LoginInput,
    ) -> Result<TokenPairResponse, BaseError> {
        let request_id = ctx.request_id.to_string();
        let subject = match self.verifier.verify(&ctx, &input).await {
            Ok(s) => s,
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(input.username.clone()),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                return Err(e);
            }
        };
        let result = ctx
            .tools()
            .token()?
            .generate_token_pair_with_refresh_claims(
                &subject.subject,
                subject.custom_claims,
                subject.refresh_claims,
            );
        match result {
            Ok((access_token, refresh_token)) => {
                self.audit
                    .on_success(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(subject.subject.clone()),
                        error_code: None,
                    })
                    .await;
                Ok(TokenPairResponse {
                    access_token,
                    refresh_token,
                })
            }
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(subject.subject.clone()),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                Err(e)
            }
        }
    }
}
