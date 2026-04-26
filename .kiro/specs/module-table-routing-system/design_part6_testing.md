# 设计文档：模块表路由系统 - 第6部分：正确性属性与测试

## 11. 正确性属性

### 11.1 类型安全属性

**属性 1：Action 路由类型安全**
```
∀ module ∈ ModuleRouter, action_name ∈ String:
  module.dispatch(action_name, context) 返回 Result<ApiResponse, BaseError>
  ⟹ 编译期保证类型安全，无运行时类型转换错误
```

**属性 2：字段配置完整性**
```
∀ config ∈ TableConfig, field_name ∈ config.fields:
  config.validate_field(field_name) = Ok(())
  ∧ config.get_field(field_name) ≠ None
```

### 11.2 权限控制属性

**属性 3：权限检查完整性**
```
∀ action ∈ Action, context ∈ ActionContext:
  action.execute(context) 成功
  ⟹ ∀ permission ∈ action.permissions():
      context.user.has_permission(permission) = true
```

**属性 4：字段级权限隔离**
```
∀ field ∈ FieldConfig, user ∈ User:
  field.permissions.can_read(user.roles) = false
  ⟹ 查询结果中不包含该字段
```

### 11.3 数据验证属性

**属性 5：字段验证完整性**
```
∀ field ∈ FieldConfig, value ∈ Value:
  field.validate(value) = Ok(())
  ⟹ field.field_type.validate(value) = Ok()
    ∧ ∀ validator ∈ field.validators:
        validator.validate(field.name, value) = Ok()
```

**属性 6：必填字段检查**
```
∀ field ∈ FieldConfig:
  field.required = true ∧ value = null
  ⟹ field.validate(value) = Err(FieldRequired)
```

### 11.4 查询构建属性

**属性 7：查询参数验证**
```
∀ config ∈ TableConfig, params ∈ QueryParams:
  config.validate_query(params) = Ok(())
  ⟹ ∀ field ∈ params.fields ∪ params.where_fields ∪ params.order_fields:
      config.validate_field(field) = Ok()
```

**属性 8：软删除一致性**
```
∀ config ∈ TableConfig:
  config.soft_delete_field = Some(field)
  ⟹ TableQuery::delete() 执行 UPDATE 而非 DELETE
```

## 12. 测试策略

### 12.1 单元测试

#### 12.1.1 TableConfig 测试

```rust
#[cfg(test)]
mod table_config_tests {
    use super::*;
    
    #[test]
    fn test_field_validation() {
        let field = FieldConfig::new("name", FieldType::String { max_length: 50 })
            .required(true)
            .validator(Validator::MinLength(3));
        
        // 测试必填验证
        assert!(field.validate(&serde_json::Value::Null).is_err());
        
        // 测试长度验证
        assert!(field.validate(&serde_json::json!("ab")).is_err());
        assert!(field.validate(&serde_json::json!("abc")).is_ok());
        
        // 测试最大长度
        let long_str = "a".repeat(51);
        assert!(field.validate(&serde_json::json!(long_str)).is_err());
    }
    
    #[test]
    fn test_enum_validation() {
        let field = FieldConfig::new("status", FieldType::Enum {
            values: vec!["active".to_string(), "inactive".to_string()]
        });
        
        assert!(field.validate(&serde_json::json!("active")).is_ok());
        assert!(field.validate(&serde_json::json!("invalid")).is_err());
    }
    
    #[test]
    fn test_table_config_validation() {
        let config = TableConfig::new("users")
            .field(FieldConfig::new("id", FieldType::Integer))
            .field(FieldConfig::new("name", FieldType::String { max_length: 50 }));
        
        // 测试字段存在性
        assert!(config.validate_field("id").is_ok());
        assert!(config.validate_field("name").is_ok());
        assert!(config.validate_field("invalid").is_err());
    }
}
```

#### 12.1.2 权限测试

```rust
#[cfg(test)]
mod permission_tests {
    use super::*;
    
    #[test]
    fn test_field_permissions() {
        let permissions = FieldPermissions {
            readable_roles: vec!["admin".to_string()],
            writable_roles: vec!["admin".to_string()],
            ..Default::default()
        };
        
        let admin_roles = vec!["admin".to_string()];
        let user_roles = vec!["user".to_string()];
        
        assert!(permissions.can_read(&admin_roles));
        assert!(!permissions.can_read(&user_roles));
        assert!(permissions.can_write(&admin_roles));
        assert!(!permissions.can_write(&user_roles));
    }
    
    #[test]
    fn test_user_permissions() {
        let user = User {
            id: 1,
            username: "test".to_string(),
            nickname: "测试用户".to_string(),
            email: None,
            roles: vec!["admin".to_string()],
            permissions: vec!["user.read".to_string(), "user.write".to_string()],
        };
        
        let permission = Permission::new("user.read");
        assert!(user.has_permission(&permission));
        
        let invalid_permission = Permission::new("user.delete");
        assert!(!user.has_permission(&invalid_permission));
    }
}
```

### 12.2 集成测试

#### 12.2.1 完整流程测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_crud_workflow() -> Result<(), BaseError> {
        // 初始化测试数据库
        GlobalDatabase::init("mysql://root:password@localhost/test_db", DatabaseConfig::default()).await?;
        
        // 创建表配置
        let config = Arc::new(
            TableConfig::new("test_users")
                .field(FieldConfig::new("id", FieldType::Integer).required(true))
                .field(FieldConfig::new("name", FieldType::String { max_length: 50 }).required(true))
        );
        
        let user_roles = vec!["admin".to_string()];
        
        // 测试插入
        let insert_data = serde_json::json!({
            "name": "测试用户"
        });
        let affected = TableQuery::new(config.clone(), user_roles.clone())?
            .insert(insert_data)
            .await?;
        assert_eq!(affected, 1);
        
        // 测试查询
        let results = TableQuery::new(config.clone(), user_roles.clone())?
            .where_eq("name".to_string(), serde_json::json!("测试用户"))?
            .select::<serde_json::Value>()
            .await?;
        assert_eq!(results.len(), 1);
        
        // 测试更新
        let update_data = serde_json::json!({
            "name": "更新后的用户"
        });
        let affected = TableQuery::new(config.clone(), user_roles.clone())?
            .where_eq("id".to_string(), serde_json::json!(1))?
            .update(update_data)
            .await?;
        assert_eq!(affected, 1);
        
        // 测试删除
        let affected = TableQuery::new(config.clone(), user_roles.clone())?
            .where_eq("id".to_string(), serde_json::json!(1))?
            .delete()
            .await?;
        assert_eq!(affected, 1);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_action_dispatch() -> Result<(), BaseError> {
        // 创建模块路由
        let config = Arc::new(TableConfig::new("users"));
        let module = ModuleRouter::new("user")
            .table_config(config.clone())
            .register_builtin_actions();
        
        // 创建上下文
        let request = Request::new(serde_json::json!({
            "data": {
                "name": "测试用户"
            }
        }));
        let tools = Arc::new(GlobalTools::new(TokenManager::new("secret")));
        let context = ActionContext::new(request, tools)
            .with_table_config(config);
        
        // 测试 add action
        let response = module.dispatch("add", context).await?;
        assert_eq!(response.code, 0);
        
        Ok(())
    }
}
```

### 12.3 性能测试

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_query_performance() -> Result<(), BaseError> {
        let config = Arc::new(TableConfig::new("users"));
        let user_roles = vec!["admin".to_string()];
        
        let start = Instant::now();
        
        for _ in 0..1000 {
            let _ = TableQuery::new(config.clone(), user_roles.clone())?
                .where_eq("status".to_string(), serde_json::json!("active"))?
                .select::<serde_json::Value>()
                .await?;
        }
        
        let duration = start.elapsed();
        println!("1000 次查询耗时: {:?}", duration);
        
        // 断言平均每次查询小于 10ms
        assert!(duration.as_millis() / 1000 < 10);
        
        Ok(())
    }
}
```

## 13. 性能考虑

### 13.1 查询优化

1. **字段选择优化**：只查询需要的字段，减少数据传输
2. **索引利用**：根据 TableConfig 的索引配置，自动使用索引
3. **分页查询**：避免一次性加载大量数据
4. **连接池**：使用 yang-db 的连接池管理

### 13.2 缓存策略

```rust
/// 带缓存的查询示例
async fn cached_query_example(
    config: Arc<TableConfig>,
    user_roles: Vec<String>,
    tools: &GlobalTools,
) -> Result<Vec<serde_json::Value>, BaseError> {
    let cache_key = "users:active";
    
    // 尝试从缓存获取
    if let Some(redis) = tools.get_tool::<RedisTools>("redis").await {
        if let Ok(Some(cached)) = redis.get(cache_key).await {
            if let Ok(data) = serde_json::from_str(&cached) {
                return Ok(data);
            }
        }
    }
    
    // 从数据库查询
    let results = TableQuery::new(config, user_roles)?
        .where_eq("status".to_string(), serde_json::json!("active"))?
        .select::<serde_json::Value>()
        .await?;
    
    // 写入缓存
    if let Some(redis) = tools.get_tool::<RedisTools>("redis").await {
        if let Ok(json) = serde_json::to_string(&results) {
            let _ = redis.set(cache_key, &json, 300).await;
        }
    }
    
    Ok(results)
}
```

## 14. 安全考虑

### 14.1 SQL 注入防护

- 使用 yang-db 的参数化查询，避免 SQL 注入
- 所有用户输入都经过验证和转义

### 14.2 权限隔离

- Action 级权限检查
- 字段级权限控制
- 行级权限（通过 WHERE 条件实现）

### 14.3 敏感数据保护

```rust
/// 敏感字段配置示例
let password_field = FieldConfig::new("password", FieldType::String { max_length: 255 })
    .permissions(FieldPermissions {
        readable_roles: vec![], // 任何人都不可读
        writable_roles: vec!["admin".to_string()],
        ..Default::default()
    });
```

## 15. 依赖关系

### 15.1 内部依赖

- `yang-db`：数据库查询构建器
- `yang-base::plugin`：插件系统
- `yang-base::token`：Token 管理
- `yang-base::database`：全局数据库访问

### 15.2 外部依赖

```toml
[dependencies]
# 异步运行时
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 数据库
sqlx = { version = "0.7", features = ["mysql", "runtime-tokio-native-tls"] }

# 错误处理
thiserror = "1.0"

# 日志
log = "0.4"

# 时间
chrono = "0.4"

# Redis（可选）
redis = { version = "0.23", optional = true }

# HTTP 框架（可选）
actix-web = { version = "4.0", optional = true }
```

## 16. 总结

### 16.1 设计优势

1. **类型安全**：充分利用 Rust 类型系统，编译期捕获错误
2. **声明式配置**：通过 TableConfig 声明表结构，减少样板代码
3. **可扩展性**：支持自定义 Action、字段类型、验证器、全局工具
4. **权限控制**：多层次权限控制（Action 级、字段级）
5. **统一接口**：标准化的请求/响应格式

### 16.2 与 scs-api 的对比

| 特性 | scs-api | yang-base 模块系统 |
|------|---------|-------------------|
| Action 路由 | 字符串匹配 | 类型安全的 trait |
| 表配置 | 分散在多个文件 | 集中的 TableConfig |
| 查询构建 | 手动拼接 SQL | yang-db 类型安全构建器 |
| 权限控制 | 运行时检查 | 编译期 + 运行时检查 |
| 错误处理 | 字符串错误码 | 类型化的 BaseError |

### 16.3 后续扩展方向

1. **GraphQL 支持**：基于 TableConfig 自动生成 GraphQL Schema
2. **OpenAPI 文档**：自动生成 API 文档
3. **数据迁移工具**：基于 TableConfig 生成迁移脚本
4. **前端代码生成**：根据 TableConfig 生成 TypeScript 类型定义
5. **审计日志**：自动记录所有数据变更
6. **数据版本控制**：支持数据的版本管理和回滚

### 16.4 实施建议

1. **分阶段实施**：
   - 第一阶段：TableConfig + TableQuery
   - 第二阶段：ModuleRouter + 内置 Actions
   - 第三阶段：权限系统 + GlobalTools
   - 第四阶段：HTTP 集成 + 完整示例

2. **测试驱动**：每个组件都编写完整的单元测试和集成测试

3. **文档完善**：提供详细的 API 文档和使用示例

4. **性能优化**：在实施过程中持续进行性能测试和优化
