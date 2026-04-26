# yang-base 项目结构整理方案

## 发现的问题

### 1. 重复文件问题 ⚠️

**位置**: `src/action/builtin/`

**问题描述**: 存在重复的 Action 实现文件

**重复文件列表**:
```
add.rs          ←→  add_action.rs
del.rs          ←→  del_action.rs  
get.rs          ←→  get_action.rs
put.rs          ←→  put_action.rs
select.rs       ←→  select_action.rs
table.rs        ←→  table_action.rs
```

**当前状态**:
- `mod.rs` 只导入了不带 `_action` 后缀的文件（add.rs, del.rs 等）
- 带 `_action` 后缀的文件（add_action.rs 等）未被使用
- 两组文件内容相似但不完全相同

**影响**:
- 造成代码冗余
- 增加维护成本
- 可能导致混淆

---

## 整理方案

### 方案 A: 删除 `_action` 后缀文件（推荐）✅

**操作步骤**:
1. 确认 `mod.rs` 使用的是不带后缀的文件
2. 删除所有 `*_action.rs` 文件
3. 保留 `add.rs`, `del.rs`, `get.rs`, `put.rs`, `select.rs`, `table.rs`

**优点**:
- 简单直接
- 文件名更简洁
- 符合当前 `mod.rs` 的导入逻辑

**需要删除的文件**:
```
src/action/builtin/add_action.rs
src/action/builtin/del_action.rs
src/action/builtin/get_action.rs
src/action/builtin/put_action.rs
src/action/builtin/select_action.rs
src/action/builtin/table_action.rs
```

---

### 方案 B: 删除不带后缀的文件

**操作步骤**:
1. 删除 `add.rs`, `del.rs` 等文件
2. 保留 `*_action.rs` 文件
3. 修改 `mod.rs` 的导入语句

**优点**:
- 文件名更明确（明确表示是 Action）

**缺点**:
- 需要修改 `mod.rs`
- 文件名较长

---

## 其他发现的问题

### 2. 文档文件较多

**位置**: 项目根目录

**文件列表**:
```
ASYNC_AWAIT_GUIDE.md      # 异步编程指南
BATCH_FIELD_CONFIG.md     # 批量字段配置
INSTALL.md                # 安装指南
PROJECT_STRUCTURE.md      # 项目结构（新创建）
QUICK_REFERENCE.md        # 快速参考
README.md                 # 主文档
REDIS_GUIDE.md            # Redis 指南
TABLE_CONFIG_GUIDE.md     # 表配置指南
USAGE_GUIDE.md            # 使用指南
```

**建议**: 
- 保持现状（文档完整性好）
- 或者创建 `docs/` 目录统一管理

---

## 推荐的整理步骤

### 第一步: 清理重复文件

```bash
# 删除 builtin 目录中的 *_action.rs 文件
rm src/action/builtin/add_action.rs
rm src/action/builtin/del_action.rs
rm src/action/builtin/get_action.rs
rm src/action/builtin/put_action.rs
rm src/action/builtin/select_action.rs
rm src/action/builtin/table_action.rs
```

### 第二步: 验证编译

```bash
cargo check --all-targets
cargo test --lib
```

### 第三步: 运行测试

```bash
cargo test
```

---

## 整理后的项目结构

```
yang-base/
├── Cargo.toml
├── README.md
├── docs/                          # 可选：统一文档目录
│   ├── ASYNC_AWAIT_GUIDE.md
│   ├── BATCH_FIELD_CONFIG.md
│   ├── INSTALL.md
│   ├── QUICK_REFERENCE.md
│   ├── REDIS_GUIDE.md
│   ├── TABLE_CONFIG_GUIDE.md
│   └── USAGE_GUIDE.md
├── examples/
│   ├── batch_field_config.rs
│   ├── database_example.rs
│   ├── database_initializer_example.rs
│   └── field_type_demo.rs
├── src/
│   ├── lib.rs
│   ├── action/
│   │   ├── mod.rs
│   │   ├── action_trait.rs
│   │   ├── context.rs
│   │   ├── request.rs
│   │   ├── response.rs
│   │   ├── builtin/
│   │   │   ├── mod.rs
│   │   │   ├── add.rs              # ✅ 保留
│   │   │   ├── del.rs              # ✅ 保留
│   │   │   ├── get.rs              # ✅ 保留
│   │   │   ├── put.rs              # ✅ 保留
│   │   │   ├── select.rs           # ✅ 保留
│   │   │   ├── table.rs            # ✅ 保留
│   │   │   └── __tests__/
│   │   └── __tests__/
│   ├── database/
│   │   ├── mod.rs
│   │   ├── global.rs
│   │   ├── global_redis.rs
│   │   └── initializer.rs
│   ├── error/
│   │   └── mod.rs
│   ├── http/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── request.rs
│   │   ├── response.rs
│   │   └── __tests__/
│   ├── plugin/
│   │   └── mod.rs
│   ├── router/
│   │   ├── mod.rs
│   │   ├── module_router.rs
│   │   └── __tests__/
│   ├── table/
│   │   ├── mod.rs
│   │   ├── field_config.rs
│   │   ├── field_type.rs
│   │   ├── query_params.rs
│   │   ├── table_config.rs
│   │   ├── table_query.rs
│   │   ├── table_query_select.rs
│   │   ├── validator.rs
│   │   └── __tests__/
│   └── token/
│       ├── mod.rs
│       ├── manager.rs
│       └── __tests__/
└── tests/
    ├── database_test.rs
    ├── database_integration_test.rs
    ├── database_initializer_test.rs
    ├── error_test.rs
    ├── field_type_test.rs
    ├── plugin_test.rs
    ├── table_query_crud_test.rs
    └── table_query_paginate_test.rs
```

---

## 检查清单

- [ ] 删除重复的 `*_action.rs` 文件
- [ ] 运行 `cargo check` 确保编译通过
- [ ] 运行 `cargo test` 确保测试通过
- [ ] 运行 `cargo clippy` 检查代码质量
- [ ] 更新 `PROJECT_STRUCTURE.md` 文档
- [ ] 提交更改

---

## 预期效果

### 文件数量减少
- **删除前**: builtin 目录 14 个 .rs 文件
- **删除后**: builtin 目录 7 个 .rs 文件（减少 50%）

### 代码质量提升
- ✅ 消除代码冗余
- ✅ 降低维护成本
- ✅ 提高代码可读性
- ✅ 避免混淆

### 编译和测试
- ✅ 编译通过
- ✅ 所有测试通过
- ✅ 无警告

---

## 风险评估

**风险等级**: 🟢 低

**原因**:
1. `mod.rs` 已经只导入不带后缀的文件
2. 带后缀的文件未被使用
3. 删除不会影响现有功能

**建议**: 
- 在删除前先备份
- 删除后立即运行测试
- 如有问题可以快速恢复
