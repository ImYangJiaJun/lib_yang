/// 数据库错误类型
///
/// 标注 `#[non_exhaustive]`：未来新增变体不构成跨 crate 破坏性变更（下游 match 需
/// 带 `_` 臂）。同 crate 内的穷举 match 不受影响。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DbError {
    #[error("连接错误: {0}")]
    ConnectionError(String),

    #[error("查询错误: {0}")]
    QueryError(String),

    #[error("SQL 语法错误: {0}")]
    SqlSyntaxError(String),

    #[error("约束错误: {0}")]
    ConstraintError(String),

    #[error("类型转换错误: {0}")]
    TypeConversionError(String),

    #[error("序列化错误: {0}")]
    SerializationError(String),

    #[error("反序列化错误: {0}")]
    DeserializationError(String),

    #[error("事务错误: {0}")]
    TransactionError(String),

    #[error("表不存在: {0}")]
    TableNotFound(String),

    #[error("缺少 WHERE 条件，禁止全表操作")]
    MissingWhereClause,

    // Redis 相关错误
    #[error("Redis 连接错误: {0}")]
    RedisConnectionError(String),

    #[error("Redis 命令错误: {0}")]
    RedisCommandError(String),

    #[error("Redis 连接池错误: {0}")]
    RedisPoolError(String),

    #[error("Redis 类型转换错误: {0}")]
    RedisTypeConversionError(String),

    #[error("Redis 超时错误: {0}")]
    RedisTimeoutError(String),

    /// 结构化 Redis 错误：保留协议层 `ErrorKind` 与重试建议，避免把
    /// `LOADING`/`TRYAGAIN`/`NOSCRIPT` 等可重试错误压成不可重试的字符串。
    #[error("Redis 错误: {0}")]
    RedisError(RedisErrorInfo),

    /// Redis 连接池已关闭（不可重试）。
    #[error("Redis 连接池已关闭")]
    RedisPoolClosed,

    #[error("HAVING 子句需要 GROUP BY 子句")]
    MissingGroupByClause,

    /// 不支持的操作符错误，当传入的操作符不在支持集合中时返回
    #[error("不支持的操作符: {0}")]
    UnsupportedOperator(String),

    /// 参数非法（调用方过错）：批量数据列集不一致、空数据、batch_size 为 0 等。
    ///
    /// 与 [`DbError::SerializationError`] 区分——后者专表「(反)序列化失败」，
    /// 此变体专表「入参本身不合法」，便于调用方按变体精确分流（DB-14）。
    #[error("参数非法: {0}")]
    InvalidArgument(String),

    #[error("未知错误: {0}")]
    Unknown(String),

    /// 连接池已关闭（非瞬时错误，不应重试）。
    #[error("连接池已关闭")]
    PoolClosed,

    /// 携带 SQLSTATE 与约束元数据的结构化数据库错误。
    ///
    /// `sqlx::Error::Database` 一律映射到本变体，便于下游按
    /// [`SqlErrorInfo::kind`] / [`SqlErrorInfo::code`] 精确分流，
    /// 而不是解析本地化报文。旧的字符串变体保留用于手工构造与向后兼容。
    #[error("数据库错误 [{code}]: {message}", code = .0.code(), message = .0.message())]
    DatabaseError(Box<SqlErrorInfo>),
}

/// SQLSTATE 归一后的错误类别。
///
/// 覆盖 PostgreSQL 与 MySQL 的常见完整性/并发/连接错误；未识别码落到
/// [`SqlErrorKind::Other`]，调用方仍可通过 [`SqlErrorInfo::code`] 读原始码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlErrorKind {
    /// 唯一约束冲突（SQLSTATE 23505 / MySQL 23000 且报文可判定）。
    UniqueViolation,
    /// 外键约束冲突（23503）。
    ForeignKeyViolation,
    /// 非空约束冲突（23502）。
    NotNullViolation,
    /// CHECK 约束冲突（23514）。
    CheckViolation,
    /// 排他约束冲突（23P01）。
    ExclusionViolation,
    /// 序列化失败，可整事务重试（40001）。
    SerializationFailure,
    /// 死锁检测，可整事务重试（40P01）。
    DeadlockDetected,
    /// 查询被取消（57014）。
    QueryCanceled,
    /// 锁不可用（55P03）。
    LockNotAvailable,
    /// 事务已中止，必须先 ROLLBACK（25P02）。
    InFailedTransaction,
    /// 连接失败/断开（08000/08003/08006 等）。
    ConnectionFailure,
    /// 连接数超限（53300）。
    TooManyConnections,
    /// 表不存在（42P01 / MySQL 42S02）。
    UndefinedTable,
    /// 列不存在（42703）。
    UndefinedColumn,
    /// SQL 语法错误（42601 / 42000）。
    SyntaxError,
    /// 文本表示的输入非法（22P02）。
    InvalidTextRepresentation,
    /// 字符串超长（22001）。
    StringDataRightTruncation,
    /// 数值超出范围（22003）。
    NumericValueOutOfRange,
    /// 未归一的 SQLSTATE。
    Other,
}

/// 结构化数据库错误信息（SQLSTATE + 常用诊断字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SqlErrorInfo {
    code: String,
    message: String,
    constraint: Option<String>,
    schema: Option<String>,
    table: Option<String>,
    column: Option<String>,
    detail: Option<String>,
    hint: Option<String>,
    /// 当 SQLSTATE 不足以区分时（MySQL 的 23000）由驱动报文补充的类别。
    kind_hint: Option<SqlErrorKind>,
}

impl SqlErrorInfo {
    /// 创建仅含 SQLSTATE 与报文的错误信息。
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            constraint: None,
            schema: None,
            table: None,
            column: None,
            detail: None,
            hint: None,
            kind_hint: None,
        }
    }

    /// 设置约束名。
    pub fn with_constraint(mut self, constraint: Option<String>) -> Self {
        self.constraint = constraint;
        self
    }

    /// 设置 schema 名。
    pub fn with_schema(mut self, schema: Option<String>) -> Self {
        self.schema = schema;
        self
    }

    /// 设置表名。
    pub fn with_table(mut self, table: Option<String>) -> Self {
        self.table = table;
        self
    }

    /// 设置列名。
    pub fn with_column(mut self, column: Option<String>) -> Self {
        self.column = column;
        self
    }

    /// 设置 DETAIL 诊断信息。
    pub fn with_detail(mut self, detail: Option<String>) -> Self {
        self.detail = detail;
        self
    }

    /// 设置 HINT 诊断信息。
    pub fn with_hint(mut self, hint: Option<String>) -> Self {
        self.hint = hint;
        self
    }

    /// 在 SQLSTATE 不足以区分时补充错误类别（如 MySQL 23000）。
    pub fn with_kind_hint(mut self, kind: SqlErrorKind) -> Self {
        self.kind_hint = Some(kind);
        self
    }

    /// 原始 SQLSTATE 五字符码（MySQL 可能返回更短的类码）。
    pub fn code(&self) -> &str {
        &self.code
    }

    /// 数据库返回的报文。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 触发错误的约束名（如唯一索引名）。
    pub fn constraint(&self) -> Option<&str> {
        self.constraint.as_deref()
    }

    /// 相关 schema。
    pub fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    /// 相关表。
    pub fn table(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 相关列。
    pub fn column(&self) -> Option<&str> {
        self.column.as_deref()
    }

    /// DETAIL 诊断信息。
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// HINT 诊断信息。
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// SQLSTATE 类别（前两位），如 `23`、`40`、`42`。
    pub fn class(&self) -> &str {
        self.code.get(..2).unwrap_or(self.code.as_str())
    }

    /// 归一后的错误类别。
    pub fn kind(&self) -> SqlErrorKind {
        if let Some(kind) = self.kind_hint {
            return kind;
        }
        use SqlErrorKind as K;
        match self.code.as_str() {
            "23505" => K::UniqueViolation,
            "23503" => K::ForeignKeyViolation,
            "23502" => K::NotNullViolation,
            "23514" => K::CheckViolation,
            "23P01" => K::ExclusionViolation,
            "40001" => K::SerializationFailure,
            "40P01" => K::DeadlockDetected,
            "57014" => K::QueryCanceled,
            "55P03" => K::LockNotAvailable,
            "25P02" => K::InFailedTransaction,
            "08000" | "08001" | "08003" | "08004" | "08006" => K::ConnectionFailure,
            "53300" => K::TooManyConnections,
            "42P01" | "42S02" => K::UndefinedTable,
            "42703" => K::UndefinedColumn,
            "42601" | "42000" => K::SyntaxError,
            "22P02" => K::InvalidTextRepresentation,
            "22001" => K::StringDataRightTruncation,
            "22003" => K::NumericValueOutOfRange,
            _ => K::Other,
        }
    }

    /// 是否为可安全重试的瞬时错误。
    ///
    /// 仅 PostgreSQL 的 `40001`（序列化失败）、`40P01`（死锁）、`08` 类
    /// （连接失败/断开）与 `53` 类（资源不足，如连接数超限）返回 `true`。
    /// 其余 SQLSTATE 一律不可重试，尤其是：
    ///
    /// - `55P03` 锁不可用（锁等待超时）：属于 [`DbErrorCategory::Timeout`]，
    ///   是否重试由调用方决定；
    /// - `57014` 语句被取消：同样归 [`DbErrorCategory::Timeout`]，不是瞬时故障；
    /// - `25P02` 事务已中止：必须先 `ROLLBACK`，重试只会在中止事务上继续失败。
    pub fn is_retryable(&self) -> bool {
        matches!(self.code(), "40001" | "40P01") || matches!(self.class(), "08" | "53")
    }
}

/// 数据库错误的引擎级分类（与具体 HTTP/传输层无关）。
///
/// 用于下游统一适配（弹性重试、错误上报分桶）。`Transient` 表示瞬时故障、可重试；
/// 其余为确定性错误，重试无益。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbErrorCategory {
    /// 调用方过错（SQL 语法、缺 WHERE/GROUP BY、不支持的操作符、参数非法、(反)序列化）
    Client,
    /// 约束冲突（唯一键/外键/非空/检查）
    Conflict,
    /// 资源不存在（表不存在）
    NotFound,
    /// 瞬时故障（连接/超时/连接池），可重试
    Transient,
    /// 操作被取消或锁等待超时（57014 / 55P03），是否重试由调用方决定
    Timeout,
    /// 服务端/未知错误
    Server,
}

/// Redis 协议层错误类别（与 `redis::ErrorKind` 对齐，但不暴露上游类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedisErrorKind {
    /// 超时
    Timeout,
    /// 连接断开 / 连接拒绝
    Connection,
    /// IO 错误
    Io,
    /// 类型不匹配
    Type,
    /// 事务因先前错误被丢弃
    ExecAbort,
    /// 服务端正在加载数据
    BusyLoading,
    /// 服务端要求重试
    TryAgain,
    /// 脚本不存在
    NoScript,
    /// 其他服务端响应错误
    Response,
    /// 认证失败
    Authentication,
    /// Cluster 拓扑错误（MOVED/ASK/CLUSTERDOWN 等）
    Cluster,
    /// 其他
    Other,
}

/// Redis 建议的重试方式（与 `redis::RetryMethod` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedisRetryMethod {
    /// 不重试（永久错误）
    NoRetry,
    /// 立即重试
    RetryImmediately,
    /// 延迟后重试
    WaitAndRetry,
    /// 重新建立连接后重试
    Reconnect,
}

/// 结构化 Redis 错误信息。
#[derive(Debug, Clone)]
pub struct RedisErrorInfo {
    /// 协议层错误类别
    pub kind: RedisErrorKind,
    /// 建议的重试方式
    pub retry_method: RedisRetryMethod,
    /// 错误消息（不含命令参数与 key）
    pub message: String,
}

impl RedisErrorInfo {
    /// 是否为可重试错误。
    pub fn is_retryable(&self) -> bool {
        !matches!(self.retry_method, RedisRetryMethod::NoRetry)
    }
}

impl std::fmt::Display for RedisErrorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}/{:?}] {}",
            self.kind, self.retry_method, self.message
        )
    }
}

impl DbError {
    /// 稳定错误码（8xxxxx 段，独立于 yang-base 的 BaseError 码命名空间）。
    ///
    /// 与 [`DbError::category`] 同源维护；新增变体需同步补码。
    pub fn code(&self) -> u32 {
        match self {
            DbError::ConnectionError(_) => 800001,
            DbError::QueryError(_) => 800002,
            DbError::SqlSyntaxError(_) => 800003,
            DbError::ConstraintError(_) => 800004,
            DbError::TypeConversionError(_) => 800005,
            DbError::SerializationError(_) => 800006,
            DbError::DeserializationError(_) => 800007,
            DbError::TransactionError(_) => 800008,
            DbError::TableNotFound(_) => 800009,
            DbError::MissingWhereClause => 800010,
            DbError::RedisConnectionError(_) => 810001,
            DbError::RedisCommandError(_) => 810002,
            DbError::RedisPoolError(_) => 810003,
            DbError::RedisTypeConversionError(_) => 810004,
            DbError::RedisTimeoutError(_) => 810005,
            DbError::RedisError(_) => 810006,
            DbError::RedisPoolClosed => 810007,
            DbError::MissingGroupByClause => 800011,
            DbError::UnsupportedOperator(_) => 800012,
            DbError::InvalidArgument(_) => 800013,
            DbError::Unknown(_) => 899999,
            DbError::PoolClosed => 800015,
            DbError::DatabaseError(_) => 800014,
        }
    }

    /// 引擎级分类。是 [`DbError::is_retryable`] 的单一事实源。
    pub fn category(&self) -> DbErrorCategory {
        use DbErrorCategory as C;
        match self {
            // 瞬时：连接/超时/连接池 —— 可重试
            DbError::ConnectionError(_)
            | DbError::RedisConnectionError(_)
            | DbError::RedisPoolError(_)
            | DbError::RedisTimeoutError(_) => C::Transient,
            // 结构化 Redis 错误：按协议层建议的重试方式分类
            DbError::RedisError(info) if info.is_retryable() => C::Transient,
            DbError::RedisError(_) | DbError::RedisPoolClosed => C::Server,
            // 约束冲突
            DbError::ConstraintError(_) => C::Conflict,
            // 不存在
            DbError::TableNotFound(_) => C::NotFound,
            // 调用方过错：语法/缺子句/不支持操作符/(反)序列化/类型转换
            DbError::SqlSyntaxError(_)
            | DbError::MissingWhereClause
            | DbError::MissingGroupByClause
            | DbError::UnsupportedOperator(_)
            | DbError::InvalidArgument(_)
            | DbError::SerializationError(_)
            | DbError::DeserializationError(_)
            | DbError::TypeConversionError(_)
            | DbError::RedisTypeConversionError(_) => C::Client,
            // 服务端/未知：查询错误、Redis 命令错误、未知
            DbError::QueryError(_)
            | DbError::TransactionError(_)
            | DbError::RedisCommandError(_)
            | DbError::Unknown(_)
            | DbError::PoolClosed => C::Server,
            // 结构化 SQLSTATE 按错误类别分流
            DbError::DatabaseError(info) => match info.kind() {
                SqlErrorKind::UniqueViolation
                | SqlErrorKind::ForeignKeyViolation
                | SqlErrorKind::NotNullViolation
                | SqlErrorKind::CheckViolation
                | SqlErrorKind::ExclusionViolation => C::Conflict,
                SqlErrorKind::UndefinedTable => C::NotFound,
                SqlErrorKind::SerializationFailure
                | SqlErrorKind::DeadlockDetected
                | SqlErrorKind::ConnectionFailure
                | SqlErrorKind::TooManyConnections => C::Transient,
                // 57014 语句取消与 55P03 锁等待超时不是瞬时故障：
                // 前者通常由调用方超时/取消主动触发，后者需由调用方决定是否重试。
                SqlErrorKind::QueryCanceled | SqlErrorKind::LockNotAvailable => C::Timeout,
                SqlErrorKind::InFailedTransaction
                | SqlErrorKind::UndefinedColumn
                | SqlErrorKind::SyntaxError
                | SqlErrorKind::InvalidTextRepresentation
                | SqlErrorKind::StringDataRightTruncation
                | SqlErrorKind::NumericValueOutOfRange => C::Client,
                SqlErrorKind::Other => {
                    if info.is_retryable() {
                        // 08xxx/53xxx 与 40001/40P01 之外的 40 类不重试。
                        C::Transient
                    } else if info.class() == "23" {
                        C::Conflict
                    } else if info.class() == "57" {
                        C::Timeout
                    } else {
                        C::Server
                    }
                }
            },
        }
    }

    /// 是否为可重试的瞬时错误（等价 `category() == Transient`）。
    pub fn is_retryable(&self) -> bool {
        self.category() == DbErrorCategory::Transient
    }

    /// 是否为约束冲突（手工构造的旧变体或结构化 SQLSTATE 的 23 类）。
    pub fn is_constraint_violation(&self) -> bool {
        match self {
            DbError::ConstraintError(_) => true,
            DbError::DatabaseError(info) => matches!(
                info.kind(),
                SqlErrorKind::UniqueViolation
                    | SqlErrorKind::ForeignKeyViolation
                    | SqlErrorKind::NotNullViolation
                    | SqlErrorKind::CheckViolation
                    | SqlErrorKind::ExclusionViolation
            ),
            _ => false,
        }
    }

    /// 结构化 SQLSTATE 错误信息（仅 [`DbError::DatabaseError`] 有值）。
    pub fn sql_error(&self) -> Option<&SqlErrorInfo> {
        match self {
            DbError::DatabaseError(info) => Some(info.as_ref()),
            _ => None,
        }
    }

    /// 原始 SQLSTATE 码。
    pub fn sqlstate(&self) -> Option<&str> {
        self.sql_error().map(SqlErrorInfo::code)
    }

    /// 触发错误的约束名。
    pub fn constraint(&self) -> Option<&str> {
        self.sql_error().and_then(SqlErrorInfo::constraint)
    }
}

/// MySQL 把唯一/外键/非空/CHECK 统一报为 SQLSTATE 23000，这里按驱动报文细分。
///
/// 仅在结构化字段（`constraint()`）不足以判定时使用；它是对 MySQL 协议限制的
/// 兜底，不是主要分流路径。
#[cfg_attr(not(any(feature = "mysql", feature = "postgres")), allow(dead_code))]
fn refine_mysql_integrity(info: SqlErrorInfo) -> SqlErrorInfo {
    let message = info.message();
    if message.contains("Duplicate entry") {
        info.with_kind_hint(SqlErrorKind::UniqueViolation)
    } else if message.contains("foreign key constraint") {
        info.with_kind_hint(SqlErrorKind::ForeignKeyViolation)
    } else if message.contains("cannot be null") {
        info.with_kind_hint(SqlErrorKind::NotNullViolation)
    } else if message.contains("Check constraint") {
        info.with_kind_hint(SqlErrorKind::CheckViolation)
    } else {
        info
    }
}

/// 从 sqlx::Error 转换为 DbError
#[cfg(any(feature = "mysql", feature = "postgres"))]
impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Configuration(_) => DbError::ConnectionError(format!("配置错误: {}", err)),
            sqlx::Error::Database(db_err) => {
                let code = db_err
                    .code()
                    .map(|code| code.into_owned())
                    .unwrap_or_default();
                let message = db_err.message().to_string();

                let mut info = SqlErrorInfo::new(code, message)
                    .with_constraint(db_err.constraint().map(str::to_string))
                    .with_table(db_err.table().map(str::to_string));

                // PostgreSQL 驱动额外携带 schema/column/DETAIL/HINT 诊断字段。
                #[cfg(feature = "postgres")]
                if let Some(pg) = db_err.try_downcast_ref::<sqlx::postgres::PgDatabaseError>() {
                    let constraint = pg
                        .constraint()
                        .map(str::to_string)
                        .or_else(|| info.constraint().map(str::to_string));
                    let table = pg
                        .table()
                        .map(str::to_string)
                        .or_else(|| info.table().map(str::to_string));
                    let schema = pg.schema().map(str::to_string);
                    let column = pg.column().map(str::to_string);
                    let detail = pg.detail().map(str::to_string);
                    let hint = pg.hint().map(str::to_string);
                    info = info
                        .with_constraint(constraint)
                        .with_schema(schema)
                        .with_table(table)
                        .with_column(column)
                        .with_detail(detail)
                        .with_hint(hint);
                }

                // MySQL 的完整性错误统一报 23000，需要按报文细分唯一/外键/非空/CHECK，
                // 否则下游无法区分「可归因的输入冲突」与真正的数据完整性故障。
                if info.code().starts_with("23") && info.kind() == SqlErrorKind::Other {
                    info = refine_mysql_integrity(info);
                }

                DbError::DatabaseError(Box::new(info))
            }
            sqlx::Error::Io(_) => DbError::ConnectionError(format!("IO 错误: {}", err)),
            sqlx::Error::Tls(_) => DbError::ConnectionError(format!("TLS 错误: {}", err)),
            sqlx::Error::Protocol(_) => DbError::QueryError(format!("协议错误: {}", err)),
            sqlx::Error::RowNotFound => DbError::QueryError("未找到记录".to_string()),
            sqlx::Error::TypeNotFound { type_name } => {
                DbError::TypeConversionError(format!("类型未找到: {}", type_name))
            }
            sqlx::Error::ColumnIndexOutOfBounds { index, len } => {
                DbError::QueryError(format!("列索引越界: {} (总列数: {})", index, len))
            }
            sqlx::Error::ColumnNotFound(col) => DbError::QueryError(format!("列不存在: {}", col)),
            sqlx::Error::ColumnDecode { index, source } => {
                DbError::TypeConversionError(format!("列 {} 解码失败: {}", index, source))
            }
            sqlx::Error::Decode(source) => {
                DbError::DeserializationError(format!("解码失败: {}", source))
            }
            sqlx::Error::PoolTimedOut => DbError::ConnectionError("连接池超时".to_string()),
            sqlx::Error::PoolClosed => DbError::PoolClosed,
            sqlx::Error::WorkerCrashed => DbError::ConnectionError("工作线程崩溃".to_string()),
            _ => DbError::Unknown(format!("未知错误: {}", err)),
        }
    }
}

/// 将 redis 协议层的 `ErrorKind` 映射为稳定的 [`RedisErrorKind`]。
#[cfg(feature = "redis")]
pub fn map_redis_error_kind(kind: redis::ErrorKind) -> RedisErrorKind {
    use redis::ErrorKind;
    match kind {
        ErrorKind::ResponseError => RedisErrorKind::Response,
        ErrorKind::ParseError => RedisErrorKind::Other,
        ErrorKind::AuthenticationFailed => RedisErrorKind::Authentication,
        ErrorKind::TypeError => RedisErrorKind::Type,
        ErrorKind::ExecAbortError => RedisErrorKind::ExecAbort,
        ErrorKind::BusyLoadingError => RedisErrorKind::BusyLoading,
        ErrorKind::NoScriptError => RedisErrorKind::NoScript,
        ErrorKind::Moved
        | ErrorKind::Ask
        | ErrorKind::ClusterDown
        | ErrorKind::CrossSlot
        | ErrorKind::MasterDown
        | ErrorKind::ClusterConnectionNotFound => RedisErrorKind::Cluster,
        ErrorKind::TryAgain => RedisErrorKind::TryAgain,
        ErrorKind::IoError => RedisErrorKind::Io,
        _ => RedisErrorKind::Other,
    }
}

/// 将 redis 协议层的 `RetryMethod` 映射为稳定的 [`RedisRetryMethod`]。
#[cfg(feature = "redis")]
pub fn map_redis_retry_method(method: redis::RetryMethod) -> RedisRetryMethod {
    use redis::RetryMethod;
    match method {
        RetryMethod::NoRetry => RedisRetryMethod::NoRetry,
        RetryMethod::RetryImmediately => RedisRetryMethod::RetryImmediately,
        RetryMethod::WaitAndRetry => RedisRetryMethod::WaitAndRetry,
        RetryMethod::Reconnect
        | RetryMethod::ReconnectFromInitialConnections
        | RetryMethod::MovedRedirect
        | RetryMethod::AskRedirect => RedisRetryMethod::Reconnect,
        // RetryMethod 是 non_exhaustive：未知策略按不可重试处理，避免误重试。
        _ => RedisRetryMethod::NoRetry,
    }
}

/// 从 redis::RedisError 转换为 DbError
#[cfg(feature = "redis")]
impl From<redis::RedisError> for DbError {
    fn from(err: redis::RedisError) -> Self {
        // 用协议层稳定 API 分类，而非脆弱的 Display 子串匹配：
        // - 超时优先（is_timeout 涵盖连接超时与响应超时）
        // - 连接断开 / IO / 连接拒绝 → 连接错误
        // - 类型不匹配（TypeError）→ 类型转换错误
        // - 服务端明确要求重试（LOADING/TRYAGAIN/MOVED 等）→ 结构化可重试错误
        // - 其余响应错误 → 命令错误（不可重试兜底）
        use redis::ErrorKind;
        if err.is_timeout() {
            return DbError::RedisTimeoutError(format!("超时错误: {}", err));
        }
        if err.is_connection_dropped() || err.is_io_error() || err.is_connection_refusal() {
            return DbError::RedisConnectionError(format!("连接错误: {}", err));
        }
        let kind = err.kind();
        if matches!(kind, ErrorKind::TypeError) {
            // redis-rs 的 Display 会附带 `(response was <Value>)`，可能包含缓存值/
            // token。类型错误只返回固定文案，绝不拼接原始响应。
            return DbError::RedisTypeConversionError(
                "Redis 响应类型不匹配（原始响应已省略）".to_string(),
            );
        }
        let info = RedisErrorInfo {
            kind: map_redis_error_kind(kind),
            retry_method: map_redis_retry_method(err.retry_method()),
            message: err.to_string(),
        };
        match kind {
            // 保留历史变体：普通响应错误与事务中止仍归为命令错误。
            ErrorKind::ResponseError | ErrorKind::ExecAbortError | ErrorKind::ExtensionError => {
                DbError::RedisCommandError(format!("Redis 错误: {}", err))
            }
            _ => DbError::RedisError(info),
        }
    }
}

/// 从 deadpool_redis::PoolError 转换为 DbError
#[cfg(feature = "redis")]
impl From<deadpool_redis::PoolError> for DbError {
    fn from(err: deadpool_redis::PoolError) -> Self {
        match err {
            deadpool_redis::PoolError::Timeout(_) => {
                DbError::RedisTimeoutError("连接池获取连接超时".to_string())
            }
            // 关闭是确定性状态，不能归入「可重试的瞬时错误」。
            deadpool_redis::PoolError::Closed => DbError::RedisPoolClosed,
            deadpool_redis::PoolError::NoRuntimeSpecified => {
                DbError::RedisPoolError("未指定运行时".to_string())
            }
            deadpool_redis::PoolError::PostCreateHook(source) => {
                DbError::RedisPoolError(format!("连接创建后钩子失败: {:?}", source))
            }
            deadpool_redis::PoolError::Backend(err) => DbError::from(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_chinese() {
        // 测试所有 19 个错误变体的中文消息
        let errors = vec![
            DbError::ConnectionError("测试".to_string()),
            DbError::QueryError("测试".to_string()),
            DbError::SqlSyntaxError("测试".to_string()),
            DbError::ConstraintError("测试".to_string()),
            DbError::TypeConversionError("测试".to_string()),
            DbError::SerializationError("测试".to_string()),
            DbError::DeserializationError("测试".to_string()),
            DbError::TransactionError("测试".to_string()),
            DbError::TableNotFound("测试".to_string()),
            DbError::MissingWhereClause,
            DbError::RedisConnectionError("测试".to_string()),
            DbError::RedisCommandError("测试".to_string()),
            DbError::RedisPoolError("测试".to_string()),
            DbError::RedisTypeConversionError("测试".to_string()),
            DbError::RedisTimeoutError("测试".to_string()),
            DbError::RedisError(RedisErrorInfo {
                kind: RedisErrorKind::TryAgain,
                retry_method: RedisRetryMethod::WaitAndRetry,
                message: "测试".to_string(),
            }),
            DbError::RedisPoolClosed,
            DbError::MissingGroupByClause,
            DbError::UnsupportedOperator("测试".to_string()),
            DbError::InvalidArgument("测试".to_string()),
            DbError::Unknown("测试".to_string()),
            DbError::PoolClosed,
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("23505", "duplicate key"))),
        ];

        for error in errors {
            let msg = format!("{}", error);
            // 验证错误消息包含中文字符
            let has_chinese = msg.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'));
            assert!(has_chinese, "错误消息应该包含中文: {}", msg);
        }
    }

    #[test]
    fn test_connection_error() {
        let err = DbError::ConnectionError("无法连接到数据库".to_string());
        assert_eq!(format!("{}", err), "连接错误: 无法连接到数据库");
    }

    #[test]
    fn test_query_error() {
        let err = DbError::QueryError("查询超时".to_string());
        assert_eq!(format!("{}", err), "查询错误: 查询超时");
    }

    #[test]
    fn test_sql_syntax_error() {
        let err = DbError::SqlSyntaxError("语法错误".to_string());
        assert_eq!(format!("{}", err), "SQL 语法错误: 语法错误");
    }

    #[test]
    fn test_constraint_error() {
        let err = DbError::ConstraintError("主键冲突".to_string());
        assert_eq!(format!("{}", err), "约束错误: 主键冲突");
    }

    #[test]
    fn test_type_conversion_error() {
        let err = DbError::TypeConversionError("无法转换类型".to_string());
        assert_eq!(format!("{}", err), "类型转换错误: 无法转换类型");
    }

    /// 对抗性验证：Redis 类型错误必须精确归类，普通响应错误不能混入类型转换错误。
    #[test]
    #[cfg(feature = "redis")]
    fn test_redis_error_classification_distinguishes_type_and_command_errors() {
        let type_error = redis::RedisError::from((redis::ErrorKind::TypeError, "返回值类型不匹配"));
        let response_error =
            redis::RedisError::from((redis::ErrorKind::ResponseError, "命令执行失败"));

        assert!(matches!(
            DbError::from(type_error),
            DbError::RedisTypeConversionError(_)
        ));
        assert!(matches!(
            DbError::from(response_error),
            DbError::RedisCommandError(_)
        ));
    }

    #[test]
    fn test_serialization_error() {
        let err = DbError::SerializationError("序列化失败".to_string());
        assert_eq!(format!("{}", err), "序列化错误: 序列化失败");
    }

    #[test]
    fn test_deserialization_error() {
        let err = DbError::DeserializationError("反序列化失败".to_string());
        assert_eq!(format!("{}", err), "反序列化错误: 反序列化失败");
    }

    #[test]
    fn test_transaction_error() {
        let err = DbError::TransactionError("事务已提交".to_string());
        assert_eq!(format!("{}", err), "事务错误: 事务已提交");
    }

    #[test]
    fn test_table_not_found() {
        let err = DbError::TableNotFound("users".to_string());
        assert_eq!(format!("{}", err), "表不存在: users");
    }

    #[test]
    fn test_missing_where_clause() {
        let err = DbError::MissingWhereClause;
        assert_eq!(format!("{}", err), "缺少 WHERE 条件，禁止全表操作");
    }

    #[test]
    fn test_unknown_error() {
        let err = DbError::Unknown("未知问题".to_string());
        assert_eq!(format!("{}", err), "未知错误: 未知问题");
    }

    #[test]
    fn test_error_implements_std_error() {
        // 验证 DbError 实现了 std::error::Error trait
        let err = DbError::QueryError("测试".to_string());
        let _: &dyn std::error::Error = &err;
    }

    // Redis 错误测试
    #[test]
    fn test_redis_connection_error() {
        let err = DbError::RedisConnectionError("连接失败".to_string());
        assert_eq!(format!("{}", err), "Redis 连接错误: 连接失败");
    }

    #[test]
    fn test_redis_command_error() {
        let err = DbError::RedisCommandError("命令执行失败".to_string());
        assert_eq!(format!("{}", err), "Redis 命令错误: 命令执行失败");
    }

    #[test]
    fn test_redis_pool_error() {
        let err = DbError::RedisPoolError("连接池错误".to_string());
        assert_eq!(format!("{}", err), "Redis 连接池错误: 连接池错误");
    }

    #[test]
    fn test_redis_type_conversion_error() {
        let err = DbError::RedisTypeConversionError("类型转换失败".to_string());
        assert_eq!(format!("{}", err), "Redis 类型转换错误: 类型转换失败");
    }

    #[test]
    fn test_redis_timeout_error() {
        let err = DbError::RedisTimeoutError("操作超时".to_string());
        assert_eq!(format!("{}", err), "Redis 超时错误: 操作超时");
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_pool_error_conversion() {
        // 测试 deadpool_redis::PoolError 转换
        let pool_err = deadpool_redis::PoolError::Closed;
        let db_err: DbError = pool_err.into();
        assert!(matches!(db_err, DbError::RedisPoolClosed));
        assert!(!db_err.is_retryable());
    }

    // 验证需求: 4.4 — UnsupportedOperator 变体错误消息格式测试
    #[test]
    fn test_unsupported_operator_error_message() {
        // 验证错误消息格式为 "不支持的操作符: {op}"
        let op = "BETWEEN";
        let err = DbError::UnsupportedOperator(op.to_string());
        assert_eq!(format!("{}", err), "不支持的操作符: BETWEEN");
    }

    #[test]
    fn test_unsupported_operator_empty_string() {
        // 验证空字符串操作符的错误消息格式
        let err = DbError::UnsupportedOperator(String::new());
        assert_eq!(format!("{}", err), "不支持的操作符: ");
    }

    #[test]
    fn test_unsupported_operator_special_chars() {
        // 验证包含特殊字符的操作符错误消息格式
        let err = DbError::UnsupportedOperator("<>".to_string());
        assert_eq!(format!("{}", err), "不支持的操作符: <>");
    }

    #[test]
    fn test_unsupported_operator_implements_std_error() {
        // 验证 UnsupportedOperator 变体实现了 std::error::Error trait
        let err = DbError::UnsupportedOperator("IN".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_unsupported_operator_message_contains_chinese() {
        // 验证错误消息包含中文字符（"不支持的操作符"）
        let err = DbError::UnsupportedOperator("XOR".to_string());
        let msg = format!("{}", err);
        let has_chinese = msg.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'));
        assert!(has_chinese, "错误消息应该包含中文: {}", msg);
    }

    /// DB-7：category() 覆盖全部 19 个变体
    #[test]
    fn test_db_error_category_coverage() {
        use super::DbErrorCategory as C;
        // Transient：连接/超时/连接池
        assert_eq!(
            DbError::ConnectionError("c".into()).category(),
            C::Transient
        );
        assert_eq!(
            DbError::RedisConnectionError("c".into()).category(),
            C::Transient
        );
        assert_eq!(DbError::RedisPoolError("p".into()).category(), C::Transient);
        assert_eq!(
            DbError::RedisTimeoutError("t".into()).category(),
            C::Transient
        );
        assert_eq!(
            DbError::RedisError(RedisErrorInfo {
                kind: RedisErrorKind::TryAgain,
                retry_method: RedisRetryMethod::WaitAndRetry,
                message: "t".into(),
            })
            .category(),
            C::Transient
        );
        assert_eq!(
            DbError::RedisError(RedisErrorInfo {
                kind: RedisErrorKind::Response,
                retry_method: RedisRetryMethod::NoRetry,
                message: "r".into(),
            })
            .category(),
            C::Server
        );
        assert_eq!(DbError::RedisPoolClosed.category(), C::Server);
        assert!(!DbError::RedisPoolClosed.is_retryable());
        // Conflict：约束冲突
        assert_eq!(DbError::ConstraintError("c".into()).category(), C::Conflict);
        // NotFound：表不存在
        assert_eq!(DbError::TableNotFound("t".into()).category(), C::NotFound);
        // Client：语法/缺子句/不支持操作符/参数非法/类型转换/(反)序列化/Redis类型转换
        assert_eq!(DbError::SqlSyntaxError("s".into()).category(), C::Client);
        assert_eq!(DbError::MissingWhereClause.category(), C::Client);
        assert_eq!(DbError::MissingGroupByClause.category(), C::Client);
        assert_eq!(
            DbError::UnsupportedOperator("o".into()).category(),
            C::Client
        );
        assert_eq!(DbError::InvalidArgument("a".into()).category(), C::Client);
        assert_eq!(
            DbError::TypeConversionError("t".into()).category(),
            C::Client
        );
        assert_eq!(
            DbError::SerializationError("s".into()).category(),
            C::Client
        );
        assert_eq!(
            DbError::DeserializationError("d".into()).category(),
            C::Client
        );
        assert_eq!(
            DbError::RedisTypeConversionError("t".into()).category(),
            C::Client
        );
        // Server：查询错误/事务错误/Redis命令错误/未知
        assert_eq!(DbError::QueryError("q".into()).category(), C::Server);
        assert_eq!(DbError::TransactionError("t".into()).category(), C::Server);
        assert_eq!(DbError::RedisCommandError("c".into()).category(), C::Server);
        assert_eq!(DbError::Unknown("u".into()).category(), C::Server);
        assert_eq!(DbError::PoolClosed.category(), C::Server);
        // DatabaseError：按 SQLSTATE 分流
        assert_eq!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("23505", "dup"))).category(),
            C::Conflict
        );
        assert_eq!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("40P01", "deadlock"))).category(),
            C::Transient
        );
        assert_eq!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("57014", "canceled"))).category(),
            C::Timeout
        );
        assert_eq!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("42P01", "missing"))).category(),
            C::NotFound
        );
    }

    /// DB-7：is_retryable = (category == Transient)
    #[test]
    fn test_db_error_is_retryable() {
        assert!(DbError::ConnectionError("c".into()).is_retryable());
        assert!(DbError::RedisTimeoutError("t".into()).is_retryable());
        assert!(!DbError::QueryError("q".into()).is_retryable());
        assert!(!DbError::MissingWhereClause.is_retryable());
        assert!(!DbError::ConstraintError("c".into()).is_retryable());
        assert!(!DbError::PoolClosed.is_retryable());
        assert!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("40001", "serialization")))
                .is_retryable()
        );
        assert!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("40P01", "deadlock"))).is_retryable()
        );
        assert!(
            !DbError::DatabaseError(Box::new(SqlErrorInfo::new("23505", "unique"))).is_retryable()
        );
        assert!(
            !DbError::DatabaseError(Box::new(SqlErrorInfo::new("57014", "canceled")))
                .is_retryable()
        );
    }

    /// P0-2：逐 SQLSTATE 校验可重试分类。
    ///
    /// 只有 40001/40P01 与 08xxx、53xxx 是瞬时错误；55P03、57014 归 Timeout
    /// 且不可重试；25P02 必须先 ROLLBACK，重试无效。
    #[test]
    fn retryable_sqlstates_are_exactly_postgres_transient_classes() {
        use DbErrorCategory as C;

        for code in [
            "40001", "40P01", // 序列化失败 / 死锁
            "08000", "08001", "08003", "08006",
            "08007", // 连接失败/断开（08 类全类）
            "53300", "53200", "53100", // 资源不足（53 类全类）
        ] {
            let info = SqlErrorInfo::new(code, "transient");
            assert!(info.is_retryable(), "{code} 应可重试");
            let error = DbError::DatabaseError(Box::new(info));
            assert_eq!(error.category(), C::Transient, "{code} 应归 Transient");
            assert!(error.is_retryable(), "{code} 应可重试");
        }

        for code in [
            "55P03", // lock_timeout：锁不可用，交由调用方决定
            "57014", // statement_timeout / 主动取消
            "25P02", // 事务已中止
            "23505", "42P01", "42601", "22001", "22003", // 确定性错误
            "40000", // 40 类中非 40001/40P01 的码不得重试
            "XX000", // 内部错误
        ] {
            let info = SqlErrorInfo::new(code, "non-transient");
            assert!(!info.is_retryable(), "{code} 不应可重试");
            assert_ne!(
                DbError::DatabaseError(Box::new(info)).category(),
                C::Transient,
                "{code} 不应归 Transient"
            );
        }

        // 超时类显式归 Timeout，而非 Transient。
        for code in ["55P03", "57014", "57000"] {
            assert_eq!(
                DbError::DatabaseError(Box::new(SqlErrorInfo::new(code, "timeout"))).category(),
                C::Timeout,
                "{code} 应归 Timeout"
            );
        }
        assert_eq!(
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("25P02", "aborted"))).category(),
            C::Client
        );
    }

    /// SQLSTATE 归一、约束元数据与 MySQL 23000 细分。
    #[test]
    fn structured_sqlstate_classification() {
        let unique = SqlErrorInfo::new("23505", "duplicate key")
            .with_constraint(Some("users_email_key".to_string()))
            .with_table(Some("users".to_string()));
        assert_eq!(unique.kind(), SqlErrorKind::UniqueViolation);
        assert_eq!(unique.constraint(), Some("users_email_key"));
        assert_eq!(unique.table(), Some("users"));
        assert_eq!(unique.class(), "23");

        let mysql_duplicate =
            SqlErrorInfo::new("23000", "Duplicate entry 'x' for key 'users_email_key'");
        assert_eq!(
            refine_mysql_integrity(mysql_duplicate).kind(),
            SqlErrorKind::UniqueViolation
        );

        let mysql_fk = SqlErrorInfo::new(
            "23000",
            "Cannot add or update a child row: a foreign key constraint fails",
        );
        assert_eq!(
            refine_mysql_integrity(mysql_fk).kind(),
            SqlErrorKind::ForeignKeyViolation
        );

        let not_null = SqlErrorInfo::new("23502", "null value in column");
        assert_eq!(not_null.kind(), SqlErrorKind::NotNullViolation);
        assert!(!not_null.is_retryable());
    }

    /// DB-7：code() 唯一性（19 个变体均为独立码段）
    #[test]
    fn test_db_error_code_unique() {
        let codes: Vec<u32> = vec![
            DbError::ConnectionError("".into()).code(),
            DbError::QueryError("".into()).code(),
            DbError::SqlSyntaxError("".into()).code(),
            DbError::ConstraintError("".into()).code(),
            DbError::TypeConversionError("".into()).code(),
            DbError::SerializationError("".into()).code(),
            DbError::DeserializationError("".into()).code(),
            DbError::TransactionError("".into()).code(),
            DbError::TableNotFound("".into()).code(),
            DbError::MissingWhereClause.code(),
            DbError::RedisConnectionError("".into()).code(),
            DbError::RedisCommandError("".into()).code(),
            DbError::RedisPoolError("".into()).code(),
            DbError::RedisTypeConversionError("".into()).code(),
            DbError::RedisTimeoutError("".into()).code(),
            DbError::RedisError(RedisErrorInfo {
                kind: RedisErrorKind::Other,
                retry_method: RedisRetryMethod::NoRetry,
                message: "".into(),
            })
            .code(),
            DbError::RedisPoolClosed.code(),
            DbError::MissingGroupByClause.code(),
            DbError::UnsupportedOperator("".into()).code(),
            DbError::InvalidArgument("".into()).code(),
            DbError::Unknown("".into()).code(),
            DbError::PoolClosed.code(),
            DbError::DatabaseError(Box::new(SqlErrorInfo::new("23505", ""))).code(),
        ];
        let mut sorted = codes.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(codes.len(), sorted.len(), "所有变体的 code 应全部唯一");
        // 所有码在 8xxxxx 段
        for &c in &codes {
            assert!((800000..900000).contains(&c), "code {} 不在 8xxxxx 段", c);
        }
    }
}
