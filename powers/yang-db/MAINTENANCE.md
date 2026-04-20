# yang-db Power 维护说明

## 📋 概述

本文档说明 yang-db Power 的自动维护机制和手动维护流程。

## 🤖 自动化机制

项目已配置自动化 Hooks，当 yang-db 代码修改时会自动提醒更新 Power 文档。

### Hook 1: 同步 yang-db Power 文档

**文件**: `.kiro/hooks/yang-db-power-sync.kiro.hook`

**触发条件**:
- `crates/yang-db/src/*.rs` - 源代码修改
- `crates/yang-db/examples/*.md` - 示例文档修改
- `crates/yang-db/README.md` - 项目文档修改

**自动行为**:
1. 分析代码变更内容
2. 判断是否影响公开 API
3. 如需要，自动更新 `powers/yang-db/POWER.md`
4. 提供更新摘要

### Hook 2: 提醒更新已安装的 Power

**文件**: `.kiro/hooks/yang-db-power-update-reminder.kiro.hook`

**触发条件**:
- `powers/yang-db/POWER.md` - Power 文档修改

**自动行为**:
1. 提醒用户在 Powers UI 中更新
2. 提供详细更新步骤
3. 确保 AI 使用最新文档

## 📝 需要更新的情况

当发生以下变更时，必须更新 Power 文档：

### API 变更
- ✅ 新增公开方法
- ✅ 修改方法签名
- ✅ 删除或废弃 API
- ✅ 修改方法行为

### 配置变更
- ✅ 新增配置选项
- ✅ 修改默认值
- ✅ 删除配置选项

### 错误处理变更
- ✅ 新增错误类型
- ✅ 修改错误消息
- ✅ 改变错误处理方式

### 使用方式变更
- ✅ 修改推荐模式
- ✅ 更新最佳实践
- ✅ 改变性能建议

## ❌ 不需要更新的情况

- 内部实现优化（不影响 API）
- 代码注释修改
- 测试代码修改
- 文档格式调整
- 性能优化（不改变用法）

## 🔧 手动更新流程

如果需要手动更新：

### 1. 分析变更
```bash
git diff HEAD~1 crates/yang-db/src/
```

### 2. 更新 POWER.md
打开 `powers/yang-db/POWER.md`，更新相关章节

### 3. 验证更新
- ✅ 代码示例正确
- ✅ API 参考一致
- ✅ 错误类型准确

### 4. 提交变更
```bash
git add powers/yang-db/POWER.md
git commit -m "docs: 更新 yang-db Power 文档"
```

### 5. 更新已安装的 Power
在 Powers UI 中：
1. 进入 "Installed Powers"
2. 找到 "YANG-DB 数据库操作库"
3. 点击 "Check for Updates"
4. 点击 "Update Power"

## 📚 相关文档

- **详细维护指南**: `.kiro/steering/yang-db-power-maintenance.md`
- **Power 文档**: `powers/yang-db/POWER.md`
- **使用说明**: `powers/yang-db/README.md`

## ✅ 更新检查清单

- [ ] 所有新增 API 已记录
- [ ] 所有修改的 API 已更新
- [ ] 代码示例可运行
- [ ] 错误类型准确
- [ ] 最佳实践最新
- [ ] 性能建议有效

## 🆘 常见问题

**Q: Hook 没有触发？**
A: 检查 Hook 是否启用，文件路径是否匹配

**Q: 如何禁用 Hook？**
A: 在 Agent Hooks 视图中点击禁用按钮

**Q: AI 还是使用旧文档？**
A: 需要在 Powers UI 中手动更新 Power

---

**维护者**: YANG Team  
**最后更新**: 2026-04-18
