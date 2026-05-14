# testcontainers 0.27.3 API 兼容性修复总结

**修复日期**: 2026-04-26  
**问题**: testcontainers 0.27.3 API 变更导致测试编译失败

---

## 问题描述

更新依赖后,testcontainers 从旧版本升级到 0.27.3,API 发生了重大变更:

1. **移除了 `clients::Cli`**: 不再需要 Docker 客户端实例
2. **移除了同步 `Container` 类型**: 需要使用异步 `ContainerAsync`
3. **`with_env_var` 需要导入 `ImageExt` trait**: 方法从 trait 提供
4. **`with_wait_for` 方法签名变更**: 需要使用 `RunnableImage` 或直接在 `GenericImage` 上调用

---

## 修复方案

### 1. 更新 import 语句

**旧代码**:
```rust
use testcontainers::{clients::Cli, core::WaitFor, GenericImage};
```

**新代码**:
```rust
use testcontainers::{core::WaitFor, runners::AsyncRunner, GenericImage, ImageExt};
```

### 2. 重构容器创建函数

**旧代码**:
```rust
fn create_mysql_container(docker: &Cli) -> Option<testcontainers::Container<'_, GenericImage>> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db")
        .with_wait_for(WaitFor::message_on_stderr(
            "port: 3306  MySQL Community Server",
        ));

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| docker.run(mysql_image))).ok()
}

fn get_db_url(container: &testcontainers::Container<'_, GenericImage>) -> String {
    let port = container.get_host_port_ipv4(3306);
    format!("mysql://root:test_password@127.0.0.1:{}/test_db", port)
}
```

**新代码**:
```rust
async fn setup_mysql() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    let container = match mysql_image.start().await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试：无法启动 Docker 容器: {}", e);
            return None;
        }
    };

    let port = container.get_host_port_ipv4(3306).await.ok()?;
    let db_url = format!("mysql://root:test_password@127.0.0.1:{}/test_db", port);

    if !wait_for_mysql(&db_url, 15).await {
        println!("跳过测试：MySQL 容器启动超时");
        return None;
    }

    Some((container, db_url))
}
```

**关键变更**:
- 函数变为 `async`
- 不再需要 `Cli` 参数
- 使用 `start().await` 代替 `docker.run()`
- 返回 `ContainerAsync` 和 `db_url` 的元组
- `get_host_port_ipv4` 变为异步方法
- 移除 `with_wait_for`,改为手动等待 MySQL 就绪

### 3. 更新测试函数调用

**旧代码**:
```rust
let docker = Cli::default();
let container = match create_mysql_container(&docker) {
    Some(c) => c,
    None => {
        println!("跳过测试：Docker 不可用");
        return;
    }
};
let db_url = get_db_url(&container);

if !wait_for_mysql(&db_url, 15).await {
    println!("跳过测试：MySQL 容器启动失败");
    return;
}
```

**新代码**:
```rust
let (_container, db_url) = match setup_mysql().await {
    Some(setup) => setup,
    None => return,
};
```

---

## 修复的文件

1. ✅ `crates/yang-base/tests/database_integration_test.rs`
2. ✅ `crates/yang-base/tests/table_query_paginate_test.rs`
3. ⏳ `crates/yang-base/tests/table_query_crud_test.rs` (待修复)

---

## 验证结果

修复后需要运行以下命令验证:

```bash
# 编译检查
cargo check --tests

# Clippy 检查
cargo clippy --all-targets --all-features

# 运行测试(需要 Docker)
cargo test --test database_integration_test -- --ignored
cargo test --test table_query_paginate_test -- --ignored
cargo test --test table_query_crud_test -- --ignored
```

---

## 注意事项

1. **Docker 依赖**: 所有测试都需要 Docker 环境,使用 `#[ignore]` 标记
2. **异步测试**: 所有测试函数都是 `async fn`,使用 `#[tokio::test]`
3. **容器生命周期**: `ContainerAsync` 在 drop 时自动清理容器
4. **手动等待**: 由于移除了 `with_wait_for`,需要手动实现 `wait_for_mysql` 函数

---

## 相关文档

- [testcontainers-rs 文档](https://docs.rs/testcontainers/0.27.3/testcontainers/)
- [testcontainers-rs GitHub](https://github.com/testcontainers/testcontainers-rs)
- [迁移指南](https://github.com/testcontainers/testcontainers-rs/blob/main/CHANGELOG.md)
