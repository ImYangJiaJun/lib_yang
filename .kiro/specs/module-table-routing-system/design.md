# 设计文档：模块表路由系统 (Module Table Routing System)

## 文档导航

本设计文档分为 6 个部分，涵盖系统的完整设计：

1. **[第1部分：概述与架构](./design_part1_overview.md)**
   - 系统概述与目标
   - 系统架构图
   - 数据流序列图
   - 组件关系图
   - Plugin Trait 扩展
   - ModuleRouter 结构设计

2. **[第2部分：表配置系统](./design_part2_table_config.md)**
   - TableConfig 结构
   - FieldConfig 结构
   - FieldType 枚举
   - Validator 验证器
   - 权限配置（FieldPermissions、PermissionConfig）
   - 辅助结构（IndexConfig、TimestampFields、RelationConfig）

3. **[第3部分：统一查询接口](./design_part3_query.md)**
   - TableQuery 结构
   - 查询参数结构（QueryParams、WhereCondition）
   - 分页结果结构（PaginatedResult）
   - CRUD 操作实现
   - 使用示例

4. **[第4部分：Action 系统](./design_part4_actions.md)**
   - Action Trait 定义
   - ActionContext 结构
   - Request 和 Response 结构
   - 内置 Actions（Add、Put、Del、Get、Select、Table）
   - 自定义 Action 示例

5. **[第5部分：全局工具与权限认证](./design_part5_tools_auth.md)**
   - GlobalTools 结构
   - 扩展工具示例（Redis、消息队列）
   - User 结构
   - 认证中间件
   - 权限检查流程
   - 错误处理（BaseError 扩展）
   - 完整使用示例

6. **[第6部分：正确性属性与测试](./design_part6_testing.md)**
   - 正确性属性（类型安全、权限控制、数据验证、查询构建）
   - 测试策略（单元测试、集成测试、性能测试）
   - 性能考虑
   - 安全考虑
   - 依赖关系
   - 总结与后续扩展方向

## 快速开始

### 核心概念

```
Plugin（插件）
  └── ModuleRouter（模块路由器）
        ├── TableConfig（表配置）
        │     └── FieldConfig（字段配置）
        └── Actions（操作）
              ├── 内置 Actions（add、put、del、get、select、table）
              └── 自定义 Actions
```

### 最小示例

```rust
use yang_base::plugin::{Plugin, ModuleRouter};
use yang_base::table::{TableConfig, FieldConfig, FieldType};

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn name(&self) -> &str {
        "my_plugin"
    }
    
    fn modules(&self) -> Vec<ModuleRouter> {
        vec![
            ModuleRouter::new("user")
                .table_config(
                    TableConfig::new("users")
                        .field(FieldConfig::new("id", FieldType::Integer))
                        .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
                )
                .register_builtin_actions()
        ]
    }
}
```

## 设计特点

### 1. 类型安全
- 使用 Rust 类型系统替代字符串匹配
- 编译期捕获错误
- 类型化的错误处理

### 2. 声明式配置
- 通过 TableConfig 声明表结构
- 自动生成 CRUD 操作
- 减少样板代码

### 3. 权限控制
- Action 级权限检查
- 字段级权限控制
- 基于角色的访问控制（RBAC）

### 4. 可扩展性
- 支持自定义 Action
- 支持自定义字段类型
- 支持自定义验证器
- 可扩展的全局工具系统

### 5. 统一接口
- 标准化的请求/响应格式
- 统一的错误处理
- 一致的 API 设计

## 技术栈

- **语言**：Rust
- **异步运行时**：Tokio
- **数据库**：MySQL（通过 yang-db）
- **序列化**：serde + serde_json
- **错误处理**：thiserror
- **HTTP 框架**：actix-web（可选）
- **缓存**：Redis（可选）

## 与 scs-api 的对比

| 特性 | scs-api | yang-base 模块系统 |
|------|---------|-------------------|
| Action 路由 | 字符串匹配 | 类型安全的 trait |
| 表配置 | 分散在多个文件 | 集中的 TableConfig |
| 查询构建 | 手动拼接 SQL | yang-db 类型安全构建器 |
| 权限控制 | 运行时检查 | 编译期 + 运行时检查 |
| 错误处理 | 字符串错误码 | 类型化的 BaseError |
| 字段验证 | 手动验证 | 声明式验证器 |

## 实施路线图

### 第一阶段：基础设施
- [ ] TableConfig 实现
- [ ] FieldConfig 实现
- [ ] FieldType 和 Validator 实现
- [ ] 单元测试

### 第二阶段：查询系统
- [ ] TableQuery 实现
- [ ] 查询参数解析
- [ ] CRUD 操作
- [ ] 集成测试

### 第三阶段：路由系统
- [ ] ModuleRouter 实现
- [ ] Action Trait 定义
- [ ] 内置 Actions 实现
- [ ] Action 分发机制

### 第四阶段：权限系统
- [ ] 权限配置实现
- [ ] 认证中间件
- [ ] 权限检查逻辑
- [ ] 字段级权限过滤

### 第五阶段：全局工具
- [ ] GlobalTools 实现
- [ ] Redis 工具集成
- [ ] 其他工具扩展
- [ ] 工具注册机制

### 第六阶段：HTTP 集成
- [ ] HTTP 路由处理
- [ ] 请求解析
- [ ] 响应格式化
- [ ] 完整示例应用

## 贡献指南

1. 阅读完整设计文档
2. 遵循 Rust 编码规范
3. 编写单元测试和集成测试
4. 添加中文文档注释
5. 提交 Pull Request

## 许可证

MIT License

---

**文档版本**：1.0.0  
**创建日期**：2025-01-XX  
**最后更新**：2025-01-XX
