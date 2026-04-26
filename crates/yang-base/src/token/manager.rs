//! Token 管理器实现
//!
//! 提供 JWT Token 的生成、验证、解析和刷新功能。

use crate::error::BaseError;
use crate::token::TokenClaims;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::time::{SystemTime, UNIX_EPOCH};

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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

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
            .map_err(|e| BaseError::TokenGenerateFailed(e.to_string()))
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

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
            .map_err(|e| BaseError::TokenGenerateFailed(e.to_string()))
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
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.leeway = 0; // 不允许时间容差

        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| BaseError::TokenVerifyFailed(e.to_string()))?;

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
        let mut validation = Validation::new(self.algorithm);
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false; // 不验证受众
        validation.set_required_spec_claims::<&str>(&[]); // 不验证任何必需声明

        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| BaseError::TokenParseFailed(e.to_string()))?;

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

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

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
impl std::fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenManager")
            .field("algorithm", &self.algorithm)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("access_token_expiry", &self.access_token_expiry)
            .field("refresh_token_expiry", &self.refresh_token_expiry)
            .finish()
    }
}
