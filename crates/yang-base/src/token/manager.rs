//! Token 管理器实现
//!
//! 提供 JWT Token 的生成、验证、解析和刷新功能。

use crate::error::BaseError;
use crate::token::TokenClaims;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

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
        .map_err(|e| {
            BaseError::ConfigError(format!(
                "系统时钟异常，早于 UNIX_EPOCH: {}",
                e
            ))
        })
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

    /// 解码密钥
    decoding_key: DecodingKey,

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
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            algorithm,
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
        }
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

        Ok(Self {
            encoding_key,
            decoding_key,
            algorithm,
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
        })
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
        // 使用辅助函数获取时间戳，避免时钟异常时 panic
        let now = current_unix_timestamp()?;

        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.access_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: "access".to_string(),
            custom: custom_claims,
        };

        let mut header = Header::new(self.algorithm);
        header.typ = Some("JWT".to_string());

        encode(&header, &claims, &self.encoding_key)
            .map_err(BaseError::TokenGenerateFailed)
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
        // 使用辅助函数获取时间戳，避免时钟异常时 panic
        let now = current_unix_timestamp()?;

        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.refresh_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: "refresh".to_string(),
            custom: serde_json::Value::Null,
        };

        let mut header = Header::new(self.algorithm);
        header.typ = Some("JWT".to_string());

        encode(&header, &claims, &self.encoding_key)
            .map_err(BaseError::TokenGenerateFailed)
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
        let access_token = self.generate_access_token(subject, custom_claims)?;
        let refresh_token = self.generate_refresh_token(subject)?;

        Ok((access_token, refresh_token))
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
    /// - `Err(BaseError)`: 验证失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let claims = manager.verify_token(&token)?;
    /// println!("用户 ID: {}", claims.sub);
    /// println!("Token 类型: {}", claims.token_type);
    /// ```
    pub fn verify_token(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let mut validation = Validation::new(self.algorithm);
        // 显式设置允许的算法白名单，防止算法混淆攻击（如 none 算法绕过）
        validation.algorithms = vec![self.algorithm];
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        // 显式要求 exp、iss、aud 三个标准声明必须存在
        let mut required_claims = HashSet::new();
        required_claims.insert("exp".to_string());
        required_claims.insert("iss".to_string());
        required_claims.insert("aud".to_string());
        validation.required_spec_claims = required_claims;
        validation.leeway = 0; // 不允许时间容差

        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(BaseError::TokenVerifyFailed)?;

        Ok(token_data.claims)
    }

    /// 解析 Token（不验证签名）
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
    pub fn parse_token_unsafe(&self, token: &str) -> Result<TokenClaims, BaseError> {
        // 使用 dangerous::insecure_decode 进行不安全解析
        // 注意：此方法不验证签名、过期时间等，仅用于调试
        let token_data = jsonwebtoken::dangerous::insecure_decode::<TokenClaims>(token)
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
    /// )?;
    /// ```
    pub fn refresh_access_token(
        &self,
        refresh_token: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        // 验证 Refresh Token
        let claims = self.verify_token(refresh_token)?;

        // 检查 Token 类型
        if claims.token_type != "refresh" {
            return Err(BaseError::TokenTypeInvalid(
                "期望 refresh token".to_string(),
            ));
        }

        // 生成新的 Access Token
        self.generate_access_token(&claims.sub, custom_claims)
    }
}

// 手动实现 Debug trait，因为 EncodingKey 和 DecodingKey 不支持 Debug
// 注意：故意不输出 encoding_key 和 decoding_key，防止密钥泄露到日志或调试输出中
impl std::fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenManager")
            .field("algorithm", &self.algorithm)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("access_token_expiry", &self.access_token_expiry)
            .field("refresh_token_expiry", &self.refresh_token_expiry)
            // 使用 finish_non_exhaustive() 表明结构体还有其他字段（密钥字段）未输出
            .finish_non_exhaustive()
    }
}
