//! 登出 Action：撤销 Token（写入 Redis 黑名单）。

use super::audit::{token_fingerprint, AuthAuditEvent, AuthAuditHook, TracingAuditHook};
use super::dto::{LogoutInput, MessageResponse};
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use yang_base_derive::Action;

// ──────────────────────────────────────────────────────────────────────────────
// LogoutAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登出 Action：撤销 Token（写入 Redis 黑名单）。公开（凭 Token 自证）。
///
/// 彻底终止会话需**同时**撤销 Access Token 与 Refresh Token：只撤 Access Token
/// 时，攻击者仍可用未失效的 Refresh Token 刷出新的 Access Token。因此调用方应在
/// `token` 传 Access Token 的同时，于 `refresh_token` 传入 Refresh Token，本 Action
/// 会将二者一并拉黑。
///
/// # 所有权校验（AUTH-4）
///
/// 当请求携带 Bearer Token（`Authorization` 头）时，本 Action 会校验该 Bearer Token
/// 的 `sub` 与待撤销 Token（`input.token`）的 `sub` 一致。不一致时返回
/// [`BaseError::PermissionDenied`]，防止用户撤销他人的 Token。
///
/// 若请求未携带 Bearer Token（匿名调用），则跳过所有权校验——此时无法确认调用者身份，
/// 撤销操作仍允许执行（向后兼容）。
#[derive(Action, Default)]
#[action(
    name = "logout",
    display_name = "登出",
    description = "撤销 Token，使其在过期前失效",
    public
)]
pub struct LogoutAction<A: AuthAuditHook = TracingAuditHook> {
    audit: A,
}

impl LogoutAction<TracingAuditHook> {
    /// 创建 LogoutAction（默认 tracing 审计钩子）。
    pub fn new() -> Self {
        Self {
            audit: TracingAuditHook,
        }
    }
}

impl<A: AuthAuditHook> LogoutAction<A> {
    /// 用自定义审计钩子创建 LogoutAction。
    pub fn with_audit(audit: A) -> Self {
        Self { audit }
    }
}

#[async_trait]
impl<A: AuthAuditHook> TypedHandler for LogoutAction<A> {
    type Input = LogoutInput;
    type Output = MessageResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LogoutInput,
    ) -> Result<MessageResponse, BaseError> {
        let request_id = ctx.request_id.to_string();
        let manager = ctx.tools().token()?;

        let run = async {
            // AUTH-4：解析待撤销 Token 的 claims（仅校验签名/过期，不查黑名单——
            // 该 Token 本身就是要被撤销的目标，查黑名单无意义）
            let target_claims = manager.verify_token(&input.token)?;

            // 若请求携带 Bearer Token，校验其 sub 与待撤销 Token 的 sub 一致
            if let Some(bearer_token) = ctx.request.token() {
                // Bearer Token 需完整验证（含黑名单），确保调用者身份有效
                let caller_claims = manager.verify_token_checked(bearer_token).await?;
                if caller_claims.sub != target_claims.sub {
                    return Err(BaseError::PermissionDenied(
                        "只能撤销自己的 Token".to_string(),
                    ));
                }
            }

            // CONC-1：用 subject 水位线一次性原子撤销该用户所有 Token
            //（Access + Refresh + 任何其他已签发 Token），避免双 revoke 非原子竞态。
            // 水位线 TTL 取 Refresh Token 有效期，早于此时间签发的 Token 全部失效。
            manager.revoke_by_subject(&target_claims.sub).await?;
            Ok::<(), BaseError>(())
        };

        match run.await {
            Ok(()) => {
                // subject 用 Access Token 指纹标识（不泄漏原文）
                self.audit
                    .on_success(AuthAuditEvent {
                        request_id,
                        action: "logout",
                        subject: Some(token_fingerprint(&input.token)),
                        error_code: None,
                    })
                    .await;
                Ok(MessageResponse {
                    message: "已登出".to_string(),
                })
            }
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "logout",
                        subject: Some(token_fingerprint(&input.token)),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                Err(e)
            }
        }
    }
}
