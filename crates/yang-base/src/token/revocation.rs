//! Token 撤销 / 黑名单机制（H-4）
//!
//! JWT 一经签发，在过期前本身无法失效。对登出、改密、强制下线等场景，
//! 需要一个额外的撤销层。本模块基于 `ToolsBuilder` 显式注入的 Redis 客户端提供两层撤销能力：
//!
//! 1. **按单个 Token 撤销（jti 黑名单）**：用于登出。撤销时把 Token 的 `jti`
//!    写入 Redis，TTL 设为 Token 剩余有效期（过期后 key 自动消失，黑名单不会
//!    无限增长）。校验时在标准签名/过期校验之外，额外查一次该黑名单。
//! 2. **按用户批量撤销（subject 时间水位线）**：用于改密、强制下线。撤销时把
//!    "当前时间戳"写入 `token:user:{hex(sub)}:min_iat`（sub 经 hex 编码，避免特殊
//!    字符导致 Redis key 歧义），TTL 取 Refresh Token 有效期。
//!    校验时若该用户存在水位线且 Token 的 `iat` 早于或等于水位线，则视为已撤销——
//!    一次写入即可让该用户在此之前签发的所有 Token 全部失效。
//!
//! 设计约束：**不修改** [`TokenManager::verify_token`] 的现有签名与行为，
//! 撤销校验通过 [`TokenManager::verify_token_checked`] 提供，保持向后兼容。

use crate::error::BaseError;
use crate::token::manager::current_unix_timestamp;
use crate::token::{TokenClaims, TokenManager};
use yang_db::RedisValue;

/// Redis 黑名单 key 前缀。最终 key 形如 `token:blacklist:{jti}`。
const BLACKLIST_PREFIX: &str = "token:blacklist:";

/// 根据 jti 构造黑名单 Redis key。
fn blacklist_key(jti: &str) -> String {
    format!("{BLACKLIST_PREFIX}{jti}")
}

/// 按用户撤销的时间水位线 key 前缀。最终 key 形如 `token:user:{hex(sub)}:min_iat`。
const SUBJECT_MIN_IAT_PREFIX: &str = "token:user:";

/// 将字节切片编码为小写 hex 字符串（无外部依赖）。
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 根据 subject 构造"最小签发时间水位线"Redis key。
///
/// `sub` 经 hex 编码后嵌入 key，避免含 `:` 等特殊字符导致 Redis key 歧义。
fn subject_min_iat_key(sub: &str) -> String {
    format!(
        "{SUBJECT_MIN_IAT_PREFIX}{}:min_iat",
        hex_encode(sub.as_bytes())
    )
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

fn invalid_revocation_state(message: &str) -> BaseError {
    BaseError::TokenRevocationStateInvalid(message.to_string())
}

/// 解析撤销水位线的唯一入口。损坏原值不进入错误文本，避免日志注入和敏感数据回显。
fn parse_subject_min_iat(raw: &str) -> Result<u64, BaseError> {
    raw.parse::<u64>()
        .map_err(|_| invalid_revocation_state("用户撤销水位线不是有效的 u64 时间戳"))
}

/// 解析 Redis GET 水位线结果；只有 Nil 和 UTF-8 String 属于合法协议形态。
fn parse_subject_min_iat_value(value: &RedisValue) -> Result<Option<u64>, BaseError> {
    match value {
        RedisValue::Nil => Ok(None),
        RedisValue::String(raw) => parse_subject_min_iat(raw).map(Some),
        _ => Err(invalid_revocation_state(
            "用户撤销水位线的 Redis 类型不是 nil/string",
        )),
    }
}

/// 解析单条 GET pipeline 结果，避免高层字符串 API 将非 UTF-8 值折叠为 None。
fn parse_subject_min_iat_pipeline_results(
    results: &[RedisValue],
) -> Result<Option<u64>, BaseError> {
    let [value] = results else {
        return Err(invalid_revocation_state(
            "用户撤销水位线 GET pipeline 返回数量不是 1",
        ));
    };
    parse_subject_min_iat_value(value)
}

/// 解析固定的 EXISTS + GET pipeline 结果，任何形态歧义均失败关闭。
fn parse_revocation_pipeline_results(
    results: &[RedisValue],
) -> Result<(bool, Option<u64>), BaseError> {
    let [blacklist_result, watermark_result] = results else {
        return Err(invalid_revocation_state(
            "Token 撤销查询 pipeline 返回数量不是 2",
        ));
    };
    let blacklisted = match blacklist_result {
        RedisValue::Int(0) => false,
        RedisValue::Int(1) => true,
        _ => {
            return Err(invalid_revocation_state(
                "Token 黑名单 EXISTS 结果不是 0/1 整数",
            ))
        }
    };
    Ok((blacklisted, parse_subject_min_iat_value(watermark_result)?))
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

        self.revocation_cache()?
            .setex(blacklist_key(&claims.jti), ttl as i64, "1")
            .await
            .map_err(BaseError::RedisOperationFailed)?;
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
        let count = self
            .revocation_cache()?
            .exists(&[blacklist_key(jti)])
            .await
            .map_err(BaseError::RedisOperationFailed)?;
        Ok(count > 0)
    }

    /// 按用户（subject）批量撤销该用户在此刻之前签发的所有 Token（token-3）。
    ///
    /// 在 Redis 中将 `token:user:{hex(sub)}:min_iat` 写为当前时间戳，作为该用户的
    /// "最小有效签发时间"水位线。此后 [`TokenManager::verify_token_checked`]
    /// 会拒绝任何 `iat` 早于或等于该水位线的 Token，从而让此前签发的全部 Token 一次性失效。
    ///
    /// 适用于**改密、强制下线**等需要让某用户全部会话立即失效的场景。
    /// 水位线 TTL 取 Refresh Token 有效期（`TokenManager::refresh_token_expiry`），
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
    /// 依赖 `ToolsBuilder` 已为 TokenManager 连接 Redis 撤销存储。
    pub async fn revoke_by_subject(&self, sub: &str) -> Result<(), BaseError> {
        let now = current_unix_timestamp()?;
        let ttl = self.refresh_token_expiry();
        self.revocation_cache()?
            .setex(subject_min_iat_key(sub), ttl as i64, now.to_string())
            .await
            .map_err(BaseError::RedisOperationFailed)?;
        Ok(())
    }

    /// 查询某用户的"最小有效签发时间"水位线（若不存在返回 `None`）。
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
    /// - `Err(BaseError::TokenRevocationStateInvalid)`: 水位线值损坏
    pub async fn subject_min_iat(&self, sub: &str) -> Result<Option<u64>, BaseError> {
        let mut pipeline = self.revocation_cache()?.pipeline();
        pipeline.get(subject_min_iat_key(sub));
        let results = pipeline
            .execute()
            .await
            .map_err(BaseError::RedisOperationFailed)?;
        parse_subject_min_iat_pipeline_results(&results)
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
    pub(crate) async fn try_revoke_once(&self, jti: &str, ttl: u64) -> Result<bool, BaseError> {
        if ttl == 0 {
            return Ok(false);
        }
        self.revocation_cache()?
            .set_nx_ex(blacklist_key(jti), "1", ttl as i64)
            .await
            .map_err(BaseError::RedisOperationFailed)
    }

    /// 验证 Token，并额外检查黑名单。
    ///
    /// 在 [`TokenManager::verify_token`] 的全部校验之上，再做两层撤销检查：
    /// 1. 查 `jti` 黑名单（单 Token 撤销 / 登出）；
    /// 2. 查该用户的 `min_iat` 水位线（按用户批量撤销 / 改密、强制下线）：
    ///    若存在水位线且 Token 的 `iat` 早于或等于它，则视为已撤销。
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
    /// - `Err(BaseError::TokenRevoked)`: Token 已被撤销（命中黑名单或不晚于用户水位线）
    /// - `Err(BaseError::RedisOperationFailed)`: 黑名单查询失败
    /// - `Err(BaseError::TokenRevocationStateInvalid)`: 撤销查询结果或水位线损坏
    pub async fn verify_token_checked(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let claims = self.verify_token(token)?;

        // PERF-2: 将两次 Redis 读（EXISTS + GET）合并为一条 pipeline，2 RTT → 1 RTT。
        // 对已黑名单 token 会失去短路（不再提前返回），但撤销场景罕见，可接受。
        let mut pipeline = self.revocation_cache()?.pipeline();
        pipeline
            .exists(blacklist_key(&claims.jti))
            .get(subject_min_iat_key(&claims.sub));
        let results = pipeline
            .execute()
            .await
            .map_err(BaseError::RedisOperationFailed)?;

        let (blacklisted, min_iat) = parse_revocation_pipeline_results(&results)?;
        if blacklisted {
            return Err(BaseError::TokenRevoked);
        }

        if min_iat.is_some_and(|watermark| iat_revoked_by_watermark(claims.iat, watermark)) {
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

    #[test]
    fn test_subject_min_iat_key_format() {
        // sub 经 hex 编码，避免特殊字符导致 key 歧义
        assert_eq!(
            subject_min_iat_key("user_42"),
            "token:user:757365725f3432:min_iat"
        );
    }

    #[test]
    fn test_subject_min_iat_key_colon_safe() {
        // sub 含冒号时 hex 编码后不再产生歧义
        let key = subject_min_iat_key("user:admin");
        assert_eq!(key, "token:user:757365723a61646d696e:min_iat");
        // 确保 key 中不会出现未转义的冒号（除固定分隔符外）
        let after_prefix = key.strip_prefix("token:user:").unwrap();
        let (hex_part, rest) = after_prefix.split_once(':').unwrap();
        assert_eq!(rest, "min_iat");
        // hex 部分只含 0-9a-f
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
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

    #[test]
    fn watermark_parser_accepts_only_absent_or_valid_u64_string() {
        assert_eq!(
            parse_subject_min_iat_value(&yang_db::RedisValue::Nil).expect("缺少水位线是合法状态"),
            None
        );
        assert_eq!(
            parse_subject_min_iat_value(&yang_db::RedisValue::String("0".to_string()))
                .expect("u64 下界应有效"),
            Some(0)
        );
        assert_eq!(
            parse_subject_min_iat_value(&yang_db::RedisValue::String(u64::MAX.to_string()))
                .expect("u64 上界应有效"),
            Some(u64::MAX)
        );
    }

    #[test]
    fn watermark_parser_rejects_corrupt_strings_and_wrong_redis_types() {
        let invalid_values = [
            yang_db::RedisValue::String(String::new()),
            yang_db::RedisValue::String("-1".to_string()),
            yang_db::RedisValue::String("18446744073709551616".to_string()),
            yang_db::RedisValue::String("secret-corrupt-watermark".to_string()),
            yang_db::RedisValue::Bytes(vec![0xff]),
            yang_db::RedisValue::Int(42),
            yang_db::RedisValue::Bool(false),
            yang_db::RedisValue::Array(Vec::new()),
        ];

        for value in invalid_values {
            let error = parse_subject_min_iat_value(&value)
                .expect_err("损坏或错误类型的水位线必须失败关闭");
            assert!(
                matches!(&error, BaseError::TokenRevocationStateInvalid(_)),
                "损坏水位线必须返回结构化错误，实际为: {error:?}"
            );
            assert_eq!(error.code(), 400008);
            assert!(error.is_server_error());
            assert!(
                !error.to_string().contains("secret-corrupt-watermark"),
                "错误不得回显 Redis 中的损坏原值"
            );
        }
    }

    #[test]
    fn subject_watermark_pipeline_parser_requires_exactly_one_result() {
        for values in [
            vec![],
            vec![yang_db::RedisValue::Nil, yang_db::RedisValue::Nil],
        ] {
            assert!(
                matches!(
                    parse_subject_min_iat_pipeline_results(&values),
                    Err(BaseError::TokenRevocationStateInvalid(_))
                ),
                "水位线 GET 必须恰好返回一个结果: {values:?}"
            );
        }

        assert_eq!(
            parse_subject_min_iat_pipeline_results(&[yang_db::RedisValue::Nil])
                .expect("缺少水位线是合法状态"),
            None
        );
        assert_eq!(
            parse_subject_min_iat_pipeline_results(&[yang_db::RedisValue::String(
                "42".to_string()
            )])
            .expect("合法水位线应通过"),
            Some(42)
        );
    }

    #[test]
    fn revocation_pipeline_parser_rejects_ambiguous_results() {
        for values in [
            vec![],
            vec![yang_db::RedisValue::Int(0)],
            vec![
                yang_db::RedisValue::Int(0),
                yang_db::RedisValue::Nil,
                yang_db::RedisValue::Nil,
            ],
            vec![
                yang_db::RedisValue::String("0".to_string()),
                yang_db::RedisValue::Nil,
            ],
            vec![yang_db::RedisValue::Int(-1), yang_db::RedisValue::Nil],
            vec![yang_db::RedisValue::Int(2), yang_db::RedisValue::Nil],
        ] {
            assert!(
                matches!(
                    parse_revocation_pipeline_results(&values),
                    Err(BaseError::TokenRevocationStateInvalid(_))
                ),
                "不明确的撤销查询结果必须失败关闭: {values:?}"
            );
        }

        assert_eq!(
            parse_revocation_pipeline_results(&[
                yang_db::RedisValue::Int(0),
                yang_db::RedisValue::String("42".to_string()),
            ])
            .expect("固定合法结果应通过"),
            (false, Some(42))
        );
        assert_eq!(
            parse_revocation_pipeline_results(&[
                yang_db::RedisValue::Int(1),
                yang_db::RedisValue::Nil,
            ])
            .expect("黑名单命中且无水位线是合法状态"),
            (true, None)
        );
    }
}
