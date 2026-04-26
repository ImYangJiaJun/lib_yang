# yang-base 表字段配置完整指南

## 目录

1. [基础概念](#1-基础概念)
2. [字段类型](#2-字段类型)
3. [字段配置](#3-字段配置)
4. [表配置](#4-表配置)
5. [验证器](#5-验证器)
6. [权限控制](#6-权限控制)
7. [关联表](#7-关联表)
8. [完整示例](#8-完整示例)

---

## 1. 基础概念

### 表配置系统组成

```
TableConfig (表配置)
├── table_name (表名)
├── display_name (显示名称)
├── primary_key (主键)
├── fields (字段列表)
│   └── FieldConfig (字段配置)
│       ├── name (字段名)
│       ├── field_type (字段类型)
│       ├── validators (验证器)
│       └── permissions (权限)
├── indexes (索引)
├── default_order (默认排序)
└── timestamp_fields (时间戳字段)
```

---

## 2. 字段类型

### 2.1 基本类型

```rust
use yang_base::table::FieldType;

// 字符串类型（带最大长度限制）
let name_type = FieldType::String { max_length: 50 };

// 文本类型（无长度限制）
let description_type = FieldType::Text;

// 整数类型
let age_type = FieldType::Integer;

// 大整数类型（用于 ID）
let id_type = FieldType::BigInt;

// 浮点数类型
let price_type = FieldType::Float;

// 布尔类型
let is_active_type = FieldType::Boolean;

// 日期类型
let birthday_type = FieldType::Date;

// 日期时间类型
let created_at_type = FieldType::DateTime;

// 时间戳类型
let timestamp_type = FieldType::Timestamp;
```

### 2.2 特殊类型

```rust
// 枚举类型
let status_type = FieldType::Enum {
    values: vec![
        "pending".to_string(),
        "active".to_string(),
        "inactive".to_string(),
        "deleted".to_string(),
    ],
};

// JSON 类型
let metadata_type = FieldType::Json;

// 数组类型
let tags_type = FieldType::Array {
    item_type: Box::new(FieldType::String { max_length: 50 }),
};

// 对象类型
let address_type = FieldType::Object {
    properties: vec![
        ("province".to_string(), FieldType::String { max_length: 50 }),
        ("city".to_string(), FieldType::String { max_length: 50 }),
        ("district".to_string(), FieldType::String { max_length: 50 }),
        ("detail".to_string(), FieldType::String { max_length: 200 }),
    ],
};
```

---

## 3. 字段配置

### 3.1 基本字段配置

```rust
use yang_base::table::{FieldConfig, FieldType};
use serde_json::json;

// 创建一个基本字段
let field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    .display_name("用户名")           // 显示名称
    .required(true)                   // 必填
    .default_value(json!("guest"));   // 默认值
```

### 3.2 字段属性

```rust
// ID 字段（主键）
let id_field = FieldConfig::new("id", FieldType::BigInt)
    .display_name("ID")
    .required(true);

// 用户名字段
let username_field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    .display_name("用户名")
    .required(true);

// 邮箱字段
let email_field = FieldConfig::new("email", FieldType::String { max_length: 100 })
    .display_name("邮箱")
    .required(true);

// 年龄字段（可选）
let age_field = FieldConfig::new("age", FieldType::Integer)
    .display_name("年龄")
    .required(false);

// 状态字段（带默认值）
let status_field = FieldConfig::new("status", FieldType::Enum {
    values: vec!["active".to_string(), "inactive".to_string()],
})
    .display_name("状态")
    .default_value(json!("active"));

// 简介字段（文本类型）
let bio_field = FieldConfig::new("bio", FieldType::Text)
    .display_name("个人简介")
    .required(false);

// 创建时间字段
let created_at_field = FieldConfig::new("created_at", FieldType::DateTime)
    .display_name("创建时间")
    .required(true);
```

### 3.3 可筛选和可排序

```rust
// 可筛选但不可排序的字段
let description_field = FieldConfig::new("description", FieldType::Text)
    .display_name("描述")
    .filterable(true)   // 可以用于 WHERE 条件
    .sortable(false);   // 不能用于 ORDER BY

// 可排序但不可筛选的字段
let score_field = FieldConfig::new("score", FieldType::Float)
    .display_name("评分")
    .filterable(false)
    .sortable(true);
```

---

## 4. 表配置

### 4.1 创建表配置

```rust
use yang_base::table::{TableConfig, FieldConfig, FieldType, SortOrder};

let table = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id");
```

### 4.2 添加字段

#### 方法1：逐个添加（传统方式）

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id")
    // 逐个添加字段
    .field(FieldConfig::new("id", FieldType::BigInt)
        .display_name("ID")
        .required(true))
    .field(FieldConfig::new("username", FieldType::String { max_length: 50 })
        .display_name("用户名")
        .required(true))
    .field(FieldConfig::new("email", FieldType::String { max_length: 100 })
        .display_name("邮箱")
        .required(true))
    .field(FieldConfig::new("age", FieldType::Integer)
        .display_name("年龄"))
    .field(FieldConfig::new("status", FieldType::Enum {
        values: vec!["active".to_string(), "inactive".to_string()],
    })
        .display_name("状态")
        .default_value(json!("active")));
```

#### 方法2：批量添加（推荐）

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id")
    // 使用 fields() 批量添加字段
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt)
            .display_name("ID")
            .required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 })
            .display_name("用户名")
            .required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 })
            .display_name("邮箱")
            .required(true),
        FieldConfig::new("age", FieldType::Integer)
            .display_name("年龄"),
        FieldConfig::new("status", FieldType::Enum {
            values: vec!["active".to_string(), "inactive".to_string()],
        })
            .display_name("状态")
            .default_value(json!("active")),
    ]);
```

#### 方法3：从迭代器添加

```rust
// 定义字段配置
let field_configs = vec![
    FieldConfig::new("id", FieldType::BigInt)
        .display_name("ID")
        .required(true),
    FieldConfig::new("username", FieldType::String { max_length: 50 })
        .display_name("用户名")
        .required(true),
    FieldConfig::new("email", FieldType::String { max_length: 100 })
        .display_name("邮箱")
        .required(true),
];

// 使用 fields_from_iter() 从迭代器添加
let table = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id")
    .fields_from_iter(field_configs.into_iter());
```

#### 方法4：混合使用

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id")
    // 先批量添加基本字段
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt)
            .display_name("ID")
            .required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 })
            .display_name("用户名")
            .required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 })
            .display_name("邮箱")
            .required(true),
    ])
    // 再单独添加特殊字段
    .field(FieldConfig::new("metadata", FieldType::Json)
        .display_name("元数据"));
```

### 4.3 配置索引

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    // 唯一索引
    .unique_index(vec!["username".to_string()])
    .unique_index(vec!["email".to_string()])
    // 普通索引
    .index(vec!["status".to_string()])
    .index(vec!["created_at".to_string()])
    // 复合索引
    .index(vec!["status".to_string(), "created_at".to_string()]);
```

### 4.4 配置排序

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    // 默认按创建时间降序，ID 升序
    .default_order(vec![
        ("created_at".to_string(), SortOrder::Desc),
        ("id".to_string(), SortOrder::Asc),
    ]);
```

### 4.5 配置时间戳

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    // 启用创建时间、更新时间、删除时间
    .timestamps(true, true, true);
    // 这会自动添加 created_at, updated_at, deleted_at 字段
```

### 4.6 配置软删除

```rust
let table = TableConfig::new("users")
    .display_name("用户表")
    // 启用软删除
    .soft_delete_field("deleted_at");
    // 删除操作会设置 deleted_at 字段而不是物理删除
```

---

## 5. 验证器

### 5.1 基本验证器

```rust
use yang_base::table::Validator;

// 最小长度
let min_length = Validator::MinLength(3);

// 最大长度
let max_length = Validator::MaxLength(50);

// 邮箱格式
let email = Validator::Email;

// 手机号格式
let phone = Validator::Phone;

// URL 格式
let url = Validator::Url;

// 正则表达式
let regex = Validator::Regex(r"^[a-zA-Z0-9_]+$".to_string());

// 最小值
let min = Validator::Min(0.0);

// 最大值
let max = Validator::Max(100.0);
```

### 5.2 添加验证器到字段

```rust
// 用户名字段：3-50 个字符，只能包含字母数字下划线
let username_field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    .display_name("用户名")
    .required(true)
    .validator(Validator::MinLength(3))
    .validator(Validator::MaxLength(50))
    .validator(Validator::Regex(r"^[a-zA-Z0-9_]+$".to_string()));

// 邮箱字段：必须是有效的邮箱格式
let email_field = FieldConfig::new("email", FieldType::String { max_length: 100 })
    .display_name("邮箱")
    .required(true)
    .validator(Validator::Email);

// 年龄字段：18-100 之间
let age_field = FieldConfig::new("age", FieldType::Integer)
    .display_name("年龄")
    .validator(Validator::Min(18.0))
    .validator(Validator::Max(100.0));

// 手机号字段
let phone_field = FieldConfig::new("phone", FieldType::String { max_length: 20 })
    .display_name("手机号")
    .validator(Validator::Phone);
```

### 5.3 自定义验证器

```rust
use yang_base::table::ValidatorFn;

// 创建自定义验证器函数
let custom_validator = ValidatorFn::new(|field_name, value| {
    if let Some(s) = value.as_str() {
        if s.contains("admin") {
            return Err(BaseError::ValidationFailed(
                field_name.to_string(),
                "用户名不能包含 'admin'".to_string(),
            ));
        }
    }
    Ok(())
});

// 使用自定义验证器
let username_field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    .display_name("用户名")
    .validator(Validator::Custom(custom_validator));
```

---

## 6. 权限控制

### 6.1 字段级权限

```rust
use yang_base::table::FieldPermissions;

// 配置字段权限
let permissions = FieldPermissions {
    // 可读角色：管理员和普通用户
    readable_roles: vec!["admin".to_string(), "user".to_string()],
    // 可写角色：仅管理员
    writable_roles: vec!["admin".to_string()],
    // 可筛选角色：管理员和普通用户
    filterable_roles: vec!["admin".to_string(), "user".to_string()],
    // 可排序角色：管理员
    sortable_roles: vec!["admin".to_string()],
};

// 应用到字段
let salary_field = FieldConfig::new("salary", FieldType::Float)
    .display_name("薪资")
    .permissions(permissions);
```

### 6.2 常见权限配置

```rust
// 公开字段（所有人可读）
let public_field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    .display_name("用户名")
    .permissions(FieldPermissions {
        readable_roles: vec!["*".to_string()],  // * 表示所有角色
        writable_roles: vec!["admin".to_string(), "user".to_string()],
        filterable_roles: vec!["*".to_string()],
        sortable_roles: vec!["*".to_string()],
    });

// 私密字段（仅管理员可见）
let private_field = FieldConfig::new("password_hash", FieldType::String { max_length: 255 })
    .display_name("密码哈希")
    .permissions(FieldPermissions {
        readable_roles: vec!["admin".to_string()],
        writable_roles: vec!["admin".to_string()],
        filterable_roles: vec![],
        sortable_roles: vec![],
    });

// 只读字段（所有人可读，无人可写）
let readonly_field = FieldConfig::new("created_at", FieldType::DateTime)
    .display_name("创建时间")
    .permissions(FieldPermissions {
        readable_roles: vec!["*".to_string()],
        writable_roles: vec![],  // 空数组表示无人可写
        filterable_roles: vec!["*".to_string()],
        sortable_roles: vec!["*".to_string()],
    });
```

---

## 7. 关联表

### 7.1 一对一关系

```rust
use yang_base::table::{RelationConfig, RelationType};

// 用户表的 profile_id 字段关联到 profiles 表
let profile_id_field = FieldConfig::new("profile_id", FieldType::BigInt)
    .display_name("个人资料ID")
    .relation(RelationConfig {
        relation_type: RelationType::BelongsTo,
        target_table: "profiles".to_string(),
        target_field: "id".to_string(),
        foreign_key: "profile_id".to_string(),
    });
```

### 7.2 一对多关系

```rust
// 订单表的 user_id 字段关联到 users 表
let user_id_field = FieldConfig::new("user_id", FieldType::BigInt)
    .display_name("用户ID")
    .required(true)
    .relation(RelationConfig {
        relation_type: RelationType::BelongsTo,
        target_table: "users".to_string(),
        target_field: "id".to_string(),
        foreign_key: "user_id".to_string(),
    });
```

### 7.3 多对多关系

```rust
// 用户-角色关联表
let user_role_table = TableConfig::new("user_roles")
    .display_name("用户角色关联表")
    .field(FieldConfig::new("user_id", FieldType::BigInt)
        .display_name("用户ID")
        .required(true)
        .relation(RelationConfig {
            relation_type: RelationType::BelongsTo,
            target_table: "users".to_string(),
            target_field: "id".to_string(),
            foreign_key: "user_id".to_string(),
        }))
    .field(FieldConfig::new("role_id", FieldType::BigInt)
        .display_name("角色ID")
        .required(true)
        .relation(RelationConfig {
            relation_type: RelationType::BelongsTo,
            target_table: "roles".to_string(),
            target_field: "id".to_string(),
            foreign_key: "role_id".to_string(),
        }))
    .unique_index(vec!["user_id".to_string(), "role_id".to_string()]);
```

---

## 8. 完整示例

### 8.1 用户表配置

```rust
use yang_base::table::{
    TableConfig, FieldConfig, FieldType, Validator, 
    FieldPermissions, SortOrder
};
use serde_json::json;

fn create_users_table() -> TableConfig {
    TableConfig::new("users")
        .display_name("用户表")
        .primary_key("id")
        
        // 使用 fields() 批量添加字段（推荐）
        .fields(vec![
            // ID 字段
            FieldConfig::new("id", FieldType::BigInt)
                .display_name("ID")
                .required(true),
            
            // 用户名字段
            FieldConfig::new("username", FieldType::String { max_length: 50 })
                .display_name("用户名")
                .required(true)
                .validator(Validator::MinLength(3))
                .validator(Validator::MaxLength(50))
                .validator(Validator::Regex(r"^[a-zA-Z0-9_]+$".to_string())),
            
            // 邮箱字段
            FieldConfig::new("email", FieldType::String { max_length: 100 })
                .display_name("邮箱")
                .required(true)
                .validator(Validator::Email),
            
            // 密码哈希字段（私密）
            FieldConfig::new("password_hash", FieldType::String { max_length: 255 })
                .display_name("密码哈希")
                .required(true)
                .permissions(FieldPermissions {
                    readable_roles: vec!["admin".to_string()],
                    writable_roles: vec!["admin".to_string()],
                    filterable_roles: vec![],
                    sortable_roles: vec![],
                }),
            
            // 手机号字段
            FieldConfig::new("phone", FieldType::String { max_length: 20 })
                .display_name("手机号")
                .validator(Validator::Phone),
            
            // 年龄字段
            FieldConfig::new("age", FieldType::Integer)
                .display_name("年龄")
                .validator(Validator::Min(18.0))
                .validator(Validator::Max(100.0)),
            
            // 状态字段
            FieldConfig::new("status", FieldType::Enum {
                values: vec![
                    "pending".to_string(),
                    "active".to_string(),
                    "inactive".to_string(),
                    "banned".to_string(),
                ],
            })
                .display_name("状态")
                .default_value(json!("pending")),
            
            // 角色字段
            FieldConfig::new("role", FieldType::Enum {
                values: vec![
                    "user".to_string(),
                    "admin".to_string(),
                    "super_admin".to_string(),
                ],
            })
                .display_name("角色")
                .default_value(json!("user")),
            
            // 个人简介字段
            FieldConfig::new("bio", FieldType::Text)
                .display_name("个人简介"),
            
            // 元数据字段（JSON）
            FieldConfig::new("metadata", FieldType::Json)
                .display_name("元数据"),
        ])
        
        // 索引配置
        .unique_index(vec!["username".to_string()])
        .unique_index(vec!["email".to_string()])
        .index(vec!["status".to_string()])
        .index(vec!["role".to_string()])
        .index(vec!["created_at".to_string()])
        
        // 默认排序
        .default_order(vec![
            ("created_at".to_string(), SortOrder::Desc),
            ("id".to_string(), SortOrder::Asc),
        ])
        
        // 软删除
        .soft_delete_field("deleted_at")
        
        // 时间戳字段
        .timestamps(true, true, true)
}
```

### 8.2 订单表配置

```rust
use yang_base::table::{RelationConfig, RelationType};

fn create_orders_table() -> TableConfig {
    TableConfig::new("orders")
        .display_name("订单表")
        .primary_key("id")
        
        // 使用 fields() 批量添加字段
        .fields(vec![
            // ID 字段
            FieldConfig::new("id", FieldType::BigInt)
                .display_name("ID")
                .required(true),
            
            // 订单号字段
            FieldConfig::new("order_no", FieldType::String { max_length: 50 })
                .display_name("订单号")
                .required(true),
            
            // 用户ID字段（外键）
            FieldConfig::new("user_id", FieldType::BigInt)
                .display_name("用户ID")
                .required(true)
                .relation(RelationConfig {
                    relation_type: RelationType::BelongsTo,
                    target_table: "users".to_string(),
                    target_field: "id".to_string(),
                    foreign_key: "user_id".to_string(),
                }),
            
            // 订单金额字段
            FieldConfig::new("amount", FieldType::Float)
                .display_name("订单金额")
                .required(true)
                .validator(Validator::Min(0.0))
                .validator(Validator::Max(999999.0)),
            
            // 订单状态字段
            FieldConfig::new("status", FieldType::Enum {
                values: vec![
                    "pending".to_string(),
                    "paid".to_string(),
                    "shipped".to_string(),
                    "completed".to_string(),
                    "cancelled".to_string(),
                ],
            })
                .display_name("订单状态")
                .default_value(json!("pending")),
            
            // 订单详情字段（JSON）
            FieldConfig::new("details", FieldType::Json)
                .display_name("订单详情"),
        ])
        
        // 索引配置
        .unique_index(vec!["order_no".to_string()])
        .index(vec!["user_id".to_string()])
        .index(vec!["status".to_string()])
        .index(vec!["created_at".to_string()])
        
        // 默认排序
        .default_order(vec![
            ("created_at".to_string(), SortOrder::Desc),
        ])
        
        // 时间戳字段
        .timestamps(true, true, false)
}
```

### 8.3 使用表配置

```rust
use yang_base::database::GlobalDatabase;
use yang_base::error::BaseError;

async fn example_usage() -> Result<(), BaseError> {
    // 创建表配置
    let users_table = create_users_table();
    
    // 验证字段是否存在
    users_table.validate_field("username")?;
    
    // 获取字段配置
    if let Some(field) = users_table.get_field("email") {
        println!("字段类型: {:?}", field.field_type);
        println!("是否必填: {}", field.required);
    }
    
    // 验证查询字段
    users_table.validate_query(&["username", "email", "status"])?;
    
    // 使用表配置进行查询
    let users = GlobalDatabase::table(&users_table.table_name)?
        .where_and("status", "=", "active")
        .select::<User>()
        .await?;
    
    Ok(())
}
```

---

## 9. 最佳实践

### 9.1 命名规范

```rust
// ✅ 推荐：使用蛇形命名法
TableConfig::new("user_profiles")
FieldConfig::new("created_at", FieldType::DateTime)

// ❌ 不推荐：使用驼峰命名法
TableConfig::new("userProfiles")
FieldConfig::new("createdAt", FieldType::DateTime)
```

### 9.2 批量配置 vs 逐个配置

```rust
// ✅ 推荐：使用 fields() 批量配置（代码更简洁）
let table = TableConfig::new("users")
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
        FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true),
    ]);

// ❌ 不推荐：逐个配置（代码冗余）
let table = TableConfig::new("users")
    .field(FieldConfig::new("id", FieldType::BigInt).required(true))
    .field(FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true))
    .field(FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true));

// ✅ 也可以：混合使用（先批量再单独）
let table = TableConfig::new("users")
    .fields(vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
    ])
    .field(FieldConfig::new("metadata", FieldType::Json)); // 特殊字段单独添加
```

### 9.3 字段顺序

```rust
// 推荐的字段顺序：
// 1. 主键
// 2. 外键
// 3. 业务字段
// 4. 状态字段
// 5. 元数据字段
// 6. 时间戳字段

TableConfig::new("orders")
    .field(/* id */)              // 主键
    .field(/* user_id */)         // 外键
    .field(/* order_no */)        // 业务字段
    .field(/* amount */)          // 业务字段
    .field(/* status */)          // 状态字段
    .field(/* metadata */)        // 元数据
    .field(/* created_at */)      // 时间戳
    .field(/* updated_at */)      // 时间戳
```

### 9.3 索引策略

```rust
// 1. 为外键添加索引
.index(vec!["user_id".to_string()])

// 2. 为常用筛选字段添加索引
.index(vec!["status".to_string()])

// 3. 为唯一字段添加唯一索引
.unique_index(vec!["email".to_string()])

// 4. 为常用组合查询添加复合索引
.index(vec!["user_id".to_string(), "status".to_string()])
```

### 9.4 验证器使用

```rust
// 1. 按照从宽到严的顺序添加验证器
.validator(Validator::MinLength(3))      // 先检查最小长度
.validator(Validator::MaxLength(50))     // 再检查最大长度
.validator(Validator::Regex(r"^[a-zA-Z0-9_]+$".to_string()))  // 最后检查格式

// 2. 为必填字段添加验证器
.required(true)
.validator(Validator::MinLength(1))
```

---

## 10. 常见问题

### Q: 如何动态修改表配置？

```rust
let mut table = create_users_table();

// 添加新字段
table.fields.insert(
    "nickname".to_string(),
    FieldConfig::new("nickname", FieldType::String { max_length: 50 })
        .display_name("昵称")
);

// 修改字段配置
if let Some(field) = table.fields.get_mut("email") {
    field.required = false;
}
```

### Q: 如何实现字段级别的数据脱敏？

```rust
// 在字段配置中添加脱敏标记
let phone_field = FieldConfig::new("phone", FieldType::String { max_length: 20 })
    .display_name("手机号")
    .permissions(FieldPermissions {
        readable_roles: vec!["admin".to_string()],  // 只有管理员能看到完整手机号
        writable_roles: vec!["admin".to_string(), "user".to_string()],
        filterable_roles: vec![],
        sortable_roles: vec![],
    });
```

### Q: 如何处理复杂的验证逻辑？

```rust
// 使用自定义验证器
let custom_validator = ValidatorFn::new(|field_name, value| {
    // 复杂的验证逻辑
    if let Some(obj) = value.as_object() {
        if obj.get("province").is_none() || obj.get("city").is_none() {
            return Err(BaseError::ValidationFailed(
                field_name.to_string(),
                "地址必须包含省份和城市".to_string(),
            ));
        }
    }
    Ok(())
});

let address_field = FieldConfig::new("address", FieldType::Json)
    .display_name("地址")
    .validator(Validator::Custom(custom_validator));
```

---

## 总结

表字段配置系统提供了：

1. ✅ **丰富的字段类型**：支持基本类型、枚举、JSON、数组等
2. ✅ **灵活的验证器**：内置多种验证器，支持自定义
3. ✅ **细粒度权限控制**：字段级别的读写权限
4. ✅ **关联表支持**：一对一、一对多、多对多关系
5. ✅ **索引管理**：唯一索引、普通索引、复合索引
6. ✅ **软删除支持**：自动处理软删除逻辑
7. ✅ **时间戳管理**：自动维护创建时间、更新时间

通过合理配置表字段，可以实现：
- 数据验证
- 权限控制
- 查询优化
- 数据完整性保证
