//! 错误处理模块
//!
//! 定义系统中所有模块使用的统一错误类型。
//!
//! # 主要组件
//!
//! - `BaseError`：统一错误枚举类型
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::error::BaseError;
//!
//! fn do_something() -> Result<(), BaseError> {
//!     // 插件未找到错误
//!     Err(BaseError::PluginNotFound("my_plugin".to_string()))
//! }
//! ```

use thiserror::Error;

/// 系统统一错误类型
///
/// 包含所有模块的错误变体，使用中文错误消息
#[derive(Debug, Error)]
pub enum BaseError {
    // ==================== 插件管理错误 ====================
    /// 插件已注册
    #[error("插件已注册: {0}")]
    PluginAlreadyRegistered(String),

    /// 插件未找到
    #[error("插件未找到: {0}")]
    PluginNotFound(String),

    /// 插件注册失败
    #[error("插件注册失败 [{0}]: {1}")]
    PluginRegisterFailed(String, String),

    /// 插件初始化失败
    #[error("插件初始化失败 [{0}]: {1}")]
    PluginInitFailed(String, String),

    /// 插件依赖缺失
    #[error("插件依赖缺失 [{0}]: 缺少依赖 {1}")]
    PluginDependencyMissing(String, String),

    /// 插件循环依赖
    #[error("插件循环依赖: {0}")]
    PluginCircularDependency(String),

    /// 插件配置无效
    #[error("插件配置无效 [{0}]: {1}")]
    PluginConfigInvalid(String, String),

    /// 插件关闭失败
    #[error("插件关闭失败 [{0}]: {1}")]
    PluginShutdownFailed(String, String),

    // ==================== 数据库错误 ====================
    /// 数据库连接失败
    #[error("数据库连接失败: {0}")]
    DatabaseConnectionFailed(String),

    /// 数据库已初始化
    #[error("数据库已初始化")]
    DatabaseAlreadyInitialized,

    /// 数据库查询失败
    #[error("数据库查询失败: {0}")]
    DatabaseQueryFailed(String),

    /// 数据库执行失败
    #[error("数据库执行失败: {0}")]
    DatabaseExecuteFailed(String),

    /// 数据库初始化失败
    #[error("数据库初始化失败: {0}")]
    DatabaseInitFailed(String),

    /// 数据库迁移失败
    #[error("数据库迁移失败 [{0}]: {1}")]
    DatabaseMigrationFailed(String, String),

    /// 迁移失败（别名）
    #[error("迁移失败 [{0}] v{1}: {2}")]
    MigrationFailed(String, String, String),

    /// 数据库未初始化
    #[error("数据库未初始化")]
    DatabaseNotInitialized,

    /// 数据库事务失败
    #[error("数据库事务失败: {0}")]
    DatabaseTransactionFailed(String),

    // ==================== HTTP 客户端错误 ====================
    /// HTTP 客户端创建失败
    #[error("HTTP 客户端创建失败: {0}")]
    HttpClientCreateFailed(String),

    /// HTTP 请求失败
    #[error("HTTP 请求失败: {0}")]
    HttpRequestFailed(String),

    /// HTTP 响应解析失败
    #[error("HTTP 响应解析失败: {0}")]
    HttpResponseParseFailed(String),

    /// HTTP 超时
    #[error("HTTP 请求超时")]
    HttpTimeout,

    // ==================== Token 管理错误 ====================
    /// Token 密钥无效
    #[error("Token 密钥无效: {0}")]
    TokenKeyInvalid(String),

    /// Token 生成失败
    #[error("Token 生成失败: {0}")]
    TokenGenerateFailed(String),

    /// Token 验证失败
    #[error("Token 验证失败: {0}")]
    TokenVerifyFailed(String),

    /// Token 解析失败
    #[error("Token 解析失败: {0}")]
    TokenParseFailed(String),

    /// Token 已过期
    #[error("Token 已过期")]
    TokenExpired,

    /// Token 类型无效
    #[error("Token 类型无效: {0}")]
    TokenTypeInvalid(String),

    // ==================== 序列化错误 ====================
    /// JSON 序列化失败
    #[error("JSON 序列化失败: {0}")]
    JsonSerializeFailed(String),

    /// JSON 反序列化失败
    #[error("JSON 反序列化失败: {0}")]
    JsonDeserializeFailed(String),

    // ==================== 通用错误 ====================
    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(String),

    /// 未知错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

// ==================== From trait 实现 ====================

/// 从 yang_db::DbError 转换为 BaseError
impl From<yang_db::DbError> for BaseError {
    fn from(err: yang_db::DbError) -> Self {
        BaseError::DatabaseQueryFailed(err.to_string())
    }
}

/// 从 std::io::Error 转换为 BaseError
impl From<std::io::Error> for BaseError {
    fn from(err: std::io::Error) -> Self {
        BaseError::IoError(err.to_string())
    }
}

/// 从 serde_json::Error 转换为 BaseError
impl From<serde_json::Error> for BaseError {
    fn from(err: serde_json::Error) -> Self {
        if err.is_syntax() || err.is_data() || err.is_eof() {
            BaseError::JsonDeserializeFailed(err.to_string())
        } else {
            BaseError::JsonSerializeFailed(err.to_string())
        }
    }
}

/// 从 reqwest::Error 转换为 BaseError
impl From<reqwest::Error> for BaseError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            BaseError::HttpTimeout
        } else if err.is_connect() {
            BaseError::HttpClientCreateFailed(err.to_string())
        } else {
            BaseError::HttpRequestFailed(err.to_string())
        }
    }
}
