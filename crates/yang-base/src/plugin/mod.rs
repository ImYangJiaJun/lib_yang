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

        // 第一阶段：读锁检查（快速路径，避免持写锁跨 await）
        {
            let plugins = self.plugins.read().await;
            if plugins.contains_key(&name) {
                return Err(BaseError::PluginAlreadyRegistered(name));
            }
        } // 尽早释放读锁

        // 调用注册回调（不持任何锁）
        plugin
            .on_register()
            .await
            .map_err(|e| BaseError::PluginRegisterFailed(name.clone(), e.to_string()))?;

        // 第二阶段：写锁 + 二次校验（防止 TOCTOU 竞态）
        {
            let mut plugins = self.plugins.write().await;
            if plugins.contains_key(&name) {
                return Err(BaseError::PluginAlreadyRegistered(name));
            }
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

        // 验证配置（插件未定义 config_schema 时跳过验证）
        if let Some(schema) = plugin.config_schema() {
            self.validate_config(name, &config, &schema)?;
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

        // 检测循环依赖：Kahn 算法结束后，未被排序的节点构成环
        if sorted.len() < plugins.len() {
            let circular: Vec<String> = plugins
                .iter()
                .filter(|p| !sorted.iter().any(|n| n == p.name()))
                .map(|p| p.name().to_string())
                .collect();
            log::error!(
                "检测到循环依赖，以下插件将无法按依赖顺序加载: {}",
                circular.join(", ")
            );
            // 将循环中的插件追加到排序末尾，保证所有插件都出现在结果中
            sorted.extend(circular);
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
    /// 使用 jsonschema crate 对配置进行 JSON Schema 验证。
    /// 未启用 `plugin-schema` feature 时跳过验证。
    ///
    /// # 参数
    /// - `plugin_name`: 插件名称（用于错误信息）
    /// - `config`: 配置 JSON
    /// - `schema`: JSON Schema
    ///
    /// # 返回
    /// - `Ok(())`: 验证通过
    /// - `Err(BaseError::PluginConfigInvalid)`: 配置不符合 Schema
    fn validate_config(
        &self,
        plugin_name: &str,
        config: &JsonValue,
        schema: &JsonValue,
    ) -> Result<(), BaseError> {
        // 使用 jsonschema crate 进行 JSON Schema 验证
        #[cfg(feature = "plugin-schema")]
        {
            // 构建可复用的验证器
            let validator = jsonschema::validator_for(schema).map_err(|e| {
                BaseError::PluginConfigInvalid(
                    plugin_name.to_string(),
                    format!("Schema 编译失败: {}", e),
                )
            })?;

            // 收集所有验证错误信息
            let error_msgs: Vec<String> = validator
                .iter_errors(config)
                .map(|e| e.to_string())
                .collect();

            if !error_msgs.is_empty() {
                return Err(BaseError::PluginConfigInvalid(
                    plugin_name.to_string(),
                    error_msgs.join("; "),
                ));
            }
        }

        // 未启用 plugin-schema feature 时，跳过验证直接返回成功
        #[cfg(not(feature = "plugin-schema"))]
        {
            let _ = (plugin_name, config, schema);
        }

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

        let mut errors: Vec<(String, String)> = Vec::new();

        for plugin in plugins {
            let name = plugin.name();
            if let Err(e) = plugin.on_shutdown().await {
                log::error!("插件 {} 关闭失败: {}", name, e);
                errors.push((name.to_string(), e.to_string()));
            } else {
                log::info!("插件已关闭: {}", name);
            }
        }

        if let Some((name, reason)) = errors.into_iter().next() {
            Err(BaseError::PluginShutdownFailed(name, reason))
        } else {
            Ok(())
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 插件管理器构建器（构建阶段使用）
///
/// 在构建阶段负责插件的注册，构建完成后生成不可变的 `PluginRegistry`。
/// 构建阶段可以进行可变操作，运行阶段通过 `PluginRegistry` 进行无锁查找。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::plugin::{Plugin, PluginManagerBuilder};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut builder = PluginManagerBuilder::new();
///
///     // 注册插件
///     builder.register(MyPlugin).await?;
///
///     // 构建不可变的 PluginRegistry
///     let registry = builder.build();
///
///     // 无锁查找插件
///     if let Some(plugin) = registry.get("my_plugin") {
///         println!("找到插件: {}", plugin.name());
///     }
///
///     Ok(())
/// }
/// ```
pub struct PluginManagerBuilder {
    /// 已注册的插件（构建阶段可变）
    /// Key: 插件名称, Value: 插件实例
    plugins: HashMap<String, Arc<dyn Plugin>>,

    /// 插件配置
    /// Key: 插件名称, Value: 配置 JSON
    configs: HashMap<String, JsonValue>,
}

impl PluginManagerBuilder {
    /// 创建新的插件管理器构建器
    ///
    /// # 返回
    /// - 新的 PluginManagerBuilder 实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let builder = PluginManagerBuilder::new();
    /// ```
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            configs: HashMap::new(),
        }
    }

    /// 注册插件（构建阶段使用）
    ///
    /// 检查插件名称是否重复，调用 `on_register()` 回调，然后插入 HashMap。
    ///
    /// # 参数
    /// - `plugin`: 插件实例
    ///
    /// # 返回
    /// - `Ok(())`: 注册成功
    /// - `Err(BaseError)`: 注册失败（如插件名称重复或注册回调失败）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::{Plugin, PluginManagerBuilder};
    ///
    /// let mut builder = PluginManagerBuilder::new();
    /// builder.register(MyPlugin).await?;
    /// ```
    pub async fn register<P: Plugin + 'static>(&mut self, plugin: P) -> Result<(), BaseError> {
        let name = plugin.name().to_string();

        // 检查插件是否已注册（构建阶段直接访问 HashMap，无需加锁）
        if self.plugins.contains_key(&name) {
            return Err(BaseError::PluginAlreadyRegistered(name));
        }

        let plugin = Arc::new(plugin);

        // 调用注册回调
        plugin
            .on_register()
            .await
            .map_err(|e| BaseError::PluginRegisterFailed(name.clone(), e.to_string()))?;

        // 插入 HashMap
        self.plugins.insert(name.clone(), plugin);

        log::info!("插件已注册（构建阶段）: {}", name);
        Ok(())
    }

    /// 消费构建器，生成不可变的 PluginRegistry
    ///
    /// 调用此方法后，构建器被消费，返回运行阶段使用的 `PluginRegistry`。
    /// `PluginRegistry` 在构建时执行一次拓扑排序并缓存结果。
    ///
    /// # 返回
    /// - `Ok(PluginRegistry)`: 构建成功
    /// - `Err(BaseError::PluginDependencyMissing)`: 某插件的依赖未注册
    /// - `Err(BaseError::PluginCircularDependency)`: 插件之间存在循环依赖
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let mut builder = PluginManagerBuilder::new();
    /// // ... 注册插件 ...
    /// let registry = builder.build()?;
    /// ```
    pub fn build(self) -> Result<PluginRegistry, BaseError> {
        // 检查每个插件的依赖是否都已注册
        for plugin in self.plugins.values() {
            let plugin_name = plugin.name().to_string();
            for dep in plugin.dependencies() {
                if !self.plugins.contains_key(dep) {
                    // 依赖未注册，返回错误
                    return Err(BaseError::PluginDependencyMissing(
                        plugin_name,
                        dep.to_string(),
                    ));
                }
            }
        }
        // 依赖完整性检查通过，构建注册表（内部会检测循环依赖）
        PluginRegistry::new(self.plugins, self.configs)
    }
}

impl Default for PluginManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 插件注册表（运行阶段使用，无锁）
///
/// 由 `PluginManagerBuilder::build()` 生成，存储不可变的插件集合。
/// 运行阶段通过 HashMap 直接查找，无需获取任何锁，实现 O(1) 无锁查找。
/// 拓扑排序结果在构建时计算一次并缓存，多次调用 `get_all()` 无需重新计算。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::plugin::{Plugin, PluginManagerBuilder};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut builder = PluginManagerBuilder::new();
///     builder.register(MyPlugin).await?;
///     let registry = builder.build();
///
///     // 无锁 O(1) 查找
///     if let Some(plugin) = registry.get("my_plugin") {
///         println!("找到插件: {}", plugin.name());
///     }
///
///     // 获取缓存的排序结果（无需重新计算）
///     let all_plugins = registry.get_all();
///
///     // 关闭所有插件
///     registry.shutdown().await?;
///
///     Ok(())
/// }
/// ```
pub struct PluginRegistry {
    /// 不可变插件映射（无 RwLock 包装，运行阶段无锁访问）
    /// Key: 插件名称, Value: 插件实例
    plugins: HashMap<String, Arc<dyn Plugin>>,

    /// 拓扑排序缓存（构建时计算一次，运行阶段直接返回引用）
    sorted_plugins: Vec<Arc<dyn Plugin>>,

    /// 插件配置
    /// Key: 插件名称, Value: 配置 JSON
    configs: HashMap<String, JsonValue>,
}

impl PluginRegistry {
    /// 创建新的插件注册表（私有构造方法）
    ///
    /// 接收 plugins 和 configs，调用 `compute_topological_sort` 并缓存排序结果。
    /// 此方法仅由 `PluginManagerBuilder::build()` 调用。
    ///
    /// # 参数
    /// - `plugins`: 插件 HashMap
    /// - `configs`: 配置 HashMap
    ///
    /// # 返回
    /// - `Ok(PluginRegistry)`: 构建成功
    /// - `Err(BaseError::PluginCircularDependency)`: 存在循环依赖
    fn new(
        plugins: HashMap<String, Arc<dyn Plugin>>,
        configs: HashMap<String, JsonValue>,
    ) -> Result<Self, BaseError> {
        // 构建时执行一次拓扑排序并缓存结果（内部检测循环依赖）
        let sorted_plugins = Self::compute_topological_sort(&plugins)?;
        Ok(Self {
            plugins,
            sorted_plugins,
            configs,
        })
    }

    /// 查找插件（无锁，O(1) HashMap 查找）
    ///
    /// 直接通过 HashMap 查找，无需获取任何锁。
    ///
    /// # 参数
    /// - `name`: 插件名称
    ///
    /// # 返回
    /// - `Some(&Arc<dyn Plugin>)`: 找到插件
    /// - `None`: 插件不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let registry = PluginManagerBuilder::new().build();
    /// if let Some(plugin) = registry.get("my_plugin") {
    ///     println!("找到插件: {}", plugin.name());
    /// }
    /// ```
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.get(name)
    }

    /// 获取所有已注册插件（返回缓存的排序结果引用）
    ///
    /// 直接返回构建时缓存的拓扑排序结果，无需重新计算。
    ///
    /// # 返回
    /// - 按依赖关系排序的插件切片引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let registry = PluginManagerBuilder::new().build();
    /// let plugins = registry.get_all();
    /// for plugin in plugins {
    ///     println!("插件: {} v{}", plugin.name(), plugin.version());
    /// }
    /// ```
    pub fn get_all(&self) -> &[Arc<dyn Plugin>] {
        &self.sorted_plugins
    }

    /// 获取插件配置
    ///
    /// # 参数
    /// - `name`: 插件名称
    ///
    /// # 返回
    /// - `Some(&JsonValue)`: 插件配置
    /// - `None`: 配置不存在
    pub fn get_config(&self, name: &str) -> Option<&JsonValue> {
        self.configs.get(name)
    }

    /// 关闭所有插件（逆序关闭）
    ///
    /// 按照拓扑排序的逆序调用每个插件的 `on_shutdown` 方法，
    /// 确保依赖其他插件的插件先被关闭。
    ///
    /// # 返回
    /// - `Ok(())`: 所有插件关闭成功
    /// - `Err(BaseError)`: 某个插件关闭失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let registry = PluginManagerBuilder::new().build();
    /// // ... 使用插件 ...
    /// registry.shutdown().await?;
    /// ```
    pub async fn shutdown(&self) -> Result<(), BaseError> {
        // 逆序遍历缓存的排序结果，确保依赖插件最后关闭
        let mut errors: Vec<(String, String)> = Vec::new();

        for plugin in self.sorted_plugins.iter().rev() {
            let name = plugin.name();
            if let Err(e) = plugin.on_shutdown().await {
                log::error!("插件 {} 关闭失败: {}", name, e);
                errors.push((name.to_string(), e.to_string()));
            } else {
                log::info!("插件已关闭: {}", name);
            }
        }

        if let Some((name, reason)) = errors.into_iter().next() {
            Err(BaseError::PluginShutdownFailed(name, reason))
        } else {
            Ok(())
        }
    }

    /// 计算拓扑排序（私有方法，构建时调用一次）
    ///
    /// 使用 Kahn 算法对插件进行拓扑排序，确保依赖插件先于当前插件出现。
    /// 当排序后的节点数小于插件总数时，说明存在循环依赖。
    ///
    /// # 参数
    /// - `plugins`: 插件 HashMap 引用
    ///
    /// # 返回
    /// - `Ok(Vec<Arc<dyn Plugin>>)`: 按依赖关系排序的插件列表
    /// - `Err(BaseError::PluginCircularDependency)`: 存在循环依赖，错误信息含未排序节点
    fn compute_topological_sort(
        plugins: &HashMap<String, Arc<dyn Plugin>>,
    ) -> Result<Vec<Arc<dyn Plugin>>, BaseError> {
        // 构建入度表和邻接图
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for plugin in plugins.values() {
            let name = plugin.name().to_string();
            in_degree.entry(name.clone()).or_insert(0);

            for dep in plugin.dependencies() {
                // dep -> name 的边：dep 是 name 的依赖，dep 先执行
                graph.entry(dep.to_string()).or_default().push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }

        // Kahn 算法：从入度为 0 的节点开始
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut sorted_names: Vec<String> = Vec::new();

        while let Some(node) = queue.pop() {
            sorted_names.push(node.clone());

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

        // 检测循环依赖：若排序后的节点数小于插件总数，说明存在循环
        if sorted_names.len() < plugins.len() {
            // 找出未被排序的节点（即循环中的插件）
            let unsorted: Vec<String> = plugins
                .keys()
                .filter(|name| !sorted_names.contains(name))
                .cloned()
                .collect();
            let unsorted_str = unsorted.join(", ");
            return Err(BaseError::PluginCircularDependency(format!(
                "循环依赖涉及的插件: {}",
                unsorted_str
            )));
        }

        // 按排序顺序构建插件列表
        let mut sorted_plugins: Vec<Arc<dyn Plugin>> = plugins.values().cloned().collect();
        sorted_plugins.sort_by_key(|p| {
            sorted_names
                .iter()
                .position(|n| n == p.name())
                .unwrap_or(usize::MAX)
        });

        Ok(sorted_plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用插件：无依赖
    struct PluginA;

    #[async_trait]
    impl Plugin for PluginA {
        fn name(&self) -> &str {
            "plugin_a"
        }
    }

    /// 测试用插件：依赖 plugin_a
    struct PluginB;

    #[async_trait]
    impl Plugin for PluginB {
        fn name(&self) -> &str {
            "plugin_b"
        }

        fn dependencies(&self) -> Vec<&str> {
            vec!["plugin_a"]
        }
    }

    /// 测试用插件：依赖 plugin_b
    struct PluginC;

    #[async_trait]
    impl Plugin for PluginC {
        fn name(&self) -> &str {
            "plugin_c"
        }

        fn dependencies(&self) -> Vec<&str> {
            vec!["plugin_b"]
        }
    }

    /// 测试用插件：注册时返回错误
    struct PluginFailing;

    #[async_trait]
    impl Plugin for PluginFailing {
        fn name(&self) -> &str {
            "plugin_failing"
        }

        async fn on_register(&self) -> Result<(), Box<dyn std::error::Error>> {
            Err("注册失败".into())
        }
    }

    /// 验证需求: P4 - PluginRegistry::get(name) 的结果与构建前注册的插件一一对应
    #[tokio::test]
    async fn test_registry_get_matches_registered_plugins() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();
        builder.register(PluginC).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");

        // 验证每个注册的插件都能通过 get 找到，且名称一致
        let plugin_a = registry.get("plugin_a");
        assert!(plugin_a.is_some(), "应能找到 plugin_a");
        assert_eq!(plugin_a.unwrap().name(), "plugin_a");

        let plugin_b = registry.get("plugin_b");
        assert!(plugin_b.is_some(), "应能找到 plugin_b");
        assert_eq!(plugin_b.unwrap().name(), "plugin_b");

        let plugin_c = registry.get("plugin_c");
        assert!(plugin_c.is_some(), "应能找到 plugin_c");
        assert_eq!(plugin_c.unwrap().name(), "plugin_c");

        // 验证不存在的插件返回 None
        assert!(
            registry.get("nonexistent").is_none(),
            "不存在的插件应返回 None"
        );
    }

    /// 验证需求: P4 - get_all() 返回缓存结果，多次调用结果一致
    #[tokio::test]
    async fn test_registry_get_all_returns_cached_result() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();
        builder.register(PluginC).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");

        // 多次调用 get_all() 应返回相同的结果（缓存）
        let all_first = registry.get_all();
        let all_second = registry.get_all();

        // 验证两次调用返回相同数量的插件
        assert_eq!(
            all_first.len(),
            all_second.len(),
            "多次调用 get_all() 应返回相同数量的插件"
        );
        assert_eq!(all_first.len(), 3, "应有 3 个插件");

        // 验证两次调用返回相同的插件名称（顺序一致）
        let names_first: Vec<&str> = all_first.iter().map(|p| p.name()).collect();
        let names_second: Vec<&str> = all_second.iter().map(|p| p.name()).collect();
        assert_eq!(
            names_first, names_second,
            "多次调用 get_all() 应返回相同顺序的插件"
        );
    }

    /// 验证需求: P4 - get_all() 返回的插件按拓扑顺序排列（依赖先于被依赖者）
    #[tokio::test]
    async fn test_registry_get_all_topological_order() {
        let mut builder = PluginManagerBuilder::new();
        // 故意以非依赖顺序注册
        builder.register(PluginC).await.unwrap();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");
        let all_plugins = registry.get_all();

        // 找到各插件在排序结果中的位置
        let pos_a = all_plugins.iter().position(|p| p.name() == "plugin_a");
        let pos_b = all_plugins.iter().position(|p| p.name() == "plugin_b");
        let pos_c = all_plugins.iter().position(|p| p.name() == "plugin_c");

        assert!(
            pos_a.is_some() && pos_b.is_some() && pos_c.is_some(),
            "所有插件应在排序结果中"
        );

        // 验证拓扑顺序：plugin_a 在 plugin_b 之前，plugin_b 在 plugin_c 之前
        assert!(
            pos_a.unwrap() < pos_b.unwrap(),
            "plugin_a（被依赖）应在 plugin_b 之前"
        );
        assert!(
            pos_b.unwrap() < pos_c.unwrap(),
            "plugin_b（被依赖）应在 plugin_c 之前"
        );
    }

    /// 验证需求: 9.3 - 注册重名插件应返回错误
    #[tokio::test]
    async fn test_builder_register_duplicate_returns_error() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();

        // 再次注册同名插件应返回错误
        struct PluginADuplicate;
        #[async_trait]
        impl Plugin for PluginADuplicate {
            fn name(&self) -> &str {
                "plugin_a"
            }
        }

        let result = builder.register(PluginADuplicate).await;
        assert!(
            matches!(result, Err(BaseError::PluginAlreadyRegistered(_))),
            "注册重名插件应返回 PluginAlreadyRegistered 错误"
        );
    }

    /// 验证需求: 9.3 - 注册回调失败时应返回错误
    #[tokio::test]
    async fn test_builder_register_callback_failure_returns_error() {
        let mut builder = PluginManagerBuilder::new();
        let result = builder.register(PluginFailing).await;
        assert!(
            matches!(result, Err(BaseError::PluginRegisterFailed(_, _))),
            "注册回调失败应返回 PluginRegisterFailed 错误"
        );
    }

    /// 验证需求: 9.4 - build() 消费构建器并返回 PluginRegistry
    #[tokio::test]
    async fn test_builder_build_produces_registry() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");

        // 验证 registry 包含注册的插件
        assert!(
            registry.get("plugin_a").is_some(),
            "registry 应包含已注册的插件"
        );
        assert_eq!(registry.get_all().len(), 1, "registry 应有 1 个插件");
    }

    /// 验证需求: 9.7 - get() 返回正确的插件引用
    #[tokio::test]
    async fn test_registry_get_returns_correct_plugin() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");

        // 验证 get() 返回正确的插件
        let plugin = registry.get("plugin_a").unwrap();
        assert_eq!(plugin.name(), "plugin_a", "get() 应返回正确名称的插件");
        assert_eq!(plugin.version(), "0.1.0", "get() 应返回正确版本的插件");
    }

    /// 验证需求: 9.9 - shutdown() 逆序关闭所有插件
    #[tokio::test]
    async fn test_registry_shutdown_succeeds() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");

        // 验证 shutdown() 成功执行
        let result = registry.shutdown().await;
        assert!(result.is_ok(), "shutdown() 应成功执行");
    }

    /// 验证需求: 9.9/TEST-6 - shutdown() 按逆拓扑顺序关闭（依赖者先关）
    ///
    /// 插件依赖关系: PluginA ← PluginB ← PluginC
    /// 拓扑排序结果: [plugin_a, plugin_b, plugin_c]（依赖在前）
    /// 关闭顺序应为逆序: [plugin_c, plugin_b, plugin_a]（依赖者先关）
    #[tokio::test]
    async fn test_shutdown_calls_in_reverse_topological_order() {
        use std::sync::Mutex;

        static SHUTDOWN_ORDER: Mutex<Vec<String>> = Mutex::new(Vec::new());
        SHUTDOWN_ORDER.lock().unwrap().clear();

        /// 无依赖插件 A
        struct PluginA;
        #[async_trait]
        impl Plugin for PluginA {
            fn name(&self) -> &str {
                "plugin_a"
            }
            async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
                SHUTDOWN_ORDER.lock().unwrap().push("plugin_a".to_string());
                Ok(())
            }
        }

        /// 依赖 plugin_a 的插件 B
        struct PluginB;
        #[async_trait]
        impl Plugin for PluginB {
            fn name(&self) -> &str {
                "plugin_b"
            }
            fn dependencies(&self) -> Vec<&str> {
                vec!["plugin_a"]
            }
            async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
                SHUTDOWN_ORDER.lock().unwrap().push("plugin_b".to_string());
                Ok(())
            }
        }

        /// 依赖 plugin_b 的插件 C（最顶层依赖者）
        struct PluginC;
        #[async_trait]
        impl Plugin for PluginC {
            fn name(&self) -> &str {
                "plugin_c"
            }
            fn dependencies(&self) -> Vec<&str> {
                vec!["plugin_b"]
            }
            async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
                SHUTDOWN_ORDER.lock().unwrap().push("plugin_c".to_string());
                Ok(())
            }
        }

        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();
        builder.register(PluginC).await.unwrap();

        let registry = builder.build().expect("构建注册表应成功");
        let result = registry.shutdown().await;
        assert!(result.is_ok(), "shutdown() 应成功执行");

        let order = SHUTDOWN_ORDER.lock().unwrap().clone();
        assert_eq!(
            order,
            vec!["plugin_c", "plugin_b", "plugin_a"],
            "shutdown 应按逆拓扑顺序关闭：C (依赖B) → B (依赖A) → A"
        );
    }

    /// 验证需求: 9.1/9.2 - PluginManagerBuilder::new() 创建空构建器
    #[tokio::test]
    async fn test_builder_new_creates_empty_builder() {
        let builder = PluginManagerBuilder::new();
        let registry = builder.build().expect("空构建器构建应成功");

        // 空构建器构建的 registry 应为空
        assert_eq!(registry.get_all().len(), 0, "空构建器应生成空 registry");
        assert!(
            registry.get("any").is_none(),
            "空 registry 不应包含任何插件"
        );
    }

    /// 验证需求: 20.1, 20.2, 20.3 - build() 检查依赖完整性
    #[tokio::test]
    async fn test_build_detects_missing_dependency() {
        let mut builder = PluginManagerBuilder::new();
        // 只注册 plugin_b，但 plugin_b 依赖 plugin_a（未注册）
        builder
            .register(PluginB)
            .await
            .expect("注册 plugin_b 应成功");

        let result = builder.build();
        assert!(
            matches!(result, Err(BaseError::PluginDependencyMissing(_, _))),
            "依赖未注册时应返回 PluginDependencyMissing 错误"
        );

        // 验证错误信息包含插件名和依赖名
        if let Err(BaseError::PluginDependencyMissing(plugin, dep)) = result {
            assert_eq!(plugin, "plugin_b", "错误应指向 plugin_b");
            assert_eq!(dep, "plugin_a", "缺失的依赖应是 plugin_a");
        }
    }

    /// 验证需求: 19.1, 19.2, 19.3, 19.4 - build() 检测循环依赖
    #[tokio::test]
    async fn test_build_detects_circular_dependency() {
        // 定义循环依赖的插件：X 依赖 Y，Y 依赖 X
        struct PluginX;
        #[async_trait]
        impl Plugin for PluginX {
            fn name(&self) -> &str {
                "plugin_x"
            }
            fn dependencies(&self) -> Vec<&str> {
                vec!["plugin_y"]
            }
        }

        struct PluginY;
        #[async_trait]
        impl Plugin for PluginY {
            fn name(&self) -> &str {
                "plugin_y"
            }
            fn dependencies(&self) -> Vec<&str> {
                vec!["plugin_x"]
            }
        }

        let mut builder = PluginManagerBuilder::new();
        builder
            .register(PluginX)
            .await
            .expect("注册 plugin_x 应成功");
        builder
            .register(PluginY)
            .await
            .expect("注册 plugin_y 应成功");

        let result = builder.build();
        assert!(
            matches!(result, Err(BaseError::PluginCircularDependency(_))),
            "循环依赖时应返回 PluginCircularDependency 错误"
        );

        // 验证错误信息包含未排序节点
        if let Err(BaseError::PluginCircularDependency(msg)) = result {
            assert!(
                msg.contains("plugin_x") || msg.contains("plugin_y"),
                "错误信息应包含循环中的插件名称"
            );
        }
    }

    /// 验证需求: 7.1, 7.2, 7.3, 7.4 - validate_config 集成 jsonschema
    #[cfg(feature = "plugin-schema")]
    #[tokio::test]
    async fn test_validate_config_with_schema() {
        use serde_json::json;

        // 定义带 Schema 的插件
        struct SchemaPlugin;
        #[async_trait]
        impl Plugin for SchemaPlugin {
            fn name(&self) -> &str {
                "schema_plugin"
            }
            fn config_schema(&self) -> Option<JsonValue> {
                Some(json!({
                    "type": "object",
                    "properties": {
                        "host": {"type": "string"},
                        "port": {"type": "integer"}
                    },
                    "required": ["host", "port"]
                }))
            }
        }

        let manager = PluginManager::new();
        manager.register(SchemaPlugin).await.expect("注册应成功");

        // 合法配置应通过验证
        let valid_config = json!({"host": "localhost", "port": 3306});
        let result = manager.load_config("schema_plugin", valid_config).await;
        assert!(result.is_ok(), "合法配置应通过验证");
    }

    /// 验证需求: 7.1, 7.2 - 配置不符合 Schema 时返回错误
    #[cfg(feature = "plugin-schema")]
    #[tokio::test]
    async fn test_validate_config_invalid_returns_error() {
        use serde_json::json;

        // 定义带 Schema 的插件
        struct StrictPlugin;
        #[async_trait]
        impl Plugin for StrictPlugin {
            fn name(&self) -> &str {
                "strict_plugin"
            }
            fn config_schema(&self) -> Option<JsonValue> {
                Some(json!({
                    "type": "object",
                    "properties": {
                        "port": {"type": "integer"}
                    },
                    "required": ["port"]
                }))
            }
        }

        let manager = PluginManager::new();
        manager.register(StrictPlugin).await.expect("注册应成功");

        // 配置缺少必填字段应返回错误
        let invalid_config = json!({"host": "localhost"});
        let result = manager.load_config("strict_plugin", invalid_config).await;
        assert!(
            matches!(result, Err(BaseError::PluginConfigInvalid(_, _))),
            "配置不符合 Schema 应返回 PluginConfigInvalid 错误"
        );
    }

    // ==================== C6 并发回归：register TOCTOU ====================

    /// 并发注册同名插件（TOCTOU 回归网，对应 I11）。
    ///
    /// `register` 的「read 检查 contains_key → on_register → write insert」是分离的
    /// 三段锁，存在 check-then-insert 竞态窗口：多个并发注册同名插件时，可能都越过
    /// 检查、最终多次 insert（后写覆盖）。本测试**锁定当前契约**：
    /// - 进程不 panic、不死锁，map 不被破坏
    /// - 并发结束后插件确实可查到且名称正确
    /// - 至少有一个 register 调用返回 Ok（拿到锁的胜出者）
    ///
    /// I11 修复（改单把 write 锁 check-and-insert）后，应能进一步断言「恰好一个
    /// Ok、其余 PluginAlreadyRegistered」——届时收紧本测试。
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_same_name_register_keeps_map_consistent() {
        struct DupPlugin;
        #[async_trait]
        impl Plugin for DupPlugin {
            fn name(&self) -> &str {
                "dup_plugin"
            }
        }

        let manager = Arc::new(PluginManager::new());

        let mut handles = Vec::new();
        for _ in 0..16 {
            let m = Arc::clone(&manager);
            handles.push(tokio::spawn(async move { m.register(DupPlugin).await }));
        }

        let mut ok_count = 0usize;
        for h in handles {
            // 任务本身不应 panic
            let res = h.await.expect("注册任务不应 panic");
            if res.is_ok() {
                ok_count += 1;
            }
        }

        // 当前契约：至少一个成功（窗口竞态下可能 >1）
        assert!(ok_count >= 1, "并发同名注册应至少有一个成功");

        // map 未被破坏：插件可查到且名称正确
        let got = manager.get("dup_plugin").await;
        assert!(got.is_some(), "并发注册后应能查到 dup_plugin");
        assert_eq!(got.unwrap().name(), "dup_plugin");
    }

    /// 并发注册不同名插件：全部成功，全部可查到，无丢失。
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_distinct_register_no_loss() {
        // 用宏生成固定数量的不同名插件，避免运行时动态 name 生命周期问题
        macro_rules! make_plugin {
            ($ty:ident, $name:literal) => {
                struct $ty;
                #[async_trait]
                impl Plugin for $ty {
                    fn name(&self) -> &str {
                        $name
                    }
                }
            };
        }
        make_plugin!(P0, "cn_p0");
        make_plugin!(P1, "cn_p1");
        make_plugin!(P2, "cn_p2");
        make_plugin!(P3, "cn_p3");
        make_plugin!(P4, "cn_p4");
        make_plugin!(P5, "cn_p5");
        make_plugin!(P6, "cn_p6");
        make_plugin!(P7, "cn_p7");

        let manager = Arc::new(PluginManager::new());
        let m = Arc::clone(&manager);

        // 并发注册 8 个不同名插件
        let (r0, r1, r2, r3, r4, r5, r6, r7) = tokio::join!(
            {
                let m = Arc::clone(&m);
                async move { m.register(P0).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P1).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P2).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P3).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P4).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P5).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P6).await }
            },
            {
                let m = Arc::clone(&m);
                async move { m.register(P7).await }
            },
        );
        for r in [r0, r1, r2, r3, r4, r5, r6, r7] {
            r.expect("不同名插件注册应全部成功");
        }

        for i in 0..8 {
            let name = format!("cn_p{}", i);
            assert!(
                manager.get(&name).await.is_some(),
                "{} 应已注册且可查到",
                name
            );
        }
    }
}
