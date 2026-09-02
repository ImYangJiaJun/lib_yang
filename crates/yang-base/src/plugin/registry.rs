//! 插件注册表：运行阶段使用的不可变、无锁插件集合。

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

use super::traits::{Plugin, PluginError, PluginLifecycleStage};
use super::{normalize_plugin_lookup_name, normalize_plugin_name};
use crate::error::BaseError;

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
    /// 创建新的插件注册表（仅 `plugin` 模块内可见的构造方法）
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
    pub(super) fn new(
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
        let name = normalize_plugin_lookup_name(name)?;
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
        let name = normalize_plugin_lookup_name(name)?;
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
        let mut errors: Vec<(String, PluginError)> = Vec::new();

        for plugin in self.sorted_plugins.iter().rev() {
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

        for (name, plugin) in plugins {
            let name = name.clone();
            in_degree.entry(name.clone()).or_insert(0);

            for dep in plugin.dependencies() {
                let dep = normalize_plugin_name(dep)?;
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

        // 按排序顺序构建插件列表。这里使用注册阶段规范化后的 HashMap key，
        // 避免插件对象 `name()` 返回带边界空格时与内部索引不一致。
        let sorted_plugins: Vec<Arc<dyn Plugin>> = sorted_names
            .iter()
            .filter_map(|name| plugins.get(name).cloned())
            .collect();

        Ok(sorted_plugins)
    }
}
