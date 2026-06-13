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

/// 以 [`BaseError`] 为默认错误类型的统一 `Result` 别名。
///
/// 允许下游以 `yang_base::Result<T>` 书写返回类型，省去重复的 `, BaseError`。
/// 保留类型参数 `E = BaseError`，少数需要其它错误类型的签名仍可显式覆盖。
pub type Result<T, E = BaseError> = std::result::Result<T, E>;

/// 系统统一错误类型
///
/// 包含所有模块的错误变体，使用中文错误消息
///
/// 标注 `#[non_exhaustive]`：未来新增变体不构成跨 crate 破坏性变更（下游 match 需
/// 带 `_` 臂）。同 crate 内的穷举 match（`code()`/`code_str()`/`category()`）不受
/// 影响，新增变体时 `test_code_str_matches_code` 测试会捕获漏编。
#[derive(Debug, Error)]
#[non_exhaustive]
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

    /// 数据库连接失败，持有底层 DbError 以保留错误链
    #[error("数据库连接失败: {0}")]
    DatabaseConnectionDbError(#[source] yang_db::DbError),

    /// 数据库已初始化
    #[error("数据库已初始化")]
    DatabaseAlreadyInitialized,

    /// 数据库查询失败，持有底层 DbError 以保留错误链
    #[error("数据库查询失败: {0}")]
    DatabaseQueryFailed(#[source] yang_db::DbError),

    /// 数据库执行失败，持有底层 DbError 以保留错误链
    #[error("数据库执行失败: {0}")]
    DatabaseExecuteFailed(#[source] yang_db::DbError),

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

    /// 数据库事务失败，持有底层 DbError 以保留错误链
    #[error("数据库事务失败: {0}")]
    DatabaseTransactionFailed(#[source] yang_db::DbError),

    /// UPDATE/DELETE 缺少 WHERE 条件，拒绝全表操作
    ///
    /// 与 yang-db 的 `MissingWhereClause` 安全网对齐：未显式调用
    /// [`crate::table::TableQuery::allow_full_table`] 时，无 WHERE 的
    /// 更新/删除会被拒绝以防止误操作整表。
    #[error("UPDATE/DELETE 缺少 WHERE 条件，拒绝全表操作: {0}")]
    MissingWhereClause(String),

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

    /// Redis 操作失败，持有底层 DbError 以保留错误链
    #[error("Redis 操作失败: {0}")]
    RedisOperationDbError(#[source] yang_db::DbError),

    // ==================== HTTP 客户端错误 ====================
    /// HTTP 客户端创建失败，持有底层 reqwest::Error 以保留错误链
    #[cfg(feature = "http")]
    #[error("HTTP 客户端创建失败: {0}")]
    HttpClientCreateFailed(#[source] reqwest::Error),

    /// HTTP 客户端创建失败（无 http feature 时使用字符串）
    #[cfg(not(feature = "http"))]
    #[error("HTTP 客户端创建失败: {0}")]
    HttpClientCreateFailed(String),

    /// HTTP 请求失败，持有底层 reqwest::Error 以保留错误链
    #[cfg(feature = "http")]
    #[error("HTTP 请求失败: {0}")]
    HttpRequestFailed(#[source] reqwest::Error),

    /// HTTP 请求失败（无 http feature 时使用字符串）
    #[cfg(not(feature = "http"))]
    #[error("HTTP 请求失败: {0}")]
    HttpRequestFailed(String),

    /// HTTP 响应解析失败
    #[error("HTTP 响应解析失败: {0}")]
    HttpResponseParseFailed(String),

    /// HTTP 超时
    #[error("HTTP 请求超时")]
    HttpTimeout,

    /// HTTP 客户端已初始化
    #[error("HTTP 客户端已初始化")]
    HttpClientAlreadyInitialized,

    /// HTTP 客户端未初始化
    #[error("HTTP 客户端未初始化")]
    HttpClientNotInitialized,

    /// HTTP 熔断器打开：目标 host 近期连续失败，请求被快速拒绝
    #[error("HTTP 熔断器已打开，目标主机暂不可用: {0}")]
    HttpCircuitBreakerOpen(String),

    // ==================== Token 管理错误 ====================
    /// Token 密钥无效
    #[error("Token 密钥无效: {0}")]
    TokenKeyInvalid(String),

    /// Token 生成失败，持有底层 jsonwebtoken::errors::Error 以保留错误链
    #[cfg(feature = "token")]
    #[error("Token 生成失败: {0}")]
    TokenGenerateFailed(#[source] jsonwebtoken::errors::Error),

    /// Token 生成失败（无 token feature 时使用字符串）
    #[cfg(not(feature = "token"))]
    #[error("Token 生成失败: {0}")]
    TokenGenerateFailed(String),

    /// Token 验证失败，持有底层 jsonwebtoken::errors::Error 以保留错误链
    #[cfg(feature = "token")]
    #[error("Token 验证失败: {0}")]
    TokenVerifyFailed(#[source] jsonwebtoken::errors::Error),

    /// Token 验证失败（无 token feature 时使用字符串）
    #[cfg(not(feature = "token"))]
    #[error("Token 验证失败: {0}")]
    TokenVerifyFailed(String),

    /// Token 解析失败，持有底层 jsonwebtoken::errors::Error 以保留错误链
    #[cfg(feature = "token")]
    #[error("Token 解析失败: {0}")]
    TokenParseFailed(#[source] jsonwebtoken::errors::Error),

    /// Token 解析失败（无 token feature 时使用字符串）
    #[cfg(not(feature = "token"))]
    #[error("Token 解析失败: {0}")]
    TokenParseFailed(String),

    /// Token 已过期
    #[error("Token 已过期")]
    TokenExpired,

    /// Token 已被撤销（命中黑名单）
    #[error("Token 已被撤销")]
    TokenRevoked,

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
///
/// 按 DbError 变体分类映射到对应的 BaseError 变体：
/// - 查询类 → DatabaseQueryFailed
/// - 执行类 → DatabaseExecuteFailed
/// - 事务类 → DatabaseTransactionFailed
/// - 连接类 → DatabaseConnectionFailed
/// - Redis 类 → RedisOperationFailed
#[allow(unreachable_patterns)]
impl From<yang_db::DbError> for BaseError {
    fn from(err: yang_db::DbError) -> Self {
        use yang_db::DbError as D;
        match &err {
            // 查询类：查询错误、表不存在、类型转换、反序列化、不支持的操作符、未知错误
            D::QueryError(_)
            | D::TableNotFound(_)
            | D::TypeConversionError(_)
            | D::DeserializationError(_)
            | D::UnsupportedOperator(_)
            | D::Unknown(_) => BaseError::DatabaseQueryFailed(err),

            // 执行类：约束错误、SQL 语法错误、缺少 WHERE 条件、缺少 GROUP BY、序列化错误
            D::ConstraintError(_)
            | D::SqlSyntaxError(_)
            | D::MissingWhereClause
            | D::MissingGroupByClause
            | D::SerializationError(_) => BaseError::DatabaseExecuteFailed(err),

            // 事务类：事务错误
            D::TransactionError(_) => BaseError::DatabaseTransactionFailed(err),

            // 连接类：连接错误（保留底层 DbError 错误链）
            D::ConnectionError(_) => BaseError::DatabaseConnectionDbError(err),

            // Redis 类：所有 Redis 相关错误（保留底层 DbError 错误链）
            D::RedisConnectionError(_)
            | D::RedisCommandError(_)
            | D::RedisPoolError(_)
            | D::RedisTypeConversionError(_)
            | D::RedisTimeoutError(_) => BaseError::RedisOperationDbError(err),

            // 未来新增变体（DbError 标注 #[non_exhaustive]）统一按查询失败处理
            _ => BaseError::DatabaseQueryFailed(err),
        }
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
#[cfg(feature = "http")]
impl From<reqwest::Error> for BaseError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            BaseError::HttpTimeout
        } else {
            // is_connect() 属于「发送请求」阶段的连接失败（DNS/TCP/TLS），
            // 客户端此时早已创建成功，故与其它发送错误统一归为 HttpRequestFailed。
            // 真正的客户端创建失败由 http/client.rs 显式 map_err(HttpClientCreateFailed) 处理。
            BaseError::HttpRequestFailed(err)
        }
    }
}

/// 从 jsonwebtoken::errors::Error 转换为 BaseError
///
/// 按 ErrorKind 分类映射：
/// - ExpiredSignature → TokenExpired
/// - InvalidToken / InvalidSignature → TokenVerifyFailed
/// - 其他 → TokenParseFailed
#[cfg(feature = "token")]
impl From<jsonwebtoken::errors::Error> for BaseError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        use jsonwebtoken::errors::ErrorKind;
        match err.kind() {
            ErrorKind::ExpiredSignature => BaseError::TokenExpired,
            ErrorKind::InvalidToken | ErrorKind::InvalidSignature => {
                BaseError::TokenVerifyFailed(err)
            }
            _ => BaseError::TokenParseFailed(err),
        }
    }
}

/// 引擎级错误分类（与具体 HTTP/传输层无关）。
///
/// 用于下游统一适配（弹性重试、错误上报分桶、HTTP status 映射的中间层）。
/// HTTP status 映射属调用方传输层边界，不在引擎层硬编码。
///
/// `is_client_error()` = `Client` + `Auth`；`is_server_error()` = `Server` +
/// `Transient` + `Conflict`。详见 [`BaseError::category`] 文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCategory {
    /// 调用方过错（参数错误、字段验证、序列化、插件依赖/配置/循环）
    Client,
    /// 认证/授权失败（登录过期、Token 撤销、密码错误、权限不足）
    Auth,
    /// 资源不存在（记录/用户/Action/表未找到）
    NotFound,
    /// 资源冲突（插件注册冲突）
    Conflict,
    /// 瞬时故障（连接失败/超时/连接池/HTTP 超时/熔断打开），可重试
    ///
    /// 在 `is_server_error` 中视为服务端错误（可重试的服务端故障，
    /// 如连接超时 = HTTP 503 语义），但 `is_client_error` 中**不**视为客户端错误。
    Transient,
    /// 服务端/基础设施错误（内部失败、配置错误、未知错误）
    Server,
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
    ///   - 21xxxx: Redis 错误（隶属数据库 2xxxxx 大类）
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
            BaseError::DatabaseConnectionDbError(_) => 200001,
            BaseError::DatabaseAlreadyInitialized => 200002,
            BaseError::DatabaseQueryFailed(_) => 200003,
            BaseError::DatabaseExecuteFailed(_) => 200004,
            BaseError::DatabaseInitFailed(_) => 200005,
            BaseError::DatabaseMigrationFailed(_, _) => 200006,
            BaseError::MigrationFailed(_, _, _) => 200007,
            BaseError::DatabaseNotInitialized => 200008,
            BaseError::DatabaseTransactionFailed(_) => 200009,
            BaseError::MissingWhereClause(_) => 200010,

            // ==================== Redis 错误 (21xxxx) ====================
            BaseError::RedisConnectionFailed(_) => 210001,
            BaseError::RedisAlreadyInitialized => 210002,
            BaseError::RedisNotInitialized => 210003,
            BaseError::RedisOperationFailed(_) => 210004,
            BaseError::RedisOperationDbError(_) => 210004,

            // ==================== HTTP 客户端错误 (3xxxxx) ====================
            BaseError::HttpClientCreateFailed(_) => 300001,
            BaseError::HttpRequestFailed(_) => 300002,
            BaseError::HttpResponseParseFailed(_) => 300003,
            BaseError::HttpTimeout => 300004,
            BaseError::HttpClientAlreadyInitialized => 300005,
            BaseError::HttpClientNotInitialized => 300006,
            BaseError::HttpCircuitBreakerOpen(_) => 300007,

            // ==================== Token 管理错误 (4xxxxx) ====================
            BaseError::TokenKeyInvalid(_) => 400001,
            BaseError::TokenGenerateFailed(_) => 400002,
            BaseError::TokenVerifyFailed(_) => 400003,
            BaseError::TokenParseFailed(_) => 400004,
            BaseError::TokenExpired => 400005,
            BaseError::TokenTypeInvalid(_) => 400006,
            BaseError::TokenRevoked => 400007,

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

    /// 返回错误码的 `&'static str` 形式，供 metrics label 使用（零分配、低基数）。
    ///
    /// 与 [`BaseError::code`] 一一对应：`code_str()` 是 `code()` 的字符串字面量版本，
    /// 专用于 `metrics` 的 label（label 必须是 `&'static str`，**严禁** `code().to_string()`
    /// 造成每次派发堆分配 + 高基数）。新增变体时两者需同步维护，并由
    /// `test_code_str_matches_code` 守护一致性。
    pub fn code_str(&self) -> &'static str {
        match self {
            // 插件管理错误 (1xxxxx)
            BaseError::PluginAlreadyRegistered(_) => "100001",
            BaseError::PluginNotFound(_) => "100002",
            BaseError::PluginRegisterFailed(_, _) => "100003",
            BaseError::PluginInitFailed(_, _) => "100004",
            BaseError::PluginDependencyMissing(_, _) => "100005",
            BaseError::PluginCircularDependency(_) => "100006",
            BaseError::PluginConfigInvalid(_, _) => "100007",
            BaseError::PluginShutdownFailed(_, _) => "100008",
            // 数据库错误 (2xxxxx)
            BaseError::DatabaseConnectionFailed(_) => "200001",
            BaseError::DatabaseConnectionDbError(_) => "200001",
            BaseError::DatabaseAlreadyInitialized => "200002",
            BaseError::DatabaseQueryFailed(_) => "200003",
            BaseError::DatabaseExecuteFailed(_) => "200004",
            BaseError::DatabaseInitFailed(_) => "200005",
            BaseError::DatabaseMigrationFailed(_, _) => "200006",
            BaseError::MigrationFailed(_, _, _) => "200007",
            BaseError::DatabaseNotInitialized => "200008",
            BaseError::DatabaseTransactionFailed(_) => "200009",
            BaseError::MissingWhereClause(_) => "200010",
            // Redis 错误 (21xxxx)
            BaseError::RedisConnectionFailed(_) => "210001",
            BaseError::RedisAlreadyInitialized => "210002",
            BaseError::RedisNotInitialized => "210003",
            BaseError::RedisOperationFailed(_) => "210004",
            BaseError::RedisOperationDbError(_) => "210004",
            // HTTP 客户端错误 (3xxxxx)
            BaseError::HttpClientCreateFailed(_) => "300001",
            BaseError::HttpRequestFailed(_) => "300002",
            BaseError::HttpResponseParseFailed(_) => "300003",
            BaseError::HttpTimeout => "300004",
            BaseError::HttpClientAlreadyInitialized => "300005",
            BaseError::HttpClientNotInitialized => "300006",
            BaseError::HttpCircuitBreakerOpen(_) => "300007",
            // Token 管理错误 (4xxxxx)
            BaseError::TokenKeyInvalid(_) => "400001",
            BaseError::TokenGenerateFailed(_) => "400002",
            BaseError::TokenVerifyFailed(_) => "400003",
            BaseError::TokenParseFailed(_) => "400004",
            BaseError::TokenExpired => "400005",
            BaseError::TokenTypeInvalid(_) => "400006",
            BaseError::TokenRevoked => "400007",
            // 序列化错误 (5xxxxx)
            BaseError::JsonSerializeFailed(_) => "500001",
            BaseError::JsonDeserializeFailed(_) => "500002",
            // 字段验证错误 (6xxxxx)
            BaseError::InvalidFieldType(_, _) => "600001",
            BaseError::InvalidEnumValue(_, _) => "600002",
            BaseError::StringTooLong(_, _, _) => "600003",
            BaseError::InvalidJsonFormat(_, _) => "600004",
            BaseError::ValidationFailed(_, _) => "600005",
            BaseError::FieldRequired(_) => "600006",
            BaseError::FieldNotFound(_, _) => "600007",
            BaseError::FieldPermissionDenied(_, _, _) => "600008",
            // Action 系统错误 (7xxxxx)
            BaseError::ActionNotFound(_) => "700001",
            BaseError::PermissionDenied(_) => "700002",
            BaseError::Unauthorized(_) => "700003",
            BaseError::ParamMissing(_) => "700004",
            BaseError::ParamInvalid(_, _) => "700005",
            BaseError::RecordNotFound(_) => "700006",
            BaseError::UserNotFound(_) => "700007",
            BaseError::InvalidPassword => "700008",
            BaseError::TableConfigNotSet => "700009",
            // 通用错误 (9xxxxx)
            BaseError::ConfigError(_) => "900001",
            BaseError::IoError(_) => "900002",
            BaseError::Unknown(_) => "999999",
        }
    }

    /// 引擎级错误分类（弹性重试与任意下游适配的共同基座）。
    ///
    /// 返回 [`ErrorCategory`]，是 `is_retryable` / `is_client_error` /
    /// `is_server_error` 的单一事实源。分类语义为**引擎自有**，与具体 HTTP status
    /// 映射无关——HTTP 映射属调用方传输层边界。
    ///
    /// `Transient` 在 `is_server_error` 中视为服务端错误（但可重试），
    /// 在 `is_client_error` 中**不**视为客户端错误。`Auth` 在 `is_client_error`
    /// 中视为客户端错误（认证/授权失败归调用方过错）。
    pub fn category(&self) -> ErrorCategory {
        use ErrorCategory as C;
        match self {
            // 插件管理错误：除注册冲突外均为服务端/配置问题
            BaseError::PluginAlreadyRegistered(_) => C::Conflict,
            BaseError::PluginCircularDependency(_) => C::Client,
            BaseError::PluginDependencyMissing(_, _) => C::Client,
            BaseError::PluginConfigInvalid(_, _) => C::Client,
            BaseError::PluginNotFound(_)
            | BaseError::PluginRegisterFailed(_, _)
            | BaseError::PluginInitFailed(_, _)
            | BaseError::PluginShutdownFailed(_, _) => C::Server,
            // 数据库错误：连接失败/超时/连接池为瞬时（可重试），
            // 已包装的 DbError 同理（底层连接/超时语义已由 From<DbError> 正确分桶）
            BaseError::DatabaseConnectionFailed(_)
            | BaseError::DatabaseConnectionDbError(_)
            | BaseError::DatabaseTransactionFailed(_) => C::Transient,
            BaseError::MissingWhereClause(_)
            | BaseError::DatabaseMigrationFailed(_, _)
            | BaseError::MigrationFailed(_, _, _) => C::Client,
            BaseError::DatabaseAlreadyInitialized
            | BaseError::DatabaseNotInitialized
            | BaseError::DatabaseInitFailed(_) => C::Server,
            BaseError::DatabaseQueryFailed(_)
            | BaseError::DatabaseExecuteFailed(_) => C::Server,
            // Redis 错误：连接失败/超时/连接池为瞬时
            BaseError::RedisConnectionFailed(_)
            | BaseError::RedisOperationDbError(_) => C::Transient,
            BaseError::RedisAlreadyInitialized | BaseError::RedisNotInitialized => C::Server,
            BaseError::RedisOperationFailed(_) => C::Server,
            // HTTP 客户端错误
            BaseError::HttpClientCreateFailed(_)
            | BaseError::HttpClientNotInitialized
            | BaseError::HttpClientAlreadyInitialized => C::Client,
            BaseError::HttpRequestFailed(_) | BaseError::HttpTimeout => C::Transient,
            BaseError::HttpResponseParseFailed(_)
            | BaseError::HttpCircuitBreakerOpen(_) => C::Transient,
            // Token 错误
            BaseError::TokenKeyInvalid(_)
            | BaseError::TokenGenerateFailed(_)
            | BaseError::TokenTypeInvalid(_)
            | BaseError::TokenParseFailed(_) => C::Client,
            BaseError::TokenExpired | BaseError::TokenRevoked | BaseError::TokenVerifyFailed(_) => C::Auth,
            // 序列化错误
            BaseError::JsonSerializeFailed(_) | BaseError::JsonDeserializeFailed(_) => C::Client,
            // 字段验证错误
            BaseError::InvalidFieldType(_, _)
            | BaseError::InvalidEnumValue(_, _)
            | BaseError::StringTooLong(_, _, _)
            | BaseError::InvalidJsonFormat(_, _)
            | BaseError::ValidationFailed(_, _)
            | BaseError::FieldRequired(_)
            | BaseError::FieldPermissionDenied(_, _, _) => C::Client,
            BaseError::FieldNotFound(_, _) => C::NotFound,
            // Action 系统错误
            BaseError::Unauthorized(_) | BaseError::PermissionDenied(_) => C::Auth,
            BaseError::ParamMissing(_) | BaseError::ParamInvalid(_, _) => C::Client,
            BaseError::RecordNotFound(_) | BaseError::UserNotFound(_) => C::NotFound,
            BaseError::InvalidPassword => C::Auth,
            BaseError::ActionNotFound(_) | BaseError::TableConfigNotSet => C::NotFound,
            // 通用错误
            BaseError::ConfigError(_) | BaseError::IoError(_) | BaseError::Unknown(_) => C::Server,
        }
    }

    /// 是否为可重试的瞬时错误（等价 `category() == Transient`）。
    pub fn is_retryable(&self) -> bool {
        self.category() == ErrorCategory::Transient
    }

    /// 是否为客户端过错（`Client` 或 `Auth` 类）。
    ///
    /// 用于下游统一适配——客户端过错不可重试，调用方应修正请求而非重试。
    /// 注意 `Transient` **不**在此列：虽然瞬时错误也可归「服务端」，但它不是
    /// 调用方过错，且可重试——`is_server_error` 已覆盖该语义。
    pub fn is_client_error(&self) -> bool {
        matches!(self.category(), ErrorCategory::Client | ErrorCategory::Auth)
    }

    /// 是否为服务端/基础设施错误（`Server`、`Transient` 或 `Conflict`）。
    ///
    /// 用于下游统一适配。`Transient` 在此视为服务端（可重试的服务端错误，
    /// 如连接超时 = HTTP 503 语义）；`Conflict` 也计入（资源冲突 = HTTP 409，
    /// 但常由并发写引起而非纯调用方过错）。
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::Server | ErrorCategory::Transient | ErrorCategory::Conflict
        )
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
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError("reason".to_string()))
                .code(),
            200003
        );
        assert_eq!(
            BaseError::DatabaseExecuteFailed(yang_db::DbError::QueryError("reason".to_string()))
                .code(),
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
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "reason".to_string()
            ))
            .code(),
            200009
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_error_codes_http() {
        // 注意：reqwest::Error 无法直接构造，通过 code() 方法间接测试
        assert_eq!(
            BaseError::HttpResponseParseFailed("reason".to_string()).code(),
            300003
        );
        assert_eq!(BaseError::HttpTimeout.code(), 300004);
        assert_eq!(BaseError::HttpClientAlreadyInitialized.code(), 300005);
        assert_eq!(BaseError::HttpClientNotInitialized.code(), 300006);
    }

    #[cfg(feature = "token")]
    #[test]
    fn test_error_codes_token() {
        assert_eq!(
            BaseError::TokenKeyInvalid("reason".to_string()).code(),
            400001
        );
        assert_eq!(
            BaseError::TokenGenerateFailed(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::InvalidToken
            ))
            .code(),
            400002
        );
        assert_eq!(
            BaseError::TokenVerifyFailed(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::InvalidToken
            ))
            .code(),
            400003
        );
        assert_eq!(
            BaseError::TokenParseFailed(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::InvalidToken
            ))
            .code(),
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
        let errors: Vec<BaseError> = vec![
            BaseError::PluginNotFound("test".to_string()),
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError("test".to_string())),
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

    /// 测试数据库错误链 source() 可遍历
    #[test]
    fn test_database_error_source_chain() {
        use std::error::Error;

        let db_err = yang_db::DbError::QueryError("底层查询错误".to_string());
        let base_err = BaseError::DatabaseQueryFailed(db_err);

        // BaseError.source() 应返回 Some，指向 DbError
        let source = base_err.source();
        assert!(source.is_some(), "BaseError.source() 应返回底层 DbError");
    }

    /// 测试 From<DbError> 转换保留错误链
    #[test]
    fn test_from_db_error_preserves_source() {
        use std::error::Error;

        let db_err = yang_db::DbError::QueryError("查询失败".to_string());
        let base_err: BaseError = db_err.into();

        // 验证转换后仍可通过 source() 访问底层错误
        assert!(base_err.source().is_some());
        assert!(matches!(base_err, BaseError::DatabaseQueryFailed(_)));
    }

    /// code_str() 必须与 code() 一致（metrics label 与数值码同源，防漂移）
    #[test]
    fn test_code_str_matches_code() {
        // 覆盖各域代表变体；新增变体时若两处不同步，此处会失败
        let samples: Vec<BaseError> = vec![
            BaseError::PluginNotFound("p".into()),
            BaseError::DatabaseConnectionFailed("c".into()),
            BaseError::DatabaseNotInitialized,
            BaseError::RedisNotInitialized,
            BaseError::HttpTimeout,
            BaseError::TokenExpired,
            BaseError::TokenRevoked,
            BaseError::JsonSerializeFailed("s".into()),
            BaseError::FieldNotFound("t".into(), "f".into()),
            BaseError::ParamInvalid("k".into(), "r".into()),
            BaseError::TableConfigNotSet,
            BaseError::ConfigError("c".into()),
            BaseError::Unknown("u".into()),
        ];
        for err in samples {
            let code = err.code();
            let code_str = err.code_str();
            assert_eq!(
                code_str.parse::<i32>().unwrap(),
                code,
                "code_str {:?} 与 code {} 不一致（变体 {:?}）",
                code_str,
                code,
                err
            );
        }
    }

    /// ErrorCategory 全变体覆盖：每个分类至少有一个代表性变体
    #[test]
    fn test_error_category_coverage() {
        // Client
        assert_eq!(
            BaseError::ParamInvalid("k".into(), "r".into()).category(),
            ErrorCategory::Client
        );
        assert_eq!(
            BaseError::ValidationFailed("f".into(), "r".into()).category(),
            ErrorCategory::Client
        );
        // Auth
        assert_eq!(
            BaseError::Unauthorized("u".into()).category(),
            ErrorCategory::Auth
        );
        assert_eq!(
            BaseError::TokenExpired.category(),
            ErrorCategory::Auth
        );
        assert_eq!(
            BaseError::InvalidPassword.category(),
            ErrorCategory::Auth
        );
        // NotFound
        assert_eq!(
            BaseError::RecordNotFound("r".into()).category(),
            ErrorCategory::NotFound
        );
        assert_eq!(
            BaseError::FieldNotFound("t".into(), "f".into()).category(),
            ErrorCategory::NotFound
        );
        // Conflict
        assert_eq!(
            BaseError::PluginAlreadyRegistered("p".into()).category(),
            ErrorCategory::Conflict
        );
        // Transient
        assert_eq!(
            BaseError::DatabaseConnectionFailed("c".into()).category(),
            ErrorCategory::Transient
        );
        assert_eq!(
            BaseError::HttpTimeout.category(),
            ErrorCategory::Transient
        );
        // Server
        assert_eq!(
            BaseError::Unknown("u".into()).category(),
            ErrorCategory::Server
        );
        assert_eq!(
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError("q".into())).category(),
            ErrorCategory::Server
        );
    }

    /// is_retryable = (category == Transient)
    #[test]
    fn test_is_retryable() {
        assert!(BaseError::DatabaseConnectionFailed("c".into()).is_retryable());
        assert!(BaseError::HttpTimeout.is_retryable());
        assert!(!BaseError::ParamInvalid("k".into(), "r".into()).is_retryable());
        assert!(!BaseError::Unauthorized("u".into()).is_retryable());
        assert!(!BaseError::Unknown("u".into()).is_retryable());
    }

    /// is_client_error = (Client | Auth)
    #[test]
    fn test_is_client_error() {
        assert!(BaseError::ParamInvalid("k".into(), "r".into()).is_client_error());
        assert!(BaseError::Unauthorized("u".into()).is_client_error());
        assert!(BaseError::TokenExpired.is_client_error());
        assert!(!BaseError::DatabaseConnectionFailed("c".into()).is_client_error());
        assert!(!BaseError::Unknown("u".into()).is_client_error());
    }

    /// is_server_error = (Server | Transient | Conflict)
    #[test]
    fn test_is_server_error() {
        assert!(BaseError::Unknown("u".into()).is_server_error());
        assert!(BaseError::DatabaseConnectionFailed("c".into()).is_server_error());
        assert!(BaseError::PluginAlreadyRegistered("p".into()).is_server_error());
        assert!(!BaseError::ParamInvalid("k".into(), "r".into()).is_server_error());
        assert!(!BaseError::Unauthorized("u".into()).is_server_error());
    }
}
