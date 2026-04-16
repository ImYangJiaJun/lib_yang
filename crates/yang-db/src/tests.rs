#[cfg(test)]
mod unit_tests {
    use crate::condition::SqlValue;
    use crate::database::{Database, DatabaseConfig};
    use crate::error::DbError;
    use crate::field::FieldType;

    #[test]
    fn test_sql_value_from_i32() {
        let val: SqlValue = 42i32.into();
        match val {
            SqlValue::Int(v) => assert_eq!(v, 42),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_sql_value_from_string() {
        let val: SqlValue = "test".into();
        match val {
            SqlValue::String(s) => assert_eq!(s, "test"),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_sql_value_from_bool() {
        let val: SqlValue = true.into();
        match val {
            SqlValue::Bool(b) => assert!(b),
            _ => panic!("期望 SqlValue::Bool"),
        }
    }

    #[test]
    fn test_sql_value_from_option_some() {
        let val: SqlValue = Some(42i32).into();
        match val {
            SqlValue::Int(v) => assert_eq!(v, 42),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_sql_value_from_option_none() {
        let val: SqlValue = None::<i32>.into();
        match val {
            SqlValue::Null => (),
            _ => panic!("期望 SqlValue::Null"),
        }
    }

    #[test]
    fn test_field_type_equality() {
        assert_eq!(FieldType::Json, FieldType::Json);
        assert_ne!(FieldType::Json, FieldType::DateTime);
    }

    #[test]
    fn test_db_error_display() {
        let err = DbError::ConnectionError("连接失败".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("连接错误"));
        assert!(msg.contains("连接失败"));
    }

    #[test]
    fn test_db_error_missing_where_clause() {
        let err = DbError::MissingWhereClause;
        let msg = format!("{}", err);
        assert!(msg.contains("缺少 WHERE 条件"));
    }

    // 数据库配置测试
    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connect_timeout, 30);
        assert_eq!(config.idle_timeout, 600);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_database_config_custom() {
        let config = DatabaseConfig {
            max_connections: 20,
            connect_timeout: 10,
            idle_timeout: 300,
            enable_logging: true,
        };
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.idle_timeout, 300);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_database_config_clone() {
        let config1 = DatabaseConfig::default();
        let config2 = config1.clone();
        assert_eq!(config1.max_connections, config2.max_connections);
        assert_eq!(config1.connect_timeout, config2.connect_timeout);
    }

    // 连接字符串验证测试
    #[tokio::test]
    async fn test_invalid_connection_string() {
        let result = Database::connect("invalid_url").await;
        assert!(result.is_err());
        if let Err(e) = result {
            // 验证返回的是连接错误
            match e {
                DbError::ConnectionError(_) => (),
                _ => panic!("期望 ConnectionError，得到: {:?}", e),
            }
        }
    }

    #[tokio::test]
    async fn test_invalid_connection_string_missing_protocol() {
        let result = Database::connect("localhost:3306/test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_connection_string_empty() {
        let result = Database::connect("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connect_with_custom_config() {
        let config = DatabaseConfig {
            max_connections: 5,
            connect_timeout: 5,
            idle_timeout: 60,
            enable_logging: false,
        };

        // 使用无效的连接字符串测试配置是否被正确应用
        let result =
            Database::connect_with_config("mysql://invalid:invalid@localhost:9999/test", config)
                .await;
        assert!(result.is_err());
    }
}
