//! JWT Token 管理模块
//!
//! 提供 Token 生成、验证、解析和刷新机制。
//!
//! # 主要组件
//!
//! - `TokenManager`：Token 管理器
//! - `TokenClaims`：Token 声明
//! - `TokenConfig`：Token 配置
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::token::{TokenManager, TokenClaims};
//! use jsonwebtoken::Algorithm;
//!
//! // 创建 Token 管理器（对称加密）
//! let manager = TokenManager::new_symmetric(
//!     "your_secret_key",
//!     Algorithm::HS256,
//!     "issuer".to_string(),
//!     "audience".to_string(),
//!     3600,
//!     86400,
//! );
//!
//! // 生成 Access Token
//! let token = manager.generate_access_token(
//!     "user_id_123",
//!     serde_json::json!({"role": "admin"}),
//! )?;
//!
//! // 验证 Token
//! let claims = manager.verify_token(&token)?;
//! println!("用户 ID: {}", claims.sub);
//! ```

mod manager;
mod revocation;

pub use manager::TokenManager;

use serde::{Deserialize, Serialize};

/// Token 类型枚举
///
/// 封闭集合，仅允许 `Access` 与 `Refresh` 两种值。
/// 序列化为小写字符串 `"access"` / `"refresh"`，与旧版 `String` 字段的 JSON 格式完全兼容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum TokenType {
    /// 访问令牌，用于 API 访问
    Access,
    /// 刷新令牌，用于获取新的访问令牌
    Refresh,
}

/// Token 声明
///
/// JWT Token 的标准声明和自定义声明。
///
/// # 标准声明字段
///
/// - `iss`：签发者（Issuer）
/// - `sub`：主题（Subject），通常是用户 ID
/// - `aud`：受众（Audience）
/// - `exp`：过期时间（Expiration Time），Unix 时间戳
/// - `nbf`：生效时间（Not Before），Unix 时间戳
/// - `iat`：签发时间（Issued At），Unix 时间戳
/// - `jti`：JWT ID，唯一标识符
///
/// # 自定义字段
///
/// - `token_type`：Token 类型（access 或 refresh）
/// - `custom`：自定义声明，使用 JSON 格式存储任意数据
///
/// # 示例
///
/// ```rust
/// use yang_base::token::{TokenClaims, TokenType};
/// use serde_json::json;
///
/// let claims = TokenClaims::new(
///     "yang-base",
///     "user_123",
///     "yang-app",
///     1234567890,
///     1234567800,
///     1234567800,
///     "unique-token-id",
///     TokenType::Access,
///     json!({
///         "role": "admin",
///         "permissions": ["read", "write"]
///     }),
/// );
/// ```
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
/// 请使用 [`TokenClaims::new`] 构造。
#[derive(Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TokenClaims {
    /// 签发者（Issuer）
    ///
    /// 标识 Token 的签发方，用于验证 Token 来源
    pub iss: String,

    /// 主题（Subject）
    ///
    /// 通常是用户 ID 或其他唯一标识符
    pub sub: String,

    /// 受众（Audience）
    ///
    /// 标识 Token 的目标接收方
    pub aud: String,

    /// 过期时间（Expiration Time）
    ///
    /// Unix 时间戳，表示 Token 的过期时间
    pub exp: u64,

    /// 生效时间（Not Before）
    ///
    /// Unix 时间戳，表示 Token 在此时间之前不可用
    pub nbf: u64,

    /// 签发时间（Issued At）
    ///
    /// Unix 时间戳，表示 Token 的签发时间
    pub iat: u64,

    /// JWT ID
    ///
    /// Token 的唯一标识符，用于防止重放攻击
    pub jti: String,

    /// Token 类型
    ///
    /// 可选值：
    /// - `TokenType::Access`：访问令牌，用于 API 访问
    /// - `TokenType::Refresh`：刷新令牌，用于获取新的访问令牌
    pub token_type: TokenType,

    /// 自定义声明
    ///
    /// 使用 JSON 格式存储任意自定义数据，例如用户角色、权限等。
    /// 使用 `#[serde(flatten)]` 将自定义字段展平到 Token 的顶层。
    ///
    /// # 示例
    ///
    /// ```json
    /// {
    ///   "role": "admin",
    ///   "permissions": ["read", "write", "delete"],
    ///   "org_id": "org_123"
    /// }
    /// ```
    #[serde(flatten)]
    pub custom: serde_json::Value,
}

// NEW-38: 手写 Debug 脱敏，防止 Token 字段（iss/sub/jti/自定义声明）明文泄漏
impl core::fmt::Debug for TokenClaims {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TokenClaims")
            .field("iss", &self.iss)
            .field("sub", &self.sub)
            .field("aud", &self.aud)
            .field("exp", &self.exp)
            .field("nbf", &self.nbf)
            .field("iat", &self.iat)
            .field("jti", &"***")
            .field("token_type", &self.token_type)
            .field("custom", &"***")
            .finish()
    }
}

impl TokenClaims {
    /// 构造 `TokenClaims`（`#[non_exhaustive]` 后的唯一公开构造入口）。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        iss: impl Into<String>,
        sub: impl Into<String>,
        aud: impl Into<String>,
        exp: u64,
        nbf: u64,
        iat: u64,
        jti: impl Into<String>,
        token_type: TokenType,
        custom: serde_json::Value,
    ) -> Self {
        Self {
            iss: iss.into(),
            sub: sub.into(),
            aud: aud.into(),
            exp,
            nbf,
            iat,
            jti: jti.into(),
            token_type,
            custom,
        }
    }
}

#[cfg(test)]
#[path = "__tests__/claims_test.rs"]
mod claims_test;

#[cfg(test)]
#[path = "__tests__/manager_test.rs"]
mod manager_test;
