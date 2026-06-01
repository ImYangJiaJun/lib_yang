//! Token 撤销 / 黑名单机制（H-4）
//!
//! JWT 一经签发，在过期前本身无法失效。对登出、改密、强制下线等场景，
//! 需要一个额外的撤销层。本模块基于 [`GlobalRedis`] 维护一份 `jti` 黑名单：
//!
//! - 撤销时把 Token 的 `jti` 写入 Redis，TTL 设为 Token 剩余有效期
//!   （过期后 key 自动消失，黑名单不会无限增长）。
//! - 校验时在标准签名/过期校验之外，额外查一次黑名单。
//!
//! 设计约束：**不修改** [`TokenManager::verify_token`] 的现有签名与行为，
//! 撤销校验通过新增的 [`TokenManager::verify_token_checked`] 提供，保持向后兼容。

use crate::database::GlobalRedis;
use crate::error::BaseError;
use crate::token::manager::current_unix_timestamp;
use crate::token::{TokenClaims, TokenManager};

/// Redis 黑名单 key 前缀。最终 key 形如 `token:blacklist:{jti}`。
const BLACKLIST_PREFIX: &str = "token:blacklist:";

/// 根据 jti 构造黑名单 Redis key。
fn blacklist_key(jti: &str) -> String {
    format!("{BLACKLIST_PREFIX}{jti}")
}

impl TokenManager {
    /// 撤销一个 Token（写入 Redis 黑名单）。
    ///
    /// 先验证 Token（确保 `jti`/`exp` 合法且签名有效），再按 `exp - now` 计算
    /// 剩余 TTL 写入黑名单。若 Token 已过期则无需撤销，直接返回 `Ok(())`。
    ///
    /// # 参数
    ///
    /// - `token`: 待撤销的 Token 字符串
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 撤销成功，或 Token 已过期无需撤销
    /// - `Err(BaseError::TokenVerifyFailed)`: Token 无效（签名/过期/声明不合法）
    /// - `Err(BaseError::RedisOperationFailed)`: 写黑名单失败
    pub async fn revoke_token(&self, token: &str) -> Result<(), BaseError> {
        let claims = self.verify_token(token)?;
        self.revoke_claims(&claims).await
    }

    /// 按已验证的 [`TokenClaims`] 撤销 Token。
    ///
    /// 当调用方手头已有验证过的 claims 时，可避免重复验证。
    ///
    /// # 参数
    ///
    /// - `claims`: 已通过验证的 Token 声明
    pub async fn revoke_claims(&self, claims: &TokenClaims) -> Result<(), BaseError> {
        let now = current_unix_timestamp()?;
        // 剩余有效期；已过期则无需写黑名单
        let ttl = claims.exp.saturating_sub(now);
        if ttl == 0 {
            return Ok(());
        }

        GlobalRedis::set(blacklist_key(&claims.jti), "1", Some(ttl as i64)).await?;
        Ok(())
    }

    /// 查询某个 `jti` 是否已被撤销。
    ///
    /// # 参数
    ///
    /// - `jti`: Token 的唯一标识
    ///
    /// # 返回
    ///
    /// - `Ok(true)`: 已撤销（命中黑名单）
    /// - `Ok(false)`: 未撤销
    /// - `Err(BaseError::RedisOperationFailed)`: Redis 查询失败
    pub async fn is_revoked(&self, jti: &str) -> Result<bool, BaseError> {
        let count = GlobalRedis::exists(&[blacklist_key(jti)]).await?;
        Ok(count > 0)
    }

    /// 验证 Token，并额外检查黑名单。
    ///
    /// 在 [`TokenManager::verify_token`] 的全部校验之上，再查一次 Redis 黑名单。
    /// 鉴权路径若需支持登出/撤销，应使用本方法替代 `verify_token`。
    ///
    /// # 参数
    ///
    /// - `token`: 待验证的 Token 字符串
    ///
    /// # 返回
    ///
    /// - `Ok(TokenClaims)`: 验证通过且未被撤销
    /// - `Err(BaseError::TokenVerifyFailed)`: 签名/过期/声明校验失败
    /// - `Err(BaseError::TokenRevoked)`: Token 已被撤销
    /// - `Err(BaseError::RedisOperationFailed)`: 黑名单查询失败
    pub async fn verify_token_checked(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let claims = self.verify_token(token)?;
        if self.is_revoked(&claims.jti).await? {
            return Err(BaseError::TokenRevoked);
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklist_key_format() {
        assert_eq!(blacklist_key("abc-123"), "token:blacklist:abc-123");
    }
}
