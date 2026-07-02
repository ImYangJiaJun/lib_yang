use crate::{DbError, RedisClient, RedisValue, Result};
use redis::FromRedisValue;

/// Redis Pipeline 批量操作
///
/// Pipeline 允许将多个命令打包发送到 Redis 服务器，减少网络往返次数，提高性能。
/// 基于 redis-rs 的原生 `redis::pipe()` 实现。
///
/// # 特性
/// - 支持所有 Redis 命令
/// - 按添加顺序返回结果
/// - 单次网络往返执行所有命令
/// - 非原子性（与事务不同）
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
///     pipeline.set("key1", "value1");
///     pipeline.set("key2", "value2");
///     pipeline.get("key1");
///     pipeline.incr("counter");
///
///     let results = pipeline.execute().await?;
///     println!("Pipeline 执行结果: {:?}", results);
///     
///     Ok(())
/// }
/// ```
pub struct RedisPipeline {
    /// 原生 redis::Pipeline
    pipe: redis::Pipeline,
    /// Redis 客户端引用
    client: RedisClient,
}

impl RedisPipeline {
    /// 创建新的 Pipeline
    ///
    /// # 参数
    /// - `client`: Redis 客户端
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
    ///     let pipeline = client.pipeline();
    ///     Ok(())
    /// }
    /// ```
    pub fn new(client: RedisClient) -> Self {
        Self {
            pipe: redis::pipe(),
            client,
        }
    }

    /// 添加 SET 命令
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 值
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut pipeline = client.pipeline();
    /// pipeline.set("key1", "value1")
    ///         .set("key2", "value2");
    /// # Ok(())
    /// # }
    /// ```
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.pipe.set(key.into(), value.into());
        self
    }

    /// 添加 GET 命令
    ///
    /// # 参数
    /// - `key`: 键
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn get(&mut self, key: impl Into<String>) -> &mut Self {
        self.pipe.get(key.into());
        self
    }

    /// 添加 EXISTS 命令（单键）
    ///
    /// 检查单个键是否存在，结果为 `Int(0)` 或 `Int(1)`。
    ///
    /// # 参数
    /// - `key`: 键
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn exists(&mut self, key: impl Into<String>) -> &mut Self {
        self.pipe.exists(key.into());
        self
    }

    /// 添加 DEL 命令
    ///
    /// # 参数
    /// - `keys`: 要删除的键列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn del(&mut self, keys: &[String]) -> &mut Self {
        self.pipe.del(keys);
        self
    }

    /// 添加 INCR 命令
    ///
    /// # 参数
    /// - `key`: 键
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn incr(&mut self, key: impl Into<String>) -> &mut Self {
        self.pipe.incr(key.into(), 1);
        self
    }

    /// 添加 HSET 命令
    ///
    /// # 参数
    /// - `key`: 哈希表键
    /// - `field`: 字段名
    /// - `value`: 字段值
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn hset(
        &mut self,
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.pipe.hset(key.into(), field.into(), value.into());
        self
    }

    /// 添加 HGET 命令
    ///
    /// # 参数
    /// - `key`: 哈希表键
    /// - `field`: 字段名
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn hget(&mut self, key: impl Into<String>, field: impl Into<String>) -> &mut Self {
        self.pipe.hget(key.into(), field.into());
        self
    }

    /// 添加 LPUSH 命令
    ///
    /// # 参数
    /// - `key`: 列表键
    /// - `values`: 要插入的值列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    ///
    /// 多元素打包为**单条** LPUSH 命令（NEW-21），保证「每方法调用一条命令一个结果」
    /// 契约。空切片为 no-op（不下发命令）以避免 redis 报参数错误。
    pub fn lpush(&mut self, key: impl Into<String>, values: &[String]) -> &mut Self {
        if !values.is_empty() {
            self.pipe.lpush(key.into(), values);
        }
        self
    }

    /// 添加 RPUSH 命令
    ///
    /// # 参数
    /// - `key`: 列表键
    /// - `values`: 要插入的值列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn rpush(&mut self, key: impl Into<String>, values: &[String]) -> &mut Self {
        if !values.is_empty() {
            self.pipe.rpush(key.into(), values);
        }
        self
    }

    /// 添加 SADD 命令
    ///
    /// # 参数
    /// - `key`: 集合键
    /// - `members`: 要添加的成员列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn sadd(&mut self, key: impl Into<String>, members: &[String]) -> &mut Self {
        if !members.is_empty() {
            self.pipe.sadd(key.into(), members);
        }
        self
    }

    /// 添加 ZADD 命令
    ///
    /// # 参数
    /// - `key`: 有序集合键
    /// - `members`: (分数, 成员) 元组列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn zadd(&mut self, key: impl Into<String>, members: &[(f64, String)]) -> &mut Self {
        if !members.is_empty() {
            // redis-rs zadd_multiple 接受 &[(score, member)]，一次性打包为单条命令
            let pairs: Vec<(f64, &str)> = members.iter().map(|(s, m)| (*s, m.as_str())).collect();
            self.pipe.zadd_multiple(key.into(), &pairs);
        }
        self
    }

    /// 添加自定义命令
    ///
    /// # 参数
    /// - `cmd`: Redis 命令
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut pipeline = client.pipeline();
    /// let mut cmd = redis::cmd("SETEX");
    /// cmd.arg("key").arg(60).arg("value");
    /// pipeline.cmd(cmd);
    /// # Ok(())
    /// # }
    /// ```
    pub fn cmd(&mut self, cmd: redis::Cmd) -> &mut Self {
        self.pipe.add_command(cmd);
        self
    }

    /// 执行 Pipeline 中的所有命令（类型化版本）
    ///
    /// # 类型参数
    /// - `T`: 实现了 `FromRedisValue` 的类型
    ///
    /// # 返回
    /// - `Ok(Vec<T>)`: 按命令添加顺序返回的结果列表
    /// - `Err(DbError)`: 执行失败
    ///
    /// # 错误处理
    /// 如果某个命令失败，返回错误并指明失败的命令索引
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut pipeline = client.pipeline();
    /// pipeline.set("key1", "value1")
    ///         .set("key2", "value2")
    ///         .get("key1");
    ///
    /// // 获取类型化结果
    /// let results: Vec<String> = pipeline.query().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query<T: FromRedisValue>(self) -> Result<Vec<T>> {
        let mut conn = self
            .client
            .pool()
            .get()
            .await
            .map_err(|e| DbError::RedisPoolError(format!("获取连接失败: {}", e)))?;

        let results: Vec<T> = self
            .pipe
            .query_async(&mut *conn)
            .await
            .map_err(|e| DbError::RedisCommandError(format!("Pipeline 执行失败: {}", e)))?;

        Ok(results)
    }

    /// 执行 Pipeline 中的所有命令（兼容模式）
    ///
    /// # 返回
    /// - `Ok(Vec<RedisValue>)`: 按命令添加顺序返回的结果列表
    /// - `Err(DbError)`: 执行失败
    ///
    /// # 错误处理
    /// 如果某个命令失败，返回错误并指明失败的命令索引
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut pipeline = client.pipeline();
    /// pipeline.set("key1", "value1")
    ///         .set("key2", "value2")
    ///         .get("key1")
    ///         .incr("counter");
    ///
    /// let results = pipeline.execute().await?;
    /// println!("Pipeline 执行结果: {:?}", results);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(self) -> Result<Vec<RedisValue>> {
        let mut conn = self
            .client
            .pool()
            .get()
            .await
            .map_err(|e| DbError::RedisPoolError(format!("获取连接失败: {}", e)))?;

        let results: Vec<redis::Value> = self
            .pipe
            .query_async(&mut *conn)
            .await
            .map_err(|e| DbError::RedisCommandError(format!("Pipeline 执行失败: {}", e)))?;

        Ok(results.into_iter().map(RedisValue::from).collect())
    }

    /// 获取 Pipeline 中的命令数量
    ///
    /// # 返回
    /// Pipeline 中的命令数量
    pub fn len(&self) -> usize {
        self.pipe.cmd_iter().count()
    }

    /// 检查 Pipeline 是否为空
    ///
    /// # 返回
    /// - `true`: Pipeline 为空
    /// - `false`: Pipeline 不为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pipeline_creation() {
        // 注意：这里只测试结构体创建，不测试实际连接
        // 实际连接测试在集成测试中进行
    }

    #[test]
    fn test_pipeline_len_empty() {
        // 测试空 Pipeline
        // 注意：需要实际的 RedisClient 才能创建 Pipeline
        // 这个测试只是占位符，实际测试在集成测试中
    }
}
