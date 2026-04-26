# 批量配置表字段功能

## 概述

为 `TableConfig` 添加了批量配置字段的便捷方法，避免重复调用 `.field()` 方法，使代码更简洁易读。

## 新增方法

### 1. `fields(Vec<FieldConfig>)` - 批量添加字段

从 Vec 批量添加字段配置。

**示例：**

```rust
let table = TableConfig::new("users")
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true),
    ]);
```

### 2. `fields_from_iter<I>(I)` - 从迭代器添加字段

从任何实现了 `IntoIterator<Item = FieldConfig>` 的类型批量添加字段。

**示例：**

```rust
let field_configs = vec![
    FieldConfig::new("id", FieldType::BigInt),
    FieldConfig::new("username", FieldType::String { max_length: 50 }),
];

let table = TableConfig::new("users")
    .fields_from_iter(field_configs.into_iter());
```

## 使用场景对比

### 传统方式（不推荐）

```rust
let table = TableConfig::new("users")
    .field(FieldConfig::new("id", FieldType::BigInt).required(true))
    .field(FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true))
    .field(FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true))
    .field(FieldConfig::new("age", FieldType::Integer))
    .field(FieldConfig::new("status", FieldType::Enum {
        values: vec!["active".to_string(), "inactive".to_string()],
    }));
```

**缺点：**
- 代码冗余，每个字段都要写 `.field(`
- 视觉上不够清晰
- 难以快速浏览字段列表

### 批量方式（推荐）

```rust
let table = TableConfig::new("users")
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true),
        FieldConfig::new("age", FieldType::Integer),
        FieldConfig::new("status", FieldType::Enum {
            values: vec!["active".to_string(), "inactive".to_string()],
        }),
    ]);
```

**优点：**
- 代码简洁，减少重复
- 字段列表一目了然
- 更符合 Rust 的惯用法

### 混合方式（灵活）

先批量添加基本字段，再单独添加特殊字段：

```rust
let table = TableConfig::new("users")
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true),
    ])
    // 单独添加复杂的 JSON 字段
    .field(FieldConfig::new("metadata", FieldType::Json)
        .display_name("元数据"));
```

## 完整示例

查看 `examples/batch_field_config.rs` 获取完整的使用示例。

运行示例：

```bash
cargo run --example batch_field_config -p yang-base
```

## 代码变更

### 修改的文件

1. **`src/table/table_config.rs`**
   - 添加 `fields()` 方法
   - 添加 `fields_from_iter()` 方法

2. **`TABLE_CONFIG_GUIDE.md`**
   - 更新文档，展示批量配置的用法
   - 添加最佳实践建议
   - 修复验证器示例（`Range` → `Min` + `Max`）

3. **`examples/batch_field_config.rs`**
   - 新增示例文件，展示四种配置方式

## 测试

所有测试通过：

```bash
✅ cargo check -p yang-base
✅ cargo clippy -p yang-base -- -D warnings
✅ cargo fmt -p yang-base --check
✅ cargo run --example batch_field_config -p yang-base
```

## 最佳实践

1. **优先使用批量配置**：当有多个字段时，使用 `fields()` 方法
2. **混合使用**：基本字段批量添加，特殊字段单独添加
3. **保持一致性**：在同一个项目中保持统一的配置风格

## 向后兼容性

✅ 完全向后兼容，原有的 `.field()` 方法仍然可用。
