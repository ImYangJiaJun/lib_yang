# Redis 操作功能测试指南

## 测试环境

### 本地 Redis 服务器

- **地址**: 127.0.0.1:6379
- **密码**: 无
- **容器名**: Redis
- **平台**: Windows 11 + Docker

## 快速开始

### 1. 启动 Redis 容器

```bash
docker start Redis
```

### 2. 验证 Redis 连接

```bash
# 进入 Redis CLI
docker exec -it Redis redis-cli

# 测试连接
127.0.0.1:6379> PING
PONG

# 退出
127.0.0.1:6379> exit
```

### 3. 运行测试

```bash
# 运行所有测试
cargo test --lib -p yang-db

# 运行 Redis 相关测试
cargo test --lib -p yang-db redis

# 运行特定测试
cargo test --lib -p yang-db test_redis_connection
```

## 手动测试命令

### 进入 Redis CLI

```bash
docker exec -it Redis redis-cli
```

### String 操作测试

```bash
# SET/GET
127.0.0.1:6379> SET test_key "test_value"
OK
127.0.0.1:6379> GET test_key
"test_value"

# SETEX (带过期时间)
127.0.0.1:6379> SETEX session:abc 60 "token_value"
OK
127.0.0.1:6379> TTL session:abc
(integer) 58

# INCR (计数器)
127.0.0.1:6379> INCR page:views
(integer) 1
127.0.0.1:6379> INCR page:views
(integer) 2
```

### Hash 操作测试

```bash
# HSET/HGET
127.0.0.1:6379> HSET user:1 name "张三"
(integer) 1
127.0.0.1:6379> HSET user:1 age 25
(integer) 1
127.0.0.1:6379> HGET user:1 name
"张三"

# HGETALL
127.0.0.1:6379> HGETALL user:1
1) "name"
2) "张三"
3) "age"
4) "25"
```

### List 操作测试

```bash
# LPUSH/RPUSH
127.0.0.1:6379> RPUSH queue:tasks "task1" "task2" "task3"
(integer) 3

# LRANGE
127.0.0.1:6379> LRANGE queue:tasks 0 -1
1) "task1"
2) "task2"
3) "task3"

# LPOP
127.0.0.1:6379> LPOP queue:tasks
"task1"
```

### Set 操作测试

```bash
# SADD
127.0.0.1:6379> SADD article:1:tags "rust" "database" "redis"
(integer) 3

# SMEMBERS
127.0.0.1:6379> SMEMBERS article:1:tags
1) "rust"
2) "database"
3) "redis"

# SISMEMBER
127.0.0.1:6379> SISMEMBER article:1:tags "rust"
(integer) 1
```

### Sorted Set 操作测试

```bash
# ZADD
127.0.0.1:6379> ZADD leaderboard 100 "player1" 200 "player2" 150 "player3"
(integer) 3

# ZRANGE (按分数排序)
127.0.0.1:6379> ZRANGE leaderboard 0 -1 WITHSCORES
1) "player1"
2) "100"
3) "player3"
4) "150"
5) "player2"
6) "200"

# ZINCRBY
127.0.0.1:6379> ZINCRBY leaderboard 50 "player1"
"150"
```

## 测试数据清理

### 清空当前数据库

```bash
docker exec -it Redis redis-cli FLUSHDB
```

### 清空所有数据库

```bash
docker exec -it Redis redis-cli FLUSHALL
```

### 删除特定键

```bash
docker exec -it Redis redis-cli DEL test_key user:1 queue:tasks
```

### 查看所有键

```bash
docker exec -it Redis redis-cli KEYS "*"
```

## Docker 管理命令

### 查看容器状态

```bash
docker ps -a | findstr Redis
```

### 启动容器

```bash
docker start Redis
```

### 停止容器

```bash
docker stop Redis
```

### 重启容器

```bash
docker restart Redis
```

### 查看日志

```bash
docker logs Redis
docker logs -f Redis  # 实时查看
```

### 查看容器信息

```bash
docker inspect Redis
```

## 常见问题

### 1. 连接被拒绝

**问题**: `Connection refused (os error 10061)`

**解决方案**:
```bash
# 检查容器是否运行
docker ps | findstr Redis

# 如果未运行，启动容器
docker start Redis
```

### 2. 测试数据残留

**问题**: 测试失败因为数据已存在

**解决方案**:
```bash
# 清空测试数据
docker exec -it Redis redis-cli FLUSHDB
```

### 3. 端口被占用

**问题**: 端口 6379 被其他程序占用

**解决方案**:
```bash
# 查看端口占用
netstat -ano | findstr :6379

# 停止占用端口的进程或更改 Redis 端口
```

## 性能测试

### 使用 redis-benchmark

```bash
# 进入容器
docker exec -it Redis bash

# 运行基准测试
redis-benchmark -h 127.0.0.1 -p 6379 -n 10000 -c 50

# 测试特定命令
redis-benchmark -h 127.0.0.1 -p 6379 -t set,get -n 10000 -q
```

## 集成测试示例

### 基本连接测试

```rust
#[tokio::test]
async fn test_redis_connection() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("Failed to connect to Redis");
    
    // 测试 PING
    let result = client.execute(&redis::cmd("PING")).await;
    assert!(result.is_ok());
}
```

### String 操作测试

```rust
#[tokio::test]
async fn test_string_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .unwrap();
    
    // SET
    client.set("test_key", "test_value").await.unwrap();
    
    // GET
    let value = client.get("test_key").await.unwrap();
    assert_eq!(value.unwrap().as_string().unwrap(), "test_value");
    
    // 清理
    client.del(vec!["test_key"]).await.unwrap();
}
```

## 测试最佳实践

1. **每个测试独立**: 使用唯一的键名避免冲突
2. **清理测试数据**: 测试结束后删除创建的键
3. **使用前缀**: 测试键使用 `test:` 前缀便于识别
4. **并发测试**: 使用不同的键名避免竞争条件
5. **错误处理**: 测试错误情况和边界条件

### 测试键命名规范

```rust
// 好的做法
let test_key = format!("test:{}:key", uuid::Uuid::new_v4());

// 避免硬编码
// let test_key = "test_key";  // 可能与其他测试冲突
```

---

**文档版本**: 1.0.0  
**创建日期**: 2026-04-25  
**最后更新**: 2026-04-25
