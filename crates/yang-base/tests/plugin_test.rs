//! 插件管理模块单元测试

use async_trait::async_trait;
use serde_json::json;
use yang_base::error::BaseError;
use yang_base::plugin::{Plugin, PluginManager};

// ==================== 测试插件定义 ====================

/// 基础测试插件
struct BasicPlugin;

#[async_trait]
impl Plugin for BasicPlugin {
    fn name(&self) -> &str {
        "basic_plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }
}

/// 带依赖的插件 A
struct PluginA;

#[async_trait]
impl Plugin for PluginA {
    fn name(&self) -> &str {
        "plugin_a"
    }
}

/// 带依赖的插件 B（依赖 A）
struct PluginB;

#[async_trait]
impl Plugin for PluginB {
    fn name(&self) -> &str {
        "plugin_b"
    }

    fn dependencies(&self) -> &[&str] {
        &["plugin_a"]
    }
}

/// 带依赖的插件 C（依赖 B）
struct PluginC;

#[async_trait]
impl Plugin for PluginC {
    fn name(&self) -> &str {
        "plugin_c"
    }

    fn dependencies(&self) -> &[&str] {
        &["plugin_b"]
    }
}

/// 带配置 Schema 的插件
struct ConfigurablePlugin;

#[async_trait]
impl Plugin for ConfigurablePlugin {
    fn name(&self) -> &str {
        "configurable_plugin"
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "host": {"type": "string"},
                "port": {"type": "number"}
            },
            "required": ["host", "port"]
        }))
    }
}

/// 带生命周期钩子的插件
struct LifecyclePlugin {
    register_called: std::sync::Arc<std::sync::Mutex<bool>>,
    init_called: std::sync::Arc<std::sync::Mutex<bool>>,
    shutdown_called: std::sync::Arc<std::sync::Mutex<bool>>,
}

impl LifecyclePlugin {
    fn new() -> Self {
        Self {
            register_called: std::sync::Arc::new(std::sync::Mutex::new(false)),
            init_called: std::sync::Arc::new(std::sync::Mutex::new(false)),
            shutdown_called: std::sync::Arc::new(std::sync::Mutex::new(false)),
        }
    }

    fn is_register_called(&self) -> bool {
        *self.register_called.lock().unwrap()
    }

    fn is_init_called(&self) -> bool {
        *self.init_called.lock().unwrap()
    }

    fn is_shutdown_called(&self) -> bool {
        *self.shutdown_called.lock().unwrap()
    }
}

#[async_trait]
impl Plugin for LifecyclePlugin {
    fn name(&self) -> &str {
        "lifecycle_plugin"
    }

    async fn on_register(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *self.register_called.lock().unwrap() = true;
        Ok(())
    }

    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *self.init_called.lock().unwrap() = true;
        Ok(())
    }

    async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *self.shutdown_called.lock().unwrap() = true;
        Ok(())
    }
}

/// 注册失败的插件
struct FailingPlugin;

#[async_trait]
impl Plugin for FailingPlugin {
    fn name(&self) -> &str {
        "failing_plugin"
    }

    async fn on_register(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("注册失败".into())
    }
}

// ==================== 测试用例 ====================

/// 测试插件注册
#[tokio::test]
async fn test_register_plugin() {
    let manager = PluginManager::new();

    // 注册插件
    let result = manager.register(BasicPlugin).await;
    assert!(result.is_ok(), "插件注册应该成功");

    // 验证插件已注册
    let plugin = manager.get("basic_plugin").await;
    assert!(plugin.is_some(), "应该能找到已注册的插件");
    assert_eq!(plugin.unwrap().name(), "basic_plugin");
}

/// 测试重复注册插件
#[tokio::test]
async fn test_register_duplicate_plugin() {
    let manager = PluginManager::new();

    // 第一次注册
    let result = manager.register(BasicPlugin).await;
    assert!(result.is_ok(), "第一次注册应该成功");

    // 第二次注册同名插件
    let result = manager.register(BasicPlugin).await;
    assert!(result.is_err(), "重复注册应该失败");

    // 验证错误类型
    match result {
        Err(BaseError::PluginAlreadyRegistered(name)) => {
            assert_eq!(name, "basic_plugin");
        }
        _ => panic!("应该返回 PluginAlreadyRegistered 错误"),
    }
}

/// 测试查找不存在的插件
#[tokio::test]
async fn test_get_nonexistent_plugin() {
    let manager = PluginManager::new();

    // 查找不存在的插件
    let plugin = manager.get("nonexistent_plugin").await;
    assert!(plugin.is_none(), "不存在的插件应该返回 None");
}

/// 测试获取所有插件
#[tokio::test]
async fn test_get_all_plugins() {
    let manager = PluginManager::new();

    // 注册多个插件
    manager.register(BasicPlugin).await.unwrap();
    manager.register(PluginA).await.unwrap();

    // 获取所有插件
    let plugins = manager.get_all().await;
    assert_eq!(plugins.len(), 2, "应该有 2 个插件");

    // 验证插件名称
    let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
    assert!(names.contains(&"basic_plugin"));
    assert!(names.contains(&"plugin_a"));
}

/// 测试依赖关系拓扑排序
#[tokio::test]
async fn test_topological_sort() {
    let manager = PluginManager::new();

    // 注册插件（逆序注册）
    manager.register(PluginC).await.unwrap();
    manager.register(PluginB).await.unwrap();
    manager.register(PluginA).await.unwrap();

    // 获取所有插件（应该按依赖顺序排序）
    let plugins = manager.get_all().await;
    assert_eq!(plugins.len(), 3, "应该有 3 个插件");

    // 验证顺序：A -> B -> C
    assert_eq!(plugins[0].name(), "plugin_a", "plugin_a 应该排在第一位");
    assert_eq!(plugins[1].name(), "plugin_b", "plugin_b 应该排在第二位");
    assert_eq!(plugins[2].name(), "plugin_c", "plugin_c 应该排在第三位");
}

/// 测试复杂依赖关系
#[tokio::test]
async fn test_complex_dependencies() {
    let manager = PluginManager::new();

    // 定义复杂依赖关系的插件
    struct PluginD;
    #[async_trait]
    impl Plugin for PluginD {
        fn name(&self) -> &str {
            "plugin_d"
        }
        fn dependencies(&self) -> &[&str] {
            &["plugin_a", "plugin_b"]
        }
    }

    // 注册插件（乱序）
    manager.register(PluginD).await.unwrap();
    manager.register(PluginC).await.unwrap();
    manager.register(PluginA).await.unwrap();
    manager.register(PluginB).await.unwrap();

    // 获取所有插件
    let plugins = manager.get_all().await;
    assert_eq!(plugins.len(), 4, "应该有 4 个插件");

    // 验证 A 在 B 和 D 之前
    let pos_a = plugins.iter().position(|p| p.name() == "plugin_a").unwrap();
    let pos_b = plugins.iter().position(|p| p.name() == "plugin_b").unwrap();
    let pos_d = plugins.iter().position(|p| p.name() == "plugin_d").unwrap();
    assert!(pos_a < pos_b, "plugin_a 应该在 plugin_b 之前");
    assert!(pos_a < pos_d, "plugin_a 应该在 plugin_d 之前");

    // 验证 B 在 C 和 D 之前
    let pos_c = plugins.iter().position(|p| p.name() == "plugin_c").unwrap();
    assert!(pos_b < pos_c, "plugin_b 应该在 plugin_c 之前");
    assert!(pos_b < pos_d, "plugin_b 应该在 plugin_d 之前");
}

/// 测试加载插件配置
#[tokio::test]
async fn test_load_config() {
    let manager = PluginManager::new();

    // 注册插件
    manager.register(ConfigurablePlugin).await.unwrap();

    // 加载配置
    let config = json!({
        "host": "localhost",
        "port": 3306
    });
    let result = manager
        .load_config("configurable_plugin", config.clone())
        .await;
    assert!(result.is_ok(), "配置加载应该成功");

    // 获取配置
    let loaded_config = manager.get_config("configurable_plugin").await;
    assert!(loaded_config.is_some(), "应该能获取到配置");
    assert_eq!(loaded_config.unwrap(), config);
}

/// 测试加载不存在插件的配置
#[tokio::test]
async fn test_load_config_nonexistent_plugin() {
    let manager = PluginManager::new();

    // 尝试加载不存在插件的配置
    let config = json!({"key": "value"});
    let result = manager.load_config("nonexistent_plugin", config).await;
    assert!(result.is_err(), "加载不存在插件的配置应该失败");

    // 验证错误类型
    match result {
        Err(BaseError::PluginNotFound(name)) => {
            assert_eq!(name, "nonexistent_plugin");
        }
        _ => panic!("应该返回 PluginNotFound 错误"),
    }
}

/// 测试获取不存在的配置
#[tokio::test]
async fn test_get_nonexistent_config() {
    let manager = PluginManager::new();

    // 注册插件但不加载配置
    manager.register(BasicPlugin).await.unwrap();

    // 获取配置
    let config = manager.get_config("basic_plugin").await;
    assert!(config.is_none(), "未加载的配置应该返回 None");
}

/// 测试生命周期钩子
#[tokio::test]
async fn test_lifecycle_hooks() {
    let manager = PluginManager::new();
    let plugin = LifecyclePlugin::new();

    // 验证初始状态
    assert!(!plugin.is_register_called(), "注册钩子未调用");
    assert!(!plugin.is_init_called(), "初始化钩子未调用");
    assert!(!plugin.is_shutdown_called(), "关闭钩子未调用");

    // 注册插件
    let register_called = plugin.register_called.clone();
    manager.register(plugin).await.unwrap();

    // 验证注册钩子被调用
    assert!(*register_called.lock().unwrap(), "注册钩子应该被调用");

    // 获取插件并调用初始化钩子
    let plugin = manager.get("lifecycle_plugin").await.unwrap();
    plugin.on_init().await.unwrap();

    // 关闭所有插件
    manager.shutdown().await.unwrap();
}

/// 测试注册失败的插件
#[tokio::test]
async fn test_register_failing_plugin() {
    use std::error::Error;

    let manager = PluginManager::new();

    // 注册会失败的插件
    let result = manager.register(FailingPlugin).await;
    assert!(result.is_err(), "注册应该失败");

    // 验证错误类型
    match result {
        Err(error @ BaseError::PluginLifecycleFailed { .. }) => {
            assert!(matches!(
                &error,
                BaseError::PluginLifecycleFailed {
                    plugin,
                    stage: yang_base::plugin::PluginLifecycleStage::Register,
                    ..
                } if plugin == "failing_plugin"
            ));
            assert!(error.to_string().contains("注册失败"));
            assert!(error.source().is_some());
        }
        _ => panic!("应该返回 PluginLifecycleFailed 错误"),
    }

    // 验证插件未被注册
    let plugin = manager.get("failing_plugin").await;
    assert!(plugin.is_none(), "失败的插件不应该被注册");
}

/// 测试关闭插件
#[tokio::test]
async fn test_shutdown() {
    let manager = PluginManager::new();

    // 注册多个插件（带依赖关系）
    manager.register(PluginA).await.unwrap();
    manager.register(PluginB).await.unwrap();
    manager.register(PluginC).await.unwrap();

    // 关闭所有插件
    let result = manager.shutdown().await;
    assert!(result.is_ok(), "关闭应该成功");
}

/// 测试 Default trait
#[tokio::test]
async fn test_default() {
    let manager = PluginManager::default();

    // 验证可以正常使用
    manager.register(BasicPlugin).await.unwrap();
    let plugin = manager.get("basic_plugin").await;
    assert!(plugin.is_some(), "应该能找到插件");
}

/// 测试插件版本
#[tokio::test]
async fn test_plugin_version() {
    let manager = PluginManager::new();
    manager.register(BasicPlugin).await.unwrap();

    let plugin = manager.get("basic_plugin").await.unwrap();
    assert_eq!(plugin.version(), "1.0.0", "版本应该是 1.0.0");
}

/// 测试插件依赖列表
#[tokio::test]
async fn test_plugin_dependencies() {
    let manager = PluginManager::new();
    manager.register(PluginB).await.unwrap();

    let plugin = manager.get("plugin_b").await.unwrap();
    let deps = plugin.dependencies();
    assert_eq!(deps.len(), 1, "应该有 1 个依赖");
    assert_eq!(deps[0], "plugin_a");
}

/// 测试插件初始化 SQL
#[tokio::test]
async fn test_plugin_init_sql() {
    struct SqlPlugin;

    #[async_trait]
    impl Plugin for SqlPlugin {
        fn name(&self) -> &str {
            "sql_plugin"
        }

        fn init_sql(&self) -> Vec<String> {
            vec![
                "CREATE TABLE IF NOT EXISTS users (id INT PRIMARY KEY)".to_string(),
                "CREATE TABLE IF NOT EXISTS posts (id INT PRIMARY KEY)".to_string(),
            ]
        }
    }

    let manager = PluginManager::new();
    manager.register(SqlPlugin).await.unwrap();

    let plugin = manager.get("sql_plugin").await.unwrap();
    let sql = plugin.init_sql();
    assert_eq!(sql.len(), 2, "应该有 2 条 SQL 语句");
    assert!(sql[0].contains("CREATE TABLE"));
}

/// 测试插件迁移 SQL
#[tokio::test]
async fn test_plugin_migration_sql() {
    struct MigrationPlugin;

    #[async_trait]
    impl Plugin for MigrationPlugin {
        fn name(&self) -> &str {
            "migration_plugin"
        }

        fn migration_sql(&self) -> Vec<(String, String)> {
            vec![
                (
                    "20240101000000".to_string(),
                    "ALTER TABLE users ADD COLUMN email VARCHAR(255)".to_string(),
                ),
                (
                    "20240102000000".to_string(),
                    "ALTER TABLE users ADD INDEX idx_email (email)".to_string(),
                ),
            ]
        }
    }

    let manager = PluginManager::new();
    manager.register(MigrationPlugin).await.unwrap();

    let plugin = manager.get("migration_plugin").await.unwrap();
    let migrations = plugin.migration_sql();
    assert_eq!(migrations.len(), 2, "应该有 2 个迁移");
    assert_eq!(migrations[0].0, "20240101000000");
    assert!(migrations[0].1.contains("ALTER TABLE"));
}
