//! 应用资源的显式所有权与统一生命周期。
//!
//! [`ToolsBuilder`] 只在启动期可变；[`Tools`] 构建完成后只提供不可变访问。
//! 高频资源使用直接字段，低频扩展与配置使用按 Rust 类型索引的只读映射。

use crate::error::BaseError;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Mutex;

#[cfg(feature = "token")]
use crate::token::TokenManager;
#[cfg(feature = "mysql")]
use yang_db::Database;
#[cfg(feature = "redis")]
use yang_db::RedisClient;

const RUNNING: u8 = 0;
const CLOSING: u8 = 1;
const CLOSED: u8 = 2;

type TypeMap = HashMap<TypeId, Box<dyn Any + Send + Sync>>;

/// 应用资源生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsState {
    /// 资源可用于请求处理。
    Running,
    /// 正在按逆初始化顺序关闭。
    Closing,
    /// 资源已经关闭。
    Closed,
}

/// 单个已配置资源的健康状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceHealth {
    /// 稳定的资源类型名。
    pub resource: &'static str,
    /// 健康检查是否成功。
    pub healthy: bool,
    /// 失败原因；健康时为 `None`。
    pub detail: Option<String>,
}

/// [`Tools`] 的健康检查快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolsHealth {
    /// 检查时的生命周期状态。
    pub state: ToolsState,
    /// 所有已配置且支持检查的核心资源。
    pub resources: Vec<ResourceHealth>,
}

impl ToolsHealth {
    /// 所有资源是否处于运行且健康状态。
    pub fn is_healthy(&self) -> bool {
        self.state == ToolsState::Running && self.resources.iter().all(|item| item.healthy)
    }
}

/// 构建完成后冻结的应用资源。
pub struct Tools {
    #[cfg(feature = "mysql")]
    database: Option<Database>,
    #[cfg(feature = "redis")]
    cache: Option<RedisClient>,
    #[cfg(feature = "token")]
    token: Option<TokenManager>,
    extensions: TypeMap,
    config: TypeMap,
    state: AtomicU8,
    close_lock: Mutex<()>,
}

impl fmt::Debug for Tools {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Tools")
            .field("state", &self.state())
            .field("extension_count", &self.extensions.len())
            .field("config_count", &self.config.len())
            .finish_non_exhaustive()
    }
}

impl Tools {
    /// 返回当前生命周期状态。
    pub fn state(&self) -> ToolsState {
        match self.state.load(Ordering::Acquire) {
            RUNNING => ToolsState::Running,
            CLOSING => ToolsState::Closing,
            _ => ToolsState::Closed,
        }
    }

    fn ensure_running(&self) -> Result<(), BaseError> {
        match self.state() {
            ToolsState::Running => Ok(()),
            ToolsState::Closing => Err(BaseError::ConfigError("Tools 正在关闭".to_string())),
            ToolsState::Closed => Err(BaseError::ConfigError("Tools 已关闭".to_string())),
        }
    }

    /// 获取 MySQL 数据库；未配置或生命周期已结束时失败。
    #[cfg(feature = "mysql")]
    pub fn db(&self) -> Result<&Database, BaseError> {
        self.ensure_running()?;
        self.database
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)
    }

    /// 返回可选数据库，供只构建查询计划而不执行 SQL 的内部路径使用。
    #[cfg(feature = "mysql")]
    pub(crate) fn optional_db(&self) -> Result<Option<&Database>, BaseError> {
        self.ensure_running()?;
        Ok(self.database.as_ref())
    }

    /// 获取 Redis 客户端；未配置或生命周期已结束时失败。
    #[cfg(feature = "redis")]
    pub fn cache(&self) -> Result<&RedisClient, BaseError> {
        self.ensure_running()?;
        self.cache.as_ref().ok_or(BaseError::RedisNotInitialized)
    }

    /// 获取 Token 管理器；未配置或生命周期已结束时失败。
    #[cfg(feature = "token")]
    pub fn token(&self) -> Result<&TokenManager, BaseError> {
        self.ensure_running()?;
        self.token
            .as_ref()
            .ok_or_else(|| BaseError::ConfigError("Tools 未配置 TokenManager".to_string()))
    }

    /// 按具体类型获取低频扩展。
    pub fn extension<T>(&self) -> Result<&T, BaseError>
    where
        T: Any + Send + Sync,
    {
        self.ensure_running()?;
        self.extensions
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .ok_or_else(|| {
                BaseError::ConfigError(format!("Tools 未配置扩展: {}", type_name::<T>()))
            })
    }

    /// 按具体类型获取只读配置。
    pub fn config<T>(&self) -> Result<&T, BaseError>
    where
        T: Any + Send + Sync,
    {
        self.ensure_running()?;
        self.config
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .ok_or_else(|| {
                BaseError::ConfigError(format!("Tools 未配置配置类型: {}", type_name::<T>()))
            })
    }

    /// 检查所有已配置核心资源并返回稳定快照。
    pub async fn health_check(&self) -> ToolsHealth {
        let state = self.state();
        if state != ToolsState::Running {
            return ToolsHealth {
                state,
                resources: Vec::new(),
            };
        }

        let mut resources = Vec::new();
        #[cfg(feature = "mysql")]
        if let Some(database) = &self.database {
            resources.push(match database.health_check().await {
                Ok(healthy) => ResourceHealth {
                    resource: "database",
                    healthy,
                    detail: None,
                },
                Err(error) => ResourceHealth {
                    resource: "database",
                    healthy: false,
                    detail: Some(error.to_string()),
                },
            });
        }
        #[cfg(feature = "redis")]
        if let Some(cache) = &self.cache {
            resources.push(match cache.health_check().await {
                Ok(healthy) => ResourceHealth {
                    resource: "cache",
                    healthy,
                    detail: None,
                },
                Err(error) => ResourceHealth {
                    resource: "cache",
                    healthy: false,
                    detail: Some(error.to_string()),
                },
            });
        }

        ToolsHealth { state, resources }
    }

    /// 幂等关闭所有核心资源。
    ///
    /// 关闭顺序与常见启动顺序相反：先 Redis，再 MySQL。并发调用会等待首个关闭流程完成。
    pub async fn close(&self) {
        let _guard = self.close_lock.lock().await;
        if self.state() == ToolsState::Closed {
            return;
        }
        self.state.store(CLOSING, Ordering::Release);

        #[cfg(feature = "redis")]
        if let Some(cache) = &self.cache {
            cache.close().await;
        }
        #[cfg(feature = "mysql")]
        if let Some(database) = &self.database {
            database.close().await;
        }

        self.state.store(CLOSED, Ordering::Release);
    }
}

/// 仅在启动期可变的应用资源构建器。
#[derive(Default)]
#[must_use = "ToolsBuilder 必须调用 build() 才会冻结并交给应用"]
pub struct ToolsBuilder {
    #[cfg(feature = "mysql")]
    database: Option<Database>,
    #[cfg(feature = "redis")]
    cache: Option<RedisClient>,
    #[cfg(feature = "token")]
    token: Option<TokenManager>,
    extensions: TypeMap,
    config: TypeMap,
    duplicate: Option<String>,
}

impl fmt::Debug for ToolsBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolsBuilder")
            .field("extension_count", &self.extensions.len())
            .field("config_count", &self.config.len())
            .field("duplicate", &self.duplicate)
            .finish_non_exhaustive()
    }
}

impl ToolsBuilder {
    /// 创建空构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置唯一 MySQL 数据库资源。
    #[cfg(feature = "mysql")]
    pub fn database(mut self, database: Database) -> Self {
        if self.database.replace(database).is_some() {
            self.record_duplicate("Database");
        }
        self
    }

    /// 设置唯一 Redis 客户端。
    #[cfg(feature = "redis")]
    pub fn cache(mut self, cache: RedisClient) -> Self {
        if self.cache.replace(cache).is_some() {
            self.record_duplicate("RedisClient");
        }
        self
    }

    /// 设置唯一 Token 管理器。
    #[cfg(feature = "token")]
    pub fn token(mut self, token: TokenManager) -> Self {
        if self.token.replace(token).is_some() {
            self.record_duplicate("TokenManager");
        }
        self
    }

    /// 注册一个按具体 Rust 类型索引的低频扩展。
    pub fn extension<T>(mut self, extension: T) -> Self
    where
        T: Any + Send + Sync,
    {
        if self
            .extensions
            .insert(TypeId::of::<T>(), Box::new(extension))
            .is_some()
        {
            self.record_duplicate(type_name::<T>());
        }
        self
    }

    /// 注册一个按具体 Rust 类型索引的只读配置。
    pub fn config<T>(mut self, config: T) -> Self
    where
        T: Any + Send + Sync,
    {
        if self
            .config
            .insert(TypeId::of::<T>(), Box::new(config))
            .is_some()
        {
            self.record_duplicate(type_name::<T>());
        }
        self
    }

    fn record_duplicate(&mut self, resource: &str) {
        if self.duplicate.is_none() {
            self.duplicate = Some(resource.to_string());
        }
    }

    /// 校验唯一性、连接 Token 撤销存储并冻结资源。
    pub fn build(mut self) -> Result<Tools, BaseError> {
        if let Some(resource) = self.duplicate {
            return Err(BaseError::ConfigError(format!(
                "Tools 资源重复注册: {resource}"
            )));
        }

        #[cfg(feature = "token")]
        if let (Some(token), Some(cache)) = (&mut self.token, &self.cache) {
            token.attach_revocation_cache(cache.clone());
        }

        Ok(Tools {
            #[cfg(feature = "mysql")]
            database: self.database,
            #[cfg(feature = "redis")]
            cache: self.cache,
            #[cfg(feature = "token")]
            token: self.token,
            extensions: self.extensions,
            config: self.config,
            state: AtomicU8::new(RUNNING),
            close_lock: Mutex::new(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestExtension(u32);

    #[derive(Debug, PartialEq, Eq)]
    struct TestConfig(&'static str);

    #[test]
    fn typed_entries_are_frozen_and_separate() {
        let tools = ToolsBuilder::new()
            .extension(TestExtension(7))
            .config(TestConfig("local"))
            .build()
            .expect("类型化资源应构建成功");

        assert_eq!(
            tools.extension::<TestExtension>().expect("扩展应存在"),
            &TestExtension(7)
        );
        assert_eq!(
            tools.config::<TestConfig>().expect("配置应存在"),
            &TestConfig("local")
        );
        assert!(tools.extension::<TestConfig>().is_err());
        assert!(tools.config::<TestExtension>().is_err());
    }

    #[test]
    fn duplicate_type_is_rejected_at_build_time() {
        let result = ToolsBuilder::new()
            .extension(TestExtension(1))
            .extension(TestExtension(2))
            .build();
        assert!(
            matches!(result, Err(BaseError::ConfigError(message)) if message.contains("重复注册"))
        );
    }

    #[tokio::test]
    async fn close_is_idempotent_and_changes_health_semantics() {
        let tools = ToolsBuilder::new()
            .build()
            .expect("空资源集合可用于纯逻辑测试");
        assert!(tools.health_check().await.is_healthy());

        tools.close().await;
        tools.close().await;

        let health = tools.health_check().await;
        assert_eq!(health.state, ToolsState::Closed);
        assert!(!health.is_healthy());
        assert!(matches!(
            tools.extension::<TestExtension>(),
            Err(BaseError::ConfigError(message)) if message == "Tools 已关闭"
        ));
    }
}
