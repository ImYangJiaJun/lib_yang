use crate::{DbError, RedisClient, RedisValue, Result};
use redis::FromRedisValue;

/// Redis 事务构建器
///
/// 提供类型安全的 Redis 事务操作接口，基于 WATCH/MULTI/EXEC 机制实现乐观锁。
///
/// # 特性
/// - 支持 WATCH 键监视（乐观锁）
/// - 原子性执行所有命令
/// - 自动处理 WATCH 冲突并重试
/// - 支持所有 Redis 命令
///
/// # 示例
/// ```no_run
/// use yang_db::RedisClient;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
///     
///     // 创建事务
///     let mut tx = client.transaction();
///     
///     // 添加命令
///     tx.set("key1", "value1");
///     tx.set("key2", "value2");
///     tx.incr("counter");
///     
///     // 执行事务
///     let results: (String, String, i64) = tx.exec().await?;
///     println!("事务执行结果: {:?}", results);
///     
///     Ok(())
/// }
/// ```
pub struct RedisTransaction {
    /// Redis 客户端引用
    client: RedisClient,
    /// 原生 redis::Pipeline
    pipe: redis::Pipeline,
    /// 要监视的键列表
    watched_keys: Vec<String>,
}

impl RedisTransaction {
    /// 创建新的事务
    ///
    /// # 参数
    /// - `client`: Redis 客户端
    ///
    /// # 返回
    /// 新的事务实例
    pub fn new(client: RedisClient) -> Self {
        let mut pipe = redis::pipe();
        pipe.atomic(); // 设置为原子模式（MULTI/EXEC）

        Self {
            client,
            pipe,
            watched_keys: Vec::new(),
        }
    }

    /// 监视一个或多个键（用于乐观锁）
    ///
    /// # 参数
    /// - `keys`: 要监视的键列表
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    ///
    /// # 注意
    /// - 如果被监视的键在事务执行前被修改，事务将被取消并自动重试
    /// - 必须在添加命令之前调用
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut tx = client.transaction();
    /// tx.watch(&["balance".to_string()]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn watch(&mut self, keys: &[String]) -> &mut Self {
        self.watched_keys.extend_from_slice(keys);
        self
    }

    /// 添加 SET 命令
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 值
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
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

    /// 添加 DECRBY 命令
    ///
    /// # 参数
    /// - `key`: 键
    /// - `decrement`: 减少的数量
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    pub fn decrby(&mut self, key: impl Into<String>, decrement: i64) -> &mut Self {
        self.pipe.decr(key.into(), decrement);
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
    pub fn lpush(&mut self, key: impl Into<String>, values: &[String]) -> &mut Self {
        let key_str = key.into();
        for value in values {
            self.pipe.lpush(&key_str, value);
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
        let key_str = key.into();
        for value in values {
            self.pipe.rpush(&key_str, value);
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
        let key_str = key.into();
        for member in members {
            self.pipe.sadd(&key_str, member);
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
        let key_str = key.into();
        for (score, member) in members {
            self.pipe.zadd(&key_str, member, *score);
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
    pub fn cmd(&mut self, cmd: redis::Cmd) -> &mut Self {
        self.pipe.add_command(cmd);
        self
    }

    /// 执行事务（类型化版本）
    ///
    /// # 类型参数
    /// - `T`: 实现了 `FromRedisValue` 的类型
    ///
    /// # 返回
    /// - `Ok(T)`: 事务执行成功，返回结果
    /// - `Err(DbError)`: 事务执行失败
    ///
    /// # 错误处理
    /// - 如果 WATCH 的键被修改，自动重试（最多 100 次）
    /// - 如果其他错误，直接返回
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut tx = client.transaction();
    /// tx.set("key1", "value1")
    ///   .set("key2", "value2")
    ///   .get("key1");
    ///
    /// // 获取类型化结果
    /// let results: (String, String, String) = tx.exec().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn exec<T: FromRedisValue>(self) -> Result<T> {
        let mut conn = self
            .client
            .pool()
            .get()
            .await
            .map_err(|e| DbError::RedisPoolError(format!("获取连接失败: {}", e)))?;

        // 最大重试次数
        const MAX_RETRIES: usize = 100;
        let mut retries = 0;

        loop {
            // WATCH 指定的键
            if !self.watched_keys.is_empty() {
                let mut watch_cmd = redis::cmd("WATCH");
                for key in &self.watched_keys {
                    watch_cmd.arg(key);
                }
                watch_cmd
                    .query_async::<()>(&mut *conn)
                    .await
                    .map_err(|e| DbError::RedisCommandError(format!("WATCH 命令失败: {}", e)))?;
            }

            // 执行事务
            match self.pipe.query_async::<T>(&mut *conn).await {
                Ok(result) => {
                    return Ok(result);
                }
                Err(e) => {
                    // 检查是否是 WATCH 冲突导致的失败
                    let err_msg = e.to_string();
                    if (err_msg.contains("EXECABORT") || err_msg.contains("nil"))
                        && !self.watched_keys.is_empty()
                    {
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(DbError::RedisCommandError(format!(
                                "事务执行失败，已重试 {} 次: {}",
                                MAX_RETRIES, e
                            )));
                        }
                        // WATCH 冲突，重试
                        continue;
                    } else {
                        // 其他错误，直接返回
                        return Err(DbError::RedisCommandError(format!("事务执行失败: {}", e)));
                    }
                }
            }
        }
    }

    /// 执行事务（兼容模式）
    ///
    /// # 返回
    /// - `Ok(Vec<RedisValue>)`: 事务执行成功，返回结果列表
    /// - `Err(DbError)`: 事务执行失败
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::RedisClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    /// let mut tx = client.transaction();
    /// tx.set("key1", "value1")
    ///   .set("key2", "value2")
    ///   .incr("counter");
    ///
    /// let results = tx.execute().await?;
    /// println!("事务执行结果: {:?}", results);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(self) -> Result<Vec<RedisValue>> {
        let results: Vec<redis::Value> = self.exec().await?;
        Ok(results.into_iter().map(RedisValue::from).collect())
    }

    /// 获取事务中的命令数量
    ///
    /// # 返回
    /// 事务中的命令数量
    pub fn len(&self) -> usize {
        self.pipe.cmd_iter().count()
    }

    /// 检查事务是否为空
    ///
    /// # 返回
    /// - `true`: 事务为空
    /// - `false`: 事务不为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_transaction_creation() {
        // 注意：这里只测试结构体创建，不测试实际连接
        // 实际连接测试在集成测试中进行
    }
}
