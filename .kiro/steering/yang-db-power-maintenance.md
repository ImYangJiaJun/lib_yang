---
inclusion: manual
---

# yang-db Power 维护指南

本文档说明如何维护 yang-db Power，确保文档与代码保持同步。

## 自动化机制

项目已配置两个 Hook 来自动化 Power 文档的维护：

### Hook 1: 同步 yang-db Power 文档 (yang-db-power-sync)

**触发条件**：当以下文件被修改时
- `crates/yang-db/src/*.rs` - yang-db 源代码
- `crates/yang-db/examples/*.md` - 示例文档
- `crates/yang-db/README.md` - 项目文档

**自动行为**：
- AI 助手会自动分析代码变更
- 判断是否影响公开 API 或用法
- 如果需要，自动更新 `powers/yang-db/POWER.md`
- 提供更新摘要

### Hook 2: 提醒更新 yang-db Power (yang-db-power-update-reminder)

**触发条件**：当 `powers/yang-db/POWER.md` 被修改时

**自动行为**：
- 提醒用户在 Powers UI 中更新已安装的 Power
- 提供详细的更新步骤
- 确保 AI 助手使用最新文档

## 需要更新 Power 文档的情况

当 yang-db 代码发生以下变更时，必须更新 Power 文档：

### 1. API 变更
- ✅ 新增公开方法或函数
- ✅ 修改方法签名（参数、返回值）
- ✅ 删除或废弃 API
- ✅ 修改方法行为或语义

### 2. 配置变更
- ✅ 新增配置选项
- ✅ 修改默认值
- ✅ 删除配置选项

### 3. 错误处理变更
- ✅ 新增错误类型
- ✅ 修改错误消息
- ✅ 改变错误处理方式

### 4. 使用方式变更
- ✅ 修改推荐的使用模式
- ✅ 更新最佳实践
- ✅ 改变性能优化建议

### 5. 示例代码变更
- ✅ 修改示例文档
- ✅ 更新代码示例
- ✅ 添加新的使用场景

## 不需要更新的情况

以下变更通常不需要更新 Power 文档：

- ❌ 内部实现优化（不影响公开 API）
- ❌ 代码注释修改
- ❌ 测试代码修改
- ❌ 文档格式调整（不涉及内容）
- ❌ 性能优化（不改变用法）

## 手动更新流程

如果需要手动更新 Power 文档：

### 1. 分析变更
```bash
# 查看最近的代码变更
git diff HEAD~1 crates/yang-db/src/

# 或查看特定文件的变更
git diff HEAD~1 crates/yang-db/src/query_builder.rs
```

### 2. 更新 POWER.md

打开 `powers/yang-db/POWER.md`，更新相关章节：

- **核心 API** - 如果 API 有变更
- **代码示例** - 如果用法有变化
- **错误处理** - 如果错误类型有变更
- **最佳实践** - 如果推荐做法有更新
- **性能优化建议** - 如果优化策略有变化

### 3. 验证更新

- ✅ 检查所有代码示例是否正确
- ✅ 确保 API 参考与实际代码一致
- ✅ 验证错误类型和消息准确
- ✅ 测试示例代码可运行

### 4. 提交变更

```bash
git add powers/yang-db/POWER.md
git commit -m "docs: 更新 yang-db Power 文档 - [简要说明变更内容]"
```

### 5. 更新已安装的 Power

如果 Power 已安装，需要在 Powers UI 中更新：

1. 打开 Powers UI（侧边栏 Powers 图标）
2. 进入 "Installed Powers" 标签
3. 找到 "YANG-DB 数据库操作库"
4. 点击进入详情页
5. 点击 "Check for Updates"
6. 点击 "Update Power"

## Power 文档结构

`powers/yang-db/POWER.md` 包含以下章节：

1. **Frontmatter** - 元数据（name, displayName, description, keywords, author）
2. **概述** - yang-db 简介和核心特性
3. **快速开始** - 安装、连接、基本 CRUD
4. **核心 API** - 完整的 API 参考
5. **查询方法** - find、select、value、count、sum
6. **数据修改** - insert、insert_batch、update、delete
7. **特殊字段类型** - JSON、DATETIME、TIMESTAMP 等
8. **事务管理** - 事务使用和错误处理
9. **原生 SQL 支持** - 执行原生 SQL
10. **数据库管理** - 创建表、删除表等
11. **错误处理** - 错误类型和处理示例
12. **安全特性** - SQL 注入防护等
13. **日志记录** - 启用和使用日志
14. **性能优化建议** - 优化策略
15. **常见模式** - 分页、条件构建、事务处理
16. **测试建议** - 测试数据库、事务测试
17. **故障排查** - 常见问题和解决方案
18. **最佳实践总结** - 核心最佳实践

## 更新检查清单

在更新 Power 文档后，使用此检查清单验证：

- [ ] 所有新增 API 都已记录
- [ ] 所有修改的 API 都已更新
- [ ] 所有废弃的 API 都已标记或移除
- [ ] 代码示例可以运行且正确
- [ ] 错误类型和消息准确
- [ ] 最佳实践反映最新推荐
- [ ] 性能建议仍然有效
- [ ] 文档内部链接正常
- [ ] 没有拼写或语法错误
- [ ] 版本信息（如果有）已更新

## 常见问题

### Q: Hook 没有触发怎么办？

**A**: 检查以下几点：
1. Hook 是否已启用（在 Agent Hooks 视图中查看）
2. 文件路径是否匹配 Hook 的 filePatterns
3. 尝试手动触发或重新创建 Hook

### Q: 如何临时禁用 Hook？

**A**: 在 Agent Hooks 视图中：
1. 找到对应的 Hook
2. 点击禁用按钮
3. 完成工作后重新启用

### Q: 更新 Power 后 AI 还是使用旧文档？

**A**: 需要在 Powers UI 中手动更新：
1. 打开 Powers UI
2. 找到 yang-db Power
3. 点击 "Check for Updates"
4. 点击 "Update Power"

### Q: 如何验证 Power 文档是否最新？

**A**: 
1. 激活 Power：`kiroPowers` action="activate" powerName="yang-db"
2. 检查返回的文档内容
3. 对比源代码确认一致性

## 相关资源

- **yang-db 源码**: `crates/yang-db/src/`
- **Power 文档**: `powers/yang-db/POWER.md`
- **示例代码**: `crates/yang-db/examples/`
- **项目文档**: `crates/yang-db/README.md`

## 维护责任

- **开发者**: 修改 yang-db 代码时，注意 Hook 提示并更新 Power 文档
- **AI 助手**: 自动检测变更并更新文档，提醒用户更新已安装的 Power
- **用户**: 在 Powers UI 中更新 Power，确保使用最新文档

---

**最后更新**: 2026-04-18
**维护者**: YANG Team
