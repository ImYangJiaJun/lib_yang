/// 数据库错误类型
#[derive(Debug, thiserror::Error)]
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

    #[error("未知错误: {0}")]
    Unknown(String),
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
        // redis 1.2.0 中的 ErrorKind 是不同的，我们直接根据错误信息判断
        let err_str = format!("{}", err);

        if err_str.contains("IO error") || err_str.contains("Connection") {
            DbError::RedisConnectionError(format!("IO 错误: {}", err))
        } else if err_str.contains("type") || err_str.contains("Type") {
            DbError::RedisTypeConversionError(format!("类型错误: {}", err))
        } else if err_str.contains("timeout") || err_str.contains("Timeout") {
            DbError::RedisTimeoutError(format!("超时错误: {}", err))
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
        // 测试所有错误类型的中文消息
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
}
