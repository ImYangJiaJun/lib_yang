//! 引擎生命周期：编排式优雅停机（I3）。
//!
//! 在 yang-db 的连接池 drain 原语（`Database::close` / `RedisClient::close`）之上，
//! 提供单一停机入口，按**与启动相反**的顺序收尾：
//!
//! 1. `PluginManager::shutdown`（业务先停接活，拓扑逆序触发各插件 `on_shutdown`）
//! 2. 关闭 Redis 连接池
//! 3. drain MySQL 连接池
//!
//! 与 [`DatabaseBundle::init`](crate::database::DatabaseBundle) 的「先 MySQL 后 Redis」
//! 启动顺序严格逆序。配套一个 tokio 信号助手，使 K8s SIGTERM 触发 drain 而非 RST 在途连接。
//!
//! `OnceLock` 单例不重置，停机为原地 drain——**停机后不应再 dispatch**。

#[cfg(feature = "mysql")]
use crate::error::BaseError;
#[cfg(feature = "mysql")]
use crate::plugin::PluginManager;

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

/// 编排式优雅停机：按启动逆序收尾（插件 → Redis → MySQL）。
///
/// 与 `DatabaseBundle::init`（先 MySQL 后 Redis）形成对称的「统一停机入口」。各步骤
/// 独立执行，插件 `on_shutdown` 失败不阻断后续连接池 drain（停机应尽力收尾全部资源）。
///
/// # 参数
///
/// - `plugins`: 可选的插件管理器引用；为 `None` 时跳过插件停机阶段。
///
/// # 返回
///
/// - `Ok(())`: 全部步骤完成
/// - `Err(BaseError)`: 插件停机返回的首个错误（连接池仍已 drain，错误仅供上报）
#[cfg(feature = "mysql")]
pub async fn graceful_shutdown(plugins: Option<&PluginManager>) -> Result<(), BaseError> {
    let mut plugin_err = None;

    // 1. 插件先停（业务停止接活）
    if let Some(pm) = plugins {
        if let Err(e) = pm.shutdown().await {
            log::error!("插件停机出错（继续 drain 连接池）: {}", e);
            plugin_err = Some(e);
        }
    }

    // 2. 关闭 Redis（与启动顺序逆序）
    crate::database::GlobalRedis::close();
    log::info!("Redis 连接池已关闭");

    // 3. drain MySQL（最后关，等待在途归还）
    crate::database::GlobalDatabase::close().await;
    log::info!("MySQL 连接池已 drain 关闭");

    match plugin_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
