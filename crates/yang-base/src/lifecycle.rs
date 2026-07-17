//! 引擎生命周期：编排式优雅停机（I3）。
//!
//! 在 [`Tools`](crate::tools::Tools) 的统一资源生命周期之上，提供单一停机入口，
//! 按**与启动相反**的顺序收尾：
//!
//! 1. `PluginManager::shutdown`（业务先停接活，拓扑逆序触发各插件 `on_shutdown`）
//! 2. `Tools::close` 统一关闭 Redis 与 MySQL
//!
//! 配套一个 tokio 信号助手，使 K8s SIGTERM 触发 drain 而非 RST 在途连接。

use crate::error::BaseError;
use crate::plugin::PluginManager;
use crate::tools::Tools;

/// 等待停机信号：`Ctrl+C`（SIGINT）或（Unix 上）`SIGTERM`。
///
/// K8s 滚动更新发 SIGTERM，监听它才能在 grace period 内 drain 而非被 RST。
///
/// 信号注册失败采用 checked 降级（log + 仅监听 ctrl_c），**不** panic（遵守禁新增
/// 生产 panic 约定）。
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        // SIGTERM 注册失败则降级为仅 ctrl_c
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        log::info!("收到 Ctrl+C，开始优雅停机");
                    }
                    _ = sigterm.recv() => {
                        log::info!("收到 SIGTERM，开始优雅停机");
                    }
                }
            }
            Err(e) => {
                log::warn!("注册 SIGTERM 处理失败，降级为仅监听 Ctrl+C: {}", e);
                let _ = tokio::signal::ctrl_c().await;
                log::info!("收到 Ctrl+C，开始优雅停机");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        log::info!("收到 Ctrl+C，开始优雅停机");
    }
}

/// 编排式优雅停机：按启动逆序收尾（插件 → Tools 资源）。
///
/// 各步骤独立执行，插件 `on_shutdown` 失败不阻断后续连接池 drain（停机应尽力收尾
/// 全部资源）。
///
/// # 参数
///
/// - `plugins`: 可选的插件管理器引用；为 `None` 时跳过插件停机阶段。
/// - `tools`: 当前应用实例显式拥有的资源。
///
/// # 返回
///
/// - `Ok(())`: 全部步骤完成
/// - `Err(BaseError)`: 插件停机返回的首个错误（连接池仍已 drain，错误仅供上报）
pub async fn graceful_shutdown(
    plugins: Option<&PluginManager>,
    tools: &Tools,
) -> Result<(), BaseError> {
    let mut plugin_err = None;

    // 1. 插件先停（业务停止接活）
    if let Some(pm) = plugins {
        if let Err(e) = pm.shutdown().await {
            log::error!("插件停机出错（继续 drain 连接池）: {}", e);
            plugin_err = Some(e);
        }
    }

    // 2. 当前应用的资源统一关闭；不存在进程全局单例。
    tools.close().await;
    log::info!("应用 Tools 资源已关闭");

    match plugin_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Plugin;
    use crate::tools::ToolsBuilder;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    /// 测试用插件：在 on_shutdown 时将名称记录到共享 Vec
    struct ShutdownRecorder {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl Plugin for ShutdownRecorder {
        fn name(&self) -> &str {
            &self.name
        }

        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.order.lock().unwrap().push(self.name.clone());
            Ok(())
        }
    }

    /// 测试用插件：依赖 plugin_a，记录 shutdown 顺序
    struct PluginB(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl Plugin for PluginB {
        fn name(&self) -> &str {
            "plugin_b"
        }

        fn dependencies(&self) -> &[&str] {
            &["plugin_a"]
        }

        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.0.lock().unwrap().push("plugin_b".to_string());
            Ok(())
        }
    }

    /// 测试用插件：依赖 plugin_b，记录 shutdown 顺序
    struct PluginC(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl Plugin for PluginC {
        fn name(&self) -> &str {
            "plugin_c"
        }

        fn dependencies(&self) -> &[&str] {
            &["plugin_b"]
        }

        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.0.lock().unwrap().push("plugin_c".to_string());
            Ok(())
        }
    }

    /// 测试用插件：on_shutdown 故意返回错误
    struct ShutdownFailer;

    #[async_trait]
    impl Plugin for ShutdownFailer {
        fn name(&self) -> &str {
            "shutdown_failer"
        }

        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("intentional shutdown failure".into())
        }
    }

    // ==================== TEST-5：graceful_shutdown 完全无测试 ====================

    /// 验证需求: TEST-5 — plugins=None 时应直接返回 Ok
    #[tokio::test]
    async fn test_graceful_shutdown_no_plugins() {
        let tools = ToolsBuilder::new().build().expect("测试 Tools 应构建成功");
        let result = graceful_shutdown(None, &tools).await;
        assert!(
            result.is_ok(),
            "plugins=None 时 graceful_shutdown 应返回 Ok"
        );
    }

    /// 验证需求: TEST-5 — 插件 on_shutdown 返回 Err 时 graceful_shutdown 应返回 Err
    #[tokio::test]
    async fn test_graceful_shutdown_plugin_error() {
        let manager = PluginManager::new();
        manager.register(ShutdownFailer).await.unwrap();

        let tools = ToolsBuilder::new().build().expect("测试 Tools 应构建成功");
        let result = graceful_shutdown(Some(&manager), &tools).await;
        assert!(
            result.is_err(),
            "插件 shutdown 失败时 graceful_shutdown 应返回 Err"
        );

        if let Err(e) = result {
            match &e {
                BaseError::PluginLifecycleFailed {
                    plugin,
                    stage: crate::plugin::PluginLifecycleStage::Shutdown,
                    source,
                } => {
                    assert_eq!(plugin, "shutdown_failer", "错误应指向 shutdown_failer");
                    assert_eq!(source.to_string(), "intentional shutdown failure");
                }
                other => panic!("期望 PluginLifecycleFailed，得到: {:?}", other),
            }
            assert_eq!(e.code(), 100008, "shutdown 阶段错误码不得漂移");
        }
    }

    /// 验证需求: TEST-5 — 共享 Vec 记录器验证 shutdown 顺序为逆拓扑序
    #[tokio::test]
    async fn test_graceful_shutdown_order() {
        let order = Arc::new(Mutex::new(Vec::new()));

        let manager = PluginManager::new();

        // 注册三个插件，依赖链：A（无依赖）← B（依赖 A）← C（依赖 B）
        // 拓扑序：A, B, C；应逆序 shutdown 即 C, B, A
        manager
            .register(ShutdownRecorder {
                name: "plugin_a".to_string(),
                order: Arc::clone(&order),
            })
            .await
            .unwrap();

        manager.register(PluginB(Arc::clone(&order))).await.unwrap();

        manager.register(PluginC(Arc::clone(&order))).await.unwrap();

        let tools = ToolsBuilder::new().build().expect("测试 Tools 应构建成功");
        let result = graceful_shutdown(Some(&manager), &tools).await;
        assert!(result.is_ok(), "全部插件 shutdown 成功时应返回 Ok");

        let recorded = order.lock().unwrap().clone();
        assert_eq!(recorded.len(), 3, "应有 3 个插件被 shutdown");
        assert_eq!(
            recorded[0], "plugin_c",
            "第一个关闭的应是 plugin_c（最内层依赖，逆拓扑序首位）"
        );
        assert_eq!(recorded[1], "plugin_b", "第二个关闭的应是 plugin_b");
        assert_eq!(
            recorded[2], "plugin_a",
            "第三个关闭的应是 plugin_a（无依赖，应最后关闭）"
        );
    }
}
