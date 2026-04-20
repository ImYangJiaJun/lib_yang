//! 错误处理模块单元测试
//!
//! 测试 BaseError 的错误消息格式和类型转换

use yang_base::error::BaseError;

// ==================== 错误消息格式测试 ====================

#[test]
fn test_plugin_already_registered_message() {
    let err = BaseError::PluginAlreadyRegistered("test_plugin".to_string());
    assert_eq!(format!("{}", err), "插件已注册: test_plugin");
}

#[test]
fn test_plugin_not_found_message() {
    let err = BaseError::PluginNotFound("missing_plugin".to_string());
    assert_eq!(format!("{}", err), "插件未找到: missing_plugin");
}

#[test]
fn test_plugin_register_failed_message() {
    let err = BaseError::PluginRegisterFailed("my_plugin".to_string(), "初始化失败".to_string());
    assert_eq!(format!("{}", err), "插件注册失败 [my_plugin]: 初始化失败");
}

#[test]
fn test_plugin_init_failed_message() {
    let err = BaseError::PluginInitFailed("my_plugin".to_string(), "数据库连接失败".to_string());
    assert_eq!(
        format!("{}", err),
        "插件初始化失败 [my_plugin]: 数据库连接失败"
    );
}

#[test]
fn test_plugin_dependency_missing_message() {
    let err = BaseError::PluginDependencyMissing("plugin_a".to_string(), "plugin_b".to_string());
    assert_eq!(
        format!("{}", err),
        "插件依赖缺失 [plugin_a]: 缺少依赖 plugin_b"
    );
}

#[test]
fn test_plugin_circular_dependency_message() {
    let err = BaseError::PluginCircularDependency("plugin_a -> plugin_b -> plugin_a".to_string());
    assert_eq!(
        format!("{}", err),
        "插件循环依赖: plugin_a -> plugin_b -> plugin_a"
    );
}

#[test]
fn test_plugin_config_invalid_message() {
    let err = BaseError::PluginConfigInvalid("my_plugin".to_string(), "缺少必需字段".to_string());
    assert_eq!(format!("{}", err), "插件配置无效 [my_plugin]: 缺少必需字段");
}

#[test]
fn test_database_connection_failed_message() {
    let err = BaseError::DatabaseConnectionFailed("无法连接到 MySQL".to_string());
    assert_eq!(format!("{}", err), "数据库连接失败: 无法连接到 MySQL");
}

#[test]
fn test_database_query_failed_message() {
    let err = BaseError::DatabaseQueryFailed("查询超时".to_string());
    assert_eq!(format!("{}", err), "数据库查询失败: 查询超时");
}

#[test]
fn test_database_init_failed_message() {
    let err = BaseError::DatabaseInitFailed("表创建失败".to_string());
    assert_eq!(format!("{}", err), "数据库初始化失败: 表创建失败");
}

#[test]
fn test_database_migration_failed_message() {
    let err = BaseError::DatabaseMigrationFailed("v1.0.0".to_string(), "SQL 语法错误".to_string());
    assert_eq!(format!("{}", err), "数据库迁移失败 [v1.0.0]: SQL 语法错误");
}

#[test]
fn test_database_not_initialized_message() {
    let err = BaseError::DatabaseNotInitialized;
    assert_eq!(format!("{}", err), "数据库未初始化");
}

#[test]
fn test_database_transaction_failed_message() {
    let err = BaseError::DatabaseTransactionFailed("事务回滚失败".to_string());
    assert_eq!(format!("{}", err), "数据库事务失败: 事务回滚失败");
}

#[test]
fn test_http_client_create_failed_message() {
    let err = BaseError::HttpClientCreateFailed("无效的配置".to_string());
    assert_eq!(format!("{}", err), "HTTP 客户端创建失败: 无效的配置");
}

#[test]
fn test_http_request_failed_message() {
    let err = BaseError::HttpRequestFailed("404 Not Found".to_string());
    assert_eq!(format!("{}", err), "HTTP 请求失败: 404 Not Found");
}

#[test]
fn test_http_response_parse_failed_message() {
    let err = BaseError::HttpResponseParseFailed("无效的 JSON".to_string());
    assert_eq!(format!("{}", err), "HTTP 响应解析失败: 无效的 JSON");
}

#[test]
fn test_http_timeout_message() {
    let err = BaseError::HttpTimeout;
    assert_eq!(format!("{}", err), "HTTP 请求超时");
}

#[test]
fn test_token_key_invalid_message() {
    let err = BaseError::TokenKeyInvalid("密钥长度不足".to_string());
    assert_eq!(format!("{}", err), "Token 密钥无效: 密钥长度不足");
}

#[test]
fn test_token_generate_failed_message() {
    let err = BaseError::TokenGenerateFailed("签名失败".to_string());
    assert_eq!(format!("{}", err), "Token 生成失败: 签名失败");
}

#[test]
fn test_token_verify_failed_message() {
    let err = BaseError::TokenVerifyFailed("签名不匹配".to_string());
    assert_eq!(format!("{}", err), "Token 验证失败: 签名不匹配");
}

#[test]
fn test_token_parse_failed_message() {
    let err = BaseError::TokenParseFailed("无效的 JWT 格式".to_string());
    assert_eq!(format!("{}", err), "Token 解析失败: 无效的 JWT 格式");
}

#[test]
fn test_token_expired_message() {
    let err = BaseError::TokenExpired;
    assert_eq!(format!("{}", err), "Token 已过期");
}

#[test]
fn test_token_type_invalid_message() {
    let err = BaseError::TokenTypeInvalid("期望 access token".to_string());
    assert_eq!(format!("{}", err), "Token 类型无效: 期望 access token");
}

#[test]
fn test_json_serialize_failed_message() {
    let err = BaseError::JsonSerializeFailed("无法序列化".to_string());
    assert_eq!(format!("{}", err), "JSON 序列化失败: 无法序列化");
}

#[test]
fn test_json_deserialize_failed_message() {
    let err = BaseError::JsonDeserializeFailed("无法反序列化".to_string());
    assert_eq!(format!("{}", err), "JSON 反序列化失败: 无法反序列化");
}

#[test]
fn test_config_error_message() {
    let err = BaseError::ConfigError("配置文件不存在".to_string());
    assert_eq!(format!("{}", err), "配置错误: 配置文件不存在");
}

#[test]
fn test_io_error_message() {
    let err = BaseError::IoError("文件读取失败".to_string());
    assert_eq!(format!("{}", err), "IO 错误: 文件读取失败");
}

#[test]
fn test_unknown_error_message() {
    let err = BaseError::Unknown("未知问题".to_string());
    assert_eq!(format!("{}", err), "未知错误: 未知问题");
}

// ==================== 错误消息中文验证测试 ====================

#[test]
fn test_all_error_messages_contain_chinese() {
    let errors = vec![
        BaseError::PluginAlreadyRegistered("test".to_string()),
        BaseError::PluginNotFound("test".to_string()),
        BaseError::PluginRegisterFailed("test".to_string(), "reason".to_string()),
        BaseError::PluginInitFailed("test".to_string(), "reason".to_string()),
        BaseError::PluginDependencyMissing("test".to_string(), "dep".to_string()),
        BaseError::PluginCircularDependency("test".to_string()),
        BaseError::PluginConfigInvalid("test".to_string(), "reason".to_string()),
        BaseError::DatabaseConnectionFailed("test".to_string()),
        BaseError::DatabaseQueryFailed("test".to_string()),
        BaseError::DatabaseInitFailed("test".to_string()),
        BaseError::DatabaseMigrationFailed("v1".to_string(), "reason".to_string()),
        BaseError::DatabaseNotInitialized,
        BaseError::DatabaseTransactionFailed("test".to_string()),
        BaseError::HttpClientCreateFailed("test".to_string()),
        BaseError::HttpRequestFailed("test".to_string()),
        BaseError::HttpResponseParseFailed("test".to_string()),
        BaseError::HttpTimeout,
        BaseError::TokenKeyInvalid("test".to_string()),
        BaseError::TokenGenerateFailed("test".to_string()),
        BaseError::TokenVerifyFailed("test".to_string()),
        BaseError::TokenParseFailed("test".to_string()),
        BaseError::TokenExpired,
        BaseError::TokenTypeInvalid("test".to_string()),
        BaseError::JsonSerializeFailed("test".to_string()),
        BaseError::JsonDeserializeFailed("test".to_string()),
        BaseError::ConfigError("test".to_string()),
        BaseError::IoError("test".to_string()),
        BaseError::Unknown("test".to_string()),
    ];

    for error in errors {
        let msg = format!("{}", error);
        // 验证错误消息包含中文字符
        let has_chinese = msg.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'));
        assert!(has_chinese, "错误消息应该包含中文: {}", msg);
    }
}

// ==================== From trait 转换测试 ====================

#[test]
fn test_from_yang_db_error() {
    let db_err = yang_db::DbError::QueryError("查询失败".to_string());
    let base_err: BaseError = db_err.into();

    match base_err {
        BaseError::DatabaseQueryFailed(msg) => {
            assert!(msg.contains("查询失败"));
        }
        _ => panic!("期望 DatabaseQueryFailed 错误"),
    }
}

#[test]
fn test_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
    let base_err: BaseError = io_err.into();

    match base_err {
        BaseError::IoError(msg) => {
            assert!(msg.contains("文件不存在"));
        }
        _ => panic!("期望 IoError 错误"),
    }
}

#[test]
fn test_from_serde_json_deserialize_error() {
    // 创建一个反序列化错误
    let json_str = "{invalid json}";
    let result: Result<serde_json::Value, _> = serde_json::from_str(json_str);
    let json_err = result.unwrap_err();
    let base_err: BaseError = json_err.into();

    match base_err {
        BaseError::JsonDeserializeFailed(_) => {
            // 成功转换为 JsonDeserializeFailed
        }
        _ => panic!("期望 JsonDeserializeFailed 错误"),
    }
}

#[test]
fn test_from_serde_json_serialize_error() {
    use serde::Serialize;

    // 创建一个无法序列化的类型
    #[derive(Serialize)]
    struct BadStruct {
        #[serde(serialize_with = "bad_serializer")]
        value: i32,
    }

    fn bad_serializer<S>(_: &i32, _: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("序列化失败"))
    }

    let bad = BadStruct { value: 42 };
    let result = serde_json::to_string(&bad);
    let json_err = result.unwrap_err();
    let base_err: BaseError = json_err.into();

    // serde_json 的自定义错误可能被分类为反序列化错误或序列化错误
    // 我们只需要验证它被转换为某种 JSON 错误即可
    match base_err {
        BaseError::JsonSerializeFailed(_) | BaseError::JsonDeserializeFailed(_) => {
            // 成功转换为 JSON 相关错误
        }
        _ => panic!("期望 JSON 相关错误"),
    }
}

// ==================== std::error::Error trait 测试 ====================

#[test]
fn test_base_error_implements_std_error() {
    // 验证 BaseError 实现了 std::error::Error trait
    let err = BaseError::PluginNotFound("test".to_string());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_base_error_is_send_sync() {
    // 验证 BaseError 实现了 Send 和 Sync trait（用于多线程）
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<BaseError>();
    assert_sync::<BaseError>();
}

// ==================== 错误链测试 ====================

#[test]
fn test_error_propagation_with_question_mark() {
    fn inner_function() -> Result<(), yang_db::DbError> {
        Err(yang_db::DbError::QueryError("内部错误".to_string()))
    }

    fn outer_function() -> Result<(), BaseError> {
        inner_function()?;
        Ok(())
    }

    let result = outer_function();
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::DatabaseQueryFailed(msg) => {
            assert!(msg.contains("内部错误"));
        }
        _ => panic!("期望 DatabaseQueryFailed 错误"),
    }
}
