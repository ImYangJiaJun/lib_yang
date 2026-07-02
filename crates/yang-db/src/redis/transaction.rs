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

/// 判定一次 EXEC 回复是否为 WATCH 冲突（DB-2）。
///
/// 被监视的键在 MULTI/EXEC 之间被改写时，Redis 的 EXEC 返回 `Nil`。只有在确实
/// WATCH 了键（`has_watched_keys`）的前提下，顶层 `Nil` 才判为乐观锁冲突需要重试；
/// 无监视键时 `Nil` 是正常业务结果（如普通 pipeline 里 GET 不存在的键），应透传。
fn is_watch_conflict(raw: &redis::Value, has_watched_keys: bool) -> bool {
    has_watched_keys && matches!(raw, redis::Value::Nil)
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
    /// # 限制（NEW-19：仅支持固定值 CAS，不支持读-改-写）
    ///
    /// 本事务为「构建一次、重试时重放同一批固定命令」模型：命令在 build 期即固定，
    /// `watch` 与 `EXEC` 之间**没有**重新读取被监视键、据此重算后续命令的钩子。因此
    /// 典型的乐观锁「读余额 → 减 100 → 写回」**无法**用本 API 表达——冲突重试只会
    /// 重放同一组固定值，退化为固定值 CAS-set。
    ///
    /// 若需读-改-写循环，请改用 Lua 脚本（`eval_script`，在 Redis 端原子读改写），
    /// 或在业务层手动「读取 → 计算 → `watch` + 固定值事务」并自行重试。
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
    ///
    /// 多元素打包为**单条**命令（NEW-21），保证事务内「每方法调用一条命令一个结果」。
    /// 空切片为 no-op。
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

            // 执行事务：先解码为原始 redis::Value，以便在协议层检测 WATCH 冲突。
            // redis-rs 对 `Vec<T>`/`()` 会把 EXEC 的 Nil 回复无声解码成 Ok(空)，
            // 使 WATCH 冲突被吞掉、乐观锁失效（DB-2）。故先取 Value 判定冲突，再转 T。
            match self.pipe.query_async::<redis::Value>(&mut *conn).await {
                Ok(raw) => {
                    // WATCH 冲突：被监视键在 EXEC 前被改写时，Redis 返回 Nil。
                    // 仅在存在监视键时才把整体 Nil 判为冲突；无监视键时 Nil 正常透传
                    // （如普通 pipeline 中 GET 不存在的键）。
                    if is_watch_conflict(&raw, !self.watched_keys.is_empty()) {
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(DbError::RedisCommandError(format!(
                                "事务执行失败：WATCH 冲突已重试 {} 次",
                                MAX_RETRIES
                            )));
                        }
                        // WATCH 冲突，重试——重新获取连接以避免复用已断开的连接
                        conn = self.client.pool().get().await.map_err(|e| {
                            DbError::RedisPoolError(format!("重试获取连接失败: {}", e))
                        })?;
                        continue;
                    }
                    // 非冲突：解码为调用方期望的类型 T
                    return T::from_redis_value(raw).map_err(|e| {
                        DbError::RedisCommandError(format!("事务结果解码失败: {}", e))
                    });
                }
                Err(e) => {
                    // 检查是否是 WATCH 冲突导致的失败（EXECABORT）
                    // 优先使用枚举匹配，避免依赖错误消息字符串（版本升级/i18n 可能改变文案）
                    let is_exec_abort = matches!(
                        e.kind(),
                        redis::ErrorKind::Server(redis::ServerErrorKind::ExecAbort)
                    ) || e.to_string().contains("EXECABORT");
                    // 仅在存在监视键时才把 EXECABORT 判为 WATCH 冲突；
                    // 无监视键时 EXECABORT 可能源于其他原因（如脚本中止），直接透传错误。
                    if is_exec_abort && !self.watched_keys.is_empty() {
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(DbError::RedisCommandError(format!(
                                "事务执行失败，已重试 {} 次: {}",
                                MAX_RETRIES, e
                            )));
                        }
                        // WATCH 冲突，重试——重新获取连接以避免复用已断开的连接
                        conn = self.client.pool().get().await.map_err(|e| {
                            DbError::RedisPoolError(format!("重试获取连接失败: {}", e))
                        })?;
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
    use super::is_watch_conflict;

    #[test]
    fn test_transaction_creation() {
        // 注意：这里只测试结构体创建，不测试实际连接
        // 实际连接测试在集成测试中进行
    }

    /// DB-2：有监视键且 EXEC 返回 Nil 判为冲突；无监视键时 Nil 透传不重试。
    #[test]
    fn test_is_watch_conflict() {
        // 有监视键 + Nil → 冲突（应重试）
        assert!(is_watch_conflict(&redis::Value::Nil, true));
        // 无监视键 + Nil → 非冲突（业务结果，透传）
        assert!(!is_watch_conflict(&redis::Value::Nil, false));
        // 有监视键但非 Nil（成功结果）→ 非冲突
        assert!(!is_watch_conflict(&redis::Value::Int(1), true));
        assert!(!is_watch_conflict(
            &redis::Value::Array(vec![redis::Value::Int(1)]),
            true
        ));
    }
}
