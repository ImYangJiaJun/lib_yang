# YANG-DB Power

这是一个 **Knowledge Base Power**，为 AI 助手提供 yang-db 数据库操作库的完整使用指南。

## 关于 yang-db

yang-db 是一个基于 Rust 的类型安全 MySQL 数据库操作库，提供：

- 链式调用 API
- 类型安全的查询构建
- 参数化查询防止 SQL 注入
- 异步操作支持
- 完整的事务管理
- 特殊字段类型支持（JSON、DATETIME、DECIMAL 等）
- 中文错误消息

## Power 类型

**Knowledge Base Power** - 纯文档型 Power，无需 MCP 服务器配置。

## 如何使用

### 对于 AI 助手

激活这个 Power 以获取 yang-db 的完整 API 参考和使用指南：

```
调用 kiroPowers 工具：
- action: "activate"
- powerName: "yang-db"
```

激活后，AI 助手将获得：
- 完整的 API 参考文档
- 代码示例和最佳实践
- 错误处理指南
- 性能优化建议
- 常见模式和故障排查

### 对于开发者

这个 Power 包含了 yang-db 库的完整使用文档，可以帮助你：

1. **快速上手** - 从连接数据库到执行 CRUD 操作
2. **学习 API** - 详细的 API 参考和参数说明
3. **掌握最佳实践** - 性能优化、安全编码、错误处理
4. **解决问题** - 常见问题和故障排查指南

## 文件结构

```
powers/yang-db/
├── POWER.md       # 主文档（包含完整使用指南）
└── README.md      # 本文件
```

## 主要内容

POWER.md 包含以下章节：

1. **概述** - yang-db 简介和核心特性
2. **快速开始** - 安装、连接、基本 CRUD
3. **核心 API** - 完整的 API 参考
   - 数据库连接
   - 查询构建器
   - WHERE 条件
   - JOIN 操作
   - 排序和分组
   - 分页
4. **查询方法** - find、select、value、count、sum
5. **数据修改** - insert、insert_batch、update、delete
6. **特殊字段类型** - JSON、DATETIME、TIMESTAMP、DECIMAL、BLOB、TEXT
7. **事务管理** - 事务的使用和错误处理
8. **原生 SQL 支持** - 执行原生 SQL 语句
9. **数据库管理** - 创建表、删除表、检查表存在
10. **错误处理** - 错误类型和处理示例
11. **安全特性** - SQL 注入防护、全表更新/删除防护
12. **日志记录** - 启用和使用日志
13. **性能优化建议** - 连接池、批量操作、索引等
14. **常见模式** - 分页查询、条件构建、事务处理
15. **测试建议** - 测试数据库、事务测试
16. **故障排查** - 常见问题和解决方案
17. **最佳实践总结** - 10 条核心最佳实践

## 示例代码

Power 中包含大量实际可运行的代码示例，涵盖：

- 基本 CRUD 操作
- 复杂查询构建
- 事务管理
- 错误处理
- 性能优化
- 测试编写

## 相关资源

- **yang-db 源码**: `crates/yang-db/`
- **示例代码**: `crates/yang-db/examples/`
- **测试代码**: `crates/yang-db/tests/`
- **项目文档**: `crates/yang-db/README.md`

## 贡献

如果发现文档有误或需要补充，请更新 `POWER.md` 文件。

## 许可证

与 yang-db 项目保持一致：MIT OR Apache-2.0
