# yang-base 集成测试

本目录包含 yang-base 的集成测试，包括数据库管理、插件管理等功能的测试。

## 测试文件

- `database_integration_test.rs` - 数据库管理集成测试（需要 Docker）
- `database_initializer_test.rs` - 数据库初始化器测试（需要手动配置数据库）
- `database_test.rs` - 数据库基础功能测试
- `plugin_test.rs` - 插件管理测试
- `error_test.rs` - 错误处理测试

## 运行测试

### 1. 运行所有测试（不包括需要 Docker 的测试）

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
# 运行所有数据库集成测试
cargo test --test database_integration_test -- --ignored --test-threads=1

# 运行特定的测试
cargo test --test database_integration_test test_global_database_initialization -- --ignored
cargo test --test database_integration_test test_migration_execution_and_idempotency -- --ignored
```

**注意**：
- 使用 `--ignored` 标志来运行被标记为 `#[ignore]` 的测试
- 使用 `--test-threads=1` 确保测试串行执行，避免端口冲突
- 首次运行会下载 MySQL Docker 镜像，可能需要较长时间

### 3. 运行手动数据库测试

某些测试需要手动配置的 MySQL 数据库：

```bash
# 修改测试文件中的数据库连接字符串
# 默认：mysql://root:password@localhost:3306/test_db

# 运行测试
cargo test --test database_initializer_test -- --ignored
```

## 测试覆盖的功能

### 数据库管理集成测试 (database_integration_test.rs)

- ✅ 全局数据库初始化 (需求 6.1, 6.4)
- ✅ 数据库初始化流程 - 非事务模式 (需求 4.1-4.6, 11.2)
- ✅ 数据库初始化流程 - 事务模式 (需求 4.1-4.6, 11.3, 11.4)
- ✅ 迁移记录表创建 (需求 9.2)
- ✅ 迁移执行和幂等性 (需求 9.3, 9.4)
- ✅ 事务回滚测试 (需求 11.3, 11.4)
- ✅ 依赖顺序初始化 (需求 4.2, 4.3)

### 测试场景

1. **全局数据库初始化**
   - 验证全局数据库实例的创建和访问
   - 验证数据库配置的应用

2. **数据库初始化流程**
   - 测试多个插件的数据库表创建
   - 测试插件依赖关系的正确处理
   - 测试迁移脚本的执行

3. **迁移管理**
   - 测试迁移记录表的创建
   - 测试迁移的幂等性（重复执行不会重复应用）
   - 测试迁移版本的记录和查询

4. **事务处理**
   - 测试事务模式下的初始化
   - 测试失败时的事务回滚
   - 验证事务的原子性

5. **依赖管理**
   - 测试插件依赖关系的拓扑排序
   - 验证依赖插件先于当前插件初始化

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
2. 使用 `setup_test_db!` 宏创建测试数据库
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
    let db_url = setup_test_db!(_docker, _container);
    
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
    cargo test --test database_integration_test -- --ignored --test-threads=1
```

## 参考资料

- [testcontainers-rs 文档](https://docs.rs/testcontainers/)
- [yang-db 文档](../yang-db/README.md)
- [设计文档](../.kiro/specs/plugin-management-system/design.md)
- [需求文档](../.kiro/specs/plugin-management-system/requirements.md)
