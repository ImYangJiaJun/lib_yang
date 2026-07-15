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
    /// 服务端/未知错误
    Server,
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
            DbError::MissingGroupByClause => 800011,
            DbError::UnsupportedOperator(_) => 800012,
            DbError::InvalidArgument(_) => 800013,
            DbError::Unknown(_) => 899999,
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
            | DbError::Unknown(_) => C::Server,
        }
    }

    /// 是否为可重试的瞬时错误（等价 `category() == Transient`）。
    pub fn is_retryable(&self) -> bool {
        self.category() == DbErrorCategory::Transient
    }
}

/// 从 sqlx::Error 转换为 DbError
impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Configuration(_) => DbError::ConnectionError(format!("配置错误: {}", err)),
            sqlx::Error::Database(db_err) => {
                let code = db_err.code().unwrap_or_default();
                let message = db_err.message();

                // 数据库错误码映射（同时覆盖 MySQL 与 PostgreSQL）
                //
                // MySQL 使用 ANSI SQLSTATE 子集，PostgreSQL 使用完整 SQLSTATE（5 位）。
                // 两者在约束冲突 / 表不存在 / 语法错误上的常见码不同，这里统一映射，
                // 使 yang-db 的 MySQL 与 PostgreSQL 后端返回一致的 DbError 语义。
                match code.as_ref() {
                    // 约束冲突：MySQL 23000；PostgreSQL 类 23xxx（唯一/外键/非空/检查）
                    "23000" | "23001" | "23502" | "23503" | "23505" | "23514" => {
                        DbError::ConstraintError(message.to_string())
                    }
                    // 表/视图不存在：MySQL 42S02；PostgreSQL 42P01
                    "42S02" | "42P01" => DbError::TableNotFound(message.to_string()),
                    // 语法/访问规则错误：MySQL 42000；PostgreSQL 42601(语法) / 42703(列不存在) / 42P02
                    "42000" | "42601" | "42703" | "42P02" => {
                        DbError::SqlSyntaxError(message.to_string())
                    }
                    _ => DbError::QueryError(format!("数据库错误 [{}]: {}", code, message)),
                }
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
            sqlx::Error::PoolClosed => DbError::ConnectionError("连接池已关闭".to_string()),
            sqlx::Error::WorkerCrashed => DbError::ConnectionError("工作线程崩溃".to_string()),
            _ => DbError::Unknown(format!("未知错误: {}", err)),
        }
    }
}

/// 从 redis::RedisError 转换为 DbError
impl From<redis::RedisError> for DbError {
    fn from(err: redis::RedisError) -> Self {
        // 用协议层稳定 API 分类，而非脆弱的 Display 子串匹配：
        // - 超时优先（is_timeout 涵盖连接超时与响应超时）
        // - 连接断开 / IO / 连接拒绝 → 连接错误
        // - 类型不匹配（TypeError）→ 类型转换错误
        // - 其余 → 命令错误（兜底）
        use redis::ErrorKind;
        if err.is_timeout() {
            DbError::RedisTimeoutError(format!("超时错误: {}", err))
        } else if err.is_connection_dropped() || err.is_io_error() || err.is_connection_refusal() {
            DbError::RedisConnectionError(format!("连接错误: {}", err))
        } else if matches!(err.kind(), ErrorKind::TypeError) {
            DbError::RedisTypeConversionError(format!("类型错误: {}", err))
        } else {
            DbError::RedisCommandError(format!("Redis 错误: {}", err))
        }
    }
}

/// 从 deadpool_redis::PoolError 转换为 DbError
impl From<deadpool_redis::PoolError> for DbError {
    fn from(err: deadpool_redis::PoolError) -> Self {
        match err {
            deadpool_redis::PoolError::Timeout(_) => {
                DbError::RedisTimeoutError("连接池获取连接超时".to_string())
            }
            deadpool_redis::PoolError::Closed => {
                DbError::RedisPoolError("连接池已关闭".to_string())
            }
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
            DbError::MissingGroupByClause,
            DbError::UnsupportedOperator("测试".to_string()),
            DbError::InvalidArgument("测试".to_string()),
            DbError::Unknown("测试".to_string()),
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
    fn test_pool_error_conversion() {
        // 测试 deadpool_redis::PoolError 转换
        let pool_err = deadpool_redis::PoolError::Closed;
        let db_err: DbError = pool_err.into();
        assert!(matches!(db_err, DbError::RedisPoolError(_)));
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
    }

    /// DB-7：is_retryable = (category == Transient)
    #[test]
    fn test_db_error_is_retryable() {
        assert!(DbError::ConnectionError("c".into()).is_retryable());
        assert!(DbError::RedisTimeoutError("t".into()).is_retryable());
        assert!(!DbError::QueryError("q".into()).is_retryable());
        assert!(!DbError::MissingWhereClause.is_retryable());
        assert!(!DbError::ConstraintError("c".into()).is_retryable());
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
            DbError::MissingGroupByClause.code(),
            DbError::UnsupportedOperator("".into()).code(),
            DbError::InvalidArgument("".into()).code(),
            DbError::Unknown("".into()).code(),
        ];
        let mut sorted = codes.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(codes.len(), sorted.len(), "19 个变体的 code 应全部唯一");
        // 所有码在 8xxxxx 段
        for &c in &codes {
            assert!((800000..900000).contains(&c), "code {} 不在 8xxxxx 段", c);
        }
    }
}
