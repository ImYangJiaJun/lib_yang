use crate::redis::backend::{reconnect_delay, should_reconnect, RedisBackend, RedisConn};
use crate::redis::namespace::KeyNamespace;
use crate::redis::{RedisPipeline, RedisTransaction};
use crate::{
    BackendCapabilities, DbError, PoolStatus, RedisConfig, RedisValue, Result, REDIS_CAPABILITIES,
};
use deadpool_redis::{Config, Pool, Runtime};
use std::time::{Duration, Instant};

/// Redis 拓扑。
///
/// 通过 [`RedisClient::connect_topology`] 选择 standalone / cluster / sentinel。
#[derive(Clone)]
#[non_exhaustive]
pub enum RedisTopology {
    /// 单机（含 TLS）
    Standalone {
        /// 连接 URL
        url: String,
    },
    /// Redis Cluster
    #[cfg(feature = "redis-cluster")]
    Cluster {
        /// 初始节点 URL 列表
        urls: Vec<String>,
    },
    /// Redis Sentinel
    #[cfg(feature = "redis-sentinel")]
    Sentinel {
        /// 哨兵 URL 列表
        sentinel_urls: Vec<String>,
        /// master 服务名
        master_name: String,
    },
}

impl std::fmt::Debug for RedisTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedisTopology::Standalone { url } => f
                .debug_struct("Standalone")
                .field("url", &RedisConfig::redacted_target(url))
                .finish(),
            #[cfg(feature = "redis-cluster")]
            RedisTopology::Cluster { urls } => f
                .debug_struct("Cluster")
                .field("nodes", &urls.len())
                .finish(),
            #[cfg(feature = "redis-sentinel")]
            RedisTopology::Sentinel {
                sentinel_urls,
                master_name,
            } => f
                .debug_struct("Sentinel")
                .field("sentinels", &sentinel_urls.len())
                .field("master_name", master_name)
                .finish(),
        }
    }
}

impl std::fmt::Debug for RedisClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisClient")
            .field("target", &self.redacted_target)
            .field("config", &self.config)
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

/// Redis 健康快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisHealth {
    /// PING 是否成功
    pub ping: bool,
    /// 节点角色（master/replica/sentinel）
    pub role: Option<String>,
    /// 已连接客户端数
    pub connected_clients: Option<u64>,
    /// 已用内存（字节）
    pub used_memory: Option<u64>,
    /// 复制状态摘要
    pub replication: Option<String>,
}

/// Redis 客户端
///
/// 提供 Redis 数据库操作的统一接口，支持连接池、命令级超时、连接断开重试，
/// 以及 standalone / cluster / sentinel 三种拓扑。
#[derive(Clone)]
pub struct RedisClient {
    /// 拓扑后端
    backend: RedisBackend,
    /// 配置
    config: RedisConfig,
    /// 直连客户端（仅 standalone；Pub/Sub 需要独立连接）
    direct: Option<redis::Client>,
    /// 日志用的脱敏目标
    redacted_target: String,
    /// 可选的 key 命名空间（类型化 API 使用）
    namespace: Option<KeyNamespace>,
}

impl RedisClient {
    /// 返回 Redis 后端的静态能力契约。
    pub const fn capabilities() -> &'static BackendCapabilities {
        &REDIS_CAPABILITIES
    }

    /// 连接到 Redis 服务器
    ///
    /// # 参数
    /// - `url`: Redis 连接 URL，格式为 `redis://host:port` 或 `redis://host:port/db`
    ///
    /// # 返回
    /// - `Ok(RedisClient)`: 连接成功
    /// - `Err(DbError)`: 连接失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let config = RedisConfig::default();
        Self::connect_with_config(url, config).await
    }

    /// 使用自定义配置连接到 Redis 服务器
    ///
    /// # 参数
    /// - `url`: Redis 连接 URL
    /// - `config`: Redis 配置
    ///
    /// # 返回
    /// - `Ok(RedisClient)`: 连接成功
    /// - `Err(DbError)`: 连接失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::{RedisClient, RedisConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RedisConfig::default()
    ///         .with_max_connections(20)
    ///         .with_connect_timeout(10)
    ///         .with_wait_timeout(15)
    ///         .with_enable_logging(true);
    ///     let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_with_config(url: impl Into<String>, config: RedisConfig) -> Result<Self> {
        config.validate()?;
        let url_str = url.into();
        RedisConfig::validate_url(&url_str)?;

        // 使用 from_url 创建配置，然后设置连接池参数
        let mut cfg = Config::from_url(url_str.clone());
        cfg.pool = Some(config.build_pool_config());

        // 创建连接池
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| DbError::RedisConnectionError(format!("创建连接池失败: {}", e)))?;

        let client = Self {
            backend: RedisBackend::Single(pool),
            config,
            direct: redis::Client::open(url_str.as_str()).ok(),
            redacted_target: RedisConfig::redacted_target(&url_str),
            namespace: None,
        };
        client.verify_connection().await?;
        client.log_connected();
        Ok(client)
    }

    /// 按拓扑连接 Redis。
    ///
    /// Cluster / Sentinel 需要在 `redis-cluster` / `redis-sentinel` feature 下编译。
    pub async fn connect_topology(topology: RedisTopology) -> Result<Self> {
        Self::connect_topology_with_config(topology, RedisConfig::default()).await
    }

    /// 按拓扑连接 Redis，并应用自定义配置。
    pub async fn connect_topology_with_config(
        topology: RedisTopology,
        config: RedisConfig,
    ) -> Result<Self> {
        config.validate()?;
        match topology {
            RedisTopology::Standalone { url } => Self::connect_with_config(url, config).await,
            #[cfg(feature = "redis-cluster")]
            RedisTopology::Cluster { urls } => {
                if urls.is_empty() {
                    return Err(DbError::InvalidArgument(
                        "Redis Cluster 至少需要一个节点 URL".to_string(),
                    ));
                }
                let mut cfg = deadpool_redis::cluster::Config::from_urls(urls.clone());
                cfg.pool = Some(config.build_pool_config());
                let pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
                    DbError::RedisConnectionError(format!("创建 Cluster 连接池失败: {}", e))
                })?;
                let client = Self {
                    backend: RedisBackend::Cluster(pool),
                    config,
                    direct: None,
                    redacted_target: format!("redis-cluster://{} 个节点", urls.len()),
                    namespace: None,
                };
                client.verify_connection().await?;
                client.log_connected();
                Ok(client)
            }
            #[cfg(feature = "redis-sentinel")]
            RedisTopology::Sentinel {
                sentinel_urls,
                master_name,
            } => {
                if sentinel_urls.is_empty() {
                    return Err(DbError::InvalidArgument(
                        "Redis Sentinel 至少需要一个哨兵 URL".to_string(),
                    ));
                }
                let mut cfg = deadpool_redis::sentinel::Config::from_urls(
                    sentinel_urls.clone(),
                    master_name.clone(),
                    deadpool_redis::sentinel::SentinelServerType::Master,
                );
                cfg.pool = Some(config.build_pool_config());
                let pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
                    DbError::RedisConnectionError(format!("创建 Sentinel 连接池失败: {}", e))
                })?;
                let client = Self {
                    backend: RedisBackend::Sentinel(pool),
                    config,
                    direct: None,
                    redacted_target: format!(
                        "redis-sentinel://{} 个哨兵 (master={})",
                        sentinel_urls.len(),
                        master_name
                    ),
                    namespace: None,
                };
                client.verify_connection().await?;
                client.log_connected();
                Ok(client)
            }
        }
    }

    /// 当前是否为 Redis Cluster 拓扑。
    pub fn is_cluster(&self) -> bool {
        #[cfg(feature = "redis-cluster")]
        {
            matches!(&self.backend, RedisBackend::Cluster(_))
        }
        #[cfg(not(feature = "redis-cluster"))]
        {
            false
        }
    }

    /// 从当前拓扑池中借出一个连接，并应用命令级响应超时。
    pub(crate) async fn connection(&self) -> Result<RedisConn> {
        let mut conn = self.backend.get().await?;
        if let Some(millis) = self.config.response_timeout_ms {
            conn.set_response_timeout(Duration::from_millis(millis));
        }
        Ok(conn)
    }

    /// 当前配置。
    pub(crate) fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// 直连客户端（Pub/Sub 使用）；非 standalone 拓扑返回错误。
    pub(crate) fn direct_client(&self) -> Result<&redis::Client> {
        self.direct.as_ref().ok_or_else(|| {
            DbError::RedisConnectionError("Pub/Sub 仅支持 standalone 拓扑".to_string())
        })
    }

    /// 设置 key 命名空间（仅影响类型化 API）。
    pub fn with_namespace(mut self, namespace: KeyNamespace) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// 当前 key 命名空间。
    pub fn namespace(&self) -> Option<&KeyNamespace> {
        self.namespace.as_ref()
    }

    /// 返回应用命名空间后的 key（未配置命名空间时原样返回）。
    pub fn key_for(&self, key: impl AsRef<[u8]>) -> Vec<u8> {
        match &self.namespace {
            Some(namespace) => namespace.key(key),
            None => key.as_ref().to_vec(),
        }
    }

    /// PING 校验连接。
    async fn verify_connection(&self) -> Result<()> {
        let mut conn = self
            .connection()
            .await
            .map_err(|e| DbError::RedisConnectionError(format!("获取连接失败: {}", e)))?;
        conn.query::<String>(&redis::cmd("PING"))
            .await
            .map_err(|e| DbError::RedisConnectionError(format!("连接测试失败: {}", e)))?;
        Ok(())
    }

    /// 连接成功日志（只输出脱敏目标，绝不输出 userinfo）。
    fn log_connected(&self) {
        if self.config.enable_logging {
            log::info!("Redis 连接成功: {}", self.redacted_target);
        }
    }

    /// 脱敏后的连接目标（可用于日志/健康输出）。
    pub fn redacted_target(&self) -> &str {
        &self.redacted_target
    }

    /// 获取 standalone 连接池引用。
    ///
    /// cluster / sentinel 拓扑返回 `None`；需要统一操作时请使用客户端方法。
    pub fn pool(&self) -> Option<&Pool> {
        match &self.backend {
            RedisBackend::Single(pool) => Some(pool),
            #[cfg(feature = "redis-cluster")]
            RedisBackend::Cluster(_) => None,
            #[cfg(feature = "redis-sentinel")]
            RedisBackend::Sentinel(_) => None,
        }
    }

    /// 优雅关闭连接池（I3）：关闭后无法再借出连接，已借出的连接归还时被丢弃。
    ///
    /// 与 `Database::close` 对称，供编排式停机调用。幂等；close 后再用会返回连接池
    /// 错误而非 panic（deadpool `Pool::get` 在关闭后返回错误）。
    pub async fn close(&self) {
        self.backend.close();
    }

    /// 关闭连接池并等待借出连接归还，最多等待 `deadline`。
    ///
    /// 阻塞命令（BLPOP/BRPOP/XREAD BLOCK）可能被该等待截断；调用方应同时配置
    /// 命令级响应超时或使用有限阻塞时间。
    pub async fn close_and_wait(&self, deadline: Duration) {
        let started = Instant::now();
        self.backend.close();
        loop {
            let status = self.backend.status();
            if status.size == 0 {
                return;
            }
            if started.elapsed() >= deadline {
                log::warn!("Redis 关闭等待超时，仍有 {} 个借出连接未归还", status.size);
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// 连接池是否已关闭（I3）。
    pub fn is_closed(&self) -> bool {
        self.backend.is_closed()
    }

    /// 创建 Pipeline 批量操作
    ///
    /// Pipeline 允许将多个命令打包发送到 Redis 服务器，减少网络往返次数，提高性能。
    ///
    /// # 返回
    /// 新的 Pipeline 实例
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     let mut pipeline = client.pipeline();
    ///     pipeline.set("key1", "value1")
    ///             .set("key2", "value2")
    ///             .get("key1")
    ///             .incr("counter");
    ///
    ///     let results = pipeline.execute().await?;
    ///     println!("Pipeline 执行结果: {:?}", results);
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub fn pipeline(&self) -> RedisPipeline {
        RedisPipeline::new(self.clone())
    }

    /// 创建 Redis 事务
    ///
    /// 使用 WATCH/MULTI/EXEC 机制实现乐观锁事务。事务会自动处理 WATCH 冲突并重试。
    ///
    /// # 返回
    /// 新的事务实例
    ///
    /// # 示例：基础事务
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     let mut tx = client.transaction();
    ///     tx.set("key1", "value1")
    ///       .set("key2", "value2")
    ///       .incr("counter");
    ///     
    ///     let results: (String, String, i64) = tx.exec().await?;
    ///     println!("事务执行结果: {:?}", results);
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 示例：乐观锁实现余额扣减
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     // 初始化余额
    ///     client.set("balance", "1000").await?;
    ///     
    ///     // 读取当前余额
    ///     let balance_str = client.get("balance").await?.unwrap();
    ///     let balance: i64 = balance_str.parse().unwrap();
    ///     
    ///     // 使用事务扣减余额
    ///     if balance >= 100 {
    ///         let mut tx = client.transaction();
    ///         tx.watch(&["balance".to_string()]);
    ///         tx.set("balance", (balance - 100).to_string());
    ///         
    ///         let result: (String,) = tx.exec().await?;
    ///         println!("余额扣减成功: {:?}", result);
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub fn transaction(&self) -> RedisTransaction {
        RedisTransaction::new(self.clone())
    }

    /// 执行 Redis 命令
    ///
    /// # 参数
    /// - `cmd`: Redis 命令
    ///
    /// # 返回
    /// - `Ok(RedisValue)`: 命令执行成功
    /// - `Err(DbError)`: 命令执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    /// use redis::cmd;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     let mut cmd = cmd("SET");
    ///     cmd.arg("key").arg("value");
    ///     let result = client.execute(&cmd).await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "redis", db.operation = "command", otel.kind = "client")
    )]
    pub async fn execute(&self, cmd: &redis::Cmd) -> Result<RedisValue> {
        let started = Instant::now();
        let operation = command_name(cmd);
        let result = self.execute_with_retry(cmd).await;
        record_metrics(operation, started, &result);
        result
    }

    /// 带连接级重试的命令执行。
    ///
    /// 只对连接已不可用类错误重新取连接重试；超时不重试，避免非幂等命令被
    /// 重复执行。重试语义为 at-least-once，调用方需自行保证幂等。
    async fn query_with_retry<T>(&self, cmd: &redis::Cmd) -> Result<T>
    where
        T: redis::FromRedisValue,
    {
        let mut attempt = 0u32;
        loop {
            let mut conn = self.connection().await?;
            match conn.query::<T>(cmd).await {
                Ok(value) => return Ok(value),
                Err(error)
                    if attempt < self.config.reconnect_retries && should_reconnect(&error) =>
                {
                    attempt += 1;
                    let delay = reconnect_delay(attempt, self.config.reconnect_max_delay_ms);
                    log::warn!(
                        "Redis 命令连接失败，第 {}/{} 次重试: {}",
                        attempt,
                        self.config.reconnect_retries,
                        error
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(DbError::from(error)),
            }
        }
    }

    async fn execute_with_retry(&self, cmd: &redis::Cmd) -> Result<RedisValue> {
        self.query_with_retry::<redis::Value>(cmd)
            .await
            .map(RedisValue::from)
    }

    /// 执行命令并解码为调用方指定的类型。
    ///
    /// 与 [`execute`](Self::execute) 共享超时、重连与指标逻辑；适用于需要
    /// `FromRedisValue` 结构化回复的命令（如 Streams）。
    pub async fn execute_typed<T>(&self, cmd: &redis::Cmd) -> Result<T>
    where
        T: redis::FromRedisValue,
    {
        let started = Instant::now();
        let operation = command_name(cmd);
        let result = self.query_with_retry::<T>(cmd).await;
        record_operation_metrics(operation, started, result.is_ok());
        result
    }

    // ==================== String 操作 ====================

    /// SET - 设置键值
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 值
    ///
    /// # 返回
    /// - `Ok(())`: 设置成功
    /// - `Err(DbError)`: 设置失败
    pub async fn set(&self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.into()).arg(value.into());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// GET - 获取键的值
    ///
    /// # 参数
    /// - `key`: 键
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 键存在，返回值
    /// - `Ok(None)`: 键不存在
    /// - `Err(DbError)`: 获取失败
    pub async fn get(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("GET");
        cmd.arg(key.into());
        let value = self.execute(&cmd).await?;
        optional_string(&value)
    }

    /// SETEX - 设置键值并指定过期时间（秒）
    ///
    /// # 参数
    /// - `key`: 键
    /// - `seconds`: 过期时间（秒）
    /// - `value`: 值
    pub async fn setex(
        &self,
        key: impl Into<String>,
        seconds: i64,
        value: impl Into<String>,
    ) -> Result<()> {
        let mut cmd = redis::cmd("SETEX");
        cmd.arg(key.into()).arg(seconds).arg(value.into());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// SETNX - 仅当键不存在时设置值
    ///
    /// # 返回
    /// - `Ok(true)`: 设置成功
    /// - `Ok(false)`: 键已存在，未设置
    pub async fn setnx(&self, key: impl Into<String>, value: impl Into<String>) -> Result<bool> {
        let mut cmd = redis::cmd("SETNX");
        cmd.arg(key.into()).arg(value.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// SET NX EX - 仅当键不存在时设置值并指定过期时间（原子操作）
    ///
    /// 组合 NX（不存在时设置）与 EX（过期时间秒）选项。
    /// 比 `setnx` 后跟 `expire` 更安全，两阶段之间有竞态窗口。
    ///
    /// # 参数
    /// - `key`: 键名
    /// - `value`: 值
    /// - `ttl`: 过期时间（秒），必须为正数
    ///
    /// # 返回
    /// - `Ok(true)`: 设置成功（键之前不存在）
    /// - `Ok(false)`: 键已存在，未设置
    pub async fn set_nx_ex(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
        ttl: i64,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.into())
            .arg(value.into())
            .arg("NX")
            .arg("EX")
            .arg(ttl);
        let result = self.execute(&cmd).await?;
        // SET NX EX 成功返回 OK（redis::Value::Okay），键已存在返回 nil
        Ok(!result.is_nil())
    }

    /// GETSET - 设置新值并返回旧值
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回旧值
    /// - `Ok(None)`: 键不存在
    pub async fn getset(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("GETSET");
        cmd.arg(key.into()).arg(value.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// MGET - 批量获取多个键的值
    ///
    /// # 返回
    /// 返回值数组，不存在的键返回 None
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut cmd = redis::cmd("MGET");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        let arr = result
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("MGET 结果不是数组".to_string()))?;
        arr.iter()
            .map(|value| match value {
                RedisValue::Nil => Ok(None),
                _ => value.as_string().map(Some).ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "MGET 值不是 UTF-8 字符串，请使用 mget_bytes".to_string(),
                    )
                }),
            })
            .collect()
    }

    /// MSET - 批量设置多个键值对
    ///
    /// # 参数
    /// - `pairs`: 键值对数组
    pub async fn mset(&self, pairs: &[(String, String)]) -> Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }

        let mut cmd = redis::cmd("MSET");
        for (key, value) in pairs {
            cmd.arg(key).arg(value);
        }
        self.execute(&cmd).await?;
        Ok(())
    }

    /// INCR - 将键的值增加 1
    ///
    /// # 返回
    /// 返回增加后的值
    pub async fn incr(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("INCR");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// INCRBY - 将键的值增加指定数量
    ///
    /// # 返回
    /// 返回增加后的值
    pub async fn incrby(&self, key: impl Into<String>, increment: i64) -> Result<i64> {
        let mut cmd = redis::cmd("INCRBY");
        cmd.arg(key.into()).arg(increment);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// DECR - 将键的值减少 1
    ///
    /// # 返回
    /// 返回减少后的值
    pub async fn decr(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("DECR");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// DECRBY - 将键的值减少指定数量
    ///
    /// # 返回
    /// 返回减少后的值
    pub async fn decrby(&self, key: impl Into<String>, decrement: i64) -> Result<i64> {
        let mut cmd = redis::cmd("DECRBY");
        cmd.arg(key.into()).arg(decrement);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// APPEND - 将值追加到键的原值末尾
    ///
    /// # 返回
    /// 返回追加后字符串的长度
    pub async fn append(&self, key: impl Into<String>, value: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("APPEND");
        cmd.arg(key.into()).arg(value.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// STRLEN - 获取键存储的字符串长度
    ///
    /// # 返回
    /// 返回字符串长度，键不存在返回 0
    pub async fn strlen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("STRLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// GETRANGE - 获取字符串的子串
    ///
    /// # 参数
    /// - `key`: 键
    /// - `start`: 起始偏移量（0 表示第一个字符，-1 表示最后一个字符）
    /// - `end`: 结束偏移量（包含）
    ///
    /// # 返回
    /// - `Ok(String)`: 子串内容
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     // 假设 key "mykey" 的值是 "Hello World"
    ///     client.set("mykey", "Hello World").await?;
    ///     let substr = client.getrange("mykey", 0, 4).await?;  // "Hello"
    ///     let substr2 = client.getrange("mykey", -5, -1).await?; // "World"
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn getrange(&self, key: impl Into<String>, start: i64, end: i64) -> Result<String> {
        let mut cmd = redis::cmd("GETRANGE");
        cmd.arg(key.into()).arg(start).arg(end);
        let result = self.execute(&cmd).await?;
        match result {
            RedisValue::Nil => Ok(String::new()),
            RedisValue::String(value) => Ok(value),
            _ => Err(DbError::RedisTypeConversionError(
                "GETRANGE 返回值不是 UTF-8 字符串，请使用 execute 原始命令".to_string(),
            )),
        }
    }

    /// SETRANGE - 从指定偏移量开始替换字符串内容
    ///
    /// # 参数
    /// - `key`: 键
    /// - `offset`: 起始偏移量
    /// - `value`: 要设置的值
    ///
    /// # 返回
    /// - `Ok(i64)`: 修改后字符串的长度
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     client.set("mykey", "Hello World").await?;
    ///     let len = client.setrange("mykey", 6, "Redis").await?; // "Hello Redis"
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn setrange(
        &self,
        key: impl Into<String>,
        offset: i64,
        value: impl Into<String>,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("SETRANGE");
        cmd.arg(key.into()).arg(offset).arg(value.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// INCRBYFLOAT - 将键的浮点数值增加指定数量
    ///
    /// # 参数
    /// - `key`: 键
    /// - `increment`: 增量（可以是负数）
    ///
    /// # 返回
    /// - `Ok(f64)`: 增加后的值
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     client.set("price", "10.5").await?;
    ///     let new_price = client.incrbyfloat("price", 2.3).await?; // 12.8
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn incrbyfloat(&self, key: impl Into<String>, increment: f64) -> Result<f64> {
        let mut cmd = redis::cmd("INCRBYFLOAT");
        cmd.arg(key.into()).arg(increment);
        let result = self.execute(&cmd).await?;
        // Redis 返回字符串形式的浮点数
        if let Some(s) = result.as_string() {
            s.parse::<f64>()
                .map_err(|_| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        } else {
            result
                .as_f64()
                .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        }
    }

    /// PSETEX - 设置键值并指定毫秒级过期时间
    ///
    /// # 参数
    /// - `key`: 键
    /// - `milliseconds`: 过期时间（毫秒）
    /// - `value`: 值
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     // 设置键值，100 毫秒后过期
    ///     client.psetex("session", 100, "data").await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn psetex(
        &self,
        key: impl Into<String>,
        milliseconds: i64,
        value: impl Into<String>,
    ) -> Result<()> {
        let mut cmd = redis::cmd("PSETEX");
        cmd.arg(key.into()).arg(milliseconds).arg(value.into());
        self.execute(&cmd).await?;
        Ok(())
    }

    // ==================== Hash 操作 ====================

    /// HSET - 设置哈希表字段的值
    ///
    /// # 返回
    /// - `Ok(true)`: 新字段被设置
    /// - `Ok(false)`: 字段已存在，值被更新
    pub async fn hset(
        &self,
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("HSET");
        cmd.arg(key.into()).arg(field.into()).arg(value.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// HGET - 获取哈希表字段的值
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 字段存在
    /// - `Ok(None)`: 字段不存在
    pub async fn hget(
        &self,
        key: impl Into<String>,
        field: impl Into<String>,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("HGET");
        cmd.arg(key.into()).arg(field.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// HDEL - 删除哈希表的一个或多个字段
    ///
    /// # 返回
    /// 返回被删除字段的数量
    pub async fn hdel(&self, key: impl Into<String>, fields: &[String]) -> Result<i64> {
        if fields.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("HDEL");
        cmd.arg(key.into());
        for field in fields {
            cmd.arg(field);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// HEXISTS - 检查哈希表字段是否存在
    ///
    /// # 返回
    /// - `Ok(true)`: 字段存在
    /// - `Ok(false)`: 字段不存在
    pub async fn hexists(&self, key: impl Into<String>, field: impl Into<String>) -> Result<bool> {
        let mut cmd = redis::cmd("HEXISTS");
        cmd.arg(key.into()).arg(field.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// HMSET - 批量设置哈希表的多个字段
    pub async fn hmset(&self, key: impl Into<String>, fields: &[(String, String)]) -> Result<()> {
        if fields.is_empty() {
            return Ok(());
        }

        let mut cmd = redis::cmd("HMSET");
        cmd.arg(key.into());
        for (field, value) in fields {
            cmd.arg(field).arg(value);
        }
        self.execute(&cmd).await?;
        Ok(())
    }

    /// HMGET - 批量获取哈希表的多个字段值
    ///
    /// # 返回
    /// 返回值数组，不存在的字段返回 None
    pub async fn hmget(
        &self,
        key: impl Into<String>,
        fields: &[String],
    ) -> Result<Vec<Option<String>>> {
        if fields.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd = redis::cmd("HMGET");
        cmd.arg(key.into());
        for field in fields {
            cmd.arg(field);
        }
        let result = self.execute(&cmd).await?;
        let arr = result
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("HMGET 结果不是数组".to_string()))?;
        arr.iter()
            .map(|value| match value {
                RedisValue::Nil => Ok(None),
                _ => value.as_string().map(Some).ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "HMGET 值不是 UTF-8 字符串，请使用 hmget_bytes".to_string(),
                    )
                }),
            })
            .collect()
    }

    /// HGETALL - 获取哈希表的所有字段和值
    ///
    /// # 返回
    /// 返回字段-值对的向量
    pub async fn hgetall(&self, key: impl Into<String>) -> Result<Vec<(String, String)>> {
        let mut cmd = redis::cmd("HGETALL");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        let arr = result
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("HGETALL 结果不是数组".to_string()))?;
        if arr.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "HGETALL 返回的字段/值数量不是偶数".to_string(),
            ));
        }
        arr.chunks_exact(2)
            .map(|pair| {
                let field = pair[0].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "HGETALL 字段不是 UTF-8 字符串，请使用 hgetall_bytes".to_string(),
                    )
                })?;
                let value = pair[1].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "HGETALL 值不是 UTF-8 字符串，请使用 hgetall_bytes".to_string(),
                    )
                })?;
                Ok((field, value))
            })
            .collect()
    }

    /// HLEN - 获取哈希表的字段数量
    pub async fn hlen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("HLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// HKEYS - 获取哈希表的所有字段名
    pub async fn hkeys(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("HKEYS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// HVALS - 获取哈希表的所有值
    pub async fn hvals(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("HVALS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// HINCRBY - 将哈希表字段的整数值增加指定数量
    pub async fn hincrby(
        &self,
        key: impl Into<String>,
        field: impl Into<String>,
        increment: i64,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("HINCRBY");
        cmd.arg(key.into()).arg(field.into()).arg(increment);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// HINCRBYFLOAT - 将哈希表字段的浮点数值增加指定数量
    pub async fn hincrbyfloat(
        &self,
        key: impl Into<String>,
        field: impl Into<String>,
        increment: f64,
    ) -> Result<f64> {
        let mut cmd = redis::cmd("HINCRBYFLOAT");
        cmd.arg(key.into()).arg(field.into()).arg(increment);
        let result = self.execute(&cmd).await?;
        // Redis 返回字符串形式的浮点数
        if let Some(s) = result.as_string() {
            s.parse::<f64>()
                .map_err(|_| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        } else {
            result
                .as_f64()
                .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        }
    }

    // ==================== List 操作 ====================

    /// LPUSH - 将一个或多个值插入列表头部
    ///
    /// # 返回
    /// 返回插入后列表的长度
    pub async fn lpush(&self, key: impl Into<String>, values: &[String]) -> Result<i64> {
        if values.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("LPUSH");
        cmd.arg(key.into());
        for value in values {
            cmd.arg(value);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// RPUSH - 将一个或多个值插入列表尾部
    ///
    /// # 返回
    /// 返回插入后列表的长度
    pub async fn rpush(&self, key: impl Into<String>, values: &[String]) -> Result<i64> {
        if values.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("RPUSH");
        cmd.arg(key.into());
        for value in values {
            cmd.arg(value);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// LPOP - 移除并返回列表的头元素
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回头元素
    /// - `Ok(None)`: 列表为空
    pub async fn lpop(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("LPOP");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// RPOP - 移除并返回列表的尾元素
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回尾元素
    /// - `Ok(None)`: 列表为空
    pub async fn rpop(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("RPOP");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// LRANGE - 获取列表指定范围内的元素
    ///
    /// # 参数
    /// - `start`: 起始索引（0 表示第一个元素）
    /// - `stop`: 结束索引（-1 表示最后一个元素）
    pub async fn lrange(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("LRANGE");
        cmd.arg(key.into()).arg(start).arg(stop);
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// LLEN - 获取列表长度
    pub async fn llen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("LLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// LINDEX - 获取列表指定索引的元素
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回元素
    /// - `Ok(None)`: 索引超出范围
    pub async fn lindex(&self, key: impl Into<String>, index: i64) -> Result<Option<String>> {
        let mut cmd = redis::cmd("LINDEX");
        cmd.arg(key.into()).arg(index);
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// LSET - 设置列表指定索引的元素值
    pub async fn lset(
        &self,
        key: impl Into<String>,
        index: i64,
        value: impl Into<String>,
    ) -> Result<()> {
        let mut cmd = redis::cmd("LSET");
        cmd.arg(key.into()).arg(index).arg(value.into());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// LTRIM - 修剪列表，仅保留指定范围内的元素
    pub async fn ltrim(&self, key: impl Into<String>, start: i64, stop: i64) -> Result<()> {
        let mut cmd = redis::cmd("LTRIM");
        cmd.arg(key.into()).arg(start).arg(stop);
        self.execute(&cmd).await?;
        Ok(())
    }

    /// LINSERT - 在列表的指定元素前或后插入新元素
    ///
    /// # 参数
    /// - `key`: 键
    /// - `before_after`: "BEFORE" 或 "AFTER"
    /// - `pivot`: 参考元素
    /// - `value`: 要插入的值
    ///
    /// # 返回
    /// - `Ok(i64)`: 插入后列表的长度，-1 表示 pivot 不存在
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     client.rpush("mylist", &["a".to_string(), "c".to_string()]).await?;
    ///     client.linsert("mylist", "BEFORE", "c", "b").await?; // ["a", "b", "c"]
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn linsert(
        &self,
        key: impl Into<String>,
        before_after: &str,
        pivot: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("LINSERT");
        cmd.arg(key.into())
            .arg(before_after)
            .arg(pivot.into())
            .arg(value.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// LREM - 删除列表中的指定元素
    ///
    /// # 参数
    /// - `key`: 键
    /// - `count`: 删除数量
    ///   - count > 0: 从头到尾删除 count 个匹配元素
    ///   - count < 0: 从尾到头删除 |count| 个匹配元素
    ///   - count = 0: 删除所有匹配元素
    /// - `value`: 要删除的值
    ///
    /// # 返回
    /// - `Ok(i64)`: 被删除元素的数量
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     client.rpush("mylist", &["a".to_string(), "b".to_string(), "a".to_string()]).await?;
    ///     let removed = client.lrem("mylist", 2, "a").await?; // 删除 2 个 "a"
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn lrem(
        &self,
        key: impl Into<String>,
        count: i64,
        value: impl Into<String>,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("LREM");
        cmd.arg(key.into()).arg(count).arg(value.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// RPOPLPUSH - 从源列表尾部弹出元素并插入到目标列表头部
    ///
    /// # 参数
    /// - `source`: 源列表键
    /// - `destination`: 目标列表键
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 被移动的元素
    /// - `Ok(None)`: 源列表为空
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     client.rpush("list1", &["a".to_string(), "b".to_string()]).await?;
    ///     let elem = client.rpoplpush("list1", "list2").await?; // "b"
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn rpoplpush(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("RPOPLPUSH");
        cmd.arg(source.into()).arg(destination.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// BLPOP - 阻塞式地从列表头部弹出元素
    ///
    /// # 参数
    /// - `keys`: 键列表（按顺序检查）
    /// - `timeout`: 超时时间（秒），0 表示无限等待
    ///
    /// # 返回
    /// - `Ok(Some((String, String)))`: (键名, 元素值)
    /// - `Ok(None)`: 超时
    ///
    /// # 连接池占用
    /// 阻塞期间会一直占用一个池连接；`timeout=0` 表示无限等待。消费者数量应
    /// 小于 `max_connections`，或为阻塞消费者使用独立 `RedisClient`/专用池。
    /// 配置 `response_timeout_ms` 会在客户端侧截断阻塞等待。
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///
    ///     let result = client.blpop(&["queue1".to_string(), "queue2".to_string()], 5).await?;
    ///     if let Some((key, value)) = result {
    ///         println!("从 {} 弹出: {}", key, value);
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn blpop(&self, keys: &[String], timeout: i64) -> Result<Option<(String, String)>> {
        let mut cmd = redis::cmd("BLPOP");
        for key in keys {
            cmd.arg(key);
        }
        cmd.arg(timeout);
        let result = self.execute(&cmd).await?;

        // BLPOP 返回数组 [key, value] 或 nil
        if let Some(arr) = result.as_array() {
            if arr.len() == 2 {
                let key = arr[0].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "阻塞弹出返回的键不是 UTF-8 字符串".to_string(),
                    )
                })?;
                let value = arr[1].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "阻塞弹出返回值不是 UTF-8 字符串，请使用原始命令/bytes API".to_string(),
                    )
                })?;
                return Ok(Some((key, value)));
            }
        }
        Ok(None)
    }

    /// BRPOP - 阻塞式地从列表尾部弹出元素
    ///
    /// # 参数
    /// - `keys`: 键列表（按顺序检查）
    /// - `timeout`: 超时时间（秒），0 表示无限等待
    ///
    /// # 返回
    /// - `Ok(Some((String, String)))`: (键名, 元素值)
    /// - `Ok(None)`: 超时
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     let result = client.brpop(&["queue1".to_string()], 10).await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn brpop(&self, keys: &[String], timeout: i64) -> Result<Option<(String, String)>> {
        let mut cmd = redis::cmd("BRPOP");
        for key in keys {
            cmd.arg(key);
        }
        cmd.arg(timeout);
        let result = self.execute(&cmd).await?;

        // BRPOP 返回数组 [key, value] 或 nil
        if let Some(arr) = result.as_array() {
            if arr.len() == 2 {
                let key = arr[0].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "阻塞弹出返回的键不是 UTF-8 字符串".to_string(),
                    )
                })?;
                let value = arr[1].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError(
                        "阻塞弹出返回值不是 UTF-8 字符串，请使用原始命令/bytes API".to_string(),
                    )
                })?;
                return Ok(Some((key, value)));
            }
        }
        Ok(None)
    }

    // ==================== Set 操作 ====================

    /// SADD - 向集合添加一个或多个成员
    ///
    /// # 返回
    /// 返回被添加到集合中的新元素数量
    pub async fn sadd(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("SADD");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// SREM - 移除集合中的一个或多个成员
    ///
    /// # 返回
    /// 返回被移除的元素数量
    pub async fn srem(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("SREM");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// SMEMBERS - 获取集合的所有成员
    pub async fn smembers(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("SMEMBERS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// SISMEMBER - 检查元素是否是集合的成员
    ///
    /// # 返回
    /// - `Ok(true)`: 元素是集合成员
    /// - `Ok(false)`: 元素不是集合成员
    pub async fn sismember(
        &self,
        key: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("SISMEMBER");
        cmd.arg(key.into()).arg(member.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// SCARD - 获取集合的成员数量
    pub async fn scard(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("SCARD");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// SPOP - 移除并返回集合中的一个随机元素
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回被移除的元素
    /// - `Ok(None)`: 集合为空
    pub async fn spop(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("SPOP");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// SRANDMEMBER - 返回集合中的一个随机元素（不移除）
    ///
    /// # 返回
    /// - `Ok(Some(String))`: 返回随机元素
    /// - `Ok(None)`: 集合为空
    pub async fn srandmember(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("SRANDMEMBER");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        optional_string(&result)
    }

    /// SINTER - 返回多个集合的交集
    ///
    /// 计算所有给定集合的共同成员。
    ///
    /// # 参数
    /// - `keys`: 参与交集运算的集合键列表
    ///
    /// # 返回
    /// 所有集合中都存在的成员列表
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let common = client.sinter(&["set_a".to_string(), "set_b".to_string()]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sinter(&self, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut cmd = redis::cmd("SINTER");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// SUNION - 返回多个集合的并集
    ///
    /// 计算所有给定集合的所有成员。
    ///
    /// # 参数
    /// - `keys`: 参与并集运算的集合键列表
    ///
    /// # 返回
    /// 所有集合的成员合并列表（去重）
    pub async fn sunion(&self, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut cmd = redis::cmd("SUNION");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// SDIFF - 返回多个集合的差集
    ///
    /// 计算第一个集合与其余集合的差集（仅存在于第一个集合中的成员）。
    ///
    /// # 参数
    /// - `keys`: 集合键列表，第一个为基准集合
    ///
    /// # 返回
    /// 仅存在于第一个集合中的成员列表
    pub async fn sdiff(&self, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut cmd = redis::cmd("SDIFF");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// SMOVE - 将指定成员从源集合移动到目标集合
    ///
    /// # 参数
    /// - `source`: 源集合键
    /// - `destination`: 目标集合键
    /// - `member`: 要移动的成员
    ///
    /// # 返回
    /// - `Ok(true)`: 移动成功
    /// - `Ok(false)`: 成员不存在于源集合
    pub async fn smove(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("SMOVE");
        cmd.arg(source.into())
            .arg(destination.into())
            .arg(member.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// SSCAN - 增量式迭代集合中的元素
    ///
    /// # 参数
    /// - `key`: 集合键
    /// - `cursor`: 游标（初始传 0）
    /// - `pattern`: 可选的匹配模式
    /// - `count`: 可选的每批返回数量提示
    ///
    /// # 返回
    /// - `Ok((u64, Vec<String>))`: (下一游标, 本批成员列表)
    pub async fn sscan(
        &self,
        key: impl Into<String>,
        cursor: u64,
        pattern: Option<&str>,
        count: Option<i64>,
    ) -> Result<(u64, Vec<String>)> {
        let mut cmd = redis::cmd("SSCAN");
        cmd.arg(key.into()).arg(cursor);
        if let Some(p) = pattern {
            cmd.arg("MATCH").arg(p);
        }
        if let Some(c) = count {
            cmd.arg("COUNT").arg(c);
        }
        let result = self.execute(&cmd).await?;
        parse_scan_result(&result)
    }

    /// HSCAN - 增量式迭代哈希表的字段与值
    ///
    /// # 参数
    /// - `key`: 哈希表键
    /// - `cursor`: 游标（初始传 0）
    /// - `pattern`: 可选的匹配模式
    /// - `count`: 可选的每批返回数量提示
    ///
    /// # 返回
    /// - `Ok((u64, Vec<(String, String)>))`: (下一游标, 字段-值对列表)
    pub async fn hscan(
        &self,
        key: impl Into<String>,
        cursor: u64,
        pattern: Option<&str>,
        count: Option<i64>,
    ) -> Result<(u64, Vec<(String, String)>)> {
        let mut cmd = redis::cmd("HSCAN");
        cmd.arg(key.into()).arg(cursor);
        if let Some(p) = pattern {
            cmd.arg("MATCH").arg(p);
        }
        if let Some(c) = count {
            cmd.arg("COUNT").arg(c);
        }
        let result = self.execute(&cmd).await?;
        let (next_cursor, fields) = parse_scan_result(&result)?;
        if fields.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "HSCAN 返回的字段/值数量不是偶数".to_string(),
            ));
        }
        let pairs = fields
            .chunks_exact(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect();
        Ok((next_cursor, pairs))
    }

    // ==================== Sorted Set 操作 ====================

    /// ZADD - 向有序集合添加一个或多个成员
    ///
    /// # 返回
    /// 返回被添加的新成员数量
    pub async fn zadd(&self, key: impl Into<String>, members: &[(f64, String)]) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("ZADD");
        cmd.arg(key.into());
        for (score, member) in members {
            cmd.arg(score).arg(member);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZREM - 移除有序集合中的一个或多个成员
    ///
    /// # 返回
    /// 返回被移除的成员数量
    pub async fn zrem(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("ZREM");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZSCORE - 获取成员的分数
    ///
    /// # 返回
    /// - `Ok(Some(f64))`: 返回分数
    /// - `Ok(None)`: 成员不存在
    pub async fn zscore(
        &self,
        key: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<Option<f64>> {
        let mut cmd = redis::cmd("ZSCORE");
        cmd.arg(key.into()).arg(member.into());
        let result = self.execute(&cmd).await?;
        if let Some(s) = result.as_string() {
            Ok(s.parse::<f64>().ok())
        } else {
            Ok(result.as_f64())
        }
    }

    /// ZCARD - 获取有序集合的成员数量
    pub async fn zcard(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("ZCARD");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZRANGE - 按索引范围获取有序集合的成员
    ///
    /// # 参数
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    pub async fn zrange(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("ZRANGE");
        cmd.arg(key.into()).arg(start).arg(stop);
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// ZRANGEBYSCORE - 按分数范围获取有序集合的成员
    ///
    /// # 参数
    /// - `min`: 最小分数
    /// - `max`: 最大分数
    pub async fn zrangebyscore(
        &self,
        key: impl Into<String>,
        min: f64,
        max: f64,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("ZRANGEBYSCORE");
        cmd.arg(key.into()).arg(min).arg(max);
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// ZCOUNT - 计算分数范围内的成员数量
    pub async fn zcount(&self, key: impl Into<String>, min: f64, max: f64) -> Result<i64> {
        let mut cmd = redis::cmd("ZCOUNT");
        cmd.arg(key.into()).arg(min).arg(max);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZINCRBY - 将成员的分数增加指定数量
    ///
    /// # 返回
    /// 返回增加后的分数
    pub async fn zincrby(
        &self,
        key: impl Into<String>,
        increment: f64,
        member: impl Into<String>,
    ) -> Result<f64> {
        let mut cmd = redis::cmd("ZINCRBY");
        cmd.arg(key.into()).arg(increment).arg(member.into());
        let result = self.execute(&cmd).await?;
        if let Some(s) = result.as_string() {
            s.parse::<f64>()
                .map_err(|_| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        } else {
            result
                .as_f64()
                .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为浮点数".to_string()))
        }
    }

    /// ZRANK - 获取成员在有序集合中的排名（从低到高，0 为第一名）
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `member`: 成员
    ///
    /// # 返回
    /// - `Ok(Some(i64))`: 成员的排名（0 开始）
    /// - `Ok(None)`: 成员不存在
    pub async fn zrank(
        &self,
        key: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<Option<i64>> {
        let mut cmd = redis::cmd("ZRANK");
        cmd.arg(key.into()).arg(member.into());
        let result = self.execute(&cmd).await?;
        if result.is_nil() {
            Ok(None)
        } else {
            Ok(result.as_i64())
        }
    }

    /// ZREVRANK - 获取成员在有序集合中的逆序排名（从高到低，0 为第一名）
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `member`: 成员
    ///
    /// # 返回
    /// - `Ok(Some(i64))`: 成员的逆序排名（0 开始）
    /// - `Ok(None)`: 成员不存在
    pub async fn zrevrank(
        &self,
        key: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<Option<i64>> {
        let mut cmd = redis::cmd("ZREVRANK");
        cmd.arg(key.into()).arg(member.into());
        let result = self.execute(&cmd).await?;
        if result.is_nil() {
            Ok(None)
        } else {
            Ok(result.as_i64())
        }
    }

    /// ZREVRANGE - 按索引范围从高到低获取有序集合的成员
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    /// 指定范围内的成员列表（从高分到低分排序）
    pub async fn zrevrange(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("ZREVRANGE");
        cmd.arg(key.into()).arg(start).arg(stop);
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// ZRANGE WITHSCORES - 按索引范围获取有序集合的成员及其分数
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    /// 成员和分数的元组列表（从低分到高分排序）
    pub async fn zrange_with_scores(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<(String, f64)>> {
        let mut cmd = redis::cmd("ZRANGE");
        cmd.arg(key.into()).arg(start).arg(stop).arg("WITHSCORES");
        let result = self.execute(&cmd).await?;
        parse_with_scores(&result)
    }

    /// ZREVRANGE WITHSCORES - 按索引范围从高到低获取有序集合的成员及其分数
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    /// 成员和分数的元组列表（从高分到低分排序）
    pub async fn zrevrange_with_scores(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<(String, f64)>> {
        let mut cmd = redis::cmd("ZREVRANGE");
        cmd.arg(key.into()).arg(start).arg(stop).arg("WITHSCORES");
        let result = self.execute(&cmd).await?;
        parse_with_scores(&result)
    }

    /// ZREMRANGEBYRANK - 移除有序集合中指定排名范围的成员
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `start`: 起始排名（0 为第一）
    /// - `stop`: 结束排名（-1 为最后一个）
    ///
    /// # 返回
    /// 被移除的成员数量
    pub async fn zremrangebyrank(
        &self,
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("ZREMRANGEBYRANK");
        cmd.arg(key.into()).arg(start).arg(stop);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZREMRANGEBYSCORE - 移除有序集合中指定分数范围的成员
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `min`: 最小分数（包含）
    /// - `max`: 最大分数（包含）
    ///
    /// # 返回
    /// 被移除的成员数量
    pub async fn zremrangebyscore(
        &self,
        key: impl Into<String>,
        min: f64,
        max: f64,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("ZREMRANGEBYSCORE");
        cmd.arg(key.into()).arg(min).arg(max);
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// ZSCAN - 增量式迭代有序集合中的成员
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `cursor`: 游标（初始传 0）
    /// - `pattern`: 可选的匹配模式
    /// - `count`: 可选的每批返回数量提示
    ///
    /// # 返回
    /// - `Ok((u64, Vec<(String, f64)>))`: (下一游标, 成员-分数对列表)
    pub async fn zscan(
        &self,
        key: impl Into<String>,
        cursor: u64,
        pattern: Option<&str>,
        count: Option<i64>,
    ) -> Result<(u64, Vec<(String, f64)>)> {
        let mut cmd = redis::cmd("ZSCAN");
        cmd.arg(key.into()).arg(cursor);
        if let Some(p) = pattern {
            cmd.arg("MATCH").arg(p);
        }
        if let Some(c) = count {
            cmd.arg("COUNT").arg(c);
        }
        let result = self.execute(&cmd).await?;
        let (next_cursor, fields) = parse_scan_result(&result)?;
        if fields.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "ZSCAN 返回的成员/分数数量不是偶数".to_string(),
            ));
        }
        let pairs = fields
            .chunks_exact(2)
            .map(|pair| {
                let score = pair[1].parse::<f64>().map_err(|_| {
                    DbError::RedisTypeConversionError("ZSCAN 分数不是合法数字".to_string())
                })?;
                Ok((pair[0].clone(), score))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((next_cursor, pairs))
    }

    // ==================== 通用键操作 ====================

    /// DEL - 删除一个或多个键
    ///
    /// # 返回
    /// 返回被删除的键数量
    pub async fn del(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("DEL");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// EXISTS - 检查一个或多个键是否存在
    ///
    /// # 返回
    /// 返回存在的键数量
    pub async fn exists(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("EXISTS");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// EXPIRE - 设置键的过期时间（秒）
    ///
    /// # 返回
    /// - `Ok(true)`: 设置成功
    /// - `Ok(false)`: 键不存在
    pub async fn expire(&self, key: impl Into<String>, seconds: i64) -> Result<bool> {
        let mut cmd = redis::cmd("EXPIRE");
        cmd.arg(key.into()).arg(seconds);
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// TTL - 获取键的剩余生存时间（秒）
    ///
    /// # 返回
    /// - 正数: 剩余秒数
    /// - -1: 键存在但没有过期时间
    /// - -2: 键不存在
    pub async fn ttl(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("TTL");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// PERSIST - 移除键的过期时间
    ///
    /// # 返回
    /// - `Ok(true)`: 移除成功
    /// - `Ok(false)`: 键不存在或没有过期时间
    pub async fn persist(&self, key: impl Into<String>) -> Result<bool> {
        let mut cmd = redis::cmd("PERSIST");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        Ok(result.as_i64() == Some(1))
    }

    /// KEYS - 查找所有匹配给定模式的键
    ///
    /// # 警告
    /// 此命令在生产环境中可能导致性能问题，请谨慎使用
    ///
    /// # 参数
    /// - `pattern`: 匹配模式（例如 "user:*"）
    pub async fn keys(&self, pattern: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("KEYS");
        cmd.arg(pattern.into());
        let result = self.execute(&cmd).await?;
        collect_string_array(&result)
    }

    /// SCAN - 增量式迭代数据库中的所有键
    ///
    /// 生产安全的 KEYS 替代方案，不会阻塞 Redis 服务器。
    ///
    /// # 参数
    /// - `cursor`: 游标（初始传 0，使用返回的游标继续迭代）
    /// - `pattern`: 可选的键名匹配模式（如 `Some("user:*")`）
    /// - `count`: 可选的每批返回数量提示
    ///
    /// # 返回
    /// - `Ok((u64, Vec<String>))`: (下一游标, 本批键列表)
    /// - 游标为 0 表示完整迭代完成
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut cursor = 0u64;
    /// loop {
    ///     let (next, keys) = client.scan(cursor, Some("user:*"), Some(100)).await?;
    ///     // 处理 keys...
    ///     cursor = next;
    ///     if cursor == 0 { break; }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn scan(
        &self,
        cursor: u64,
        pattern: Option<&str>,
        count: Option<i64>,
    ) -> Result<(u64, Vec<String>)> {
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor);
        if let Some(p) = pattern {
            cmd.arg("MATCH").arg(p);
        }
        if let Some(c) = count {
            cmd.arg("COUNT").arg(c);
        }
        let result = self.execute(&cmd).await?;
        parse_scan_result(&result)
    }

    /// PUBLISH - 向指定频道发布消息
    ///
    /// # 参数
    /// - `channel`: 频道名称
    /// - `message`: 消息内容
    ///
    /// # 返回
    /// 接收到此消息的订阅者数量
    pub async fn publish(
        &self,
        channel: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<i64> {
        let mut cmd = redis::cmd("PUBLISH");
        cmd.arg(channel.into()).arg(message.into());
        let result = self.execute(&cmd).await?;
        require_i64(&result)
    }

    /// 健康检查 - 验证 Redis 连接是否正常
    ///
    /// 执行 PING 命令检查 Redis 服务可达性。连接池或命令错误原样返回，避免把基础设施
    /// 故障降格成不可诊断的布尔值。
    ///
    /// # 返回
    /// - `Ok(true)`: PING 成功，Redis 连接正常
    /// - `Ok(false)`: PING 返回空响应
    /// - `Err(DbError)`: 无法获取连接或命令执行失败
    pub async fn health_check(&self) -> Result<bool> {
        let cmd = redis::cmd("PING");
        let result = self.execute(&cmd).await?;
        Ok(!result.is_nil())
    }

    /// 详细健康检查：PING + INFO 关键字段。
    ///
    /// PING 失败直接返回错误；INFO 字段缺失不视为失败（部分托管服务会裁剪
    /// INFO 输出），以 `None` 表示。
    pub async fn health_check_detailed(&self) -> Result<RedisHealth> {
        self.health_check().await?;

        let mut health = RedisHealth {
            ping: true,
            role: None,
            connected_clients: None,
            used_memory: None,
            replication: None,
        };
        let mut cmd = redis::cmd("INFO");
        cmd.arg("server")
            .arg("clients")
            .arg("memory")
            .arg("replication");
        if let Ok(value) = self.execute(&cmd).await {
            if let Some(info) = value.as_string() {
                health.role = info_field(&info, "role");
                health.connected_clients =
                    info_field(&info, "connected_clients").and_then(|v| v.parse().ok());
                health.used_memory = info_field(&info, "used_memory").and_then(|v| v.parse().ok());
                health.replication = info_field(&info, "master_repl_offset")
                    .map(|offset| format!("offset={offset}"));
            }
        }
        Ok(health)
    }

    /// 获取连接池当前状态
    ///
    /// # 返回
    /// PoolStatus 结构体，包含 max_size、size、available、waiting 统计信息
    pub fn pool_status(&self) -> PoolStatus {
        self.backend.status()
    }

    // ==================== Lua 脚本操作 ====================

    /// 创建 Lua 脚本对象
    ///
    /// 返回 redis-rs 的原生 Script 类型，可用于执行 Lua 脚本。
    /// Script 会自动处理 EVALSHA 和 EVAL 的回退机制。
    ///
    /// # 参数
    /// - `code`: Lua 脚本代码
    ///
    /// # 返回
    /// redis::Script 对象
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     // 创建脚本：原子性地增加两个计数器
    ///     let script = client.script(
    ///         r#"
    ///         redis.call('INCR', KEYS[1])
    ///         redis.call('INCR', KEYS[2])
    ///         return redis.call('GET', KEYS[1])
    ///         "#
    ///     );
    ///     
    ///     // 执行脚本
    ///     let result: String = client.eval_script(
    ///         &script,
    ///         &["counter1".to_string(), "counter2".to_string()],
    ///         &[]
    ///     ).await?;
    ///     
    ///     println!("counter1 的值: {}", result);
    ///     Ok(())
    /// }
    /// ```
    pub fn script(&self, code: &str) -> redis::Script {
        redis::Script::new(code)
    }

    /// 执行 Lua 脚本
    ///
    /// 使用 redis::Script 执行 Lua 脚本，自动处理 EVALSHA 和 EVAL 的回退。
    /// 脚本内的所有操作都是原子性的。
    ///
    /// # 参数
    /// - `script`: 脚本对象（通过 `script()` 方法创建）
    /// - `keys`: KEYS 参数列表（在脚本中通过 KEYS[1], KEYS[2] 访问）
    /// - `args`: ARGV 参数列表（在脚本中通过 ARGV[1], ARGV[2] 访问）
    ///
    /// # 返回
    /// - `Ok(T)`: 脚本执行成功，返回类型化结果
    /// - `Err(DbError)`: 脚本执行失败
    ///
    /// # 示例：原子性计数器增加
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     let script = client.script(
    ///         r#"
    ///         local current = redis.call('GET', KEYS[1])
    ///         if not current then
    ///             current = 0
    ///         end
    ///         local new_value = tonumber(current) + tonumber(ARGV[1])
    ///         redis.call('SET', KEYS[1], new_value)
    ///         return new_value
    ///         "#
    ///     );
    ///     
    ///     let result: i64 = client.eval_script(
    ///         &script,
    ///         &["my_counter".to_string()],
    ///         &["10".to_string()]
    ///     ).await?;
    ///     
    ///     println!("新值: {}", result);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 示例：条件更新（乐观锁）
    /// ```no_run
    /// use yang_db::RedisClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    ///     
    ///     // 只有当余额足够时才扣减
    ///     let script = client.script(
    ///         r#"
    ///         local balance = tonumber(redis.call('GET', KEYS[1]) or 0)
    ///         local amount = tonumber(ARGV[1])
    ///         if balance >= amount then
    ///             redis.call('DECRBY', KEYS[1], amount)
    ///             return 1
    ///         else
    ///             return 0
    ///         end
    ///         "#
    ///     );
    ///     
    ///     let success: i64 = client.eval_script(
    ///         &script,
    ///         &["user:1000:balance".to_string()],
    ///         &["100".to_string()]
    ///     ).await?;
    ///     
    ///     if success == 1 {
    ///         println!("扣款成功");
    ///     } else {
    ///         println!("余额不足");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 性能优化
    /// - 首次执行时，脚本会被发送到 Redis 服务器并缓存
    /// - 后续执行使用 EVALSHA 命令，只传输脚本的 SHA1 哈希值
    /// - 如果脚本不在缓存中，自动回退到 EVAL 命令
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "redis", db.operation = "eval", otel.kind = "client")
    )]
    pub async fn eval_script<T>(
        &self,
        script: &redis::Script,
        keys: &[String],
        args: &[String],
    ) -> Result<T>
    where
        T: redis::FromRedisValue,
    {
        let mut attempt = 0u32;
        loop {
            let mut conn = self.connection().await?;

            // 每次尝试都重新构建 invocation（invoke_async 会消费它）
            let mut invocation = script.prepare_invoke();
            for key in keys {
                invocation.key(key);
            }
            for arg in args {
                invocation.arg(arg);
            }

            match conn.invoke_script::<T>(invocation).await {
                Ok(value) => return Ok(value),
                Err(error)
                    if attempt < self.config.reconnect_retries && should_reconnect(&error) =>
                {
                    attempt += 1;
                    let delay = reconnect_delay(attempt, self.config.reconnect_max_delay_ms);
                    log::warn!(
                        "Redis 脚本连接失败，第 {}/{} 次重试: {}",
                        attempt,
                        self.config.reconnect_retries,
                        error
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(DbError::from(error)),
            }
        }
    }
}

/// 从命令参数中提取命令名（只用于指标/tracing，绝不包含 key 或参数）。
fn command_name(cmd: &redis::Cmd) -> &str {
    match cmd.args_iter().next() {
        Some(redis::Arg::Simple(bytes)) => std::str::from_utf8(bytes).unwrap_or("unknown"),
        _ => "unknown",
    }
}

/// 记录命令级指标（仅 `metrics` feature）。
#[cfg(feature = "metrics")]
fn record_metrics(operation: &str, started: Instant, result: &Result<RedisValue>) {
    metrics::histogram!(
        "db.client.operation.duration",
        "db.system" => "redis",
        "db.operation" => operation.to_string()
    )
    .record(started.elapsed().as_secs_f64());

    if let Err(error) = result {
        metrics::counter!(
            "db.client.operation.errors",
            "db.system" => "redis",
            "db.operation" => operation.to_string(),
            "error.type" => format!("{:?}", error.category())
        )
        .increment(1);
    }
}

#[cfg(not(feature = "metrics"))]
fn record_metrics(_operation: &str, _started: Instant, _result: &Result<RedisValue>) {}

/// 记录类型化命令的指标（不区分错误类别）。
#[cfg(feature = "metrics")]
fn record_operation_metrics(operation: &str, started: Instant, ok: bool) {
    metrics::histogram!(
        "db.client.operation.duration",
        "db.system" => "redis",
        "db.operation" => operation.to_string()
    )
    .record(started.elapsed().as_secs_f64());
    if !ok {
        metrics::counter!(
            "db.client.operation.errors",
            "db.system" => "redis",
            "db.operation" => operation.to_string()
        )
        .increment(1);
    }
}

#[cfg(not(feature = "metrics"))]
fn record_operation_metrics(_operation: &str, _started: Instant, _ok: bool) {}

/// 从 INFO 文本中提取单个字段。
fn info_field(info: &str, field: &str) -> Option<String> {
    info.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim() == field)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 将 RedisValue 提取为可选字符串：Nil → None，非 UTF-8 → 明确错误。
pub(crate) fn optional_string(value: &RedisValue) -> Result<Option<String>> {
    match value {
        RedisValue::Nil => Ok(None),
        _ => value.as_string().map(Some).ok_or_else(|| {
            DbError::RedisTypeConversionError(
                "Redis 值不是 UTF-8 字符串，请使用 bytes API".to_string(),
            )
        }),
    }
}

/// 将 RedisValue 提取为整数，失败时返回统一的类型转换错误
///
/// 抽出 client 中大量重复的 `as_i64().ok_or_else(整数错误)` 样板
fn require_i64(value: &RedisValue) -> Result<i64> {
    value
        .as_i64()
        .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
}

/// 将 RedisValue 数组中的元素收集为字符串列表，非数组时返回空列表
///
/// 抽出 client 中大量重复的 `as_array → filter_map(as_string) → collect` 样板
pub(crate) fn collect_string_array(value: &RedisValue) -> Result<Vec<String>> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            item.as_string().ok_or_else(|| {
                DbError::RedisTypeConversionError(
                    "Redis 数组包含非 UTF-8/非字符串元素，请使用 bytes API".to_string(),
                )
            })
        })
        .collect()
}

/// 解析 SCAN/SSCAN/HSCAN 结果。
///
/// 游标在 Redis 协议中是 u64 十进制字符串；解析失败必须报错，不能静默当作 0，
/// 否则会把「遍历未完成」误判为「遍历结束」。
pub(crate) fn parse_scan_result(result: &RedisValue) -> crate::Result<(u64, Vec<String>)> {
    let Some(arr) = result.as_array() else {
        return Err(DbError::RedisTypeConversionError(
            "Redis SCAN 结果不是数组".to_string(),
        ));
    };
    let cursor_raw = arr
        .first()
        .ok_or_else(|| DbError::RedisTypeConversionError("Redis SCAN 结果缺少游标".to_string()))?;
    // RESP2 返回 bulk string；部分 RESP3/代理实现可能返回整数，两者都接受。
    let cursor = cursor_raw
        .as_string()
        .or_else(|| cursor_raw.as_i64().map(|value| value.to_string()))
        .ok_or_else(|| {
            DbError::RedisTypeConversionError("Redis SCAN 结果游标类型非法".to_string())
        })?;
    let cursor = cursor
        .parse::<u64>()
        .map_err(|_| DbError::RedisTypeConversionError(format!("Redis SCAN 游标非法: {cursor}")))?;
    let members = arr
        .get(1)
        .map(collect_string_array)
        .transpose()?
        .unwrap_or_default();
    Ok((cursor, members))
}

/// 解析 WITHSCORES 结果（交替的成员/分数对）
pub(crate) fn parse_with_scores(result: &RedisValue) -> Result<Vec<(String, f64)>> {
    let Some(arr) = result.as_array() else {
        return Ok(Vec::new());
    };
    if arr.len() % 2 != 0 {
        return Err(DbError::RedisTypeConversionError(
            "WITHSCORES 返回的成员/分数数量不是偶数".to_string(),
        ));
    }
    arr.chunks_exact(2)
        .map(|pair| {
            let member = pair[0].as_string().ok_or_else(|| {
                DbError::RedisTypeConversionError("WITHSCORES 成员不是 UTF-8 字符串".to_string())
            })?;
            let score = pair[1].as_f64().or_else(|| {
                pair[1]
                    .as_string()
                    .and_then(|value| value.parse::<f64>().ok())
            });
            let score = score.ok_or_else(|| {
                DbError::RedisTypeConversionError("WITHSCORES 分数不是合法数字".to_string())
            })?;
            Ok((member, score))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_redis_client_clone() {
        // 测试 Clone trait
        let config = RedisConfig::default();
        // 注意：这里只测试结构体的 Clone，不测试实际连接
        let _ = config.clone();
    }

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connect_timeout, 5);
        assert_eq!(config.wait_timeout, 10);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_redis_config_custom() {
        let config = RedisConfig::default()
            .with_max_connections(20)
            .with_connect_timeout(10)
            .with_wait_timeout(15)
            .with_enable_logging(true);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.wait_timeout, 15);
        assert!(config.enable_logging);
    }

    /// 测试连接池配置构建逻辑
    ///
    /// 验证 PoolConfig 和 Timeouts 是否正确应用了 RedisConfig 参数
    #[test]
    fn test_pool_config_construction() {
        let redis_config = RedisConfig::default()
            .with_max_connections(25)
            .with_connect_timeout(8)
            .with_wait_timeout(12)
            .with_enable_logging(true);

        // 构建 PoolConfig（期望值来自同一映射 build_pool_config）
        let pool_config = redis_config.build_pool_config();

        // 验证配置参数
        assert_eq!(pool_config.max_size, 25, "最大连接数应为 25");
        assert_eq!(
            pool_config.timeouts.wait,
            Some(Duration::from_secs(12)),
            "等待超时应为 12 秒"
        );
        assert_eq!(
            pool_config.timeouts.create,
            Some(Duration::from_secs(8)),
            "创建超时应为 8 秒"
        );
        // recycle 是归还连接回收钩子超时，映射 idle_timeout（默认 300s），不是 connect_timeout
        assert_eq!(
            pool_config.timeouts.recycle,
            Some(Duration::from_secs(300)),
            "回收超时应为默认 idle_timeout 300 秒"
        );
    }

    /// 测试默认配置的连接池构建
    #[test]
    fn test_default_pool_config_construction() {
        let redis_config = RedisConfig::default();

        let pool_config = redis_config.build_pool_config();

        assert_eq!(pool_config.max_size, 10, "默认最大连接数应为 10");
        assert_eq!(
            pool_config.timeouts.wait,
            Some(Duration::from_secs(10)),
            "默认等待超时应为 10 秒"
        );
        assert_eq!(
            pool_config.timeouts.create,
            Some(Duration::from_secs(5)),
            "默认创建超时应为 5 秒"
        );
    }

    #[test]
    fn test_pool_status_reflects_configured_max_size() {
        let config = RedisConfig::default()
            .with_max_connections(25)
            .with_connect_timeout(8)
            .with_wait_timeout(12)
            .with_enable_logging(true);
        let mut pool_config = Config::from_url("redis://127.0.0.1:6379");
        pool_config.pool = Some(config.build_pool_config());
        let pool = pool_config
            .create_pool(Some(Runtime::Tokio1))
            .expect("无法创建测试连接池");
        let client = test_client(pool);

        assert_eq!(client.pool_status().max_size, 25);
    }

    fn test_client(pool: Pool) -> RedisClient {
        RedisClient {
            backend: RedisBackend::Single(pool),
            config: RedisConfig::default(),
            direct: None,
            redacted_target: "redis://127.0.0.1:6379".to_string(),
            namespace: None,
        }
    }

    #[tokio::test]
    async fn health_check_propagates_closed_pool_error() {
        let config = Config::from_url("redis://127.0.0.1:6379");
        let pool = config
            .create_pool(Some(Runtime::Tokio1))
            .expect("应能创建无需立即连接的测试池");
        let client = test_client(pool);

        client.close().await;

        assert!(client.is_closed());
        let result = client.health_check().await;
        assert!(
            matches!(&result, Err(DbError::RedisPoolClosed)),
            "关闭后的健康检查应返回不可重试的 RedisPoolClosed，实际为: {result:?}"
        );
        assert!(!result.unwrap_err().is_retryable());
    }

    #[tokio::test]
    async fn test_connect_with_config_rejects_invalid_config_before_connecting() {
        let config = RedisConfig::default().with_max_connections(0);
        let result = RedisClient::connect_with_config("redis://127.0.0.1:1", config).await;

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_parse_scan_result_valid() {
        let result = RedisValue::Array(vec![
            RedisValue::String("5".to_string()),
            RedisValue::Array(vec![
                RedisValue::String("key1".to_string()),
                RedisValue::String("key2".to_string()),
            ]),
        ]);
        let (cursor, keys) = parse_scan_result(&result).expect("有效 scan 结果应解析成功");
        assert_eq!(cursor, 5);
        assert_eq!(keys, vec!["key1".to_string(), "key2".to_string()]);
    }

    #[test]
    fn test_parse_scan_result_empty() {
        let result = RedisValue::Array(vec![
            RedisValue::String("0".to_string()),
            RedisValue::Array(vec![]),
        ]);
        let (cursor, keys) = parse_scan_result(&result).expect("有效 scan 结果应解析成功");
        assert_eq!(cursor, 0);
        assert!(keys.is_empty());
    }

    #[test]
    fn test_parse_scan_result_rejects_invalid_cursor() {
        let result = RedisValue::Array(vec![
            RedisValue::String("not-a-cursor".to_string()),
            RedisValue::Array(vec![]),
        ]);
        assert!(matches!(
            parse_scan_result(&result),
            Err(DbError::RedisTypeConversionError(_))
        ));
    }

    #[test]
    fn test_parse_scan_result_rejects_non_utf8_members() {
        let result = RedisValue::Array(vec![
            RedisValue::String("0".to_string()),
            RedisValue::Array(vec![RedisValue::Bytes(vec![0xFF])]),
        ]);
        assert!(matches!(
            parse_scan_result(&result),
            Err(DbError::RedisTypeConversionError(_))
        ));
    }

    #[test]
    fn test_parse_with_scores_valid() {
        let result = RedisValue::Array(vec![
            RedisValue::String("alice".to_string()),
            RedisValue::String("100".to_string()),
            RedisValue::String("bob".to_string()),
            RedisValue::String("95.5".to_string()),
        ]);
        let pairs = parse_with_scores(&result).expect("valid withscores");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("alice".to_string(), 100.0));
        assert_eq!(pairs[1], ("bob".to_string(), 95.5));
    }

    #[test]
    fn test_parse_with_scores_empty() {
        let result = RedisValue::Array(vec![]);
        let pairs = parse_with_scores(&result).expect("empty withscores");
        assert!(pairs.is_empty());
    }
}
