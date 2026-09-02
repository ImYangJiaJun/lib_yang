//! 插件管理器构建器：构建阶段注册插件并生成不可变 `PluginRegistry`。

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

use super::normalize_plugin_name;
use super::registry::PluginRegistry;
use super::traits::{Plugin, PluginLifecycleStage};
use crate::error::BaseError;

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
        let name = normalize_plugin_name(plugin.name())?.to_string();

        // 检查插件是否已注册（构建阶段直接访问 HashMap，无需加锁）
        if self.plugins.contains_key(&name) {
            return Err(BaseError::PluginAlreadyRegistered(name));
        }

        let plugin = Arc::new(plugin);

        // 调用注册回调
        plugin
            .on_register()
            .await
            .map_err(|source| BaseError::PluginLifecycleFailed {
                plugin: name.clone(),
                stage: PluginLifecycleStage::Register,
                source,
            })?;

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
        for (plugin_name, plugin) in &self.plugins {
            for dep in plugin.dependencies() {
                let dep = normalize_plugin_name(dep)?;
                if !self.plugins.contains_key(dep) {
                    // 依赖未注册，返回错误
                    return Err(BaseError::PluginDependencyMissing(
                        plugin_name.clone(),
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
