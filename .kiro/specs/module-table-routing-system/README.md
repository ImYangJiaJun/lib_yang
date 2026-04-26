# 模块表路由系统设计文档

## 📚 文档结构

本目录包含 yang-base 库的模块表路由系统的完整设计文档。

### 主文档

- **[design.md](./design.md)** - 主索引文档，包含快速开始和文档导航

### 详细设计文档（按阅读顺序）

1. **[design_part1_overview.md](./design_part1_overview.md)** - 概述与架构
   - 系统目标和核心功能
   - 系统架构图和数据流
   - Plugin Trait 扩展
   - ModuleRouter 核心设计

2. **[design_part2_table_config.md](./design_part2_table_config.md)** - 表配置系统
   - TableConfig 完整定义
   - FieldConfig 和 FieldType
   - 验证器系统
   - 权限配置

3. **[design_part3_query.md](./design_part3_query.md)** - 统一查询接口
   - TableQuery 查询构建器
   - CRUD 操作实现
   - 分页查询
   - 使用示例

4. **[design_part4_actions.md](./design_part4_actions.md)** - Action 系统
   - Action Trait 定义
   - ActionContext 上下文
   - 6 个内置 Actions
   - 自定义 Action 示例

5. **[design_part5_tools_auth.md](./design_part5_tools_auth.md)** - 全局工具与权限
   - GlobalTools 扩展机制
   - Redis 等工具集成
   - 认证中间件
   - 完整使用示例

6. **[design_part6_testing.md](./design_part6_testing.md)** - 测试与总结
   - 正确性属性
   - 测试策略
   - 性能和安全考虑
   - 实施建议

## 🚀 快速导航

### 我想了解...

- **系统整体架构** → 阅读 [design.md](./design.md) 和 [design_part1_overview.md](./design_part1_overview.md)
- **如何定义表结构** → 阅读 [design_part2_table_config.md](./design_part2_table_config.md)
- **如何查询数据** → 阅读 [design_part3_query.md](./design_part3_query.md)
- **如何创建自定义操作** → 阅读 [design_part4_actions.md](./design_part4_actions.md)
- **如何集成 Redis 等工具** → 阅读 [design_part5_tools_auth.md](./design_part5_tools_auth.md)
- **如何测试** → 阅读 [design_part6_testing.md](./design_part6_testing.md)

## 📖 阅读建议

### 首次阅读
1. 先阅读 [design.md](./design.md) 了解整体概况
2. 按顺序阅读 Part 1-6，建立完整理解
3. 重点关注代码示例和使用场景

### 实施开发
1. 参考 [design_part6_testing.md](./design_part6_testing.md) 的实施路线图
2. 按模块查阅对应的设计文档
3. 参考代码示例进行实现

### 问题排查
1. 查看 [design_part5_tools_auth.md](./design_part5_tools_auth.md) 的错误处理部分
2. 参考 [design_part6_testing.md](./design_part6_testing.md) 的测试用例

## 🎯 核心特性

- ✅ **类型安全**：充分利用 Rust 类型系统
- ✅ **声明式配置**：通过 TableConfig 声明表结构
- ✅ **权限控制**：Action 级和字段级权限
- ✅ **可扩展**：支持自定义 Action、字段类型、验证器
- ✅ **统一接口**：标准化的 API 设计

## 📝 设计原则

1. **类型安全优先**：编译期捕获错误
2. **声明式优于命令式**：减少样板代码
3. **可扩展性**：支持插件化扩展
4. **性能考虑**：查询优化和缓存策略
5. **安全第一**：多层次权限控制

## 🔗 相关资源

- [yang-db 库文档](../../yang-db/README.md)
- [yang-base Plugin 系统](../../yang-base/src/plugin/mod.rs)
- [scs-api 参考实现](../../../scs-api/AGENTS.md)

## 📅 版本历史

- **v1.0.0** (2025-01-XX) - 初始设计文档

## 👥 贡献者

- 设计：AI Assistant
- 审核：待定

---

**注意**：这是设计文档，实际实现可能会根据开发过程中的发现进行调整。
