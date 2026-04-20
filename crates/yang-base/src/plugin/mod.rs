//! 插件管理模块
//!
//! 提供插件注册、管理和生命周期控制功能。
//!
//! # 主要组件
//!
//! - `Plugin` trait：插件接口定义
//! - `PluginManager`：插件管理器，负责插件的注册和查找
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::plugin::{Plugin, PluginManager};
//!
//! // 定义插件
//! struct MyPlugin;
//!
//! #[async_trait::async_trait]
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &str {
//!         "my_plugin"
//!     }
//! }
//!
//! // 注册插件
//! let manager = PluginManager::new();
//! manager.register(MyPlugin).await?;
//! ```

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::BaseError;

/// 插件接口
///
/// 所有插件必须实现此 trait
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::plugin::Plugin;
/// use async_trait::async_trait;
///
/// struct MyPlugin;
///
/// #[async_trait]
/// impl Plugin for MyPlugin {
///     fn name(&self) -> &str {
///         "my_plugin"
///     }
///     
///     fn version(&self) -> &str {
///         "1.0.0"
///     }
///     
///     fn init_sql(&self) -> Vec<String> {
///         vec![
///             "CREATE TABLE IF NOT EXISTS my_table (id INT PRIMARY KEY)".to_string()
///         ]
///     }
/// }
/// ```
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件名称
    ///
    /// 插件名称必须唯一，用于标识和查找插件
    ///
    /// # 返回
    /// - 插件名称字符串
    fn name(&self) -> &str;

    /// 获取插件版本
    ///
    /// 使用语义化版本号，格式：major.minor.patch
    ///
    /// # 返回
    /// - 版本号字符串，默认为 "0.1.0"
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// 获取插件依赖列表
    ///
    /// 返回当前插件依赖的其他插件名称列表
    /// 系统会确保依赖插件先于当前插件初始化
    ///
    /// # 返回
    /// - 依赖插件名称列表，默认为空
    fn dependencies(&self) -> Vec<&str> {
        Vec::new()
    }

    /// 获取数据库初始化 SQL 脚本
    ///
    /// 返回创建表的 SQL 语句列表
    /// 建议使用 IF NOT EXISTS 确保幂等性
    ///
    /// # 返回
    /// - SQL 语句列表，默认为空
    fn init_sql(&self) -> Vec<String> {
        Vec::new()
    }

    /// 获取数据库迁移脚本
    ///
    /// 返回 (版本号, SQL 脚本) 的列表
    /// 版本号格式：YYYYMMDDHHMMSS
    ///
    /// # 返回
    /// - (版本号, SQL 脚本) 元组列表，默认为空
    fn migration_sql(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// 获取插件配置 Schema
    ///
    /// 返回 JSON Schema 格式的配置定义
    ///
    /// # 返回
    /// - Some(JsonValue): 配置 Schema
    /// - None: 无配置要求（默认）
    fn config_schema(&self) -> Option<JsonValue> {
        None
    }

    /// 插件注册时的回调
    ///
    /// 在插件被注册到 PluginManager 时调用
    ///
    /// # 返回
    /// - Ok(()): 注册成功
    /// - Err: 注册失败
    async fn on_register(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// 数据库初始化后的回调
    ///
    /// 在插件的数据库表创建完成后调用
    /// 可用于插入初始数据或执行其他初始化逻辑
    ///
    /// # 返回
    /// - Ok(()): 初始化成功
    /// - Err: 初始化失败
    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// 系统关闭时的回调
    ///
    /// 在系统关闭时调用，用于清理资源
    ///
    /// # 返回
    /// - Ok(()): 关闭成功
    /// - Err: 关闭失败
    async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// 插件管理器
///
/// 负责插件的注册、查找和生命周期管理
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::plugin::{Plugin, PluginManager};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let manager = PluginManager::new();
///     
///     // 注册插件
///     manager.register(MyPlugin).await?;
///     
///     // 查找插件
///     if let Some(plugin) = manager.get("my_plugin").await {
///         println!("找到插件: {}", plugin.name());
///     }
///     
///     // 获取所有插件（按依赖顺序）
///     let plugins = manager.get_all().await;
///     
///     // 关闭所有插件
///     manager.shutdown().await?;
///     
///     Ok(())
/// }
/// ```
pub struct PluginManager {
    /// 已注册的插件
    /// Key: 插件名称, Value: 插件实例
    plugins: Arc<RwLock<HashMap<String, Arc<dyn Plugin>>>>,

    /// 插件配置
    /// Key: 插件名称, Value: 配置 JSON
    configs: Arc<RwLock<HashMap<String, JsonValue>>>,
}

impl PluginManager {
    /// 创建新的插件管理器
    ///
    /// # 返回
    /// - 新的 PluginManager 实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::plugin::PluginManager;
    ///
    /// let manager = PluginManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册插件
    ///
    /// # 参数
    /// - plugin: 插件实例
    ///
    /// # 返回
    /// - Ok(()): 注册成功
    /// - Err(BaseError): 注册失败（如插件名称重复）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::{Plugin, PluginManager};
    ///
    /// let manager = PluginManager::new();
    /// manager.register(MyPlugin).await?;
    /// ```
    pub async fn register<P: Plugin + 'static>(&self, plugin: P) -> Result<(), BaseError> {
        let name = plugin.name().to_string();
        let plugin = Arc::new(plugin);

        // 检查插件是否已注册
        {
            let plugins = self.plugins.read().await;
            if plugins.contains_key(&name) {
                return Err(BaseError::PluginAlreadyRegistered(name));
            }
        }

        // 调用注册回调
        plugin
            .on_register()
            .await
            .map_err(|e| BaseError::PluginRegisterFailed(name.clone(), e.to_string()))?;

        // 注册插件
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(name.clone(), plugin);
        }

        log::info!("插件已注册: {}", name);
        Ok(())
    }

    /// 查找插件
    ///
    /// # 参数
    /// - name: 插件名称
    ///
    /// # 返回
    /// - `Some(Arc<dyn Plugin>)`: 找到插件
    /// - None: 插件不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManager;
    ///
    /// let manager = PluginManager::new();
    /// if let Some(plugin) = manager.get("my_plugin").await {
    ///     println!("找到插件: {}", plugin.name());
    /// }
    /// ```
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).cloned()
    }

    /// 获取所有已注册插件
    ///
    /// # 返回
    /// - 插件列表（按依赖关系排序）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManager;
    ///
    /// let manager = PluginManager::new();
    /// let plugins = manager.get_all().await;
    /// for plugin in plugins {
    ///     println!("插件: {} v{}", plugin.name(), plugin.version());
    /// }
    /// ```
    pub async fn get_all(&self) -> Vec<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        let mut plugin_list: Vec<_> = plugins.values().cloned().collect();

        // 按依赖关系排序
        self.topological_sort(&mut plugin_list);

        plugin_list
    }

    /// 加载插件配置
    ///
    /// # 参数
    /// - name: 插件名称
    /// - config: 配置 JSON
    ///
    /// # 返回
    /// - Ok(()): 加载成功
    /// - Err(BaseError): 加载失败（如配置验证失败）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManager;
    /// use serde_json::json;
    ///
    /// let manager = PluginManager::new();
    /// let config = json!({"key": "value"});
    /// manager.load_config("my_plugin", config).await?;
    /// ```
    pub async fn load_config(&self, name: &str, config: JsonValue) -> Result<(), BaseError> {
        // 获取插件
        let plugin = self
            .get(name)
            .await
            .ok_or_else(|| BaseError::PluginNotFound(name.to_string()))?;

        // 验证配置
        if let Some(schema) = plugin.config_schema() {
            self.validate_config(&config, &schema)?;
        }

        // 存储配置
        {
            let mut configs = self.configs.write().await;
            configs.insert(name.to_string(), config);
        }

        Ok(())
    }

    /// 获取插件配置
    ///
    /// # 参数
    /// - name: 插件名称
    ///
    /// # 返回
    /// - Some(JsonValue): 插件配置
    /// - None: 配置不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManager;
    ///
    /// let manager = PluginManager::new();
    /// if let Some(config) = manager.get_config("my_plugin").await {
    ///     println!("配置: {:?}", config);
    /// }
    /// ```
    pub async fn get_config(&self, name: &str) -> Option<JsonValue> {
        let configs = self.configs.read().await;
        configs.get(name).cloned()
    }

    /// 拓扑排序（按依赖关系排序）
    ///
    /// # 参数
    /// - plugins: 插件列表
    ///
    /// # 说明
    /// 使用 Kahn 算法进行拓扑排序，确保依赖插件先于当前插件
    fn topological_sort(&self, plugins: &mut Vec<Arc<dyn Plugin>>) {
        // 构建依赖图
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for plugin in plugins.iter() {
            let name = plugin.name().to_string();
            in_degree.entry(name.clone()).or_insert(0);

            for dep in plugin.dependencies() {
                graph.entry(dep.to_string()).or_default().push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }

        // Kahn 算法
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut sorted = Vec::new();

        while let Some(node) = queue.pop() {
            sorted.push(node.clone());

            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        // 重新排序插件列表
        plugins.sort_by_key(|p| {
            sorted
                .iter()
                .position(|n| n == p.name())
                .unwrap_or(usize::MAX)
        });
    }

    /// 验证配置
    ///
    /// # 参数
    /// - config: 配置 JSON
    /// - schema: JSON Schema
    ///
    /// # 返回
    /// - Ok(()): 验证通过
    /// - Err(BaseError): 验证失败
    fn validate_config(&self, _config: &JsonValue, _schema: &JsonValue) -> Result<(), BaseError> {
        // TODO: 实现 JSON Schema 验证
        // 可以使用 jsonschema crate
        Ok(())
    }

    /// 关闭所有插件
    ///
    /// 按照依赖关系的逆序调用插件的 on_shutdown 方法
    ///
    /// # 返回
    /// - Ok(()): 关闭成功
    /// - Err(BaseError): 关闭失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManager;
    ///
    /// let manager = PluginManager::new();
    /// // ... 注册插件 ...
    /// manager.shutdown().await?;
    /// ```
    pub async fn shutdown(&self) -> Result<(), BaseError> {
        let mut plugins = self.get_all().await;
        plugins.reverse(); // 逆序关闭

        for plugin in plugins {
            let name = plugin.name();
            if let Err(e) = plugin.on_shutdown().await {
                log::error!("插件 {} 关闭失败: {}", name, e);
                return Err(BaseError::PluginShutdownFailed(
                    name.to_string(),
                    e.to_string(),
                ));
            }
            log::info!("插件已关闭: {}", name);
        }

        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
