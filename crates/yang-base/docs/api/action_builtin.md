# 内置 CRUD Actions

本模块提供了标准的 CRUD 操作 Actions，包括新增、更新、删除、查询等功能。

## Actions 列表

### 1. AddAction - 新增数据

向数据表中插入新记录。

**请求参数：**
- `data`: HashMap<String, Value> - 要插入的数据

**响应：**
```json
{
    "code": 0,
    "message": "新增成功",
    "data": {
        "affected": 1
    }
}
```

**示例：**
```rust
use yang_base::action::builtin::AddAction;
use yang_base::table::TableConfig;
use std::sync::Arc;

let table_config = Arc::new(TableConfig::new("users"));
let action = AddAction::new(table_config);
```

### 2. PutAction - 更新数据

更新数据表中的记录。

**请求参数：**
- `id`: 主键值（参数名由表配置的 primary_key 决定）
- `data`: HashMap<String, Value> - 要更新的数据

**响应：**
```json
{
    "code": 0,
    "message": "更新成功",
    "data": {
        "affected": 1
    }
}
```

### 3. DelAction - 删除数据

删除数据表中的记录（支持软删除）。

**请求参数：**
- `id`: 主键值（参数名由表配置的 primary_key 决定）

**响应：**
```json
{
    "code": 0,
    "message": "删除成功",
    "data": {
        "affected": 1
    }
}
```

**软删除：**
如果表配置中设置了 `soft_delete_field`，则执行软删除（UPDATE 设置删除标记），否则执行物理删除（DELETE）。

### 4. GetAction - 获取单条数据

根据主键获取单条记录。

**请求参数：**
- `id`: 主键值（参数名由表配置的 primary_key 决定）

**响应：**
```json
{
    "code": 0,
    "message": "获取成功",
    "data": {
        "id": 1,
        "name": "Alice",
        "email": "alice@example.com"
    }
}
```

**注意：** 由于 Rust 类型系统限制，内置 GetAction 需要用户自定义实现。请参考下面的自定义实现示例。

### 5. SelectAction - 列表查询

分页查询数据列表，支持字段选择、筛选条件和排序。

**请求参数：**
- `fields`: 字段选择列表（可选）
- `where_conditions`: WHERE 条件列表（可选）
- `order_by`: 排序规则列表（可选）
- `page`: 当前页码，从 1 开始（可选，默认 1）
- `page_size`: 每页大小（可选，默认 20）

**响应：**
```json
{
    "code": 0,
    "message": "查询成功",
    "data": {
        "data": [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ],
        "total": 100,
        "page": 1,
        "page_size": 20,
        "total_pages": 5
    }
}
```

**注意：** 由于 Rust 类型系统限制，内置 SelectAction 需要用户自定义实现。请参考下面的自定义实现示例。

### 6. TableAction - 获取表元数据

获取表的元数据信息，包括字段定义、权限配置等。

**请求参数：** 无

**响应：**
```json
{
    "code": 0,
    "message": "获取成功",
    "data": {
        "table_name": "users",
        "display_name": "用户表",
        "primary_key": "id",
        "fields": [
            {
                "name": "id",
                "display_name": "ID",
                "type": "BigInt",
                "required": true,
                "readable": true,
                "writable": false,
                "filterable": true,
                "sortable": true
            }
        ],
        "default_order": [["created_at", "desc"]]
    }
}
```

**特点：** TableAction 是公开 action，不需要认证。

## 自定义实现示例

由于 Rust 的类型系统限制，GetAction 和 SelectAction 无法直接返回动态类型的数据。在实际应用中，建议用户自定义这些 Actions 并指定具体的返回类型。

### 自定义 GetAction

```rust
use yang_base::action::{Action, ActionContext, ApiResponse};
use yang_base::error::BaseError;
use yang_base::table::TableConfig;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// 定义用户结构体
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i64,
    name: String,
    email: String,
}

// 自定义 GetAction
pub struct UserGetAction {
    config: Arc<TableConfig>,
}

impl UserGetAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for UserGetAction {
    fn name(&self) -> &str {
        "get"
    }

    fn display_name(&self) -> &str {
        "获取用户详情"
    }

    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键值
        let id: i64 = context.param(&self.config.primary_key)?;

        // 创建查询
        let query = context
            .table_query()?
            .where_eq(&self.config.primary_key, serde_json::json!(id))?;

        // 执行查询
        let mut results = query.select::<User>().await?;

        // 检查结果
        if results.is_empty() {
            return Err(BaseError::RecordNotFound(format!(
                "{}={}",
                self.config.primary_key, id
            )));
        }

        // 返回第一条记录
        Ok(ApiResponse::success(results.remove(0), "获取成功"))
    }
}
```

### 自定义 SelectAction

```rust
use yang_base::action::{Action, ActionContext, ApiResponse};
use yang_base::error::BaseError;
use yang_base::table::{TableConfig, QueryParams, WhereCondition};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// 定义用户结构体
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i64,
    name: String,
    email: String,
}

// 自定义 SelectAction
pub struct UserSelectAction {
    config: Arc<TableConfig>,
}

impl UserSelectAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for UserSelectAction {
    fn name(&self) -> &str {
        "select"
    }

    fn display_name(&self) -> &str {
        "查询用户列表"
    }

    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 解析查询参数
        let params: QueryParams = if context.request.body.is_null() {
            QueryParams::new()
        } else {
            serde_json::from_value(context.request.body.clone())
                .map_err(|e| BaseError::ParamInvalid("query".to_string(), e.to_string()))?
        };

        // 构建查询
        let mut query = context.table_query()?;

        // 应用字段选择
        if let Some(fields) = params.fields {
            if !fields.is_empty() {
                let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
                query = query.select_fields(&field_refs)?;
            }
        }

        // 应用 WHERE 条件
        for condition in params.where_conditions {
            match condition {
                WhereCondition::Eq { field, value } => {
                    query = query.where_eq(&field, value)?;
                }
                WhereCondition::In { field, values } => {
                    query = query.where_in(&field, values)?;
                }
                WhereCondition::Like { field, pattern } => {
                    query = query.where_like(&field, pattern)?;
                }
                _ => {}
            }
        }

        // 应用排序
        for (field, order) in params.order_by {
            query = query.order_by(&field, order)?;
        }

        // 设置分页
        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);
        query = query.page(page, page_size)?;

        // 执行查询
        let result = query.paginate::<User>().await?;

        // 返回结果
        Ok(ApiResponse::success(result, "查询成功"))
    }
}
```

## 使用建议

1. **AddAction、PutAction、DelAction** 可以直接使用，无需自定义实现
2. **GetAction、SelectAction** 建议自定义实现，指定具体的返回类型
3. **TableAction** 可以直接使用，用于获取表元数据
4. 所有 Actions 都支持字段级权限控制
5. DelAction 支持软删除，只需在 TableConfig 中配置 `soft_delete_field`

## 完整示例

```rust
use yang_base::action::builtin::{AddAction, PutAction, DelAction, TableAction};
use yang_base::table::{TableConfig, FieldConfig, FieldType};
use std::sync::Arc;

// 创建表配置
let table_config = Arc::new(
    TableConfig::new("users")
        .primary_key("id")
        .field(FieldConfig::new("id", FieldType::BigInt))
        .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
        .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
        .field(FieldConfig::new("deleted_at", FieldType::BigInt))
        .soft_delete_field("deleted_at")
);

// 创建内置 Actions
let add_action = AddAction::new(table_config.clone());
let put_action = PutAction::new(table_config.clone());
let del_action = DelAction::new(table_config.clone());
let table_action = TableAction::new(table_config.clone());

// 创建自定义 GetAction 和 SelectAction
let get_action = UserGetAction::new(table_config.clone());
let select_action = UserSelectAction::new(table_config.clone());
```
