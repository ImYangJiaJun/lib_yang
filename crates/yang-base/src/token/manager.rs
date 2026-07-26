//! Token 管理器实现
//!
//! 提供 JWT Token 的生成、验证、解析和刷新功能。

use crate::error::BaseError;
use crate::token::TokenClaims;
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use yang_db::RedisClient;

const MAX_SYMMETRIC_KEYRING_KEYS: usize = 8;
const MAX_KEY_ID_BYTES: usize = 64;
const MIN_HMAC_SECRET_BYTES: usize = 32;

/// 获取当前 Unix 时间戳（秒）
///
/// # 返回
///
/// - `Ok(u64)`: 当前 Unix 时间戳（秒）
/// - `Err(BaseError::ConfigError)`: 系统时钟早于 UNIX_EPOCH（时钟异常）
///
/// # 错误
///
/// 当系统时钟早于 1970-01-01 00:00:00 UTC 时返回错误，
/// 这通常意味着系统时钟配置异常。
pub(crate) fn current_unix_timestamp() -> Result<u64, BaseError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| BaseError::ConfigError(format!("系统时钟异常，早于 UNIX_EPOCH: {}", e)))
}

/// 构建 Token 验证规则（[`Validation`]）。
///
/// 在构造 [`TokenManager`] 时一次性生成并缓存，避免每次 [`TokenManager::verify_token`]
/// 调用都重新分配算法白名单、签发者/受众集合与必需声明集合（token-10 优化）。
///
/// 规则与历史行为保持一致：
/// - 算法白名单仅含构造时指定的单一算法，防止算法混淆攻击（如 `none` 算法绕过）。
/// - 显式校验签发者与受众。
/// - 要求 `exp`、`iss`、`aud` 三个标准声明必须存在。
/// - `leeway = 0`，不允许时间容差。
fn build_validation(algorithm: Algorithm, issuer: &str, audience: &str) -> Validation {
    let mut validation = Validation::new(algorithm);
    validation.algorithms = vec![algorithm];
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    let mut required_claims = HashSet::new();
    required_claims.insert("exp".to_string());
    required_claims.insert("iss".to_string());
    required_claims.insert("aud".to_string());
    validation.required_spec_claims = required_claims;
    validation.leeway = 0;
    validation
}

enum VerificationKeys {
    /// 兼容既有调用方，并保留无需解析 Header 的单密钥验证快路径。
    Single(DecodingKey),
    /// `kid` 到验证密钥的一对一映射；只在显式 keyring 模式启用。
    Keyring(HashMap<String, DecodingKey>),
}

fn validate_hmac_algorithm(algorithm: Algorithm) -> Result<(), BaseError> {
    if !matches!(
        algorithm,
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512
    ) {
        return Err(BaseError::TokenKeyInvalid(
            "对称 Token keyring 仅支持 HS256、HS384、HS512".to_string(),
        ));
    }
    Ok(())
}

fn validate_key_id(key_id: &str) -> Result<(), BaseError> {
    if key_id.is_empty()
        || key_id.len() > MAX_KEY_ID_BYTES
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(BaseError::TokenKeyInvalid(
            "kid 必须是 1..=64 字节的 ASCII 字母、数字、点、下划线或连字符".to_string(),
        ));
    }
    Ok(())
}

fn validate_hmac_secret(secret: &str) -> Result<(), BaseError> {
    if secret.len() < MIN_HMAC_SECRET_BYTES {
        return Err(BaseError::TokenKeyInvalid(format!(
            "HMAC secret 至少需要 {MIN_HMAC_SECRET_BYTES} 字节"
        )));
    }
    Ok(())
}

fn invalid_key_selection() -> BaseError {
    BaseError::TokenVerifyFailed(jsonwebtoken::errors::Error::from(
        jsonwebtoken::errors::ErrorKind::InvalidToken,
    ))
}

/// Token 管理器
///
/// 提供 JWT Token 的生成、验证、解析和刷新功能。
///
/// # 支持的算法
///
/// - 对称加密：HS256、HS384、HS512
/// - 非对称加密：RS256、RS384、RS512
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::token::TokenManager;
/// use jsonwebtoken::Algorithm;
///
/// // 创建对称加密的 Token 管理器
/// let manager = TokenManager::new_symmetric(
///     "your_secret_key",
///     Algorithm::HS256,
///     "issuer".to_string(),
///     "audience".to_string(),
///     3600,    // Access Token 有效期 1 小时
///     86400,   // Refresh Token 有效期 1 天
/// );
///
/// // 生成 Access Token
/// let token = manager.generate_access_token(
///     "user_123",
///     serde_json::json!({"role": "admin"}),
/// )?;
///
/// // 验证 Token
/// let claims = manager.verify_token(&token)?;
/// println!("用户 ID: {}", claims.sub);
/// ```
pub struct TokenManager {
    /// 编码密钥
    encoding_key: EncodingKey,

    /// 单密钥快路径或按 `kid` 索引的验证 keyring。
    verification_keys: VerificationKeys,

    /// 算法
    algorithm: Algorithm,

    /// 签发者
    issuer: String,

    /// 受众
    audience: String,

    /// Access Token 有效期（秒）
    access_token_expiry: u64,

    /// Refresh Token 有效期（秒）
    refresh_token_expiry: u64,

    /// 缓存的验证规则
    ///
    /// 在构造时由 [`build_validation`] 生成，[`TokenManager::verify_token`]
    /// 直接复用，避免每次验证重复分配算法白名单与声明集合（token-10 优化）。
    validation: Validation,

    /// 预构建的 JWT Header
    ///
    /// 在构造时一次性设置 `typ: "JWT"` 与对应算法，生成 Token 时直接
    /// 传引用，避免每次 `generate_*` 都重新分配 Header 与 `"JWT"` String
    /// （PERF-11 优化）。
    jwt_header: Header,

    /// 显式注入的撤销存储；由 `ToolsBuilder` 在启动期连接。
    revocation_cache: Option<RedisClient>,
}

impl TokenManager {
    /// 创建新的 Token 管理器（对称加密）
    ///
    /// # 参数
    ///
    /// - `secret`: 密钥字符串
    /// - `algorithm`: 加密算法（HS256、HS384、HS512）
    /// - `issuer`: 签发者标识
    /// - `audience`: 受众标识
    /// - `access_token_expiry`: Access Token 有效期（秒）
    /// - `refresh_token_expiry`: Refresh Token 有效期（秒）
    ///
    /// # 返回
    ///
    /// - `TokenManager`: Token 管理器实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::token::TokenManager;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let manager = TokenManager::new_symmetric(
    ///     "my_secret_key",
    ///     Algorithm::HS256,
    ///     "my_app".to_string(),
    ///     "my_users".to_string(),
    ///     3600,
    ///     86400,
    /// );
    /// ```
    pub fn new_symmetric(
        secret: &str,
        algorithm: Algorithm,
        issuer: String,
        audience: String,
        access_token_expiry: u64,
        refresh_token_expiry: u64,
    ) -> Self {
        // AUTH-8: 白名单拦截非对称算法，避免构造后首次签发才失败
        matches!(
            algorithm,
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512
        )
        .then_some(())
        .expect("new_symmetric 仅支持 HMAC 算法 (HS256/HS384/HS512)，请使用 new_asymmetric 处理非对称算法");

        let mut jwt_header = Header::new(algorithm);
        jwt_header.typ = Some("JWT".to_string());

        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            verification_keys: VerificationKeys::Single(DecodingKey::from_secret(
                secret.as_bytes(),
            )),
            algorithm,
            validation: build_validation(algorithm, &issuer, &audience),
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
            jwt_header,
            revocation_cache: None,
        }
    }

    /// 创建带稳定 `kid` 的对称 Token keyring。
    ///
    /// 新 Token 只由 active key 签发；验证时只按 Header 中的 `kid` 精确选择
    /// active 或 retiring key。keyring 模式不接受缺失/未知 `kid`，避免回退到
    /// “逐把试签名”的模糊验证链。retiring key 应至少保留一个 Refresh Token
    /// 最大有效期，再由运维显式移除。
    #[allow(clippy::too_many_arguments)]
    pub fn new_symmetric_keyring(
        active_key_id: String,
        active_secret: &str,
        retiring_keys: Vec<(String, String)>,
        algorithm: Algorithm,
        issuer: String,
        audience: String,
        access_token_expiry: u64,
        refresh_token_expiry: u64,
    ) -> Result<Self, BaseError> {
        validate_hmac_algorithm(algorithm)?;
        validate_key_id(&active_key_id)?;
        validate_hmac_secret(active_secret)?;
        if retiring_keys.len() + 1 > MAX_SYMMETRIC_KEYRING_KEYS {
            return Err(BaseError::TokenKeyInvalid(format!(
                "Token keyring 最多允许 {MAX_SYMMETRIC_KEYRING_KEYS} 把密钥"
            )));
        }

        let mut verification_keys = HashMap::with_capacity(retiring_keys.len() + 1);
        verification_keys.insert(
            active_key_id.clone(),
            DecodingKey::from_secret(active_secret.as_bytes()),
        );
        for (key_id, secret) in retiring_keys {
            validate_key_id(&key_id)?;
            validate_hmac_secret(&secret)?;
            if verification_keys
                .insert(key_id, DecodingKey::from_secret(secret.as_bytes()))
                .is_some()
            {
                return Err(BaseError::TokenKeyInvalid(
                    "Token keyring 的 kid 必须唯一".to_string(),
                ));
            }
        }

        let mut jwt_header = Header::new(algorithm);
        jwt_header.typ = Some("JWT".to_string());
        jwt_header.kid = Some(active_key_id);

        Ok(Self {
            encoding_key: EncodingKey::from_secret(active_secret.as_bytes()),
            verification_keys: VerificationKeys::Keyring(verification_keys),
            algorithm,
            validation: build_validation(algorithm, &issuer, &audience),
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
            jwt_header,
            revocation_cache: None,
        })
    }

    /// 创建新的 Token 管理器（非对称加密）
    ///
    /// # 参数
    ///
    /// - `private_key`: 私钥（PEM 格式）
    /// - `public_key`: 公钥（PEM 格式）
    /// - `algorithm`: 加密算法（RS256、RS384、RS512）
    /// - `issuer`: 签发者标识
    /// - `audience`: 受众标识
    /// - `access_token_expiry`: Access Token 有效期（秒）
    /// - `refresh_token_expiry`: Refresh Token 有效期（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(TokenManager)`: Token 管理器实例
    /// - `Err(BaseError)`: 密钥格式无效
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::token::TokenManager;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// let public_key = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----";
    ///
    /// let manager = TokenManager::new_asymmetric(
    ///     private_key,
    ///     public_key,
    ///     Algorithm::RS256,
    ///     "my_app".to_string(),
    ///     "my_users".to_string(),
    ///     3600,
    ///     86400,
    /// )?;
    /// ```
    pub fn new_asymmetric(
        private_key: &str,
        public_key: &str,
        algorithm: Algorithm,
        issuer: String,
        audience: String,
        access_token_expiry: u64,
        refresh_token_expiry: u64,
    ) -> Result<Self, BaseError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key.as_bytes())
            .map_err(|e| BaseError::TokenKeyInvalid(format!("私钥无效: {}", e)))?;

        let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
            .map_err(|e| BaseError::TokenKeyInvalid(format!("公钥无效: {}", e)))?;

        let mut jwt_header = Header::new(algorithm);
        jwt_header.typ = Some("JWT".to_string());

        Ok(Self {
            encoding_key,
            verification_keys: VerificationKeys::Single(decoding_key),
            algorithm,
            validation: build_validation(algorithm, &issuer, &audience),
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
            jwt_header,
            revocation_cache: None,
        })
    }

    /// 由应用资源构建器注入 Token 撤销所使用的 Redis 客户端。
    pub(crate) fn attach_revocation_cache(&mut self, cache: RedisClient) {
        self.revocation_cache = Some(cache);
    }

    /// 返回显式配置的 Token 撤销存储。
    pub(crate) fn revocation_cache(&self) -> Result<&RedisClient, BaseError> {
        self.revocation_cache
            .as_ref()
            .ok_or(BaseError::RedisNotInitialized)
    }

    /// 生成 Access Token
    ///
    /// # 参数
    ///
    /// - `subject`: 主题（通常是用户 ID）
    /// - `custom_claims`: 自定义声明（JSON 格式）
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: Token 字符串
    /// - `Err(BaseError)`: 生成失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let token = manager.generate_access_token(
    ///     "user_123",
    ///     serde_json::json!({
    ///         "role": "admin",
    ///         "permissions": ["read", "write"]
    ///     }),
    /// )?;
    /// ```
    pub fn generate_access_token(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        let now = current_unix_timestamp()?;
        self.generate_access_token_at(subject, custom_claims, now)
    }

    /// 生成 Refresh Token
    ///
    /// # 参数
    ///
    /// - `subject`: 主题（通常是用户 ID）
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: Token 字符串
    /// - `Err(BaseError)`: 生成失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let refresh_token = manager.generate_refresh_token("user_123")?;
    /// ```
    pub fn generate_refresh_token(&self, subject: &str) -> Result<String, BaseError> {
        let now = current_unix_timestamp()?;
        self.generate_refresh_token_at(subject, serde_json::Value::Null, now)
    }

    /// 生成携带自定义声明的 Refresh Token。
    ///
    /// Refresh Token 通常只应携带版本号等刷新流程必需的最小声明，不应复制
    /// Access Token 中的角色、权限等高频变化信息。
    pub fn generate_refresh_token_with_claims(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        let now = current_unix_timestamp()?;
        self.generate_refresh_token_at(subject, custom_claims, now)
    }

    /// 生成 Token 对（Access Token + Refresh Token）
    ///
    /// # 参数
    ///
    /// - `subject`: 主题（通常是用户 ID）
    /// - `custom_claims`: 自定义声明（JSON 格式）
    ///
    /// # 返回
    ///
    /// - `Ok((access_token, refresh_token))`: Token 对
    /// - `Err(BaseError)`: 生成失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let (access_token, refresh_token) = manager.generate_token_pair(
    ///     "user_123",
    ///     serde_json::json!({"role": "admin"}),
    /// )?;
    /// ```
    pub fn generate_token_pair(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        self.generate_token_pair_with_refresh_claims(
            subject,
            custom_claims,
            serde_json::Value::Null,
        )
    }

    /// 生成分别携带自定义声明的 Token 对。
    ///
    /// `access_claims` 与 `refresh_claims` 必须由同一次业务授权快照派生，避免
    /// 同一 Token 对内部出现版本撕裂。调用方应保持 Refresh 声明最小化。
    pub fn generate_token_pair_with_refresh_claims(
        &self,
        subject: &str,
        access_claims: serde_json::Value,
        refresh_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        let now = current_unix_timestamp()?;
        self.generate_token_pair_at(subject, access_claims, refresh_claims, now)
    }

    /// 使用预取时间戳生成 Token 对（内部方法，PERF-10 优化）。
    ///
    /// 与 [`generate_token_pair`] 功能相同，但接受外部传入的 `now` 时间戳，
    /// 避免 access/refresh 各调一次 `current_unix_timestamp()` 系统调用。
    ///
    /// 供 [`rotate_refresh_token_from_claims`] 等已持有时间戳的调用方使用。
    pub(crate) fn generate_token_pair_at(
        &self,
        subject: &str,
        access_claims: serde_json::Value,
        refresh_claims: serde_json::Value,
        now: u64,
    ) -> Result<(String, String), BaseError> {
        let access_token = self.generate_access_token_at(subject, access_claims, now)?;
        let refresh_token = self.generate_refresh_token_at(subject, refresh_claims, now)?;

        Ok((access_token, refresh_token))
    }

    /// 使用预取时间戳生成 Access Token（内部方法，PERF-10 优化）。
    ///
    /// 与 [`generate_access_token`] 功能相同，但接受外部传入的 `now` 时间戳。
    fn generate_access_token_at(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
        now: u64,
    ) -> Result<String, BaseError> {
        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.access_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: crate::token::TokenType::Access,
            custom: custom_claims,
        };

        encode(&self.jwt_header, &claims, &self.encoding_key)
            .map_err(BaseError::TokenGenerateFailed)
    }

    /// 使用预取时间戳生成 Refresh Token（内部方法，PERF-10 优化）。
    ///
    /// 与 [`generate_refresh_token`] 功能相同，但接受外部传入的 `now` 时间戳。
    fn generate_refresh_token_at(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
        now: u64,
    ) -> Result<String, BaseError> {
        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.refresh_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: crate::token::TokenType::Refresh,
            custom: custom_claims,
        };

        encode(&self.jwt_header, &claims, &self.encoding_key)
            .map_err(BaseError::TokenGenerateFailed)
    }

    /// 返回 Refresh Token 的有效期（秒）。
    ///
    /// 供同 crate 的撤销层（`revocation`）复用：按用户批量撤销时以此作为
    /// `min_iat` 标记的 TTL，避免标记无限增长。
    pub(crate) fn refresh_token_expiry(&self) -> u64 {
        self.refresh_token_expiry
    }

    /// 验证 Token
    ///
    /// 验证 Token 的签名、过期时间、签发者和受众。
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    ///
    /// # 返回
    ///
    /// - `Ok(TokenClaims)`: Token 声明
    /// - `Err(BaseError::TokenExpired)`: Token 已过期
    /// - `Err(BaseError::TokenVerifyFailed)`: 签名或声明校验失败
    /// - `Err(BaseError::TokenParseFailed)`: Token 解析失败
    ///
    /// 错误分类由 [`From<jsonwebtoken::errors::Error>`] 自动分流：
    /// `ExpiredSignature` → `TokenExpired`，`InvalidToken`/`InvalidSignature` → `TokenVerifyFailed`，
    /// 其他 → `TokenParseFailed`。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let claims = manager.verify_token(&token)?;
    /// println!("用户 ID: {}", claims.sub);
    /// println!("Token 类型: {}", claims.token_type);
    /// ```
    pub fn verify_token(&self, token: &str) -> Result<TokenClaims, BaseError> {
        // 复用构造时缓存的 Validation（token-10），避免每次验证重复分配
        // 算法白名单、签发者/受众集合与必需声明集合。
        let decoding_key = match &self.verification_keys {
            VerificationKeys::Single(key) => key,
            VerificationKeys::Keyring(keys) => {
                let header = decode_header(token).map_err(BaseError::TokenParseFailed)?;
                if header.alg != self.algorithm {
                    return Err(invalid_key_selection());
                }
                let key_id = header.kid.as_deref().ok_or_else(invalid_key_selection)?;
                keys.get(key_id).ok_or_else(invalid_key_selection)?
            }
        };
        let token_data = decode::<TokenClaims>(token, decoding_key, &self.validation)?;

        Ok(token_data.claims)
    }

    /// 解析 Token（不验证签名）
    ///
    /// ⚠️ **安全警告：此方法不验证签名、不检查过期、不校验签发者/受众。
    /// 攻击者可伪造任意 JWT。严禁在鉴权/授权决策中使用此方法的返回值。**
    ///
    /// 用于调试或获取 Token 信息而不验证其有效性。
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    ///
    /// # 返回
    ///
    /// - `Ok(TokenClaims)`: Token 声明
    /// - `Err(BaseError)`: 解析失败
    ///
    /// # Safety
    ///
    /// **此方法绝对不能用于鉴权决策。**
    ///
    /// 该方法跳过签名验证、过期时间检查及所有标准声明校验，
    /// 任何人均可伪造 Token 内容并通过此方法解析。
    /// 仅允许在以下场景使用：
    /// - 调试与日志记录（打印 Token 内容）
    /// - 从已过期 Token 中提取用户标识以便刷新
    /// - 单元测试中检查 Token 结构
    ///
    /// 在任何涉及权限判断、身份认证的代码路径中，
    /// 必须使用 [`TokenManager::verify_token`] 代替本方法。
    ///
    /// # 警告
    ///
    /// 此方法不验证签名和过期时间，仅用于调试目的。
    /// 不要在生产环境中使用此方法进行身份验证。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let claims = manager.parse_token_unsafe(&token)?;
    /// println!("Token 内容: {:?}", claims);
    /// ```
    #[deprecated(
        since = "0.1.0",
        note = "此方法跳过所有 JWT 安全验证（签名/过期/签发者），仅可用于调试日志和单元测试。鉴权路径必须使用 verify_token 或 verify_token_checked。"
    )]
    pub fn parse_token_unsafe(&self, token: &str) -> Result<TokenClaims, BaseError> {
        // 先读取 token 自带算法，再显式关闭所有鉴权相关校验。这样仍由 jsonwebtoken
        // 负责 JWT 结构和 claims 反序列化，同时不会把本调试 API 误装成鉴权入口。
        let header = jsonwebtoken::decode_header(token).map_err(BaseError::TokenParseFailed)?;
        let mut validation = jsonwebtoken::Validation::new(header.alg);
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.required_spec_claims.clear();
        let token_data = jsonwebtoken::decode::<TokenClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(&[]),
            &validation,
        )
        .map_err(BaseError::TokenParseFailed)?;

        Ok(token_data.claims)
    }

    /// 检查 Token 是否即将过期
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    /// - `threshold_secs`: 阈值（秒），如果剩余时间少于此值则返回 true
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: 是否即将过期
    /// - `Err(BaseError)`: 检查失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 检查 Token 是否在 5 分钟内过期
    /// if manager.is_token_expiring_soon(&token, 300)? {
    ///     println!("Token 即将过期，需要刷新");
    /// }
    /// ```
    pub fn is_token_expiring_soon(
        &self,
        token: &str,
        threshold_secs: u64,
    ) -> Result<bool, BaseError> {
        let claims = self.verify_token(token)?;

        // 使用辅助函数获取时间戳，避免时钟异常时 panic
        let now = current_unix_timestamp()?;

        let remaining = claims.exp.saturating_sub(now);

        Ok(remaining < threshold_secs)
    }

    /// 使用 Refresh Token 刷新 Access Token
    ///
    /// 此方法会检查 Redis 黑名单，已撤销的 Refresh Token 将被拒绝。
    ///
    /// # 参数
    ///
    /// - `refresh_token`: Refresh Token 字符串
    /// - `custom_claims`: 新的自定义声明
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 新的 Access Token
    /// - `Err(BaseError)`: 刷新失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let new_access_token = manager.refresh_access_token(
    ///     &refresh_token,
    ///     serde_json::json!({"role": "admin"}),
    /// ).await?;
    /// ```
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        // 验证 Refresh Token（含黑名单检查，阻止已撤销的 Token 获取新 Access Token）
        let claims = self.verify_token_checked(refresh_token).await?;

        // 检查 Token 类型
        if claims.token_type != crate::token::TokenType::Refresh {
            return Err(BaseError::TokenTypeInvalid(
                "期望 refresh token".to_string(),
            ));
        }

        // 生成新的 Access Token
        self.generate_access_token(&claims.sub, custom_claims)
    }

    /// 轮换 Refresh Token（刷新令牌轮换 / Refresh Token Rotation，token-1）。
    ///
    /// 每次刷新都签发全新的 Token 对，并立刻把旧 Refresh Token 原子拉黑，
    /// 使其无法再次使用。这能限制刷新令牌泄露的影响面：一旦旧令牌被重放，
    /// 因已进入黑名单而验证失败。
    ///
    /// 注：原子拉黑使用 [`SET key val NX EX ttl`]，竞态安全——
    /// 并发请求中仅第一个能成功写入黑名单，后续返回 [`BaseError::TokenRevoked`]。
    ///
    /// 执行流程：
    /// 1. [`TokenManager::verify_token_checked`] 验证旧 Refresh Token 且确认未被撤销；
    /// 2. 校验其 `token_type` 必须为 `"refresh"`；
    /// 3. `TokenManager::try_revoke_once` 原子写入黑名单（SET NX EX）；
    ///    若返回 false（竞态中落败）则返回 [`BaseError::TokenRevoked`]；
    /// 4. [`TokenManager::generate_token_pair`] 以原 `sub` 与新的自定义声明签发新 Token 对。
    ///
    /// # 参数
    ///
    /// - `old_refresh`: 旧的 Refresh Token 字符串
    /// - `custom_claims`: 新 Access Token 的自定义声明（JSON 格式）
    ///
    /// # 返回
    ///
    /// - `Ok((access_token, refresh_token))`: 新签发的 Token 对
    /// - `Err(BaseError::TokenVerifyFailed)`: 旧 Token 签名/声明校验失败
    /// - `Err(BaseError::TokenExpired)`: 旧 Token 已过期
    /// - `Err(BaseError::TokenRevoked)`: 旧 Token 已被撤销（含竞态中落败）
    /// - `Err(BaseError::TokenTypeInvalid)`: 传入的不是 Refresh Token
    /// - `Err(BaseError::RedisOperationFailed)`: 原子写入黑名单失败
    ///
    /// # 依赖
    ///
    /// 本方法依赖基于 Redis 的 Token 黑名单（见 [`TokenManager::verify_token_checked`]
    /// 与 `TokenManager::try_revoke_once`），调用前需由 `ToolsBuilder` 注入 Redis 撤销存储。
    pub async fn rotate_refresh_token(
        &self,
        old_refresh: &str,
        custom_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        self.rotate_refresh_token_with_refresh_claims(
            old_refresh,
            custom_claims,
            serde_json::Value::Null,
        )
        .await
    }

    /// 轮换 Refresh Token，并分别设置新 Access/Refresh Token 的自定义声明。
    pub async fn rotate_refresh_token_with_refresh_claims(
        &self,
        old_refresh: &str,
        access_claims: serde_json::Value,
        refresh_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        // 1. 验证旧 Refresh Token 且确认未被撤销
        let old_claims = self.verify_token_checked(old_refresh).await?;

        // 2~4：委托已验证 claims 的版本，避免重复逻辑
        self.rotate_refresh_token_from_claims_with_refresh_claims(
            &old_claims,
            access_claims,
            refresh_claims,
        )
        .await
    }

    /// 基于已验证 claims 轮换 Refresh Token（跳过内部二次验证）。
    ///
    /// 当调用方已在外部完成 [`TokenManager::verify_token_checked`] 并持有 [`TokenClaims`] 时，
    /// 使用本方法可避免重复验证（节省 2 次 Redis RTT：黑名单查询 + 水位线查询）。
    ///
    /// 本方法仅执行：
    /// 1. 校验 `token_type` 必须为 `"refresh"`；
    /// 2. 原子拉黑旧 Refresh Token（`SET NX EX`）；
    /// 3. 以原 `sub` 与新自定义声明签发新 Token 对。
    ///
    /// # 参数
    ///
    /// - `old_claims`: 已通过 [`TokenManager::verify_token_checked`] 验证的旧 Refresh Token 声明
    /// - `custom_claims`: 新 Access Token 的自定义声明（JSON 格式）
    ///
    /// # 返回
    ///
    /// - `Ok((access_token, refresh_token))`: 新签发的 Token 对
    /// - `Err(BaseError::TokenTypeInvalid)`: `old_claims.token_type` 不是 `"refresh"`
    /// - `Err(BaseError::TokenRevoked)`: 旧 Token 已被撤销（竞态中落败）
    /// - `Err(BaseError::RedisOperationFailed)`: 原子写入黑名单失败
    pub async fn rotate_refresh_token_from_claims(
        &self,
        old_claims: &TokenClaims,
        custom_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        self.rotate_refresh_token_from_claims_with_refresh_claims(
            old_claims,
            custom_claims,
            serde_json::Value::Null,
        )
        .await
    }

    /// 基于已验证 claims 轮换 Refresh Token，并分别设置新 Token 对的自定义声明。
    pub async fn rotate_refresh_token_from_claims_with_refresh_claims(
        &self,
        old_claims: &TokenClaims,
        access_claims: serde_json::Value,
        refresh_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        // 1. 校验 Token 类型必须为 refresh（防御性检查）
        if old_claims.token_type != crate::token::TokenType::Refresh {
            return Err(BaseError::TokenTypeInvalid(
                "期望 refresh token".to_string(),
            ));
        }

        // 2. 原子拉黑旧 Refresh Token（SET NX EX），防止并发双重使用
        let now = current_unix_timestamp()?;
        let ttl = old_claims.exp.saturating_sub(now);
        if !self.try_revoke_once(&old_claims.jti, ttl).await? {
            return Err(BaseError::TokenRevoked);
        }

        // 3. 以原 subject 与新的自定义声明签发新的 Token 对
        //    复用已有的 now 时间戳，避免重复系统调用（PERF-10）
        self.generate_token_pair_at(&old_claims.sub, access_claims, refresh_claims, now)
    }
}

// 手动实现 Debug trait，因为 EncodingKey 和 DecodingKey 不支持 Debug。
// 注意：故意不输出任何密钥，只暴露非敏感的 keyring 模式、数量和 active kid。
impl std::fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (verification_mode, verification_key_count) = match &self.verification_keys {
            VerificationKeys::Single(_) => ("single", 1),
            VerificationKeys::Keyring(keys) => ("keyring", keys.len()),
        };
        f.debug_struct("TokenManager")
            .field("algorithm", &self.algorithm)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("access_token_expiry", &self.access_token_expiry)
            .field("refresh_token_expiry", &self.refresh_token_expiry)
            .field("verification_mode", &verification_mode)
            .field("verification_key_count", &verification_key_count)
            .field("active_key_id", &self.jwt_header.kid)
            // validation 仅含算法/签发者/受众/必需声明等非敏感校验规则，可安全输出
            .field("validation", &self.validation)
            // 使用 finish_non_exhaustive() 表明结构体还有其他字段（密钥字段）未输出
            .finish_non_exhaustive()
    }
}
