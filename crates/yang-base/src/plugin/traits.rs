//! 插件接口（`Plugin` trait）与生命周期类型定义。

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::fmt;

/// 插件生命周期回调的结构化错误类型。
pub type PluginError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// 插件生命周期阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginLifecycleStage {
    /// 注册回调。
    Register,
    /// 初始化 SQL 或初始化回调。
    Initialize,
    /// 关闭回调。
    Shutdown,
}

impl fmt::Display for PluginLifecycleStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Register => "register",
            Self::Initialize => "initialize",
            Self::Shutdown => "shutdown",
        })
    }
}

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
    fn dependencies(&self) -> &[&str] {
        &[]
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
    async fn on_register(&self) -> Result<(), PluginError> {
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
    async fn on_init(&self) -> Result<(), PluginError> {
        Ok(())
    }

    /// 系统关闭时的回调
    ///
    /// 在系统关闭时调用，用于清理资源
    ///
    /// # 返回
    /// - Ok(()): 关闭成功
    /// - Err: 关闭失败
    async fn on_shutdown(&self) -> Result<(), PluginError> {
        Ok(())
    }
}
