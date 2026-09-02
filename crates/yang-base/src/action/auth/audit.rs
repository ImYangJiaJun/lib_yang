//! 认证审计钩子：登录/刷新/登出的成功与失败事件（可观测性 C4）。

use async_trait::async_trait;

// ──────────────────────────────────────────────────────────────────────────────
// AuthAuditHook：认证审计钩子（可观测性 C4）
// ──────────────────────────────────────────────────────────────────────────────

/// 认证审计事件。
///
/// 描述一次认证相关操作（登录/刷新/登出）的结果。**绝不携带凭据明文或 Token
/// 原文**：需要标识 Token 时只放指纹（[`token_fingerprint`]）。
///
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AuthAuditEvent {
    /// 本次派发的 request_id（十六进制串）
    pub request_id: String,
    /// 操作名（`"login"` / `"refresh"` / `"logout"`，静态）
    pub action: &'static str,
    /// 主体标识（用户名或 sub）；失败且未知时为 `None`
    pub subject: Option<String>,
    /// 失败时的错误码（`BaseError::code_str`，静态）；成功时为 `None`
    pub error_code: Option<&'static str>,
}

/// 计算 Token 的短指纹（SHA256 前若干字节的十六进制），用于审计日志而不泄漏原文。
///
/// 这里不引入额外哈希依赖，采用 FNV-1a 64 位哈希取十六进制——审计场景只需「同一
/// Token 多次出现可对应」而非密码学强度，FNV 足够且零依赖。
pub fn token_fingerprint(token: &str) -> String {
    // FNV-1a 64-bit
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in token.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// 认证审计钩子：在登录/刷新/登出的成功/失败处被调用。
///
/// object-safe，可经构造参数注入（与 [`CredentialVerifier`](super::CredentialVerifier) 注入同构）。默认实现
/// [`TracingAuditHook`] 发 tracing event。实现者**绝不应**记录凭据明文/Token 原文。
#[async_trait]
pub trait AuthAuditHook: Send + Sync + 'static {
    /// 认证成功事件。
    async fn on_success(&self, event: AuthAuditEvent);
    /// 认证失败事件。
    async fn on_failure(&self, event: AuthAuditEvent);
}

/// 默认审计钩子：成功发 `tracing::info!`，失败发 `tracing::warn!`。
#[derive(Debug, Clone, Default)]
pub struct TracingAuditHook;

#[async_trait]
impl AuthAuditHook for TracingAuditHook {
    async fn on_success(&self, event: AuthAuditEvent) {
        tracing::info!(
            request_id = %event.request_id,
            action = event.action,
            subject = event.subject.as_deref().unwrap_or("-"),
            "认证成功",
        );
    }

    async fn on_failure(&self, event: AuthAuditEvent) {
        tracing::warn!(
            request_id = %event.request_id,
            action = event.action,
            subject = event.subject.as_deref().unwrap_or("-"),
            error_code = event.error_code.unwrap_or("-"),
            "认证失败",
        );
    }
}
