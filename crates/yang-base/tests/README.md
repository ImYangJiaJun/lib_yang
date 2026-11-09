# yang-base 集成测试

本目录包含 yang-base 的集成测试，包括数据库管理、插件管理等功能的测试。

## 测试文件

- 契约与基础测试：`admin_metadata_contract.rs`、`compatibility_contract.rs`、`release_candidate_contract.rs`、`release_docs_contract.rs`、`error_test.rs`、`field_type_test.rs`、`observability_test.rs`、`plugin_test.rs`、`schema_snapshots.rs`
- MySQL testcontainers（需要 Docker）：`table_query_crud_test.rs`、`table_query_paginate_test.rs`、`table_query_transaction_test.rs`、`schema_sync_integration_test.rs`、`typed_action_integration.rs`
- PostgreSQL 集成（`postgres` feature，需要 `PG_TEST_URL`；仅部分测试额外接受 `YANG_POSTGRES_URL` 作为回退）：`integration_pg_wave*.rs`、`integration_record_pg.rs`、`integration_schema_sync_pg*.rs`、`integration_table_query_pg*.rs`
- Redis 集成（`redis` feature，可选设置 `REDIS_TEST_URL` 复用外部实例；`cache_integration.rs` 未设置时连接本地 `redis://127.0.0.1:6379`（连接失败即失败），其余测试启动 Docker Redis）：`cache_integration.rs`、`token_revocation_integration.rs`、`email_verification_integration.rs`
- Axum 传输（`transport-axum` feature）：`transport_axum.rs`、`transport_inbound_observability.rs`
- `database_test.rs` - 数据库基础功能测试（需要本地 MySQL，`#[ignore]`）

## 运行测试

### 1. 运行所有测试

默认 `cargo test` 需要可达的 Redis（`REDIS_TEST_URL` 或 `redis://127.0.0.1:6379`）；需要 Docker 的测试均标记 `#[ignore]`，默认不执行。

```bash
cargo test
```

### 2. 运行数据库集成测试（需要 Docker）

数据库集成测试使用 testcontainers 创建隔离的 MySQL 测试环境，需要 Docker 环境。

**前置条件**：
- 安装 Docker Desktop 或 Docker Engine
- 确保 Docker 服务正在运行

**运行测试**：

```bash
# 运行所有 MySQL 容器化测试（typed_action_integration 同时需要 Docker Redis）
cargo test --test table_query_crud_test -- --ignored --test-threads=1
cargo test --test table_query_paginate_test -- --ignored --test-threads=1
cargo test --test table_query_transaction_test -- --ignored --test-threads=1
cargo test --test schema_sync_integration_test -- --ignored --test-threads=1
cargo test --test typed_action_integration -- --ignored --test-threads=1

# 运行特定的测试
cargo test --test table_query_crud_test test_crud_complete_flow -- --ignored
```

**注意**：
- 使用 `--ignored` 标志来运行被标记为 `#[ignore]` 的测试
- 使用 `--test-threads=1` 确保测试串行执行，避免端口冲突
- 首次运行会下载 MySQL Docker 镜像，可能需要较长时间

### 3. 运行 PostgreSQL / Redis 集成测试

PostgreSQL 测试需要设置 `PG_TEST_URL`（仅部分测试额外接受 `YANG_POSTGRES_URL` 作为回退）并启用 `postgres` feature；Redis 测试使用 `REDIS_TEST_URL`，`cache_integration` 未设置时回退到 `redis://127.0.0.1:6379`：

```bash
# 运行特定 PostgreSQL 集成测试
PG_TEST_URL=postgres://postgres:postgres@localhost:5432/test \
  cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave2_read -- --ignored --test-threads=1

# 运行 Redis 集成测试
REDIS_TEST_URL=redis://127.0.0.1:6379 cargo test --test token_revocation_integration -- --ignored --test-threads=1
```

## 故障排除

### Docker 相关问题

**问题**：测试提示 "Docker 不可用"

**解决方案**：
1. 确保 Docker Desktop 或 Docker Engine 已安装并运行
2. 检查 Docker 服务状态：`docker ps`
3. 确保当前用户有权限访问 Docker

**问题**：测试提示 "MySQL 容器启动失败"

**解决方案**：
1. 检查 Docker 镜像是否可以正常拉取：`docker pull mysql:8.0`
2. 检查端口 3306 是否被占用
3. 增加等待时间（修改测试代码中的 `max_retries` 参数）

### 数据库连接问题

**问题**：连接超时或连接失败

**解决方案**：
1. 检查数据库服务是否正常运行
2. 验证连接字符串是否正确
3. 检查防火墙设置
4. 增加连接超时时间

## 开发指南

### 添加新的集成测试

1. 在测试函数上添加 `#[tokio::test]` 和 `#[ignore]` 属性
2. 参考现有测试文件中的 testcontainers 辅助函数（`GenericImage::new("mysql", "8.0")` + `wait_for_mysql`）创建测试数据库
3. 编写测试逻辑
4. 在测试注释中标注验证的需求编号

示例：

```rust
/// 测试新功能
///
/// **验证需求**: X.X, Y.Y
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_new_feature() {
    // 参考现有测试文件中的 setup_mysql / wait_for_mysql 辅助函数
    let (_container, db_url) = setup_mysql().await.expect("需要 Docker 环境");
    
    // 测试逻辑
    let db = Database::connect(&db_url).await.unwrap();
    // ...
}
```

### 测试最佳实践

1. **隔离性**：每个测试应该独立，不依赖其他测试的状态
2. **清理**：测试容器会自动清理，无需手动删除
3. **命名**：测试函数名应清晰描述测试内容
4. **文档**：添加注释说明测试验证的需求
5. **断言**：使用有意义的断言消息

## 持续集成

在 CI/CD 环境中运行测试：

```yaml
# GitHub Actions 示例
- name: Run integration tests
  run: |
    # 启动 Docker 服务
    sudo systemctl start docker
    
    # 运行测试
    cargo test --test table_query_crud_test -- --ignored --test-threads=1
```

## 参考资料

- [testcontainers-rs 文档](https://docs.rs/testcontainers/)
- [yang-db 文档](../../yang-db/README.md)
