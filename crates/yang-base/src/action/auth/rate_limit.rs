//! 认证入口的 Redis 原子限流。
//!
//! 登录、注册、改密、Step-up 等凭据猜测入口共用同一套限流机制：按来源 IP 与业务身份
//! （用户名/用户 ID/指纹）双维度计数，所有计数与窗口过期在单个 Lua 脚本内原子完成，
//! 超限返回 [`BaseError::RateLimitExceeded`] 并携带 `retry_after_seconds`。
//! 阈值、窗口与 Redis key 前缀全部经 [`AuthRateLimitConfig`] 注入，不读取应用配置。

use crate::action::ActionContext;
use crate::transport::client_ip::client_ip_identity;
use yang_base::BaseError;

const RATE_LIMIT_SCRIPT: &str = r#"
local exceeded = 0
local retry_after = 0
local window = tonumber(ARGV[1])

for index, key in ipairs(KEYS) do
    local current = redis.call('INCR', key)
    if current == 1 then
        redis.call('EXPIRE', key, window)
    end

    local ttl = redis.call('TTL', key)
    if ttl < 1 then
        redis.call('EXPIRE', key, window)
        ttl = window
    end

    local limit = tonumber(ARGV[index + 1])
    if current > limit then
        exceeded = 1
        if ttl > retry_after then
            retry_after = ttl
        end
    end
end

return { exceeded, retry_after }
"#;

const FAILURE_COUNT_SCRIPT: &str = r#"
local exceeded = 0
local retry_after = 0
local window = tonumber(ARGV[1])

for index, key in ipairs(KEYS) do
    local current = redis.call('INCR', key)
    if current == 1 then
        redis.call('EXPIRE', key, window)
    end

    local ttl = redis.call('TTL', key)
    if ttl < 1 then
        redis.call('EXPIRE', key, window)
        ttl = window
    end

    local limit = tonumber(ARGV[index + 1])
    if current > limit then
        exceeded = 1
        if ttl > retry_after then
            retry_after = ttl
        end
    end
end

return { exceeded, retry_after }
"#;

/// 认证相关限流操作：决定 Redis key 的操作段与身份段。
#[derive(Clone, Copy)]
pub enum AuthOperation {
    /// 修改密码（按用户 ID 计数）。
    ChangePassword,
    /// 创建密码重置凭证（按操作者-目标对计数）。
    PasswordResetCreate,
    /// 消费密码重置凭证（按凭证指纹计数）。
    PasswordResetConsume,
    /// 登录（按用户名计数）。
    Login,
    /// 注册（按用户名计数）。
    Register,
    /// 完成 Step-up 重认证（按用户名计数）。
    StepUpComplete,
    /// 校验 TOTP 第二因子码（按用户名计数；凭据猜测的在线入口）。
    TotpVerify,
    /// 生成/激活 TOTP 配置（按用户名计数；激活码重试入口）。
    TotpSetup,
}

impl AuthOperation {
    fn key(self) -> &'static str {
        match self {
            Self::ChangePassword => "change-password",
            Self::PasswordResetCreate => "password-reset-create",
            Self::PasswordResetConsume => "password-reset-consume",
            Self::Login => "login",
            Self::Register => "register",
            Self::StepUpComplete => "step-up-complete",
            Self::TotpVerify => "totp-verify",
            Self::TotpSetup => "totp-setup",
        }
    }

    fn identity_key(self) -> &'static str {
        match self {
            Self::ChangePassword => "user",
            Self::PasswordResetCreate => "actor-target",
            Self::PasswordResetConsume => "fingerprint",
            Self::Login
            | Self::Register
            | Self::StepUpComplete
            | Self::TotpVerify
            | Self::TotpSetup => "username",
        }
    }
}

/// [`AuthRateLimiter`] 的独立参数结构：阈值、窗口、Redis key 前缀与指标名。
#[derive(Clone)]
pub struct AuthRateLimitConfig {
    /// 计数窗口长度（秒）。
    pub window_seconds: u64,
    /// 窗口内单来源 IP 允许的最大尝试次数。
    pub ip_attempts: u64,
    /// 窗口内单身份（用户名等）允许的最大尝试次数。
    pub username_attempts: u64,
    /// Redis key 前缀（按应用隔离，如 `"yang-system"`）。
    pub key_prefix: String,
    /// 限流判定计数器指标名（`metrics` feature 启用时发出）。
    pub metric_name: &'static str,
}

/// 认证入口限流器：来源 IP + 业务身份双维度的 Redis 原子计数。
#[derive(Clone)]
pub struct AuthRateLimiter {
    config: AuthRateLimitConfig,
}

impl AuthRateLimiter {
    /// 用独立参数结构创建限流器。
    pub fn new(config: AuthRateLimitConfig) -> Self {
        Self { config }
    }

    /// 记录一次尝试并判定是否超限。
    pub async fn check(
        &self,
        ctx: &ActionContext,
        operation: AuthOperation,
        identity: &str,
    ) -> Result<(), BaseError> {
        let source = client_ip_identity(ctx);
        let prefix = format!("{}:auth-rate:{}", self.config.key_prefix, operation.key());
        let keys = [
            format!("{prefix}:ip:{source}"),
            format!("{prefix}:{}:{identity}", operation.identity_key()),
        ];
        let args = [
            self.config.window_seconds.to_string(),
            self.config.ip_attempts.to_string(),
            self.config.username_attempts.to_string(),
        ];
        let cache = ctx.tools().cache()?;
        let script = cache.script(RATE_LIMIT_SCRIPT);
        let decision: Result<(i64, i64), _> = cache.eval_script(&script, &keys, &args).await;
        match decision {
            Ok((exceeded, retry_after)) => {
                let result = rate_limit_result(exceeded, retry_after);
                #[cfg(feature = "metrics")]
                metrics::counter!(
                    self.config.metric_name,
                    "operation" => operation.key(),
                    "result" => if result.is_ok() { "allowed" } else { "limited" }
                )
                .increment(1);
                result
            }
            Err(error) => {
                #[cfg(feature = "metrics")]
                metrics::counter!(
                    self.config.metric_name,
                    "operation" => operation.key(),
                    "result" => "unavailable"
                )
                .increment(1);
                Err(error.into())
            }
        }
    }

    /// 记录一次认证失败；失败计数超限同样返回限流错误。
    pub async fn record_failure(
        &self,
        ctx: &ActionContext,
        operation: AuthOperation,
        identity: &str,
    ) -> Result<(), BaseError> {
        let keys = self.failure_keys(ctx, operation, identity);
        let args = [
            self.config.window_seconds.to_string(),
            self.config.ip_attempts.to_string(),
            self.config.username_attempts.to_string(),
        ];
        let cache = ctx.tools().cache()?;
        let script = cache.script(FAILURE_COUNT_SCRIPT);
        let decision: (i64, i64) = cache.eval_script(&script, &keys, &args).await?;
        rate_limit_result(decision.0, decision.1)
    }

    /// 认证成功后清除该身份的失败计数。
    pub async fn clear_failures(
        &self,
        ctx: &ActionContext,
        operation: AuthOperation,
        identity: &str,
    ) -> Result<(), BaseError> {
        let keys = self.failure_keys(ctx, operation, identity);
        ctx.tools().cache()?.del(&keys).await?;
        Ok(())
    }

    fn failure_keys(
        &self,
        ctx: &ActionContext,
        operation: AuthOperation,
        identity: &str,
    ) -> [String; 2] {
        let source = client_ip_identity(ctx);
        let prefix = format!(
            "{}:auth-failure:{}",
            self.config.key_prefix,
            operation.key()
        );
        [
            format!("{prefix}:ip:{source}"),
            format!("{prefix}:{}:{identity}", operation.identity_key()),
        ]
    }
}

fn rate_limit_result(exceeded: i64, retry_after: i64) -> Result<(), BaseError> {
    if exceeded == 0 {
        return Ok(());
    }
    Err(BaseError::RateLimitExceeded {
        retry_after_seconds: u64::try_from(retry_after).unwrap_or(1).max(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Request, RequestMeta};
    use crate::tools::ToolsBuilder;
    use crate::transport::client_ip::CLIENT_IP_META_KEY;
    use std::net::SocketAddr;
    use std::sync::Arc;

    fn test_config() -> AuthRateLimitConfig {
        AuthRateLimitConfig {
            window_seconds: 60,
            ip_attempts: 10,
            username_attempts: 5,
            key_prefix: "yang-system".to_string(),
            metric_name: "yang_system_auth_rate_limit_total",
        }
    }

    #[test]
    fn maps_redis_decision_to_standard_rate_limit_error() {
        assert!(rate_limit_result(0, 0).is_ok());
        assert!(matches!(
            rate_limit_result(1, 17),
            Err(BaseError::RateLimitExceeded {
                retry_after_seconds: 17
            })
        ));
        assert!(matches!(
            rate_limit_result(1, -1),
            Err(BaseError::RateLimitExceeded {
                retry_after_seconds: 1
            })
        ));
    }

    #[test]
    fn operation_keys_are_isolated() {
        assert_ne!(AuthOperation::Login.key(), AuthOperation::Register.key());
        assert_ne!(
            AuthOperation::ChangePassword.key(),
            AuthOperation::Login.key()
        );
        assert_eq!(AuthOperation::ChangePassword.identity_key(), "user");
        assert_eq!(AuthOperation::Login.identity_key(), "username");
        assert_ne!(
            AuthOperation::StepUpComplete.key(),
            AuthOperation::Login.key()
        );
        assert_eq!(AuthOperation::StepUpComplete.identity_key(), "username");
    }

    #[test]
    fn rate_limit_identity_prefers_trusted_transport_extension() {
        let tools = ToolsBuilder::new()
            .build()
            .unwrap_or_else(|error| panic!("测试 Tools 应构建成功: {error}"));
        let peer = "10.0.0.2:443"
            .parse::<SocketAddr>()
            .unwrap_or_else(|error| panic!("测试对端地址应有效: {error}"));
        let mut context =
            ActionContext::new(Request::new(serde_json::Value::Null), Arc::new(tools))
                .with_request_meta(RequestMeta::new().with_peer_addr(peer));
        context
            .request_meta
            .extensions
            .insert(CLIENT_IP_META_KEY.to_string(), "198.51.100.7".to_string());

        assert_eq!(client_ip_identity(&context), "198.51.100.7");
        context.request_meta.extensions.clear();
        assert_eq!(client_ip_identity(&context), "10.0.0.2");
        context.request_meta.peer_addr = None;
        assert_eq!(client_ip_identity(&context), "unknown");
    }

    #[test]
    fn step_up_failure_keys_are_separate_from_login_attempt_keys() {
        let limiter = AuthRateLimiter::new(test_config());
        let tools = ToolsBuilder::new()
            .build()
            .unwrap_or_else(|error| panic!("测试 Tools 应构建成功: {error}"));
        let context = ActionContext::new(Request::new(serde_json::Value::Null), Arc::new(tools));
        let keys = limiter.failure_keys(&context, AuthOperation::StepUpComplete, "alice");

        assert_eq!(
            keys,
            [
                "yang-system:auth-failure:step-up-complete:ip:unknown".to_string(),
                "yang-system:auth-failure:step-up-complete:username:alice".to_string(),
            ]
        );
        assert!(keys.iter().all(|key| !key.contains("auth-rate:login")));
    }
}
