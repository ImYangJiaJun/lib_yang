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

mod builder;
mod manager;
mod registry;
mod traits;

#[cfg(test)]
mod __tests__;

pub use builder::PluginManagerBuilder;
pub use manager::PluginManager;
pub use registry::PluginRegistry;
pub use traits::{Plugin, PluginError, PluginLifecycleStage};

use crate::error::BaseError;

fn normalize_plugin_name(name: &str) -> Result<&str, BaseError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(BaseError::PluginRegisterFailed(
            "<empty>".to_string(),
            "插件名称不能为空".to_string(),
        ));
    }

    Ok(name)
}

fn normalize_plugin_lookup_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty()).then_some(name)
}
