# 任务 1.1 修复总结：RedisConfig 连接池参数不生效

## 问题描述

在 `src/redis/client.rs` 的 `connect_with_config` 方法中，虽然代码尝试设置连接池参数，但由于 deadpool-redis 的 Config 结构限制，导致配置无法正确应用。

### 根本原因

deadpool-redis 的 `Config` 结构不允许同时设置 `url` 字段和 `pool` 字段。当我们尝试这样做时：

```rust
let mut pool_config = Config {
    url: Some(url.clone()),
    ..Default::default()
};

pool_config.pool = Some(PoolConfig { ... });
```

会导致错误：`Config: url and connection must not be specified at the same time.`

## 修复方案

使用 `Config::from_url()` 方法创建配置，然后设置 `pool` 字段：

```rust
let mut cfg = Config::from_url(url_str.clone());
cfg.pool = Some(PoolConfig {
    max_size: config.max_connections,
    timeouts: Timeouts {
        wait: Some(Duration::from_secs(config.wait_timeout)),
        create: Some(Duration::from_secs(config.connect_timeout)),
        recycle: Some(Duration::from_secs(config.connect_timeout)),
    },
    ..Default::default()
});
```

## 修改的文件

- `crates/yang-db/src/redis/client.rs` (第 63-92 行)

## 验证结果

所有测试通过：

### test_redis_config.rs (8 个测试)
- ✅ test_config_clone
- ✅ test_default_config
- ✅ test_custom_config
- ✅ test_logging_config
- ✅ test_connect_timeout_config
- ✅ test_concurrent_connection_limit
- ✅ test_wait_timeout_config
- ✅ test_max_connections_config

### test_redis_client.rs (8 个测试)
- ✅ test_redis_connect
- ✅ test_redis_connect_with_config
- ✅ test_redis_pool
- ✅ test_redis_execute_ping
- ✅ test_redis_invalid_command
- ✅ test_redis_execute_set_get
- ✅ test_redis_concurrent_connections
- ✅ test_redis_invalid_url

### 代码质量检查
- ✅ cargo clippy --all-targets -- -D warnings (无警告)
- ✅ cargo fmt --check (代码格式正确)

## 测试覆盖

现有的测试已经充分验证了配置参数的生效：

1. **最大连接数测试** (`test_max_connections_config`)：验证 `max_connections` 参数限制连接池大小
2. **等待超时测试** (`test_wait_timeout_config`)：验证 `wait_timeout` 参数控制等待时间
3. **连接超时测试** (`test_connect_timeout_config`)：验证 `connect_timeout` 参数控制连接超时
4. **并发连接限制测试** (`test_concurrent_connection_limit`)：验证连接池在并发场景下的行为

## 影响范围

此修复仅影响 `connect_with_config` 方法的内部实现，不改变公共 API，因此：
- ✅ 向后兼容
- ✅ 不需要修改现有调用代码
- ✅ 所有现有测试通过

## 结论

Bug 已成功修复，RedisConfig 的所有参数（max_connections、connect_timeout、wait_timeout、enable_logging）现在都能正确应用到连接池配置中。
