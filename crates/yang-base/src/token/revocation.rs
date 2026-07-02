//! Token 撤销 / 黑名单机制（H-4）
//!
//! JWT 一经签发，在过期前本身无法失效。对登出、改密、强制下线等场景，
//! 需要一个额外的撤销层。本模块基于 [`GlobalRedis`] 提供两层撤销能力：
//!
//! 1. **按单个 Token 撤销（jti 黑名单）**：用于登出。撤销时把 Token 的 `jti`
//!    写入 Redis，TTL 设为 Token 剩余有效期（过期后 key 自动消失，黑名单不会
//!    无限增长）。校验时在标准签名/过期校验之外，额外查一次该黑名单。
//! 2. **按用户批量撤销（subject 时间水位线）**：用于改密、强制下线。撤销时把
//!    “当前时间戳”写入 `token:user:{sub}:min_iat`，TTL 取 Refresh Token 有效期。
//!    校验时若该用户存在水位线且 Token 的 `iat` 早于水位线，则视为已撤销——
//!    一次写入即可让该用户在此之前签发的所有 Token 全部失效。
//!
//! 设计约束：**不修改** [`TokenManager::verify_token`] 的现有签名与行为，
//! 撤销校验通过 [`TokenManager::verify_token_checked`] 提供，保持向后兼容。

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

/// 按用户撤销的时间水位线 key 前缀。最终 key 形如 `token:user:{sub}:min_iat`。
const SUBJECT_MIN_IAT_PREFIX: &str = "token:user:";

/// 根据 subject 构造“最小签发时间水位线”Redis key。
fn subject_min_iat_key(sub: &str) -> String {
    format!("{SUBJECT_MIN_IAT_PREFIX}{sub}:min_iat")
}

/// 判定一个 Token 的签发时间 `iat` 是否被用户水位线 `min_iat` 撤销。
///
/// 采用 `iat <= min_iat`（含等号）而非严格小于：撤销与签发都是**秒级**时间戳，
/// 若用 `<`，则在改密/强制下线那一秒内签发的 Token（`iat == min_iat`）会逃过
/// 撤销，留下 1 秒旁路窗口（NEW-6）。安全取「过撤一侧」——同秒签发的 Token 一并
/// 撤销（用户至多需在该秒后重新登录一次），杜绝旁路。
fn iat_revoked_by_watermark(iat: u64, min_iat: u64) -> bool {
    iat <= min_iat
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
    /// - `Err(BaseError::TokenVerifyFailed)`: Token 签名/声明校验失败
    /// - `Err(BaseError::TokenExpired)`: Token 已过期（无需撤销，返回 `Ok(())`）
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

    /// 按用户（subject）批量撤销该用户在此刻之前签发的所有 Token（token-3）。
    ///
    /// 在 Redis 中将 `token:user:{sub}:min_iat` 写为当前时间戳，作为该用户的
    /// “最小有效签发时间”水位线。此后 [`TokenManager::verify_token_checked`]
    /// 会拒绝任何 `iat` 早于该水位线的 Token，从而让此前签发的全部 Token 一次性失效。
    ///
    /// 适用于**改密、强制下线**等需要让某用户全部会话立即失效的场景。
    /// 水位线 TTL 取 Refresh Token 有效期（[`TokenManager::refresh_token_expiry`]），
    /// 因为更早签发的 Token 至此必然已过期，水位线无需再保留，避免无限增长。
    ///
    /// # 参数
    ///
    /// - `sub`: 用户主题标识（即 Token 的 `sub`）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 水位线写入成功
    /// - `Err(BaseError::RedisOperationFailed)`: Redis 写入失败
    ///
    /// # 依赖
    ///
    /// 依赖 [`crate::database::GlobalRedis`] 已初始化可用。
    pub async fn revoke_by_subject(&self, sub: &str) -> Result<(), BaseError> {
        let now = current_unix_timestamp()?;
        let ttl = self.refresh_token_expiry();
        GlobalRedis::set(subject_min_iat_key(sub), now.to_string(), Some(ttl as i64)).await?;
        Ok(())
    }

    /// 查询某用户的“最小有效签发时间”水位线（若不存在返回 `None`）。
    ///
    /// # 参数
    ///
    /// - `sub`: 用户主题标识
    ///
    /// # 返回
    ///
    /// - `Ok(Some(ts))`: 该用户已被批量撤销，水位线时间戳为 `ts`
    /// - `Ok(None)`: 该用户无批量撤销记录
    /// - `Err(BaseError::RedisOperationFailed)`: Redis 查询失败
    pub async fn subject_min_iat(&self, sub: &str) -> Result<Option<u64>, BaseError> {
        match GlobalRedis::get(subject_min_iat_key(sub)).await? {
            // 解析失败视为无水位线，避免因脏数据误杀合法 Token
            Some(raw) => Ok(raw.parse::<u64>().ok()),
            None => Ok(None),
        }
    }

    /// 尝试原子撤销一次（SET key val NX EX ttl）。
    ///
    /// 与 `revoke_claims`（无条件 SETEX）不同，本方法仅在黑名单中尚不存在该 jti
    /// 时执行写入。为轮换流程 [`TokenManager::rotate_refresh_token`] 提供竞态保护：
    /// 并发请求中仅第一个能成功写入，后续返回 `false`。
    ///
    /// # 参数
    ///
    /// - `jti`: Token 唯一标识
    /// - `ttl`: 黑名单过期时间（秒），源自 Token 剩余有效期。为 `0` 时直接返回
    ///   `false`，避免向 Redis 发送 `EX 0`（非法参数）
    ///
    /// # 返回
    ///
    /// - `Ok(true)`: 成功将 jti 写入黑名单（本次是第一个到达的）
    /// - `Ok(false)`: jti 已在黑名单中（竞态中落败），或 ttl 为 0（Token 已过期）
    pub(crate) async fn try_revoke_once(jti: &str, ttl: u64) -> Result<bool, BaseError> {
        if ttl == 0 {
            return Ok(false);
        }
        GlobalRedis::set_nx_ex(blacklist_key(jti), "1", ttl as i64).await
    }

    /// 验证 Token，并额外检查黑名单。
    ///
    /// 在 [`TokenManager::verify_token`] 的全部校验之上，再做两层撤销检查：
    /// 1. 查 `jti` 黑名单（单 Token 撤销 / 登出）；
    /// 2. 查该用户的 `min_iat` 水位线（按用户批量撤销 / 改密、强制下线）：
    ///    若存在水位线且 Token 的 `iat` 早于它，则视为已撤销。
    ///
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
    /// - `Err(BaseError::TokenRevoked)`: Token 已被撤销（命中黑名单或早于用户水位线）
    /// - `Err(BaseError::RedisOperationFailed)`: 黑名单查询失败
    pub async fn verify_token_checked(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let claims = self.verify_token(token)?;
        // 第一层：单 Token 黑名单（登出）
        if self.is_revoked(&claims.jti).await? {
            return Err(BaseError::TokenRevoked);
        }
        // 第二层：按用户水位线（改密、强制下线）。Token 在水位线当秒或之前签发即视为已撤销。
        if let Some(min_iat) = self.subject_min_iat(&claims.sub).await? {
            if iat_revoked_by_watermark(claims.iat, min_iat) {
                return Err(BaseError::TokenRevoked);
            }
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

    #[test]
    fn test_subject_min_iat_key_format() {
        assert_eq!(subject_min_iat_key("user_42"), "token:user:user_42:min_iat");
    }

    /// NEW-6：同秒签发（iat == min_iat）必须被撤销，杜绝 1 秒旁路窗口。
    #[test]
    fn test_iat_revoked_by_watermark_same_second() {
        // 早于水位线：撤销
        assert!(iat_revoked_by_watermark(100, 200));
        // 同秒签发：必须撤销（含等号，这是修复点）
        assert!(iat_revoked_by_watermark(200, 200));
        // 晚于水位线：放行
        assert!(!iat_revoked_by_watermark(201, 200));
    }
}
