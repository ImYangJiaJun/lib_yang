# Redis Lua 脚本支持

## 概述

yang-db 提供了对 Redis Lua 脚本的完整支持，基于 redis-rs 的原生 `Script` 类型实现。Lua 脚本允许在 Redis 服务器端执行复杂的原子操作，提高性能并保证操作的原子性。

## 主要特性

- **原子性执行**：脚本内的所有操作都是原子性的，不会被其他客户端的命令中断
- **性能优化**：自动使用 EVALSHA 命令缓存脚本，减少网络传输
- **类型安全**：支持类型化的返回值，利用 Rust 的类型系统
- **自动回退**：如果脚本不在缓存中，自动回退到 EVAL 命令

## 基本用法

### 创建和执行脚本

```rust
use yang_db::RedisClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
    
    // 创建脚本
    let script = client.script(r#"return "Hello, Lua!""#);
    
    // 执行脚本
    let result: String = client.eval_script(&script, &[], &[]).await?;
    println!("结果: {}", result);
    
    Ok(())
}
```

### 使用 KEYS 和 ARGV 参数

```rust
// 创建脚本：使用 KEYS 和 ARGV 参数
let script = client.script(
    r#"
    redis.call('SET', KEYS[1], ARGV[1])
    return redis.call('GET', KEYS[1])
    "#
);

// 执行脚本
let result: String = client.eval_script(
    &script,
    &["my_key".to_string()],      // KEYS 参数
    &["my_value".to_string()]     // ARGV 参数
).await?;
```

## 实用示例

### 1. 原子性计数器增加

```rust
let script = client.script(
    r#"
    local current = redis.call('GET', KEYS[1])
    if not current then
        current = 0
    end
    local new_value = tonumber(current) + tonumber(ARGV[1])
    redis.call('SET', KEYS[1], new_value)
    return new_value
    "#
);

let result: i64 = client.eval_script(
    &script,
    &["counter".to_string()],
    &["10".to_string()]
).await?;

println!("新值: {}", result);
```

### 2. 条件更新（乐观锁）

```rust
// 只有当余额足够时才扣减
let script = client.script(
    r#"
    local balance = tonumber(redis.call('GET', KEYS[1]) or 0)
    local amount = tonumber(ARGV[1])
    if balance >= amount then
        redis.call('DECRBY', KEYS[1], amount)
        return 1
    else
        return 0
    end
    "#
);

let success: i64 = client.eval_script(
    &script,
    &["user:1000:balance".to_string()],
    &["100".to_string()]
).await?;

if success == 1 {
    println!("扣款成功");
} else {
    println!("余额不足");
}
```

### 3. 原子性转账

```rust
// 原子性地从一个账户转账到另一个账户
let script = client.script(
    r#"
    local from_balance = tonumber(redis.call('GET', KEYS[1]))
    local amount = tonumber(ARGV[1])
    
    if from_balance >= amount then
        redis.call('DECRBY', KEYS[1], amount)
        redis.call('INCRBY', KEYS[2], amount)
        return 1
    else
        return 0
    end
    "#
);

let success: i64 = client.eval_script(
    &script,
    &["account_a".to_string(), "account_b".to_string()],
    &["30".to_string()]
).await?;
```

### 4. 批量操作

```rust
// 批量获取多个键的值
let script = client.script(
    r#"
    local result = {}
    for i = 1, #KEYS do
        table.insert(result, redis.call('GET', KEYS[i]))
    end
    return result
    "#
);

let result: Vec<String> = client.eval_script(
    &script,
    &["key1".to_string(), "key2".to_string(), "key3".to_string()],
    &[]
).await?;
```

## 返回值类型

Lua 脚本支持多种返回值类型：

- **字符串**：`String`
- **整数**：`i64`
- **浮点数**：`f64`
- **数组**：`Vec<T>`
- **混合数组**：`Vec<redis::Value>`
- **nil**：`Option<T>`

示例：

```rust
// 返回字符串
let result: String = client.eval_script(&script, &[], &[]).await?;

// 返回整数
let result: i64 = client.eval_script(&script, &[], &[]).await?;

// 返回数组
let result: Vec<String> = client.eval_script(&script, &[], &[]).await?;

// 返回可选值
let result: Option<String> = client.eval_script(&script, &[], &[]).await?;
```

## 性能优化

### EVALSHA 自动缓存

redis-rs 的 `Script` 类型会自动处理脚本缓存：

1. **首次执行**：脚本被发送到 Redis 服务器并缓存，返回 SHA1 哈希值
2. **后续执行**：使用 EVALSHA 命令，只传输 SHA1 哈希值，节省网络带宽
3. **自动回退**：如果脚本不在缓存中（例如 Redis 重启），自动回退到 EVAL 命令

这个过程完全自动化，无需手动管理。

### 最佳实践

1. **复用脚本对象**：对于频繁执行的脚本，创建一次脚本对象并复用
2. **减少网络往返**：将多个操作合并到一个脚本中
3. **避免长时间运行**：Lua 脚本会阻塞 Redis，避免执行耗时操作
4. **使用局部变量**：在 Lua 中使用 `local` 关键字声明变量

## 错误处理

```rust
let result: Result<i64, _> = client.eval_script(
    &script,
    &["key".to_string()],
    &[]
).await;

match result {
    Ok(value) => println!("成功: {}", value),
    Err(e) => eprintln!("脚本执行失败: {}", e),
}
```

## 注意事项

1. **原子性**：脚本内的所有操作都是原子性的，但脚本执行期间会阻塞 Redis
2. **时间复杂度**：注意脚本的时间复杂度，避免在脚本中执行大量操作
3. **键命名**：使用 KEYS 参数传递键名，而不是在脚本中硬编码
4. **参数传递**：使用 ARGV 参数传递值，保持脚本的可复用性
5. **错误处理**：脚本中的错误会导致整个脚本失败，注意处理边界情况

## API 参考

### `RedisClient::script()`

创建 Lua 脚本对象。

```rust
pub fn script(&self, code: &str) -> redis::Script
```

**参数**：
- `code`: Lua 脚本代码

**返回**：
- `redis::Script` 对象

### `RedisClient::eval_script()`

执行 Lua 脚本。

```rust
pub async fn eval_script<T>(
    &self,
    script: &redis::Script,
    keys: &[String],
    args: &[String],
) -> Result<T>
where
    T: redis::FromRedisValue,
```

**参数**：
- `script`: 脚本对象（通过 `script()` 方法创建）
- `keys`: KEYS 参数列表（在脚本中通过 KEYS[1], KEYS[2] 访问）
- `args`: ARGV 参数列表（在脚本中通过 ARGV[1], ARGV[2] 访问）

**返回**：
- `Ok(T)`: 脚本执行成功，返回类型化结果
- `Err(DbError)`: 脚本执行失败

## 测试

项目包含完整的单元测试和集成测试：

```bash
# 运行单元测试
cargo test --test test_redis_script

# 运行集成测试（需要 Docker）
cargo test --test integration_redis_script
```

## 相关资源

- [Redis Lua 脚本官方文档](https://redis.io/docs/manual/programmability/eval-intro/)
- [redis-rs Script 文档](https://docs.rs/redis/latest/redis/struct.Script.html)
- [Lua 5.1 参考手册](https://www.lua.org/manual/5.1/)
