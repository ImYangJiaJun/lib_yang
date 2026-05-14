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
    /// - 不可变的 `PluginRegistry` 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::plugin::PluginManagerBuilder;
    ///
    /// let mut builder = PluginManagerBuilder::new();
    /// // ... 注册插件 ...
    /// let registry = builder.build();
    /// ```
    pub fn build(self) -> PluginRegistry {
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
    /// - 新的 PluginRegistry 实例
    fn new(
        plugins: HashMap<String, Arc<dyn Plugin>>,
        configs: HashMap<String, JsonValue>,
    ) -> Self {
        // 构建时执行一次拓扑排序并缓存结果
        let sorted_plugins = Self::compute_topological_sort(&plugins);
        Self {
            plugins,
            sorted_plugins,
            configs,
        }
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
        for plugin in self.sorted_plugins.iter().rev() {
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

    /// 计算拓扑排序（私有方法，构建时调用一次）
    ///
    /// 使用 Kahn 算法对插件进行拓扑排序，确保依赖插件先于当前插件出现。
    /// 与 `PluginManager::topological_sort` 逻辑一致，但接收 HashMap 引用。
    ///
    /// # 参数
    /// - `plugins`: 插件 HashMap 引用
    ///
    /// # 返回
    /// - 按依赖关系排序的插件列表
    fn compute_topological_sort(
        plugins: &HashMap<String, Arc<dyn Plugin>>,
    ) -> Vec<Arc<dyn Plugin>> {
        // 构建入度表和邻接图
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for plugin in plugins.values() {
            let name = plugin.name().to_string();
            in_degree.entry(name.clone()).or_insert(0);

            for dep in plugin.dependencies() {
                // dep -> name 的边：dep 是 name 的依赖，dep 先执行
                graph
                    .entry(dep.to_string())
                    .or_default()
                    .push(name.clone());
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

        // 按排序顺序构建插件列表
        let mut sorted_plugins: Vec<Arc<dyn Plugin>> = plugins.values().cloned().collect();
        sorted_plugins.sort_by_key(|p| {
            sorted_names
                .iter()
                .position(|n| n == p.name())
                .unwrap_or(usize::MAX)
        });

        sorted_plugins
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

        let registry = builder.build();

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
        assert!(registry.get("nonexistent").is_none(), "不存在的插件应返回 None");
    }

    /// 验证需求: P4 - get_all() 返回缓存结果，多次调用结果一致
    #[tokio::test]
    async fn test_registry_get_all_returns_cached_result() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();
        builder.register(PluginC).await.unwrap();

        let registry = builder.build();

        // 多次调用 get_all() 应返回相同的结果（缓存）
        let all_first = registry.get_all();
        let all_second = registry.get_all();

        // 验证两次调用返回相同数量的插件
        assert_eq!(all_first.len(), all_second.len(), "多次调用 get_all() 应返回相同数量的插件");
        assert_eq!(all_first.len(), 3, "应有 3 个插件");

        // 验证两次调用返回相同的插件名称（顺序一致）
        let names_first: Vec<&str> = all_first.iter().map(|p| p.name()).collect();
        let names_second: Vec<&str> = all_second.iter().map(|p| p.name()).collect();
        assert_eq!(names_first, names_second, "多次调用 get_all() 应返回相同顺序的插件");
    }

    /// 验证需求: P4 - get_all() 返回的插件按拓扑顺序排列（依赖先于被依赖者）
    #[tokio::test]
    async fn test_registry_get_all_topological_order() {
        let mut builder = PluginManagerBuilder::new();
        // 故意以非依赖顺序注册
        builder.register(PluginC).await.unwrap();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();

        let registry = builder.build();
        let all_plugins = registry.get_all();

        // 找到各插件在排序结果中的位置
        let pos_a = all_plugins.iter().position(|p| p.name() == "plugin_a");
        let pos_b = all_plugins.iter().position(|p| p.name() == "plugin_b");
        let pos_c = all_plugins.iter().position(|p| p.name() == "plugin_c");

        assert!(pos_a.is_some() && pos_b.is_some() && pos_c.is_some(), "所有插件应在排序结果中");

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

        let registry = builder.build();

        // 验证 registry 包含注册的插件
        assert!(registry.get("plugin_a").is_some(), "registry 应包含已注册的插件");
        assert_eq!(registry.get_all().len(), 1, "registry 应有 1 个插件");
    }

    /// 验证需求: 9.7 - get() 返回正确的插件引用
    #[tokio::test]
    async fn test_registry_get_returns_correct_plugin() {
        let mut builder = PluginManagerBuilder::new();
        builder.register(PluginA).await.unwrap();
        builder.register(PluginB).await.unwrap();

        let registry = builder.build();

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

        let registry = builder.build();

        // 验证 shutdown() 成功执行
        let result = registry.shutdown().await;
        assert!(result.is_ok(), "shutdown() 应成功执行");
    }

    /// 验证需求: 9.1/9.2 - PluginManagerBuilder::new() 创建空构建器
    #[tokio::test]
    async fn test_builder_new_creates_empty_builder() {
        let builder = PluginManagerBuilder::new();
        let registry = builder.build();

        // 空构建器构建的 registry 应为空
        assert_eq!(registry.get_all().len(), 0, "空构建器应生成空 registry");
        assert!(registry.get("any").is_none(), "空 registry 不应包含任何插件");
    }
}
