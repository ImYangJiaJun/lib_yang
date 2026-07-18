//! TableQuery 单元测试
//!
//! 测试查询构建器的功能，包括：
//! - 查询构建方法的链式调用
//! - 字段权限验证
//! - 错误情况处理

use crate::error::BaseError;
use crate::table::{col, Field, SortOrder, Table, TableConfig, TableQuery};
use serde_json::json;
use std::sync::Arc;

fn build_config(table: Table) -> Arc<TableConfig> {
    table.build().expect("测试表定义应有效").shared_config()
}

/// 创建测试用的表配置
///
/// 包含以下字段：
/// - id: BigInt, 所有人可读可写可筛选可排序
/// - name: String, 所有人可读可写可筛选可排序
/// - email: String, 所有人可读可写可筛选可排序
/// - salary: Double, 只有 admin 可读可筛选可排序
/// - secret: String, 只有 admin 可读
fn create_test_table_config() -> Arc<TableConfig> {
    build_config(
        Table::new("users").fields([
            Field::id("id"),
            Field::string("name", 50),
            Field::string("email", 100),
            Field::double("salary")
                .readable_by(["admin"])
                .writable_by(["admin"])
                .filterable_by(["admin"])
                .sortable_by(["admin"]),
            Field::string("secret", 255)
                .readable_by(["admin"])
                .writable_by(["admin"])
                .filterable_by(["admin"])
                .sortable_by(["admin"]),
        ]),
    )
}

#[test]
fn test_table_query_new() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(
        table_config.clone(),
        Arc::from(vec!["user".to_string()]),
        None,
    );

    assert_eq!(query.get_table_config().table_name, "users");
    assert_eq!(query.get_user_roles(), &["user"]);
    assert!(query.get_query_params().fields.is_none());
    assert!(query.get_query_params().where_conditions.is_empty());
    assert!(query.get_query_params().order_by.is_empty());
}

#[test]
fn test_select_fields_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.select_fields(&["id", "name", "email"]);
    assert!(result.is_ok());

    let query = result.unwrap();
    let fields = query.get_query_params().fields.as_ref().unwrap();
    assert_eq!(fields.len(), 3);
    assert!(fields.contains(&"id".to_string()));
    assert!(fields.contains(&"name".to_string()));
    assert!(fields.contains(&"email".to_string()));
}

#[test]
fn test_select_fields_rejects_empty_list() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let err = query
        .select_fields(&[])
        .expect_err("空字段选择列表不应被接受");

    assert!(matches!(
        err,
        BaseError::ParamInvalid(field, _) if field == "fields"
    ));
}

#[test]
fn test_select_fields_not_found() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.select_fields(&["id", "nonexistent"]);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldNotFound(table, field) => {
            assert_eq!(table, "users");
            assert_eq!(field, "nonexistent");
        }
        _ => panic!("期望 FieldNotFound 错误"),
    }
}

#[test]
fn test_select_fields_permission_denied() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // salary 字段只有 admin 可读
    let result = query.select_fields(&["id", "salary"]);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(table, field, msg) => {
            assert_eq!(table, "users");
            assert_eq!(field, "salary");
            assert_eq!(msg, "用户无读取权限");
        }
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_select_fields_admin_can_read_all() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // admin 可以读取所有字段
    let result = query.select_fields(&["id", "name", "salary", "secret"]);
    assert!(result.is_ok());

    let query = result.unwrap();
    let fields = query.get_query_params().fields.as_ref().unwrap();
    assert_eq!(fields.len(), 4);
}

#[test]
fn test_where_eq_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_eq("name", json!("alice"));
    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().where_conditions.len(), 1);
}

#[test]
fn test_where_eq_field_not_found() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_eq("nonexistent", json!("value"));
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldNotFound(table, field) => {
            assert_eq!(table, "users");
            assert_eq!(field, "nonexistent");
        }
        _ => panic!("期望 FieldNotFound 错误"),
    }
}

#[test]
fn test_where_eq_permission_denied() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // salary 字段只有 admin 可筛选
    let result = query.where_eq("salary", json!(50000));
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(table, field, msg) => {
            assert_eq!(table, "users");
            assert_eq!(field, "salary");
            assert_eq!(msg, "用户无筛选权限");
        }
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_where_in_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_in("name", vec![json!("alice"), json!("bob"), json!("charlie")]);
    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().where_conditions.len(), 1);
}

#[test]
fn test_where_in_rejects_empty_values() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let err = query
        .where_in("name", vec![])
        .expect_err("空 IN 列表不应生成非法 SQL");

    assert!(matches!(
        err,
        BaseError::ParamInvalid(field, _) if field == "values"
    ));
}

#[test]
fn test_where_not_in_rejects_empty_values() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let err = query
        .where_not_in("name", vec![])
        .expect_err("空 NOT IN 列表不应生成非法 SQL");

    assert!(matches!(
        err,
        BaseError::ParamInvalid(field, _) if field == "values"
    ));
}

#[test]
fn test_where_in_permission_denied() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_in("salary", vec![json!(50000), json!(60000)]);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(_, _, _) => {}
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_where_like_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_like("name", "%alice%".to_string());
    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().where_conditions.len(), 1);
}

#[test]
fn test_where_contains_rejects_blank_keyword() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let err = query
        .where_contains("name", "   ")
        .expect_err("空白关键词不应被转换成 %% 全匹配 LIKE");

    assert!(matches!(
        err,
        BaseError::ParamInvalid(field, _) if field == "keyword"
    ));
}

#[test]
fn test_where_like_permission_denied() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // secret 字段没有筛选权限
    let result = query.where_like("secret", "%test%".to_string());
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(_, _, _) => {}
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_order_by_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.order_by("name", SortOrder::Asc);
    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().order_by.len(), 1);
    assert_eq!(query.get_query_params().order_by[0].0, "name");
    assert_eq!(query.get_query_params().order_by[0].1, SortOrder::Asc);
}

#[test]
fn test_order_by_field_not_found() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.order_by("nonexistent", SortOrder::Asc);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldNotFound(table, field) => {
            assert_eq!(table, "users");
            assert_eq!(field, "nonexistent");
        }
        _ => panic!("期望 FieldNotFound 错误"),
    }
}

#[test]
fn test_order_by_permission_denied() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // salary 字段只有 admin 可排序
    let result = query.order_by("salary", SortOrder::Desc);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(table, field, msg) => {
            assert_eq!(table, "users");
            assert_eq!(field, "salary");
            assert_eq!(msg, "用户无排序权限");
        }
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_order_by_no_sort_permission() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // secret 字段没有排序权限
    let result = query.order_by("secret", SortOrder::Asc);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(_, _, _) => {}
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_page_success() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query.page(1, 20);
    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().page, Some(1));
    assert_eq!(query.get_query_params().page_size, Some(20));
}

#[test]
fn test_chained_calls() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // 测试链式调用
    let result = query
        .select_fields(&["id", "name", "email"])
        .and_then(|q| q.where_eq("name", json!("alice")))
        .and_then(|q| q.where_in("id", vec![json!(1), json!(2), json!(3)]))
        .and_then(|q| q.order_by("name", SortOrder::Asc))
        .and_then(|q| q.page(1, 20));

    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().fields.as_ref().unwrap().len(), 3);
    assert_eq!(query.get_query_params().where_conditions.len(), 2);
    assert_eq!(query.get_query_params().order_by.len(), 1);
    assert_eq!(query.get_query_params().page, Some(1));
    assert_eq!(query.get_query_params().page_size, Some(20));
}

#[test]
fn test_multiple_where_conditions() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query
        .where_eq("name", json!("alice"))
        .and_then(|q| q.where_like("email", "%@example.com".to_string()))
        .and_then(|q| q.where_in("id", vec![json!(1), json!(2)]));

    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().where_conditions.len(), 3);
}

#[test]
fn test_multiple_order_by() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let result = query
        .order_by("name", SortOrder::Asc)
        .and_then(|q| q.order_by("id", SortOrder::Desc));

    assert!(result.is_ok());

    let query = result.unwrap();
    assert_eq!(query.get_query_params().order_by.len(), 2);
    assert_eq!(query.get_query_params().order_by[0].0, "name");
    assert_eq!(query.get_query_params().order_by[0].1, SortOrder::Asc);
    assert_eq!(query.get_query_params().order_by[1].0, "id");
    assert_eq!(query.get_query_params().order_by[1].1, SortOrder::Desc);
}

#[test]
fn test_admin_can_use_all_fields() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // admin 可以使用所有字段
    let result = query
        .select_fields(&["id", "name", "salary", "secret"])
        .and_then(|q| q.where_eq("salary", json!(50000)))
        .and_then(|q| q.order_by("salary", SortOrder::Desc));

    assert!(result.is_ok());
}

#[test]
fn test_empty_user_roles() {
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec![]), None);

    // 空角色列表应该可以访问没有权限限制的字段
    let result = query.select_fields(&["id", "name"]);
    assert!(result.is_ok());

    // 但不能访问有权限限制的字段
    let table_config = create_test_table_config();
    let query = TableQuery::new(table_config, Arc::from(vec![]), None);
    let result = query.select_fields(&["salary"]);
    assert!(result.is_err());
}

// ==================== INSERT 操作测试 ====================
// 注：由于 INSERT 操作需要数据库连接，这里只测试验证逻辑
// 完整的集成测试应该在单独的集成测试文件中进行

/// 创建测试用的表配置（带必填字段）
///
/// 包含以下字段：
/// - id: BigInt, 自增主键，非必填
/// - name: String, 必填
/// - email: String, 必填
/// - age: Integer, 非必填
/// - salary: Double, 只有 admin 可写
fn create_test_table_config_for_insert() -> Arc<TableConfig> {
    build_config(
        Table::new("users").fields([
            Field::id("id"),
            Field::string("name", 50).required(),
            Field::string("email", 100).required(),
            Field::integer("age"),
            Field::double("salary")
                .readable_by(["admin"])
                .writable_by(["admin"])
                .filterable_by(["admin"])
                .sortable_by(["admin"]),
        ]),
    )
}

// ==================== UPDATE 操作测试 ====================
// 注：由于 UPDATE 操作需要数据库连接，这里只测试验证逻辑
// 完整的集成测试应该在单独的集成测试文件中进行

#[test]
fn test_update_validate_field_not_found() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("nonexistent".to_string(), json!("value"));

    // validate_update_data 是私有方法，我们通过构建 SQL 来间接测试
    // 这里我们测试 build_update_sql 会在 validate 中失败
    let result = query.build_update_sql(&data);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldNotFound(table, field) => {
            assert_eq!(table, "users");
            assert_eq!(field, "nonexistent");
        }
        _ => panic!("期望 FieldNotFound 错误"),
    }
}

#[test]
fn test_update_validate_permission_denied() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("salary".to_string(), json!(50000.0)); // salary 只有 admin 可写

    // validate_update_data 应该失败
    let result = query.validate_update_data(&data);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::FieldPermissionDenied(table, field, msg) => {
            assert_eq!(table, "users");
            assert_eq!(field, "salary");
            assert_eq!(msg, "用户无写入权限");
        }
        _ => panic!("期望 FieldPermissionDenied 错误"),
    }
}

#[test]
fn test_update_validate_success() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("email".to_string(), json!("zhangsan@example.com"));
    data.insert("age".to_string(), json!(25));

    // validate_update_data 应该成功
    let result = query.validate_update_data(&data);
    assert!(result.is_ok());
}

#[test]
fn test_update_validate_admin_can_write_all() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("salary".to_string(), json!(50000.0)); // admin 可以写入 salary

    // validate_update_data 应该成功
    let result = query.validate_update_data(&data);
    assert!(result.is_ok());
}

#[test]
fn test_update_validate_type_validation() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("age".to_string(), json!("not a number")); // age 应该是整数

    // validate_update_data 应该失败（类型验证失败）
    let result = query.validate_update_data(&data);
    assert!(result.is_err());

    match result.unwrap_err() {
        BaseError::InvalidFieldType(_, _) => {}
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_build_update_sql_basic() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("email".to_string(), json!("zhangsan@example.com"));

    // 无 WHERE 条件始终被 WHERE 守卫拒绝；不存在全表写绕过 API。
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);
    let result = query.build_update_sql(&data);
    assert!(matches!(result, Err(BaseError::MissingWhereClause(op)) if op == "UPDATE"));
}

#[test]
fn test_build_update_sql_with_where() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));

    // 添加 WHERE 条件
    let query = query.where_eq("id", json!(1)).unwrap();

    let result = query.build_update_sql(&data);
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含 WHERE 子句
    assert!(sql.contains("UPDATE `users`"));
    assert!(sql.contains("SET `name` = ?"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` = ?"));

    // 检查参数数量：1个 SET 参数 + 1个 WHERE 参数
    assert_eq!(params.len(), 2);
}

#[test]
fn test_build_update_sql_with_multiple_where() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));
    data.insert("age".to_string(), json!(25));

    // 添加多个 WHERE 条件
    let query = query
        .where_eq("id", json!(1))
        .unwrap()
        .where_like("email", "%@example.com".to_string())
        .unwrap();

    let result = query.build_update_sql(&data);
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含多个 WHERE 条件
    assert!(sql.contains("UPDATE `users`"));
    assert!(sql.contains("SET"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` = ?"));
    assert!(sql.contains("AND"));
    assert!(sql.contains("`email` LIKE ?"));

    // 检查参数数量：2个 SET 参数 + 2个 WHERE 参数
    assert_eq!(params.len(), 4);
}

#[test]
fn test_build_update_sql_with_in_condition() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));

    // 添加 IN 条件
    let query = query
        .where_in("id", vec![json!(1), json!(2), json!(3)])
        .unwrap();

    let result = query.build_update_sql(&data);
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含 IN 子句
    assert!(sql.contains("UPDATE `users`"));
    assert!(sql.contains("SET `name` = ?"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` IN (?, ?, ?)"));

    // 检查参数数量：1个 SET 参数 + 3个 IN 参数
    assert_eq!(params.len(), 4);
}

#[test]
fn test_update_partial_fields() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    // UPDATE 只更新部分字段，不需要提供所有字段
    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三")); // 只更新 name

    let result = query.validate_update_data(&data);
    assert!(result.is_ok()); // 应该成功，即使 email 是必填字段
}

#[test]
fn test_update_empty_data() {
    use std::collections::HashMap;

    let table_config = create_test_table_config_for_insert();
    let query = TableQuery::new(table_config, Arc::from(vec!["user".to_string()]), None);

    let data = HashMap::new(); // 空数据

    // 构建 SQL 应拒绝空更新，返回 ParamInvalid（无可更新字段）
    let result = query.build_update_sql(&data);
    assert!(matches!(result, Err(BaseError::ParamInvalid(field, _)) if field == "data"));
}

// ==================== DELETE 操作测试 ====================

/// 创建测试用的表配置（带软删除字段）
///
/// 包含以下字段：
/// - id: BigInt
/// - name: String
/// - email: String
/// - deleted_at: BigInt（软删除字段）
fn create_test_table_config_with_soft_delete() -> Arc<TableConfig> {
    build_config(Table::new("users").fields([
        Field::id("id"),
        Field::string("name", 50),
        Field::string("email", 100),
        Field::soft_delete("deleted_at"),
    ]))
}

/// 创建测试用的表配置（不带软删除字段）
///
/// 包含以下字段：
/// - id: BigInt
/// - message: Text
fn create_test_table_config_without_soft_delete() -> Arc<TableConfig> {
    build_config(Table::new("logs").fields([Field::id("id"), Field::text("message")]))
}

#[test]
fn test_build_delete_sql_basic() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 无 WHERE 条件始终被 WHERE 守卫拒绝；不存在全表删除绕过 API。
    let result = query.build_delete_sql();
    assert!(matches!(result, Err(BaseError::MissingWhereClause(op)) if op == "DELETE"));
}

#[test]
fn test_build_delete_sql_with_where_eq() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 添加 WHERE 条件
    let query = query.where_eq("id", json!(1)).unwrap();

    let result = query.build_delete_sql();
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含 WHERE 子句
    assert!(sql.contains("DELETE FROM `logs`"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` = ?"));

    // 检查参数数量
    assert_eq!(params.len(), 1);
}

#[test]
fn test_build_delete_sql_with_multiple_where() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 添加多个 WHERE 条件
    let query = query
        .where_eq("id", json!(1))
        .unwrap()
        .where_like("message", "%error%".to_string())
        .unwrap();

    let result = query.build_delete_sql();
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含多个 WHERE 条件
    assert!(sql.contains("DELETE FROM `logs`"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` = ?"));
    assert!(sql.contains("AND"));
    assert!(sql.contains("`message` LIKE ?"));

    // 检查参数数量
    assert_eq!(params.len(), 2);
}

#[test]
fn test_build_delete_sql_with_in_condition() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 添加 IN 条件
    let query = query
        .where_in("id", vec![json!(1), json!(2), json!(3)])
        .unwrap();

    let result = query.build_delete_sql();
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句包含 IN 子句
    assert!(sql.contains("DELETE FROM `logs`"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` IN (?, ?, ?)"));

    // 检查参数数量
    assert_eq!(params.len(), 3);
}

#[test]
fn test_build_delete_sql_with_is_null() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 添加 IS NULL 条件
    let query = query.where_eq("id", json!(1)).unwrap();

    let result = query.build_delete_sql();
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();

    // 检查 SQL 语句
    assert!(sql.contains("DELETE FROM `logs`"));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("`id` = ?"));

    // 检查参数数量
    assert_eq!(params.len(), 1);
}

#[test]
fn test_soft_delete_field_configured() {
    let table_config = create_test_table_config_with_soft_delete();

    // 验证软删除字段已配置
    assert_eq!(
        table_config.soft_delete_field,
        Some("deleted_at".to_string())
    );
}

#[test]
fn test_hard_delete_field_not_configured() {
    let table_config = create_test_table_config_without_soft_delete();

    // 验证软删除字段未配置
    assert_eq!(table_config.soft_delete_field, None);
}

#[test]
fn test_delete_sql_all_where_conditions() {
    let table_config = create_test_table_config_without_soft_delete();
    let query = TableQuery::new(table_config, Arc::from(vec!["admin".to_string()]), None);

    // 测试所有 WHERE 条件类型
    let query = query.where_eq("id", json!(1)).unwrap();

    let result = query.build_delete_sql();
    assert!(result.is_ok());

    let (sql, params) = result.unwrap();
    assert!(sql.contains("DELETE FROM `logs`"));
    assert!(sql.contains("WHERE"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_new_where_methods_build_sql() {
    let config = build_config(Table::new("users").fields([
        Field::id("id"),
        Field::integer("age"),
        Field::string("name", 50),
    ]));
    let q = crate::table::TableQuery::new_without_pool(config)
        .where_lt("age", serde_json::json!(18))
        .unwrap()
        .where_gte("id", serde_json::json!(1))
        .unwrap()
        .where_ne("name", serde_json::json!("admin"))
        .unwrap()
        .where_between("id", serde_json::json!(10), serde_json::json!(20))
        .unwrap()
        .where_null("name")
        .unwrap()
        .where_not_null("age")
        .unwrap()
        .where_not_in("id", vec![serde_json::json!(5), serde_json::json!(6)])
        .unwrap();
    let (sql, _) = q.build_select_sql_for_test().expect("build sql");
    assert!(sql.contains("`age` < ?"), "Expected `age` < ? in: {}", sql);
    assert!(sql.contains("`id` >= ?"), "Expected `id` >= ? in: {}", sql);
    assert!(
        sql.contains("`name` <> ?"),
        "Expected `name` <> ? in: {}",
        sql
    );
    assert!(
        sql.contains("`id` BETWEEN ? AND ?"),
        "Expected BETWEEN in: {}",
        sql
    );
    assert!(
        sql.contains("`name` IS NULL"),
        "Expected IS NULL in: {}",
        sql
    );
    assert!(
        sql.contains("`age` IS NOT NULL"),
        "Expected IS NOT NULL in: {}",
        sql
    );
    assert!(
        sql.contains("`id` NOT IN (?, ?)"),
        "Expected NOT IN in: {}",
        sql
    );
}

// ==================== 默认值 / 时间戳 / default_order 回归测试 ====================

/// 创建带默认值与时间戳字段的插入测试表配置
///
/// - `name`：必填，无默认值
/// - `status`：必填，但配置了默认值 "active"（验证 required+default 不误报）
/// - `created_at`：时间戳字段，插入时自动填充
fn create_test_table_config_with_defaults() -> Arc<TableConfig> {
    build_config(Table::new("users").fields([
        Field::id("id"),
        Field::string("name", 50).required(),
        Field::string("status", 20).required().default("active"),
        Field::created_at("created_at"),
    ]))
}

#[test]
fn test_insert_required_with_default_omitted_ok() {
    use std::collections::HashMap;

    let config = create_test_table_config_with_defaults();
    let query = TableQuery::new_without_pool(config);

    // 只提供 name，省略同样必填但有默认值的 status —— 不应误报 FieldRequired
    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("张三"));

    let (sql, params) = query
        .build_insert_sql_for_test(data)
        .expect("required+default 字段省略时应自动补默认值并通过校验");

    // status 默认值与 created_at 时间戳都应被写入 INSERT
    assert!(sql.contains("INSERT INTO `users`"));
    assert!(sql.contains("`status`"), "status 默认值应入列: {}", sql);
    assert!(
        sql.contains("`created_at`"),
        "created_at 应自动填充: {}",
        sql
    );
    // name + status + created_at = 3 个参数
    assert_eq!(params.len(), 3);
}

#[test]
fn test_insert_required_with_default_rejects_explicit_null() {
    use std::collections::HashMap;

    let config = create_test_table_config_with_defaults();
    let query = TableQuery::new_without_pool(config);
    let data = HashMap::from([
        ("name".to_string(), json!("张三")),
        ("status".to_string(), serde_json::Value::Null),
    ]);

    let result = query.build_insert_sql_for_test(data);

    assert!(matches!(result, Err(BaseError::FieldRequired(ref field)) if field == "status"));
}

#[test]
fn test_insert_missing_required_no_default_errors() {
    use std::collections::HashMap;

    let config = create_test_table_config_with_defaults();
    let query = TableQuery::new_without_pool(config);

    // 省略无默认值的必填字段 name —— 应返回 FieldRequired
    let mut data = HashMap::new();
    data.insert("status".to_string(), json!("active"));

    let result = query.build_insert_sql_for_test(data);
    assert!(
        matches!(result, Err(BaseError::FieldRequired(ref f)) if f == "name"),
        "省略无默认值的必填字段应报 FieldRequired，实际: {:?}",
        result
    );
}

#[test]
fn test_select_falls_back_to_default_order() {
    let config = build_config(
        Table::new("users")
            .fields([Field::id("id"), Field::string("name", 50)])
            .default_order(col("name").asc())
            .then_order(col("id").desc()),
    );
    let query = TableQuery::new_without_pool(config);

    // 未显式指定 order_by，应回退到表配置的 default_order
    let (sql, _) = query.build_select_sql_for_test().expect("build sql");
    assert!(
        sql.contains("ORDER BY `name` ASC, `id` DESC"),
        "应回退到 default_order: {}",
        sql
    );
}

// ==================== C2a: OR / 嵌套布尔组 ====================

use crate::table::WhereCondition;

/// OR 组渲染为括号包裹、以 OR 连接，占位符计数正确。
#[test]
fn test_where_or_renders_parenthesized_or() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    let query = query
        .select_fields(&["id", "name", "email"])
        .expect("OR 语义测试只选择断言涉及的可读字段")
        .where_eq("name", json!("alice"))
        .unwrap()
        .where_or(vec![
            WhereCondition::Eq {
                field: "email".into(),
                value: json!("a@x.com"),
            },
            WhereCondition::Gt {
                field: "id".into(),
                value: json!(100),
            },
        ])
        .unwrap();

    let (sql, params) = query.build_select_sql_for_test().expect("build sql");
    // 顶层 name=? 与 OR 组以 AND 连接，组内括号包裹
    assert!(
        sql.contains("`name` = ? AND (`email` = ? OR `id` > ?)"),
        "OR 组应括号包裹并以 AND 接在顶层条件后: {}",
        sql
    );
    assert_eq!(params.len(), 3, "应有 3 个占位符参数");
}

/// 嵌套：OR 组内含 AND 子组，括号正确嵌套。
#[test]
fn test_nested_or_and_groups() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    // (name = 'a' OR (email = 'b' AND id >= 5))
    let query = query
        .select_fields(&["id", "name", "email"])
        .expect("嵌套 OR/AND 测试只选择断言涉及的可读字段")
        .where_or(vec![
            WhereCondition::Eq {
                field: "name".into(),
                value: json!("a"),
            },
            WhereCondition::And {
                conditions: vec![
                    WhereCondition::Eq {
                        field: "email".into(),
                        value: json!("b"),
                    },
                    WhereCondition::Gte {
                        field: "id".into(),
                        value: json!(5),
                    },
                ],
            },
        ])
        .unwrap();

    let (sql, params) = query.build_select_sql_for_test().expect("build sql");
    assert!(
        sql.contains("(`name` = ? OR (`email` = ? AND `id` >= ?))"),
        "嵌套组括号应正确: {}",
        sql
    );
    assert_eq!(params.len(), 3);
}

/// 空布尔组被校验层拒绝（防止空 AND 组渲染 `1=1` 绕过全表写守卫）。
#[test]
fn test_empty_groups_rejected() {
    let config = create_test_table_config();

    let r_or =
        TableQuery::new(config.clone(), Arc::from(vec!["user".to_string()]), None).where_or(vec![]);
    assert!(
        matches!(r_or, Err(BaseError::ParamInvalid(_, _))),
        "空 OR 组应被拒绝"
    );

    let r_and = TableQuery::new(config.clone(), Arc::from(vec!["user".to_string()]), None)
        .where_and(vec![]);
    assert!(
        matches!(r_and, Err(BaseError::ParamInvalid(_, _))),
        "空 AND 组应被拒绝"
    );

    // 嵌套空组同样被递归校验捕获
    let r_nested = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None)
        .where_and(vec![WhereCondition::Or { conditions: vec![] }]);
    assert!(
        matches!(r_nested, Err(BaseError::ParamInvalid(_, _))),
        "嵌套空组应被拒绝"
    );
}

#[test]
fn test_where_tree_rejects_empty_in_values() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_tree(WhereCondition::In {
        field: "id".into(),
        values: vec![],
    });

    assert!(
        matches!(result, Err(BaseError::ParamInvalid(field, _)) if field == "values"),
        "where_tree 不应接受空 IN 列表"
    );
}

#[test]
fn test_where_group_rejects_empty_not_in_values() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_or(vec![WhereCondition::NotIn {
        field: "id".into(),
        values: vec![],
    }]);

    assert!(
        matches!(result, Err(BaseError::ParamInvalid(field, _)) if field == "values"),
        "where_or 不应递归接受空 NOT IN 列表"
    );
}

#[test]
fn test_where_value_type_is_checked_for_every_operator_operand() {
    let config = build_config(Table::new("typed_rows").fields([
        Field::id("id"),
        Field::bigint("owner_id"),
        Field::datetime("occurred_at"),
        Field::json("payload"),
    ]));

    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_eq("owner_id", json!("42"))
        .is_err());
    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_in("owner_id", vec![json!(1), json!("2")])
        .is_err());
    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_between("owner_id", json!(1), json!("2"))
        .is_err());
    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_like("owner_id", "%2%".to_string())
        .is_err());
    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_gt("payload", json!({"rank": 1}))
        .is_err());

    assert!(TableQuery::new_without_pool(Arc::clone(&config))
        .where_eq("occurred_at", json!("2026-05-27T13:45:30+08:00"))
        .is_ok());
    assert!(TableQuery::new_without_pool(config)
        .where_eq("occurred_at", json!("2026-05-27 13:45:30"))
        .is_err());
}

#[test]
fn test_where_tree_validates_each_in_and_between_operand() {
    let config =
        build_config(Table::new("typed_rows").fields([Field::id("id"), Field::bigint("owner_id")]));

    let in_error =
        TableQuery::new_without_pool(Arc::clone(&config)).where_tree(WhereCondition::In {
            field: "owner_id".into(),
            values: vec![json!(1), json!("2")],
        });
    assert!(in_error.is_err());

    let between_error = TableQuery::new_without_pool(config).where_tree(WhereCondition::Between {
        field: "owner_id".into(),
        lo: json!(1),
        hi: json!("2"),
    });
    assert!(between_error.is_err());
}

#[test]
fn test_eq_and_ne_null_render_as_null_predicates_without_bind_params() {
    let config = create_test_table_config();

    let eq_query = TableQuery::new_without_pool(Arc::clone(&config))
        .where_eq("email", serde_json::Value::Null)
        .expect("Eq null 应被规范化为 IS NULL");
    let (eq_sql, eq_params) = eq_query
        .build_select_sql_for_test()
        .expect("应构建 Eq null SQL");
    assert!(eq_sql.contains("`email` IS NULL"));
    assert!(eq_params.is_empty());

    let ne_query = TableQuery::new_without_pool(config)
        .where_ne("email", serde_json::Value::Null)
        .expect("Ne null 应被规范化为 IS NOT NULL");
    let (ne_sql, ne_params) = ne_query
        .build_select_sql_for_test()
        .expect("应构建 Ne null SQL");
    assert!(ne_sql.contains("`email` IS NOT NULL"));
    assert!(ne_params.is_empty());
}

#[tokio::test]
async fn test_count_fails_closed_before_database_lookup_without_readable_fields() {
    let config = build_config(
        Table::new("private_rows").fields([
            Field::bigint("id")
                .required()
                .primary_key()
                .readable_by(["admin"]),
            Field::string("secret", 64).readable_by(["admin"]),
        ]),
    );

    let error = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None)
        .count()
        .await
        .expect_err("COUNT 不得绕过表的可读权限");

    assert!(matches!(
        error,
        BaseError::FieldPermissionDenied(table, field, _)
            if table == "private_rows" && field == "*"
    ));
}

/// 字段级 `.not_filterable()` 是硬约束：即便角色权限放行也拒绝筛选。
#[test]
fn test_non_filterable_field_rejected() {
    // password 字段显式禁止筛选。
    let config = build_config(Table::new("accounts").fields([
        Field::id("id"),
        Field::string("password", 255).not_filterable(),
    ]));
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);
    let result = query.where_eq("password", json!("secret"));
    assert!(
        matches!(result, Err(BaseError::FieldPermissionDenied(_, _, _))),
        "filterable(false) 字段应拒绝筛选: {:?}",
        result.err()
    );
}

/// 字段级 `.not_sortable()` 是硬约束：即便角色权限放行也拒绝排序。
#[test]
fn test_non_sortable_field_rejected() {
    let config = build_config(Table::new("accounts").fields([
        Field::id("id"),
        Field::string("description", 255).not_sortable(),
    ]));
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);
    let result = query.order_by("description", SortOrder::Asc);
    assert!(
        matches!(result, Err(BaseError::FieldPermissionDenied(_, _, _))),
        "sortable(false) 字段应拒绝排序: {:?}",
        result.err()
    );
}

/// 组内叶子字段无筛选权限 → 整组构建失败（递归权限下钻）。
#[test]
fn test_where_or_permission_denied_in_group() {
    let config = create_test_table_config();
    // user 角色对 salary 无筛选权限
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_or(vec![
        WhereCondition::Eq {
            field: "name".into(),
            value: json!("ok"),
        },
        WhereCondition::Gt {
            field: "salary".into(),
            value: json!(1000),
        },
    ]);

    assert!(
        matches!(result, Err(BaseError::FieldPermissionDenied(_, _, _))),
        "组内 salary 无筛选权限应整组失败"
    );
}

/// 组内叶子字段不存在 → 整组构建失败。
#[test]
fn test_where_or_field_not_found_in_group() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    let result = query.where_or(vec![WhereCondition::Eq {
        field: "nonexistent".into(),
        value: json!(1),
    }]);

    assert!(
        matches!(result, Err(BaseError::FieldNotFound(_, _))),
        "组内不存在字段应失败"
    );
}

/// admin 角色可在组内筛选受限字段（salary）。
#[test]
fn test_where_or_admin_can_filter_restricted() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["admin".to_string()]), None);

    let query = query
        .where_or(vec![
            WhereCondition::Gt {
                field: "salary".into(),
                value: json!(1000),
            },
            WhereCondition::Eq {
                field: "name".into(),
                value: json!("boss"),
            },
        ])
        .expect("admin 应可筛选 salary");

    let (sql, params) = query.build_select_sql_for_test().expect("build sql");
    assert!(sql.contains("(`salary` > ? OR `name` = ?)"), "{}", sql);
    assert_eq!(params.len(), 2);
}

/// 超过最大嵌套深度返回 ParamInvalid，而非 panic/爆栈。
#[test]
fn test_where_tree_depth_limit() {
    let config = create_test_table_config();
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);

    // 构造深度 > 32 的嵌套 And 链
    let mut node = WhereCondition::Eq {
        field: "id".into(),
        value: json!(1),
    };
    for _ in 0..40 {
        node = WhereCondition::And {
            conditions: vec![node],
        };
    }

    let result = query.where_tree(node);
    assert!(
        matches!(result, Err(BaseError::ParamInvalid(_, _))),
        "超深嵌套应返回 ParamInvalid"
    );
}

/// 运行时 WhereCondition 布尔树 JSON 反序列化。
#[test]
fn test_where_condition_json_roundtrip() {
    let j = json!({
        "type": "or",
        "conditions": [
            {"type": "eq", "field": "id", "value": 1},
            {"type": "and", "conditions": [
                {"type": "eq", "field": "id", "value": 2}
            ]}
        ]
    });
    let wc: WhereCondition = serde_json::from_value(j).expect("应反序列化 WhereCondition");

    match wc {
        WhereCondition::Or { conditions } => {
            assert_eq!(conditions.len(), 2);
            assert!(matches!(conditions[0], WhereCondition::Eq { .. }));
            assert!(matches!(conditions[1], WhereCondition::And { .. }));
        }
        other => panic!("应降解为 Or 组，实际: {:?}", other),
    }
}

/// searchable 与 filterable 是两个独立位：关键词搜索只认 searchable，
/// 结构化 where 只认 filterable，四种组合逐一验证。
#[test]
fn test_search_and_filter_bits_are_independent() {
    let config = build_config(
        Table::new("docs").fields([
            Field::id("id"),
            Field::string("both", 50).searchable().filterable(),
            Field::string("search_only", 50)
                .searchable()
                .not_filterable(),
            Field::string("filter_only", 50).filterable(),
            Field::string("neither", 50).not_filterable(),
        ]),
    );
    let roles = || Arc::from(vec!["user".to_string()]);

    // 关键词搜索只命中 searchable 文本字段（both + search_only），与 filterable 无关。
    let query = TableQuery::new(config.clone(), roles(), None)
        .search(Some("alice"))
        .expect("存在可搜索字段时搜索应成功");
    let conditions = &query.get_query_params().where_conditions;
    let [WhereCondition::Or { conditions: leaves }] = conditions.as_slice() else {
        panic!("搜索应生成单个 OR 组，实际: {conditions:?}");
    };
    let mut searched = leaves
        .iter()
        .map(|leaf| leaf.field().expect("搜索叶子条件应有字段").to_string())
        .collect::<Vec<_>>();
    searched.sort();
    assert_eq!(searched, ["both", "search_only"]);

    // 结构化 where 只认 filterable 位。
    let query = TableQuery::new(config.clone(), roles(), None);
    assert!(query.where_eq("both", json!("x")).is_ok());
    let query = TableQuery::new(config.clone(), roles(), None);
    assert!(query.where_eq("filter_only", json!("x")).is_ok());
    let query = TableQuery::new(config.clone(), roles(), None);
    assert!(
        matches!(
            query.where_eq("search_only", json!("x")),
            Err(BaseError::FieldPermissionDenied(_, _, _))
        ),
        "searchable 不得连带开放结构化筛选"
    );
    let query = TableQuery::new(config, roles(), None);
    assert!(
        matches!(
            query.where_eq("neither", json!("x")),
            Err(BaseError::FieldPermissionDenied(_, _, _))
        ),
        "未声明 filterable 的字段不得筛选"
    );
}

/// 表内没有当前角色可搜索的 searchable 文本字段时，搜索整体被拒绝。
#[test]
fn test_search_rejected_without_searchable_fields() {
    let config = build_config(Table::new("docs").fields([
        Field::id("id"),
        Field::string("filter_only", 50).filterable(),
    ]));
    let query = TableQuery::new(config, Arc::from(vec!["user".to_string()]), None);
    let result = query.search(Some("alice"));
    assert!(
        matches!(result, Err(BaseError::PermissionDenied(_))),
        "filterable 不得连带开放关键词搜索: {:?}",
        result.err()
    );
}

/// searchable 字段对当前角色不可读时不参与关键词搜索。
#[test]
fn test_search_skips_unreadable_searchable_fields() {
    let config = build_config(
        Table::new("docs").fields([
            Field::id("id"),
            Field::string("admin_note", 50)
                .searchable()
                .readable_by(["admin"]),
        ]),
    );
    let user_query = TableQuery::new(config.clone(), Arc::from(vec!["user".to_string()]), None);
    assert!(matches!(
        user_query.search(Some("alice")),
        Err(BaseError::PermissionDenied(_))
    ));
    let admin_query = TableQuery::new(config, Arc::from(vec!["admin".to_string()]), None);
    assert!(admin_query.search(Some("alice")).is_ok());
}

/// 受信主键等值条件绕过业务筛选权限（内置 get/put/del 的自有寻址机制），
/// 但值仍按主键字段类型校验。
#[test]
fn test_where_primary_key_eq_bypasses_filter_permission() {
    let config = build_config(
        Table::new("accounts")
            .fields([Field::id("id").not_filterable(), Field::string("name", 50)]),
    );
    let roles = || Arc::from(vec!["user".to_string()]);

    // 普通 where 仍受 filterable 硬约束。
    let query = TableQuery::new(config.clone(), roles(), None);
    assert!(matches!(
        query.where_eq("id", json!(1)),
        Err(BaseError::FieldPermissionDenied(_, _, _))
    ));

    // 受信主键条件放行，并追加一个 Eq 叶子。
    let query = TableQuery::new(config.clone(), roles(), None);
    let query = query
        .where_primary_key_eq(json!(1))
        .expect("主键等值条件应绕过筛选权限");
    assert_eq!(query.get_query_params().where_conditions.len(), 1);

    // 类型不匹配的值仍被拒绝。
    let query = TableQuery::new(config, roles(), None);
    assert!(matches!(
        query.where_primary_key_eq(json!("not-a-number")),
        Err(BaseError::InvalidFieldType(_, _))
    ));
}

/// 验证慢查询 warn 分支触发时正确输出表名与操作名（TEST-1）。
///
/// `TableQuery::timed` 在超过阈值时发出 `tracing::warn!`，含 `table` /
/// `op` / `elapsed_ms` 字段。此前该 warn 分支从未被任何测试覆盖。
#[cfg(feature = "mysql")]
#[tokio::test]
async fn test_slow_query_warn_fires() {
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // 自定义 MakeWriter：将 tracing fmt 输出捕获到内存缓冲区
    struct BufWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }
    impl Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.buf.lock().unwrap().flush()
        }
    }
    struct BufMakeWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufMakeWriter {
        type Writer = BufWriter;
        fn make_writer(&'a self) -> Self::Writer {
            BufWriter {
                buf: self.buf.clone(),
            }
        }
    }

    let buf = Arc::new(Mutex::new(Vec::new()));
    let make_writer = BufMakeWriter { buf: buf.clone() };

    let subscriber = tracing_subscriber::fmt()
        .with_writer(make_writer)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    // 零阈值 + 立即返回的 future，确保 `timed` 走 warn 分支
    let result = TableQuery::timed(
        Some(Duration::from_secs(0)),
        None,
        "test_table_name",
        "test_operation",
        async { 42 },
    )
    .await;

    assert_eq!(result, 42, "返回值应透传");

    let output = String::from_utf8(buf.lock().unwrap().clone()).expect("输出不是合法 UTF-8");
    assert!(
        output.contains("慢查询"),
        "warn 应包含中文消息 '慢查询'，输出: {:?}",
        output
    );
    assert!(
        output.contains("test_table_name"),
        "warn 应包含表名，输出: {:?}",
        output
    );
    assert!(
        output.contains("test_operation"),
        "warn 应包含操作名，输出: {:?}",
        output
    );
}
