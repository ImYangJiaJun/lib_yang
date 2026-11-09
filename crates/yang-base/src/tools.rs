//! 应用资源的显式所有权与统一生命周期。
//!
//! [`ToolsBuilder`] 只在启动期可变；[`Tools`] 构建完成后只提供不可变访问。
//! 高频资源使用直接字段，低频扩展与配置使用按 Rust 类型索引的只读映射。

use crate::error::BaseError;
use crate::table::RegexCache;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Mutex;

#[cfg(any(feature = "mysql", feature = "postgres"))]
use crate::database::AnyDatabase;
#[cfg(feature = "http")]
use crate::http::HttpClient;
#[cfg(feature = "token")]
use crate::token::TokenManager;
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
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    database: Option<AnyDatabase>,
    #[cfg(feature = "redis")]
    cache: Option<RedisClient>,
    #[cfg(feature = "token")]
    token: Option<TokenManager>,
    #[cfg(feature = "http")]
    http: Option<HttpClient>,
    regex_cache: RegexCache,
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

    /// 获取运行时数据库后端；未配置或生命周期已结束时失败。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn database(&self) -> Result<&AnyDatabase, BaseError> {
        self.ensure_running()?;
        self.database
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)
    }

    /// 获取 MySQL 连接池；未配置 MySQL 后端或生命周期已结束时失败。
    ///
    /// 当运行期配置的是 PostgreSQL 后端时同样返回 [`BaseError::DatabaseNotInitialized`]，
    /// 避免把 PG 连接池误当作 MySQL 使用。
    #[cfg(feature = "mysql")]
    pub fn mysql(&self) -> Result<&yang_db::mysql::Database, BaseError> {
        self.ensure_running()?;
        self.database
            .as_ref()
            .and_then(AnyDatabase::as_mysql)
            .ok_or(BaseError::DatabaseNotInitialized)
    }

    /// 返回当前数据库连接池状态快照；未配置数据库时为 `None`。
    ///
    /// 不检查生命周期状态，便于关闭后继续观察连接池回收情况。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn pool_status(&self) -> Option<yang_db::PoolStatus> {
        self.database.as_ref().map(AnyDatabase::pool_status)
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

    /// 获取 HTTP 客户端；未配置或生命周期已结束时失败。
    ///
    /// reqwest 客户端无需显式关闭，故该槽位不参与 close/health_check 流程。
    #[cfg(feature = "http")]
    pub fn http(&self) -> Result<&HttpClient, BaseError> {
        self.ensure_running()?;
        self.http
            .as_ref()
            .ok_or(BaseError::HttpClientNotInitialized)
    }

    /// 获取共享的有界正则缓存。
    ///
    /// 缓存为冻结的不可变资源，无需参与 close/health_check 流程；始终可用。
    pub fn regex_cache(&self) -> &RegexCache {
        &self.regex_cache
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

        #[cfg(any(feature = "mysql", feature = "postgres", feature = "redis"))]
        let mut resources = Vec::new();
        #[cfg(not(any(feature = "mysql", feature = "postgres", feature = "redis")))]
        let resources = Vec::new();
        #[cfg(any(feature = "mysql", feature = "postgres"))]
        if let Some(database) = &self.database {
            let resource = match database.backend() {
                yang_db::BackendKind::MySql => "mysql",
                yang_db::BackendKind::PostgreSql => "postgres",
                _ => "database",
            };
            resources.push(match database.health_check().await {
                Ok(healthy) => ResourceHealth {
                    resource,
                    healthy,
                    detail: None,
                },
                Err(error) => ResourceHealth {
                    resource,
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
    /// 关闭顺序与常见启动顺序相反：先 Redis，再数据库。并发调用会等待首个关闭流程完成。
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
        #[cfg(any(feature = "mysql", feature = "postgres"))]
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
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    database: Option<AnyDatabase>,
    #[cfg(feature = "redis")]
    cache: Option<RedisClient>,
    #[cfg(feature = "token")]
    token: Option<TokenManager>,
    #[cfg(feature = "http")]
    http: Option<HttpClient>,
    regex_cache: RegexCache,
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

    /// 设置唯一 MySQL 资源（兼容入口，内部包装为 [`AnyDatabase`]）。
    #[cfg(feature = "mysql")]
    pub fn mysql(mut self, mysql: yang_db::mysql::Database) -> Self {
        self.set_database(AnyDatabase::mysql(mysql), "MySQL");
        self
    }

    /// 设置唯一运行时数据库后端（MySQL 或 PostgreSQL）。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn database(mut self, database: AnyDatabase) -> Self {
        self.set_database(database, "Database");
        self
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    fn set_database(&mut self, database: AnyDatabase, resource: &str) {
        if self.database.replace(database).is_some() {
            self.record_duplicate(resource);
        }
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

    /// 设置唯一 HTTP 客户端。
    ///
    /// reqwest 客户端无需显式关闭，构建后只读共享，不参与 close/health_check 流程。
    #[cfg(feature = "http")]
    pub fn http(mut self, client: HttpClient) -> Self {
        if self.http.replace(client).is_some() {
            self.record_duplicate("HttpClient");
        }
        self
    }

    /// 覆盖默认容量的共享正则缓存。
    pub fn regex_cache(mut self, cache: RegexCache) -> Self {
        self.regex_cache = cache;
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
    pub fn build(self) -> Result<Tools, BaseError> {
        if let Some(resource) = self.duplicate {
            return Err(BaseError::ConfigError(format!(
                "Tools 资源重复注册: {resource}"
            )));
        }

        #[cfg(feature = "token")]
        let token = {
            let mut token = self.token;
            if let (Some(token), Some(cache)) = (&mut token, &self.cache) {
                token.attach_revocation_cache(cache.clone());
            }
            token
        };

        Ok(Tools {
            #[cfg(any(feature = "mysql", feature = "postgres"))]
            database: self.database,
            #[cfg(feature = "redis")]
            cache: self.cache,
            #[cfg(feature = "token")]
            token,
            #[cfg(feature = "http")]
            http: self.http,
            regex_cache: self.regex_cache,
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

    #[cfg(feature = "http")]
    fn test_http_client() -> crate::http::HttpClient {
        crate::http::HttpClient::new(30).expect("测试 HTTP 客户端应创建成功")
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_slot_returns_client_when_configured() {
        let tools = ToolsBuilder::new()
            .http(test_http_client())
            .build()
            .expect("配置 HTTP 客户端后应构建成功");

        let client = tools.http().expect("已配置时应返回 HTTP 客户端");
        // 验证拿到的是可用客户端（能构建请求）
        let _builder = client.get("https://api.example.com/test");
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_slot_errors_when_not_configured() {
        let tools = ToolsBuilder::new().build().expect("空资源集合应构建成功");

        assert!(matches!(
            tools.http(),
            Err(BaseError::HttpClientNotInitialized)
        ));
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_slot_duplicate_registration_is_rejected_at_build_time() {
        let result = ToolsBuilder::new()
            .http(test_http_client())
            .http(test_http_client())
            .build();

        assert!(
            matches!(result, Err(BaseError::ConfigError(message)) if message.contains("重复注册"))
        );
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn http_slot_errors_after_close() {
        let tools = ToolsBuilder::new()
            .http(test_http_client())
            .build()
            .expect("配置 HTTP 客户端后应构建成功");

        tools.close().await;

        assert!(matches!(
            tools.http(),
            Err(BaseError::ConfigError(message)) if message == "Tools 已关闭"
        ));
    }

    /// 用惰性连接池构造 MySQL 连接池：不建立真实网络连接。
    #[cfg(feature = "mysql")]
    fn test_mysql_pool() -> yang_db::mysql::Database {
        let pool = sqlx::MySqlPool::connect_lazy("mysql://root:test@127.0.0.1:3306/test")
            .expect("惰性 MySQL 连接池应创建成功");
        yang_db::mysql::Database::from_pool(pool, yang_db::mysql::DatabaseConfig::default())
            .expect("默认 MySQL 配置应通过校验")
    }

    #[cfg(feature = "mysql")]
    fn test_mysql_database() -> AnyDatabase {
        AnyDatabase::mysql(test_mysql_pool())
    }

    /// 用惰性连接池构造 PostgreSQL 连接池：不建立真实网络连接。
    #[cfg(feature = "postgres")]
    fn test_postgres_pool() -> yang_db::postgres::Database {
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres:postgres@127.0.0.1:55432/test")
            .expect("惰性 PostgreSQL 连接池应创建成功");
        yang_db::postgres::Database::from_pool(pool, yang_db::postgres::DatabaseConfig::default())
            .expect("默认 PostgreSQL 配置应通过校验")
    }

    #[cfg(feature = "postgres")]
    fn test_postgres_database() -> AnyDatabase {
        AnyDatabase::postgres(test_postgres_pool())
    }

    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn mysql_handler_reports_backend_and_pool_status() {
        let tools = ToolsBuilder::new()
            .mysql(test_mysql_pool())
            .build()
            .expect("配置 MySQL 后应构建成功");

        assert_eq!(
            tools.database().expect("数据库应存在").backend(),
            yang_db::BackendKind::MySql
        );
        assert!(tools.mysql().is_ok());
        assert!(tools.pool_status().is_some());
        assert!(tools.database().expect("数据库应存在").as_mysql().is_some());
    }

    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_handler_reports_backend_and_pool_status() {
        let tools = ToolsBuilder::new()
            .database(test_postgres_database())
            .build()
            .expect("配置 PostgreSQL 后应构建成功");

        assert_eq!(
            tools.database().expect("数据库应存在").backend(),
            yang_db::BackendKind::PostgreSql
        );
        assert!(tools.pool_status().is_some());
        assert!(tools
            .database()
            .expect("数据库应存在")
            .as_postgres()
            .is_some());
    }

    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn database_slot_errors_after_close() {
        let tools = ToolsBuilder::new()
            .database(test_mysql_database())
            .build()
            .expect("配置数据库后应构建成功");

        tools.close().await;

        assert!(matches!(
            tools.database(),
            Err(BaseError::ConfigError(message)) if message == "Tools 已关闭"
        ));
    }

    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn duplicate_database_registration_is_rejected_at_build_time() {
        let result = ToolsBuilder::new()
            .database(test_mysql_database())
            .database(test_mysql_database())
            .build();

        assert!(
            matches!(result, Err(BaseError::ConfigError(message)) if message.contains("重复注册"))
        );
    }

    #[cfg(all(feature = "mysql", feature = "postgres"))]
    #[tokio::test]
    async fn mysql_accessor_rejects_postgres_backend() {
        let tools = ToolsBuilder::new()
            .database(test_postgres_database())
            .build()
            .expect("配置 PostgreSQL 后应构建成功");

        assert_eq!(
            tools.database().expect("数据库应存在").backend(),
            yang_db::BackendKind::PostgreSql
        );
        assert!(matches!(
            tools.mysql(),
            Err(BaseError::DatabaseNotInitialized)
        ));
        assert!(tools
            .database()
            .expect("数据库应存在")
            .as_postgres()
            .is_some());
    }
}
