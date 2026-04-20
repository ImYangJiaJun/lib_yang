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

pub use manager::TokenManager;

use serde::{Deserialize, Serialize};

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
/// use yang_base::token::TokenClaims;
/// use serde_json::json;
///
/// let claims = TokenClaims {
///     iss: "yang-base".to_string(),
///     sub: "user_123".to_string(),
///     aud: "yang-app".to_string(),
///     exp: 1234567890,
///     nbf: 1234567800,
///     iat: 1234567800,
///     jti: "unique-token-id".to_string(),
///     token_type: "access".to_string(),
///     custom: json!({
///         "role": "admin",
///         "permissions": ["read", "write"]
///     }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// - `"access"`：访问令牌，用于 API 访问
    /// - `"refresh"`：刷新令牌，用于获取新的访问令牌
    pub token_type: String,

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

#[cfg(test)]
#[path = "__tests__/claims_test.rs"]
mod claims_test;

#[cfg(test)]
#[path = "__tests__/manager_test.rs"]
mod manager_test;
