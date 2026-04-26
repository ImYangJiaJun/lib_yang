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

    // ==================== Redis 错误 ====================
    /// Redis 连接失败
    #[error("Redis 连接失败: {0}")]
    RedisConnectionFailed(String),

    /// Redis 已初始化
    #[error("Redis 已初始化")]
    RedisAlreadyInitialized,

    /// Redis 未初始化
    #[error("Redis 未初始化")]
    RedisNotInitialized,

    /// Redis 操作失败
    #[error("Redis 操作失败: {0}")]
    RedisOperationFailed(String),

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

    // ==================== 字段验证错误 ====================
    /// 字段类型无效
    #[error("字段类型无效 [{0}]: {1}")]
    InvalidFieldType(String, String),

    /// 枚举值无效
    #[error("枚举值无效 [{0}]: 值 '{1}' 不在可选值列表中")]
    InvalidEnumValue(String, String),

    /// 字符串长度超出限制
    #[error("字符串长度超出限制 [{0}]: 当前长度 {1}，最大长度 {2}")]
    StringTooLong(String, usize, usize),

    /// JSON 格式无效
    #[error("JSON 格式无效 [{0}]: {1}")]
    InvalidJsonFormat(String, String),

    /// 验证失败
    #[error("验证失败 [{0}]: {1}")]
    ValidationFailed(String, String),

    /// 字段必填
    #[error("字段必填: {0}")]
    FieldRequired(String),

    /// 字段未找到
    #[error("字段未找到 [表: {0}]: {1}")]
    FieldNotFound(String, String),

    /// 字段权限被拒绝
    #[error("字段权限被拒绝 [表: {0}, 字段: {1}]: {2}")]
    FieldPermissionDenied(String, String, String),

    // ==================== Action 系统错误 ====================
    /// Action 未找到
    #[error("Action 未找到: {0}")]
    ActionNotFound(String),

    /// 权限被拒绝
    #[error("权限被拒绝: {0}")]
    PermissionDenied(String),

    /// 未授权
    #[error("未授权: {0}")]
    Unauthorized(String),

    /// 参数缺失
    #[error("参数缺失: {0}")]
    ParamMissing(String),

    /// 参数无效
    #[error("参数无效 [{0}]: {1}")]
    ParamInvalid(String, String),

    /// 记录未找到
    #[error("记录未找到: {0}")]
    RecordNotFound(String),

    /// 用户未找到
    #[error("用户未找到: {0}")]
    UserNotFound(String),

    /// 密码无效
    #[error("密码无效")]
    InvalidPassword,

    /// 表配置未设置
    #[error("表配置未设置")]
    TableConfigNotSet,

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

impl BaseError {
    /// 获取错误码
    ///
    /// 返回与错误类型对应的数字错误码，用于 API 响应
    ///
    /// # 错误码规范
    ///
    /// - 1xxxxx: 插件管理错误
    /// - 2xxxxx: 数据库错误
    /// - 3xxxxx: HTTP 客户端错误
    /// - 4xxxxx: Token 管理错误
    /// - 5xxxxx: 序列化错误
    /// - 6xxxxx: 字段验证错误
    /// - 7xxxxx: Action 系统错误
    /// - 9xxxxx: 通用错误
    ///
    /// # 返回
    ///
    /// - 错误码（非零整数）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::error::BaseError;
    ///
    /// let error = BaseError::FieldRequired("username".to_string());
    /// assert_eq!(error.code(), 600006);
    /// ```
    pub fn code(&self) -> i32 {
        match self {
            // ==================== 插件管理错误 (1xxxxx) ====================
            BaseError::PluginAlreadyRegistered(_) => 100001,
            BaseError::PluginNotFound(_) => 100002,
            BaseError::PluginRegisterFailed(_, _) => 100003,
            BaseError::PluginInitFailed(_, _) => 100004,
            BaseError::PluginDependencyMissing(_, _) => 100005,
            BaseError::PluginCircularDependency(_) => 100006,
            BaseError::PluginConfigInvalid(_, _) => 100007,
            BaseError::PluginShutdownFailed(_, _) => 100008,

            // ==================== 数据库错误 (2xxxxx) ====================
            BaseError::DatabaseConnectionFailed(_) => 200001,
            BaseError::DatabaseAlreadyInitialized => 200002,
            BaseError::DatabaseQueryFailed(_) => 200003,
            BaseError::DatabaseExecuteFailed(_) => 200004,
            BaseError::DatabaseInitFailed(_) => 200005,
            BaseError::DatabaseMigrationFailed(_, _) => 200006,
            BaseError::MigrationFailed(_, _, _) => 200007,
            BaseError::DatabaseNotInitialized => 200008,
            BaseError::DatabaseTransactionFailed(_) => 200009,

            // ==================== Redis 错误 (21xxxx) ====================
            BaseError::RedisConnectionFailed(_) => 210001,
            BaseError::RedisAlreadyInitialized => 210002,
            BaseError::RedisNotInitialized => 210003,
            BaseError::RedisOperationFailed(_) => 210004,

            // ==================== HTTP 客户端错误 (3xxxxx) ====================
            BaseError::HttpClientCreateFailed(_) => 300001,
            BaseError::HttpRequestFailed(_) => 300002,
            BaseError::HttpResponseParseFailed(_) => 300003,
            BaseError::HttpTimeout => 300004,

            // ==================== Token 管理错误 (4xxxxx) ====================
            BaseError::TokenKeyInvalid(_) => 400001,
            BaseError::TokenGenerateFailed(_) => 400002,
            BaseError::TokenVerifyFailed(_) => 400003,
            BaseError::TokenParseFailed(_) => 400004,
            BaseError::TokenExpired => 400005,
            BaseError::TokenTypeInvalid(_) => 400006,

            // ==================== 序列化错误 (5xxxxx) ====================
            BaseError::JsonSerializeFailed(_) => 500001,
            BaseError::JsonDeserializeFailed(_) => 500002,

            // ==================== 字段验证错误 (6xxxxx) ====================
            BaseError::InvalidFieldType(_, _) => 600001,
            BaseError::InvalidEnumValue(_, _) => 600002,
            BaseError::StringTooLong(_, _, _) => 600003,
            BaseError::InvalidJsonFormat(_, _) => 600004,
            BaseError::ValidationFailed(_, _) => 600005,
            BaseError::FieldRequired(_) => 600006,
            BaseError::FieldNotFound(_, _) => 600007,
            BaseError::FieldPermissionDenied(_, _, _) => 600008,

            // ==================== Action 系统错误 (7xxxxx) ====================
            BaseError::ActionNotFound(_) => 700001,
            BaseError::PermissionDenied(_) => 700002,
            BaseError::Unauthorized(_) => 700003,
            BaseError::ParamMissing(_) => 700004,
            BaseError::ParamInvalid(_, _) => 700005,
            BaseError::RecordNotFound(_) => 700006,
            BaseError::UserNotFound(_) => 700007,
            BaseError::InvalidPassword => 700008,
            BaseError::TableConfigNotSet => 700009,

            // ==================== 通用错误 (9xxxxx) ====================
            BaseError::ConfigError(_) => 900001,
            BaseError::IoError(_) => 900002,
            BaseError::Unknown(_) => 999999,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_plugin() {
        assert_eq!(
            BaseError::PluginAlreadyRegistered("test".to_string()).code(),
            100001
        );
        assert_eq!(BaseError::PluginNotFound("test".to_string()).code(), 100002);
        assert_eq!(
            BaseError::PluginRegisterFailed("test".to_string(), "reason".to_string()).code(),
            100003
        );
        assert_eq!(
            BaseError::PluginInitFailed("test".to_string(), "reason".to_string()).code(),
            100004
        );
        assert_eq!(
            BaseError::PluginDependencyMissing("test".to_string(), "dep".to_string()).code(),
            100005
        );
        assert_eq!(
            BaseError::PluginCircularDependency("test".to_string()).code(),
            100006
        );
        assert_eq!(
            BaseError::PluginConfigInvalid("test".to_string(), "reason".to_string()).code(),
            100007
        );
        assert_eq!(
            BaseError::PluginShutdownFailed("test".to_string(), "reason".to_string()).code(),
            100008
        );
    }

    #[test]
    fn test_error_codes_database() {
        assert_eq!(
            BaseError::DatabaseConnectionFailed("reason".to_string()).code(),
            200001
        );
        assert_eq!(BaseError::DatabaseAlreadyInitialized.code(), 200002);
        assert_eq!(
            BaseError::DatabaseQueryFailed("reason".to_string()).code(),
            200003
        );
        assert_eq!(
            BaseError::DatabaseExecuteFailed("reason".to_string()).code(),
            200004
        );
        assert_eq!(
            BaseError::DatabaseInitFailed("reason".to_string()).code(),
            200005
        );
        assert_eq!(
            BaseError::DatabaseMigrationFailed("plugin".to_string(), "reason".to_string()).code(),
            200006
        );
        assert_eq!(
            BaseError::MigrationFailed(
                "plugin".to_string(),
                "v1".to_string(),
                "reason".to_string()
            )
            .code(),
            200007
        );
        assert_eq!(BaseError::DatabaseNotInitialized.code(), 200008);
        assert_eq!(
            BaseError::DatabaseTransactionFailed("reason".to_string()).code(),
            200009
        );
    }

    #[test]
    fn test_error_codes_http() {
        assert_eq!(
            BaseError::HttpClientCreateFailed("reason".to_string()).code(),
            300001
        );
        assert_eq!(
            BaseError::HttpRequestFailed("reason".to_string()).code(),
            300002
        );
        assert_eq!(
            BaseError::HttpResponseParseFailed("reason".to_string()).code(),
            300003
        );
        assert_eq!(BaseError::HttpTimeout.code(), 300004);
    }

    #[test]
    fn test_error_codes_token() {
        assert_eq!(
            BaseError::TokenKeyInvalid("reason".to_string()).code(),
            400001
        );
        assert_eq!(
            BaseError::TokenGenerateFailed("reason".to_string()).code(),
            400002
        );
        assert_eq!(
            BaseError::TokenVerifyFailed("reason".to_string()).code(),
            400003
        );
        assert_eq!(
            BaseError::TokenParseFailed("reason".to_string()).code(),
            400004
        );
        assert_eq!(BaseError::TokenExpired.code(), 400005);
        assert_eq!(
            BaseError::TokenTypeInvalid("reason".to_string()).code(),
            400006
        );
    }

    #[test]
    fn test_error_codes_serialization() {
        assert_eq!(
            BaseError::JsonSerializeFailed("reason".to_string()).code(),
            500001
        );
        assert_eq!(
            BaseError::JsonDeserializeFailed("reason".to_string()).code(),
            500002
        );
    }

    #[test]
    fn test_error_codes_field_validation() {
        assert_eq!(
            BaseError::InvalidFieldType("field".to_string(), "reason".to_string()).code(),
            600001
        );
        assert_eq!(
            BaseError::InvalidEnumValue("field".to_string(), "value".to_string()).code(),
            600002
        );
        assert_eq!(
            BaseError::StringTooLong("field".to_string(), 100, 50).code(),
            600003
        );
        assert_eq!(
            BaseError::InvalidJsonFormat("field".to_string(), "reason".to_string()).code(),
            600004
        );
        assert_eq!(
            BaseError::ValidationFailed("field".to_string(), "reason".to_string()).code(),
            600005
        );
        assert_eq!(BaseError::FieldRequired("field".to_string()).code(), 600006);
        assert_eq!(
            BaseError::FieldNotFound("table".to_string(), "field".to_string()).code(),
            600007
        );
        assert_eq!(
            BaseError::FieldPermissionDenied(
                "table".to_string(),
                "field".to_string(),
                "reason".to_string()
            )
            .code(),
            600008
        );
    }

    #[test]
    fn test_error_codes_action_system() {
        assert_eq!(
            BaseError::ActionNotFound("test_action".to_string()).code(),
            700001
        );
        assert_eq!(
            BaseError::PermissionDenied("需要管理员权限".to_string()).code(),
            700002
        );
        assert_eq!(
            BaseError::Unauthorized("Token 无效".to_string()).code(),
            700003
        );
        assert_eq!(
            BaseError::ParamMissing("user_id".to_string()).code(),
            700004
        );
        assert_eq!(
            BaseError::ParamInvalid("age".to_string(), "必须是正整数".to_string()).code(),
            700005
        );
        assert_eq!(
            BaseError::RecordNotFound("用户 ID: 123".to_string()).code(),
            700006
        );
        assert_eq!(
            BaseError::UserNotFound("user_123".to_string()).code(),
            700007
        );
        assert_eq!(BaseError::InvalidPassword.code(), 700008);
        assert_eq!(BaseError::TableConfigNotSet.code(), 700009);
    }

    #[test]
    fn test_error_codes_general() {
        assert_eq!(BaseError::ConfigError("reason".to_string()).code(), 900001);
        assert_eq!(BaseError::IoError("reason".to_string()).code(), 900002);
        assert_eq!(BaseError::Unknown("reason".to_string()).code(), 999999);
    }

    #[test]
    fn test_all_error_codes_are_nonzero() {
        // 确保所有错误码都是非零的
        let errors = vec![
            BaseError::PluginNotFound("test".to_string()),
            BaseError::DatabaseQueryFailed("test".to_string()),
            BaseError::HttpTimeout,
            BaseError::TokenExpired,
            BaseError::JsonSerializeFailed("test".to_string()),
            BaseError::FieldRequired("test".to_string()),
            BaseError::ActionNotFound("test".to_string()),
            BaseError::PermissionDenied("test".to_string()),
            BaseError::Unauthorized("test".to_string()),
            BaseError::ParamMissing("test".to_string()),
            BaseError::RecordNotFound("test".to_string()),
            BaseError::UserNotFound("test".to_string()),
            BaseError::InvalidPassword,
            BaseError::TableConfigNotSet,
            BaseError::Unknown("test".to_string()),
        ];

        for error in errors {
            assert_ne!(error.code(), 0, "错误码不应该为 0: {:?}", error);
        }
    }
}
