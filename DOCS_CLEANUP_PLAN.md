# lib_yang 项目文档整理方案

**整理日期**: 2026-04-26

---

## 📋 当前文档分布情况

### 根目录文档
```
lib_yang/
├── README.md                    # 项目主文档
└── COMMIT_MESSAGE.md            # 临时文件（可删除）
```

### yang-base 文档（10个）
```
crates/yang-base/
├── README.md                    # 模块主文档
├── INSTALL.md                   # 安装指南
├── USAGE_GUIDE.md               # 使用指南
├── QUICK_REFERENCE.md           # 快速参考
├── ASYNC_AWAIT_GUIDE.md         # 异步编程指南
├── BATCH_FIELD_CONFIG.md        # 批量字段配置
├── TABLE_CONFIG_GUIDE.md        # 表配置指南
├── REDIS_GUIDE.md               # Redis 指南
├── PROJECT_STRUCTURE.md         # 项目结构
├── CLEANUP_PLAN.md              # 清理方案（临时）
└── CLEANUP_SUMMARY.md           # 清理总结（临时）
```

### yang-base 子模块文档（4个）
```
crates/yang-base/src/
├── action/README.md             # Action 系统文档
├── action/ACTION_EXAMPLES.md    # Action 示例
├── action/builtin/README.md     # 内置 Actions 文档
├── database/README.md           # 数据库管理文档
├── http/README.md               # HTTP 客户端文档
└── tests/README.md              # 测试文档
```

### yang-db 文档（5个）
```
crates/yang-db/
├── README.md                    # 模块主文档
├── INSTALL.md                   # 安装指南
├── 测试完善总结.md              # 测试总结（中文）
├── docs/                        # 文档目录
│   ├── integration_tests_summary.md
│   ├── property_tests_summary.md
│   └── update_delete_property_tests.md
└── examples/                    # 示例文档（9个 .md 文件）
    ├── and_or_priority_demo.md
    ├── count_sum_demo.md
    ├── delete_demo.md
    ├── insert_batch_demo.md
    ├── select_method_demo.md
    ├── select_statement_generation.md
    ├── table_alias_demo.md
    ├── update_demo.md
    └── where_clause_verification.md
```

### yang-pcg 文档（1个）
```
crates/yang-pcg/
└── INSTALL.md                   # 安装指南
```

---

## 🎯 整理目标

### 1. 统一文档结构
- 每个 crate 使用统一的文档组织方式
- 将散落的文档集中到 `docs/` 目录
- 保持根目录简洁

### 2. 删除临时文件
- 删除清理相关的临时文档
- 删除提交信息文档

### 3. 规范文档命名
- 使用英文命名
- 使用小写加下划线或连字符

---

## 📁 规整后的目标结构

```
lib_yang/
├── README.md                    # 项目主文档
├── Cargo.toml
├── Cargo.lock
├── .gitignore
├── docs/                        # 📁 新建：项目级文档目录
│   ├── ARCHITECTURE.md          # 架构设计
│   ├── CONTRIBUTING.md          # 贡献指南
│   └── CHANGELOG.md             # 变更日志
├── crates/
│   ├── yang-base/
│   │   ├── README.md            # 模块主文档
│   │   ├── Cargo.toml
│   │   ├── docs/                # 📁 文档目录
│   │   │   ├── guides/          # 📁 使用指南
│   │   │   │   ├── installation.md
│   │   │   │   ├── quick_start.md
│   │   │   │   ├── async_await.md
│   │   │   │   ├── redis.md
│   │   │   │   └── table_config.md
│   │   │   ├── api/             # 📁 API 文档
│   │   │   │   ├── action.md
│   │   │   │   ├── database.md
│   │   │   │   ├── http.md
│   │   │   │   ├── plugin.md
│   │   │   │   ├── router.md
│   │   │   │   ├── table.md
│   │   │   │   └── token.md
│   │   │   ├── examples/        # 📁 示例文档
│   │   │   │   ├── action_examples.md
│   │   │   │   └── batch_field_config.md
│   │   │   └── reference/       # 📁 参考文档
│   │   │       ├── quick_reference.md
│   │   │       └── project_structure.md
│   │   ├── examples/            # Rust 示例代码
│   │   ├── src/
│   │   └── tests/
│   ├── yang-db/
│   │   ├── README.md            # 模块主文档
│   │   ├── Cargo.toml
│   │   ├── docs/                # 📁 文档目录
│   │   │   ├── guides/          # 📁 使用指南
│   │   │   │   └── installation.md
│   │   │   ├── api/             # 📁 API 文档
│   │   │   │   ├── mysql.md
│   │   │   │   └── redis.md
│   │   │   ├── examples/        # 📁 示例文档
│   │   │   │   ├── mysql_examples.md
│   │   │   │   └── redis_examples.md
│   │   │   └── testing/         # 📁 测试文档
│   │   │       ├── integration_tests.md
│   │   │       ├── property_tests.md
│   │   │       └── test_summary.md
│   │   ├── examples/            # Rust 示例代码
│   │   ├── src/
│   │   └── tests/
│   └── yang-pcg/
│       ├── README.md            # 模块主文档
│       ├── Cargo.toml
│       ├── docs/                # 📁 文档目录
│       │   └── guides/
│       │       └── installation.md
│       └── src/
└── .kiro/                       # Kiro 配置（保持不变）
```

---

## 🔧 具体整理步骤

### 步骤 1: 删除临时文件

**删除的文件**:
```bash
# 根目录
lib_yang/COMMIT_MESSAGE.md

# yang-base
crates/yang-base/CLEANUP_PLAN.md
crates/yang-base/CLEANUP_SUMMARY.md
```

### 步骤 2: 创建文档目录结构

**yang-base**:
```bash
mkdir -p crates/yang-base/docs/guides
mkdir -p crates/yang-base/docs/api
mkdir -p crates/yang-base/docs/examples
mkdir -p crates/yang-base/docs/reference
```

**yang-db**:
```bash
mkdir -p crates/yang-db/docs/guides
mkdir -p crates/yang-db/docs/api
mkdir -p crates/yang-db/docs/examples
mkdir -p crates/yang-db/docs/testing
```

**yang-pcg**:
```bash
mkdir -p crates/yang-pcg/docs/guides
```

### 步骤 3: 移动和重命名文档

#### yang-base 文档移动

| 原文件 | 新位置 | 说明 |
|--------|--------|------|
| `INSTALL.md` | `docs/guides/installation.md` | 安装指南 |
| `USAGE_GUIDE.md` | `docs/guides/quick_start.md` | 快速开始 |
| `ASYNC_AWAIT_GUIDE.md` | `docs/guides/async_await.md` | 异步编程 |
| `REDIS_GUIDE.md` | `docs/guides/redis.md` | Redis 指南 |
| `TABLE_CONFIG_GUIDE.md` | `docs/guides/table_config.md` | 表配置 |
| `BATCH_FIELD_CONFIG.md` | `docs/examples/batch_field_config.md` | 批量配置示例 |
| `QUICK_REFERENCE.md` | `docs/reference/quick_reference.md` | 快速参考 |
| `PROJECT_STRUCTURE.md` | `docs/reference/project_structure.md` | 项目结构 |
| `src/action/README.md` | `docs/api/action.md` | Action API |
| `src/action/ACTION_EXAMPLES.md` | `docs/examples/action_examples.md` | Action 示例 |
| `src/action/builtin/README.md` | `docs/api/action_builtin.md` | 内置 Actions |
| `src/database/README.md` | `docs/api/database.md` | 数据库 API |
| `src/http/README.md` | `docs/api/http.md` | HTTP API |

#### yang-db 文档移动

| 原文件 | 新位置 | 说明 |
|--------|--------|------|
| `INSTALL.md` | `docs/guides/installation.md` | 安装指南 |
| `测试完善总结.md` | `docs/testing/test_summary.md` | 测试总结 |
| `docs/integration_tests_summary.md` | `docs/testing/integration_tests.md` | 集成测试 |
| `docs/property_tests_summary.md` | `docs/testing/property_tests.md` | 属性测试 |
| `docs/update_delete_property_tests.md` | `docs/testing/update_delete_tests.md` | 更新删除测试 |
| `examples/*.md` | `docs/examples/mysql_examples.md` | 合并为一个文件 |

#### yang-pcg 文档移动

| 原文件 | 新位置 | 说明 |
|--------|--------|------|
| `INSTALL.md` | `docs/guides/installation.md` | 安装指南 |

### 步骤 4: 更新 README.md

每个 crate 的 README.md 需要更新文档链接，指向新的位置。

### 步骤 5: 创建文档索引

在每个 `docs/` 目录下创建 `README.md` 作为文档索引。

---

## ✅ 整理后的优势

1. **结构清晰**: 文档按类型分类，易于查找
2. **易于维护**: 统一的组织方式，降低维护成本
3. **专业规范**: 符合 Rust 项目的最佳实践
4. **便于扩展**: 新增文档有明确的归属位置

---

## 📝 注意事项

1. **保留 Git 历史**: 使用 `git mv` 移动文件，保留历史记录
2. **更新链接**: 移动文档后需要更新所有内部链接
3. **测试链接**: 确保所有文档链接可用
4. **更新 CI**: 如果有文档检查的 CI，需要更新路径

---

## 🚀 执行计划

### 阶段 1: 准备（5分钟）
- [ ] 创建文档目录结构
- [ ] 备份当前文档

### 阶段 2: 移动文档（15分钟）
- [ ] 移动 yang-base 文档
- [ ] 移动 yang-db 文档
- [ ] 移动 yang-pcg 文档

### 阶段 3: 清理（5分钟）
- [ ] 删除临时文件
- [ ] 删除空目录

### 阶段 4: 更新（10分钟）
- [ ] 更新 README.md 链接
- [ ] 创建文档索引
- [ ] 更新内部链接

### 阶段 5: 验证（5分钟）
- [ ] 检查所有链接
- [ ] 编译文档
- [ ] 提交更改

**预计总时间**: 40分钟

---

## 📊 整理效果预期

### 文档数量
- **整理前**: 散落在各处的 30+ 个文档
- **整理后**: 结构化组织的 30+ 个文档

### 目录层级
- **整理前**: 最多 2 层（crate/文档）
- **整理后**: 最多 4 层（crate/docs/类型/文档）

### 查找效率
- **整理前**: 需要在多个位置查找
- **整理后**: 按类型快速定位

---

**是否开始执行整理？**
