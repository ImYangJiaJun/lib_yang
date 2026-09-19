//! 插件管理器：运行期可变的插件注册、查找、配置加载与关闭。

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::traits::{Plugin, PluginError, PluginLifecycleStage};
use super::{normalize_plugin_lookup_name, normalize_plugin_name};
use crate::error::BaseError;

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
        let name = normalize_plugin_name(plugin.name())?.to_string();
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
            .map_err(|source| BaseError::PluginLifecycleFailed {
                plugin: name.clone(),
                stage: PluginLifecycleStage::Register,
                source,
            })?;

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
        let name = normalize_plugin_lookup_name(name)?;
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
        let mut plugin_list: Vec<(String, Arc<dyn Plugin>)> = plugins
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // 按依赖关系排序（节点名使用注册时归一化后的 key，见 topological_sort）
        self.topological_sort(&mut plugin_list);

        plugin_list.into_iter().map(|(_, p)| p).collect()
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
        let normalized_name = normalize_plugin_lookup_name(name)
            .ok_or_else(|| BaseError::PluginNotFound(name.to_string()))?;

        // 获取插件
        let plugin = self
            .get(normalized_name)
            .await
            .ok_or_else(|| BaseError::PluginNotFound(normalized_name.to_string()))?;

        // 验证配置（插件未定义 config_schema 时跳过验证）
        if let Some(schema) = plugin.config_schema() {
            self.validate_config(normalized_name, &config, &schema)?;
        }

        // 存储配置
        {
            let mut configs = self.configs.write().await;
            configs.insert(normalized_name.to_string(), config);
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
        let name = normalize_plugin_lookup_name(name)?;
        let configs = self.configs.read().await;
        configs.get(name).cloned()
    }

    /// 拓扑排序（按依赖关系排序）
    ///
    /// # 参数
    /// - plugins: `(注册 key, 插件)` 列表，key 为注册时经 `normalize_plugin_name` 归一化后的名称
    ///
    /// # 说明
    /// 使用 Kahn 算法进行拓扑排序，确保依赖插件先于当前插件。
    ///
    /// 节点名一律使用注册 key、依赖名一律经 `normalize_plugin_name` 归一化后再建边，
    /// 与 [`super::registry::PluginRegistry`] 的 `compute_topological_sort` 同口径：
    /// 避免插件对象 `name()` 返回带边界空格时与内部 key 失配，造成断边或误报循环依赖。
    fn topological_sort(&self, plugins: &mut Vec<(String, Arc<dyn Plugin>)>) {
        // 构建依赖图
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for (name, plugin) in plugins.iter() {
            in_degree.entry(name.clone()).or_insert(0);

            for dep in plugin.dependencies() {
                // 与 registry 同口径：trim 后再建边
                let Ok(dep) = normalize_plugin_name(dep) else {
                    continue;
                };
                if !plugins.iter().any(|(n, _)| n == dep) {
                    log::warn!("插件 {} 声明的依赖 {:?} 未注册，无法建立依赖边", name, dep);
                }
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

        let mut sorted: Vec<String> = Vec::new();

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

        // 检测循环依赖或断边：Kahn 算法结束后，未被排序的节点降级到最后
        if sorted.len() < plugins.len() {
            let circular: Vec<String> = plugins
                .iter()
                .map(|(n, _)| n.clone())
                .filter(|n| !sorted.contains(n))
                .collect();
            log::error!(
                "插件依赖图无法排序（存在环或断边），以下插件降级到最后: {}",
                circular.join(", ")
            );
            // 将无法排序的插件追加到排序末尾，保证所有插件都出现在结果中
            sorted.extend(circular);
        }

        // 重新排序插件列表（比较器同样用归一化 key，避免二次失配）
        plugins.sort_by_key(|(n, _)| sorted.iter().position(|s| s == n).unwrap_or(usize::MAX));
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

        let mut errors: Vec<(String, PluginError)> = Vec::new();

        for plugin in plugins {
            let name = plugin.name();
            if let Err(e) = plugin.on_shutdown().await {
                log::error!("插件 {} 关闭失败: {}", name, e);
                errors.push((name.to_string(), e));
            } else {
                log::info!("插件已关闭: {}", name);
            }
        }

        if let Some((plugin, source)) = errors.into_iter().next() {
            Err(BaseError::PluginLifecycleFailed {
                plugin,
                stage: PluginLifecycleStage::Shutdown,
                source,
            })
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
