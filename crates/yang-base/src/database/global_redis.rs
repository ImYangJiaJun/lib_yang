//! 全局 Redis 访问器
//!
//! 提供线程安全的全局 Redis 实例访问。
//!
//! # 设计说明
//!
//! GlobalRedis 是对 yang-db::RedisClient 的封装，使用 OnceLock 实现全局单例模式。
//! 所有 Redis 操作都通过 yang-db 库实现，确保类型安全和统一的 Redis 访问接口。
//!
//! # 批量参数说明
//!
//! 批量操作方法（如 `del`、`exists`、`lpush` 等）接受 `&[impl AsRef<str>]` 类型参数，
//! 支持传入 `&[&str]`、`&[String]`、`&[&String]` 等多种形式，使用更灵活。
//!
//! # Pipeline / Transaction 说明
//!
//! 可通过 `GlobalRedis::transaction()` 获取 `yang_db::redis::RedisTransaction`，
//! 支持 WATCH/MULTI/EXEC 乐观锁事务。也可通过 `GlobalRedis::client()` 直接访问
//! `yang_db::RedisClient`，使用其 `pipeline()` 方法进行批量操作，或使用
//! `script()` / `eval_script()` 执行 Lua 脚本。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::GlobalRedis;
//! use yang_db::redis::RedisConfig;
//!
//! // 初始化全局 Redis
//! GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
//!
//! // String 操作
//! GlobalRedis::set("key", "value", None).await?;
//! let value = GlobalRedis::get("key").await?;
//!
//! // Hash 操作
//! GlobalRedis::hset("user:1", "name", "Alice").await?;
//! let name = GlobalRedis::hget("user:1", "name").await?;
//!
//! // List 操作（支持 &[&str] 或 &[String]）
//! GlobalRedis::lpush("queue", &["task1", "task2"]).await?;
//! let task = GlobalRedis::lpop("queue").await?;
//!
//! // 批量操作
//! GlobalRedis::mset(&[("k1".to_string(), "v1".to_string())]).await?;
//! let vals = GlobalRedis::mget(&["k1", "k2"]).await?;
//!
//! // 事务操作
//! let mut tx = GlobalRedis::transaction()?;
//! tx.set("key1", "value1").incr("counter");
//! let result: (String, i64) = tx.exec().await?;
//! ```

use crate::error::BaseError;
use std::sync::OnceLock;
use yang_db::redis::{RedisClient, RedisConfig, RedisTransaction};

/// 全局 Redis 实例
///
/// 使用 OnceLock 确保线程安全的单例模式
static GLOBAL_REDIS: OnceLock<RedisClient> = OnceLock::new();

/// 全局 Redis 访问器
///
/// 封装 yang-db::RedisClient，提供全局静态访问接口。
/// 所有 Redis 操作都通过 yang-db 库实现。
pub struct GlobalRedis;

impl GlobalRedis {
    /// 初始化全局 Redis
    ///
    /// # 参数
    ///
    /// - `url`: Redis 连接字符串，格式：`redis://host:port` 或 `redis://:password@host:port`
    /// - `config`: Redis 配置，包含连接池参数
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 错误
    ///
    /// - `RedisConnectionFailed`: Redis 连接失败
    /// - `RedisAlreadyInitialized`: Redis 已经初始化
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::database::GlobalRedis;
    /// use yang_db::redis::RedisConfig;
    ///
    /// let config = RedisConfig::new(10, 5, 300, true);
    ///
    /// GlobalRedis::init("redis://127.0.0.1:6379", config).await?;
    /// ```
    pub async fn init(url: &str, config: RedisConfig) -> Result<(), BaseError> {
        // 使用 yang-db::RedisClient::connect_with_config 创建 Redis 连接
        let client = RedisClient::connect_with_config(url, config)
            .await
            .map_err(|e| BaseError::RedisConnectionFailed(e.to_string()))?;

        // 设置全局 Redis 实例
        GLOBAL_REDIS
            .set(client)
            .map_err(|_| BaseError::RedisAlreadyInitialized)?;

        log::info!("全局 Redis 已初始化");
        Ok(())
    }

    /// 获取全局 Redis 实例
    ///
    /// # 返回
    ///
    /// - `Ok(&'static RedisClient)`: yang-db::RedisClient 实例的静态引用
    /// - `Err(BaseError)`: Redis 未初始化
    ///
    /// # 错误
    ///
    /// - `RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    pub fn client() -> Result<&'static RedisClient, BaseError> {
        GLOBAL_REDIS.get().ok_or(BaseError::RedisNotInitialized)
    }

    // ==================== String 操作 ====================

    /// 设置字符串值
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `value`: 值
    /// - `expire_seconds`: 过期时间（秒），None 表示不过期
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err(BaseError)`: 设置失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn set(
        key: impl Into<String>,
        value: impl Into<String>,
        expire_seconds: Option<i64>,
    ) -> Result<(), BaseError> {
        let key_str = key.into();
        let value_str = value.into();

        if let Some(seconds) = expire_seconds {
            // 使用 SETEX 设置带过期时间的值
            Self::client()?
                .setex(key_str, seconds, value_str)
                .await
                .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
        } else {
            // 使用 SET 设置不过期的值
            Self::client()?
                .set(key_str, value_str)
                .await
                .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
        }
    }

    /// 获取字符串值
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 获取成功，返回值
    /// - `Ok(None)`: 键不存在
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn get(key: impl Into<String>) -> Result<Option<String>, BaseError> {
        Self::client()?
            .get(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 删除键
    ///
    /// # 参数
    ///
    /// - `keys`: 键名列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 删除的键数量
    /// - `Err(BaseError)`: 删除失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn del(keys: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let keys_vec: Vec<String> = keys.iter().map(|k| k.as_ref().to_string()).collect();
        Self::client()?
            .del(&keys_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 检查键是否存在
    ///
    /// # 参数
    ///
    /// - `keys`: 键名列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 存在的键数量
    /// - `Err(BaseError)`: 检查失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn exists(keys: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let keys_vec: Vec<String> = keys.iter().map(|k| k.as_ref().to_string()).collect();
        Self::client()?
            .exists(&keys_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 设置键的过期时间
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `seconds`: 过期时间（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示设置成功，false 表示键不存在
    /// - `Err(BaseError)`: 设置失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn expire(key: impl Into<String>, seconds: i64) -> Result<bool, BaseError> {
        Self::client()?
            .expire(key, seconds)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取键的剩余生存时间
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 剩余秒数，-1 表示没有过期时间，-2 表示键不存在
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn ttl(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .ttl(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 移除键的过期时间
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示成功，false 表示键不存在或没有过期时间
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn persist(key: impl Into<String>) -> Result<bool, BaseError> {
        Self::client()?
            .persist(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 查找所有匹配给定模式的键
    ///
    /// # 参数
    ///
    /// - `pattern`: 匹配模式（支持 * ? [] 等通配符）
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 匹配的键列表
    /// - `Err(BaseError)`: 查找失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn keys(pattern: impl Into<String>) -> Result<Vec<String>, BaseError> {
        Self::client()?
            .keys(pattern)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== Hash 操作 ====================

    /// 设置 Hash 字段值
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `field`: 字段名
    /// - `value`: 值
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示新字段，false 表示更新已有字段
    /// - `Err(BaseError)`: 设置失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hset(
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<bool, BaseError> {
        Self::client()?
            .hset(key, field, value)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取 Hash 字段值
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `field`: 字段名
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 获取成功，返回值
    /// - `Ok(None)`: 字段不存在
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hget(
        key: impl Into<String>,
        field: impl Into<String>,
    ) -> Result<Option<String>, BaseError> {
        Self::client()?
            .hget(key, field)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 删除 Hash 字段
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `fields`: 字段名列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 删除的字段数量
    /// - `Err(BaseError)`: 删除失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hdel(key: impl Into<String>, fields: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let fields_vec: Vec<String> = fields.iter().map(|f| f.as_ref().to_string()).collect();
        Self::client()?
            .hdel(key, &fields_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取 Hash 所有字段和值
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<(String, String)>)`: 所有字段和值的列表
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hgetall(key: impl Into<String>) -> Result<Vec<(String, String)>, BaseError> {
        Self::client()?
            .hgetall(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 检查 Hash 字段是否存在
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `field`: 字段名
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示存在，false 表示不存在
    /// - `Err(BaseError)`: 检查失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hexists(
        key: impl Into<String>,
        field: impl Into<String>,
    ) -> Result<bool, BaseError> {
        Self::client()?
            .hexists(key, field)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取 Hash 字段数量
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 字段数量
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn hlen(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .hlen(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== List 操作 ====================

    /// 从列表左侧推入元素
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `values`: 值列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 推入后列表长度
    /// - `Err(BaseError)`: 推入失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn lpush(key: impl Into<String>, values: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let values_vec: Vec<String> = values.iter().map(|v| v.as_ref().to_string()).collect();
        Self::client()?
            .lpush(key, &values_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 从列表右侧推入元素
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `values`: 值列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 推入后列表长度
    /// - `Err(BaseError)`: 推入失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn rpush(key: impl Into<String>, values: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let values_vec: Vec<String> = values.iter().map(|v| v.as_ref().to_string()).collect();
        Self::client()?
            .rpush(key, &values_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 从列表左侧弹出元素
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 弹出的元素
    /// - `Ok(None)`: 列表为空
    /// - `Err(BaseError)`: 弹出失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn lpop(key: impl Into<String>) -> Result<Option<String>, BaseError> {
        Self::client()?
            .lpop(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 从列表右侧弹出元素
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 弹出的元素
    /// - `Ok(None)`: 列表为空
    /// - `Err(BaseError)`: 弹出失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn rpop(key: impl Into<String>) -> Result<Option<String>, BaseError> {
        Self::client()?
            .rpop(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取列表长度
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 列表长度
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn llen(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .llen(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取列表指定范围的元素
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `start`: 起始索引（0 表示第一个元素）
    /// - `stop`: 结束索引（-1 表示最后一个元素）
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 元素列表
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn lrange(
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>, BaseError> {
        Self::client()?
            .lrange(key, start, stop)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== Set 操作 ====================

    /// 向集合添加成员
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `members`: 成员列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 添加的成员数量
    /// - `Err(BaseError)`: 添加失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn sadd(key: impl Into<String>, members: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let members_vec: Vec<String> = members.iter().map(|m| m.as_ref().to_string()).collect();
        Self::client()?
            .sadd(key, &members_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 从集合移除成员
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `members`: 成员列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 移除的成员数量
    /// - `Err(BaseError)`: 移除失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn srem(key: impl Into<String>, members: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let members_vec: Vec<String> = members.iter().map(|m| m.as_ref().to_string()).collect();
        Self::client()?
            .srem(key, &members_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 检查成员是否在集合中
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `member`: 成员
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示存在，false 表示不存在
    /// - `Err(BaseError)`: 检查失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn sismember(
        key: impl Into<String>,
        member: impl Into<String>,
    ) -> Result<bool, BaseError> {
        Self::client()?
            .sismember(key, member)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取集合所有成员
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 成员列表
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn smembers(key: impl Into<String>) -> Result<Vec<String>, BaseError> {
        Self::client()?
            .smembers(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取集合成员数量
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 成员数量
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn scard(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .scard(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== Sorted Set 操作 ====================

    /// 向有序集合添加成员
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `members`: 成员和分数列表 (score, member)
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 添加的成员数量
    /// - `Err(BaseError)`: 添加失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zadd(key: impl Into<String>, members: &[(f64, String)]) -> Result<i64, BaseError> {
        Self::client()?
            .zadd(key, members)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 从有序集合移除成员
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `members`: 成员列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 移除的成员数量
    /// - `Err(BaseError)`: 移除失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zrem(key: impl Into<String>, members: &[impl AsRef<str>]) -> Result<i64, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let members_vec: Vec<String> = members.iter().map(|m| m.as_ref().to_string()).collect();
        Self::client()?
            .zrem(key, &members_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取有序集合成员数量
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 成员数量
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zcard(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .zcard(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 获取有序集合指定范围的成员（按分数从小到大）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 成员列表
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zrange(
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>, BaseError> {
        Self::client()?
            .zrange(key, start, stop)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== 新增 API：String 计数器操作 ====================

    /// 将键的整数値增加 1（INCR）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 增加后的値
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败（如值不是整数）
    pub async fn incr(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .incr(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 将键的整数値减少 1（DECR）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 减少后的値
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败（如值不是整数）
    pub async fn decr(key: impl Into<String>) -> Result<i64, BaseError> {
        Self::client()?
            .decr(key)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 将键的整数値增加指定数量（INCRBY）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `increment`: 增量（可为负数）
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 增加后的値
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败（如值不是整数）
    pub async fn incrby(key: impl Into<String>, increment: i64) -> Result<i64, BaseError> {
        Self::client()?
            .incrby(key, increment)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== 新增 API：Hash 计数器操作 ====================

    /// 将 Hash 字段的整数値增加指定数量（HINCRBY）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `field`: 字段名
    /// - `increment`: 增量（可为负数）
    ///
    /// # 返回
    ///
    /// - `Ok(i64)`: 增加后的値
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败（如字段値不是整数）
    pub async fn hincrby(
        key: impl Into<String>,
        field: impl Into<String>,
        increment: i64,
    ) -> Result<i64, BaseError> {
        Self::client()?
            .hincrby(key, field, increment)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== 新增 API：批量 String 操作 ====================

    /// 批量获取多个键的値（MGET）
    ///
    /// # 参数
    ///
    /// - `keys`: 键名列表，支持 `&[&str]`、`&[String]`、`&[&String]` 等多种形式
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<Option<String>>)`: 各键对应的値，不存在的键返回 `None`
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn mget(keys: &[impl AsRef<str>]) -> Result<Vec<Option<String>>, BaseError> {
        // 将 &[impl AsRef<str>] 转换为 Vec<String> 以调用 yang-db
        let keys_vec: Vec<String> = keys.iter().map(|k| k.as_ref().to_string()).collect();
        Self::client()?
            .mget(&keys_vec)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 批量设置多个键値对（MSET）
    ///
    /// # 参数
    ///
    /// - `pairs`: 键値对数组，每个元素为 `(key, value)` 元组
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn mset(pairs: &[(String, String)]) -> Result<(), BaseError> {
        Self::client()?
            .mset(pairs)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== 新增 API：有序集合扩展操作 ====================

    /// 获取有序集合指定范围的成员及其分数（ZRANGE WITHSCORES）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<(String, f64)>)`: 成员和分数的元组列表（从低分到高分排序）
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zrange_with_scores(
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<(String, f64)>, BaseError> {
        Self::client()?
            .zrange_with_scores(key, start, stop)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 按索引范围从高到低获取有序集合的成员（ZREVRANGE）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `start`: 起始索引
    /// - `stop`: 结束索引
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 成员列表（从高分到低分排序）
    /// - `Err(BaseError)`: 获取失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zrevrange(
        key: impl Into<String>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<String>, BaseError> {
        Self::client()?
            .zrevrange(key, start, stop)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    /// 将有序集合成员的分数增加指定数量（ZINCRBY）
    ///
    /// # 参数
    ///
    /// - `key`: 键名
    /// - `increment`: 增量（可为负数）
    /// - `member`: 成员
    ///
    /// # 返回
    ///
    /// - `Ok(f64)`: 增加后的分数
    /// - `Err(BaseError)`: 操作失败
    ///
    /// # 错误
    ///
    /// - `BaseError::RedisNotInitialized`: Redis 未初始化，需要先调用 `init` 方法
    /// - `BaseError::RedisOperationFailed`: Redis 命令执行失败
    pub async fn zincrby(
        key: impl Into<String>,
        increment: f64,
        member: impl Into<String>,
    ) -> Result<f64, BaseError> {
        Self::client()?
            .zincrby(key, increment, member)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }

    // ==================== Pipeline / Transaction 入口 ====================

    /// 获取 Redis 事务对象
    ///
    /// 返回 `yang_db::redis::RedisTransaction`，支持 WATCH/MULTI/EXEC 乐观锁事务。
    ///
    /// # 返回
    ///
    /// - `Ok(RedisTransaction)`: 事务对象
    /// - `Err(BaseError)`: Redis 未初始化
    ///
    /// # 说明
    ///
    /// 事务支持链式调用，可在事务中执行多个命令。
    /// 如需使用 Pipeline 批量操作，可通过 `GlobalRedis::client()?.pipeline()` 获取。
    /// 如需执行 Lua 脚本，可通过 `GlobalRedis::client()?.script(code)` 创建脚本对象。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 基础事务
    /// let mut tx = GlobalRedis::transaction()?;
    /// tx.set("key1", "value1")
    ///   .set("key2", "value2")
    ///   .incr("counter");
    /// let results: (String, String, i64) = tx.exec().await?;
    ///
    /// // 乘观锁事务
    /// let mut tx = GlobalRedis::transaction()?;
    /// tx.watch(&["balance".to_string()]);
    /// tx.set("balance", "900");
    /// let result: (String,) = tx.exec().await?;
    ///
    /// // Pipeline 批量操作
    /// let mut pipeline = GlobalRedis::client()?.pipeline();
    /// pipeline.set("k1", "v1").set("k2", "v2");
    /// let results = pipeline.execute().await?;
    /// ```
    pub fn transaction() -> Result<RedisTransaction, BaseError> {
        // 通过全局客户端创建事务对象
        Ok(Self::client()?.transaction())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_not_initialized() {
        // 测试未初始化时获取 Redis 实例
        let result = GlobalRedis::client();
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::RedisNotInitialized)));
    }

    #[test]
    fn test_transaction_not_initialized() {
        // 测试未初始化时获取事务对象应返回错误
        let result = GlobalRedis::transaction();
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::RedisNotInitialized)));
    }
}
