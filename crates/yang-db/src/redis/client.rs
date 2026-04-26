use crate::{DbError, RedisConfig, RedisValue, Result};
use deadpool_redis::{Config, Pool, Runtime};

/// Redis 客户端
///
/// 提供 Redis 数据库操作的统一接口，支持连接池管理
#[derive(Clone)]
pub struct RedisClient {
    /// 连接池
    pool: Pool,
    /// 配置
    #[allow(dead_code)]
    config: RedisConfig,
}

impl RedisClient {
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
    ///     let config = RedisConfig::new(20, 10, 15, true);
    ///     let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_with_config(url: impl Into<String>, config: RedisConfig) -> Result<Self> {
        let url = url.into();

        // 创建连接池配置
        let pool_config = Config {
            url: Some(url.clone()),
            ..Default::default()
        };

        // 创建连接池
        let pool = pool_config
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| DbError::RedisConnectionError(format!("创建连接池失败: {}", e)))?;

        // 测试连接
        let mut conn = pool
            .get()
            .await
            .map_err(|e| DbError::RedisConnectionError(format!("获取连接失败: {}", e)))?;

        // 执行 PING 命令测试连接
        redis::cmd("PING")
            .query_async::<String>(&mut *conn)
            .await
            .map_err(|e| DbError::RedisConnectionError(format!("连接测试失败: {}", e)))?;

        if config.enable_logging {
            log::info!("Redis 连接成功: {}", url);
        }

        Ok(Self { pool, config })
    }

    /// 获取连接池引用
    ///
    /// # 返回
    /// 连接池的引用
    pub fn pool(&self) -> &Pool {
        &self.pool
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
    pub async fn execute(&self, cmd: &redis::Cmd) -> Result<RedisValue> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| DbError::RedisPoolError(format!("获取连接失败: {}", e)))?;

        let value: redis::Value = cmd
            .query_async(&mut *conn)
            .await
            .map_err(|e| DbError::RedisCommandError(format!("命令执行失败: {}", e)))?;

        Ok(RedisValue::from(value))
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
        Ok(value.as_string())
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
        Ok(result.as_string())
    }

    /// MGET - 批量获取多个键的值
    ///
    /// # 返回
    /// 返回值数组，不存在的键返回 None
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        let mut cmd = redis::cmd("MGET");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// MSET - 批量设置多个键值对
    ///
    /// # 参数
    /// - `pairs`: 键值对数组
    pub async fn mset(&self, pairs: &[(String, String)]) -> Result<()> {
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
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// INCRBY - 将键的值增加指定数量
    ///
    /// # 返回
    /// 返回增加后的值
    pub async fn incrby(&self, key: impl Into<String>, increment: i64) -> Result<i64> {
        let mut cmd = redis::cmd("INCRBY");
        cmd.arg(key.into()).arg(increment);
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// DECR - 将键的值减少 1
    ///
    /// # 返回
    /// 返回减少后的值
    pub async fn decr(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("DECR");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// DECRBY - 将键的值减少指定数量
    ///
    /// # 返回
    /// 返回减少后的值
    pub async fn decrby(&self, key: impl Into<String>, decrement: i64) -> Result<i64> {
        let mut cmd = redis::cmd("DECRBY");
        cmd.arg(key.into()).arg(decrement);
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// APPEND - 将值追加到键的原值末尾
    ///
    /// # 返回
    /// 返回追加后字符串的长度
    pub async fn append(&self, key: impl Into<String>, value: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("APPEND");
        cmd.arg(key.into()).arg(value.into());
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// STRLEN - 获取键存储的字符串长度
    ///
    /// # 返回
    /// 返回字符串长度，键不存在返回 0
    pub async fn strlen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("STRLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        Ok(result.as_string())
    }

    /// HDEL - 删除哈希表的一个或多个字段
    ///
    /// # 返回
    /// 返回被删除字段的数量
    pub async fn hdel(&self, key: impl Into<String>, fields: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("HDEL");
        cmd.arg(key.into());
        for field in fields {
            cmd.arg(field);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        let mut cmd = redis::cmd("HMGET");
        cmd.arg(key.into());
        for field in fields {
            cmd.arg(field);
        }
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// HGETALL - 获取哈希表的所有字段和值
    ///
    /// # 返回
    /// 返回字段-值对的向量
    pub async fn hgetall(&self, key: impl Into<String>) -> Result<Vec<(String, String)>> {
        let mut cmd = redis::cmd("HGETALL");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            let mut pairs = Vec::new();
            for i in (0..arr.len()).step_by(2) {
                if i + 1 < arr.len()
                    && let (Some(field), Some(value)) = (arr[i].as_string(), arr[i + 1].as_string())
                {
                    pairs.push((field, value));
                }
            }
            Ok(pairs)
        } else {
            Ok(vec![])
        }
    }

    /// HLEN - 获取哈希表的字段数量
    pub async fn hlen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("HLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// HKEYS - 获取哈希表的所有字段名
    pub async fn hkeys(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("HKEYS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// HVALS - 获取哈希表的所有值
    pub async fn hvals(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("HVALS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
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
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        let mut cmd = redis::cmd("LPUSH");
        cmd.arg(key.into());
        for value in values {
            cmd.arg(value);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// RPUSH - 将一个或多个值插入列表尾部
    ///
    /// # 返回
    /// 返回插入后列表的长度
    pub async fn rpush(&self, key: impl Into<String>, values: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("RPUSH");
        cmd.arg(key.into());
        for value in values {
            cmd.arg(value);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        Ok(result.as_string())
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
        Ok(result.as_string())
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
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// LLEN - 获取列表长度
    pub async fn llen(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("LLEN");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        Ok(result.as_string())
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

    // ==================== Set 操作 ====================

    /// SADD - 向集合添加一个或多个成员
    ///
    /// # 返回
    /// 返回被添加到集合中的新元素数量
    pub async fn sadd(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("SADD");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// SREM - 移除集合中的一个或多个成员
    ///
    /// # 返回
    /// 返回被移除的元素数量
    pub async fn srem(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("SREM");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// SMEMBERS - 获取集合的所有成员
    pub async fn smembers(&self, key: impl Into<String>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("SMEMBERS");
        cmd.arg(key.into());
        let result = self.execute(&cmd).await?;
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
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
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        Ok(result.as_string())
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
        Ok(result.as_string())
    }

    // ==================== Sorted Set 操作 ====================

    /// ZADD - 向有序集合添加一个或多个成员
    ///
    /// # 返回
    /// 返回被添加的新成员数量
    pub async fn zadd(&self, key: impl Into<String>, members: &[(f64, String)]) -> Result<i64> {
        let mut cmd = redis::cmd("ZADD");
        cmd.arg(key.into());
        for (score, member) in members {
            cmd.arg(score).arg(member);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// ZREM - 移除有序集合中的一个或多个成员
    ///
    /// # 返回
    /// 返回被移除的成员数量
    pub async fn zrem(&self, key: impl Into<String>, members: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("ZREM");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
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
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }

    /// ZCOUNT - 计算分数范围内的成员数量
    pub async fn zcount(&self, key: impl Into<String>, min: f64, max: f64) -> Result<i64> {
        let mut cmd = redis::cmd("ZCOUNT");
        cmd.arg(key.into()).arg(min).arg(max);
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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

    // ==================== 通用键操作 ====================

    /// DEL - 删除一个或多个键
    ///
    /// # 返回
    /// 返回被删除的键数量
    pub async fn del(&self, keys: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("DEL");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
    }

    /// EXISTS - 检查一个或多个键是否存在
    ///
    /// # 返回
    /// 返回存在的键数量
    pub async fn exists(&self, keys: &[String]) -> Result<i64> {
        let mut cmd = redis::cmd("EXISTS");
        for key in keys {
            cmd.arg(key);
        }
        let result = self.execute(&cmd).await?;
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        result
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("无法转换为整数".to_string()))
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
        if let Some(arr) = result.as_array() {
            Ok(arr.iter().filter_map(|v| v.as_string()).collect())
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = RedisConfig::new(20, 10, 15, true);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.wait_timeout, 15);
        assert!(config.enable_logging);
    }
}
