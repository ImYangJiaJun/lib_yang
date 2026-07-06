//! 查询参数结构单元测试

use crate::table::{PaginatedResult, QueryParams, SortOrder, WhereCondition};
use serde_json::json;

#[test]
fn test_where_condition_eq() {
    let condition = WhereCondition::Eq {
        field: "status".to_string(),
        value: json!("active"),
    };

    assert_eq!(condition.field(), Some("status"));
}

#[test]
fn test_where_condition_in() {
    let condition = WhereCondition::In {
        field: "role".to_string(),
        values: vec![json!("admin"), json!("user")],
    };

    assert_eq!(condition.field(), Some("role"));
}

#[test]
fn test_where_condition_like() {
    let condition = WhereCondition::Like {
        field: "name".to_string(),
        pattern: "%alice%".to_string(),
    };

    assert_eq!(condition.field(), Some("name"));
}

#[test]
fn test_where_condition_gt() {
    let condition = WhereCondition::Gt {
        field: "age".to_string(),
        value: json!(18),
    };

    assert_eq!(condition.field(), Some("age"));
}

#[test]
fn test_where_condition_gte() {
    let condition = WhereCondition::Gte {
        field: "age".to_string(),
        value: json!(18),
    };

    assert_eq!(condition.field(), Some("age"));
}

#[test]
fn test_where_condition_lt() {
    let condition = WhereCondition::Lt {
        field: "age".to_string(),
        value: json!(65),
    };

    assert_eq!(condition.field(), Some("age"));
}

#[test]
fn test_where_condition_lte() {
    let condition = WhereCondition::Lte {
        field: "age".to_string(),
        value: json!(65),
    };

    assert_eq!(condition.field(), Some("age"));
}

#[test]
fn test_where_condition_is_null() {
    let condition = WhereCondition::IsNull {
        field: "deleted_at".to_string(),
    };

    assert_eq!(condition.field(), Some("deleted_at"));
}

#[test]
fn test_where_condition_is_not_null() {
    let condition = WhereCondition::IsNotNull {
        field: "deleted_at".to_string(),
    };

    assert_eq!(condition.field(), Some("deleted_at"));
}

#[test]
fn test_where_condition_serialization() {
    let condition = WhereCondition::Eq {
        field: "status".to_string(),
        value: json!("active"),
    };

    let json_str = serde_json::to_string(&condition).unwrap();
    let deserialized: WhereCondition = serde_json::from_str(&json_str).unwrap();

    assert_eq!(condition, deserialized);
}

#[test]
fn test_query_params_new() {
    let params = QueryParams::new();

    assert!(params.fields.is_none());
    assert!(params.where_conditions.is_empty());
    assert!(params.order_by.is_empty());
    assert!(params.page.is_none());
    assert!(params.page_size.is_none());
}

#[test]
fn test_query_params_with_fields() {
    let params = QueryParams::new().with_fields(vec!["id".to_string(), "name".to_string()]);

    assert_eq!(
        params.fields,
        Some(vec!["id".to_string(), "name".to_string()])
    );
}

#[test]
fn test_query_params_with_condition() {
    let params = QueryParams::new().with_condition(WhereCondition::Eq {
        field: "status".to_string(),
        value: json!("active"),
    });

    assert_eq!(params.where_conditions.len(), 1);
    assert_eq!(params.where_conditions[0].field(), Some("status"));
}

#[test]
fn test_query_params_with_multiple_conditions() {
    let params = QueryParams::new()
        .with_condition(WhereCondition::Eq {
            field: "status".to_string(),
            value: json!("active"),
        })
        .with_condition(WhereCondition::Gt {
            field: "age".to_string(),
            value: json!(18),
        })
        .with_condition(WhereCondition::In {
            field: "role".to_string(),
            values: vec![json!("admin"), json!("user")],
        });

    assert_eq!(params.where_conditions.len(), 3);
}

#[test]
fn test_query_params_with_order_by() {
    let params = QueryParams::new().with_order_by("created_at".to_string(), SortOrder::Desc);

    assert_eq!(params.order_by.len(), 1);
    assert_eq!(params.order_by[0].0, "created_at");
    assert_eq!(params.order_by[0].1, SortOrder::Desc);
}

#[test]
fn test_query_params_with_multiple_order_by() {
    let params = QueryParams::new()
        .with_order_by("created_at".to_string(), SortOrder::Desc)
        .with_order_by("name".to_string(), SortOrder::Asc);

    assert_eq!(params.order_by.len(), 2);
}

#[test]
fn test_query_params_with_pagination() {
    let params = QueryParams::new().with_pagination(1, 20);

    assert_eq!(params.page, Some(1));
    assert_eq!(params.page_size, Some(20));
}

#[test]
fn test_query_params_normalize_clamps_invalid_pagination() {
    let mut params = QueryParams::new().with_pagination(0, 0);
    params.normalize();

    assert_eq!(params.page, Some(1));
    assert_eq!(params.page_size, Some(10));

    let mut params = QueryParams::new().with_pagination(1, 101);
    params.normalize();

    assert_eq!(params.page, Some(1));
    assert_eq!(params.page_size, Some(100));
}

#[test]
fn test_query_params_builder_chain() {
    let params = QueryParams::new()
        .with_fields(vec!["id".to_string(), "name".to_string()])
        .with_condition(WhereCondition::Eq {
            field: "status".to_string(),
            value: json!("active"),
        })
        .with_order_by("created_at".to_string(), SortOrder::Desc)
        .with_pagination(1, 20);

    assert!(params.fields.is_some());
    assert_eq!(params.where_conditions.len(), 1);
    assert_eq!(params.order_by.len(), 1);
    assert_eq!(params.page, Some(1));
    assert_eq!(params.page_size, Some(20));
}

#[test]
fn test_query_params_serialization() {
    let params = QueryParams::new()
        .with_fields(vec!["id".to_string(), "name".to_string()])
        .with_condition(WhereCondition::Eq {
            field: "status".to_string(),
            value: json!("active"),
        })
        .with_pagination(1, 20);

    let json_str = serde_json::to_string(&params).unwrap();
    let deserialized: QueryParams = serde_json::from_str(&json_str).unwrap();

    assert_eq!(params.fields, deserialized.fields);
    assert_eq!(
        params.where_conditions.len(),
        deserialized.where_conditions.len()
    );
    assert_eq!(params.page, deserialized.page);
    assert_eq!(params.page_size, deserialized.page_size);
}

#[test]
fn test_paginated_result_new() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct User {
        id: i64,
        name: String,
    }

    let users = vec![
        User {
            id: 1,
            name: "Alice".to_string(),
        },
        User {
            id: 2,
            name: "Bob".to_string(),
        },
    ];

    let result = PaginatedResult::new(users, 100, 1, 20);

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.total, 100);
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 20);
    assert_eq!(result.total_pages, 5);
}

#[test]
fn test_paginated_result_total_pages_calculation() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Item {
        id: i64,
    }

    // 测试整除情况
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 1, 20);
    assert_eq!(result.total_pages, 5);

    // 测试有余数情况
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 101, 1, 20);
    assert_eq!(result.total_pages, 6);

    // 测试总数为 0
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 0, 1, 20);
    assert_eq!(result.total_pages, 0);

    // 测试 page_size 为 0
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 1, 0);
    assert_eq!(result.total_pages, 0);
}

#[test]
fn test_paginated_result_empty() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct User {
        id: i64,
        name: String,
    }

    let result: PaginatedResult<User> = PaginatedResult::empty(1, 20);

    assert_eq!(result.data.len(), 0);
    assert_eq!(result.total, 0);
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 20);
    assert_eq!(result.total_pages, 0);
}

#[test]
fn test_paginated_result_has_next() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Item {
        id: i64,
    }

    // 第一页，有下一页
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 1, 20);
    assert!(result.has_next());

    // 最后一页，没有下一页
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 5, 20);
    assert!(!result.has_next());

    // 空结果，没有下一页
    let result: PaginatedResult<Item> = PaginatedResult::empty(1, 20);
    assert!(!result.has_next());
}

#[test]
fn test_paginated_result_has_prev() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Item {
        id: i64,
    }

    // 第一页，没有上一页
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 1, 20);
    assert!(!result.has_prev());

    // 第二页，有上一页
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 2, 20);
    assert!(result.has_prev());

    // 最后一页，有上一页
    let result: PaginatedResult<Item> = PaginatedResult::new(vec![], 100, 5, 20);
    assert!(result.has_prev());
}

#[test]
fn test_paginated_result_serialization() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct User {
        id: i64,
        name: String,
    }

    let users = vec![User {
        id: 1,
        name: "Alice".to_string(),
    }];

    let result = PaginatedResult::new(users, 100, 1, 20);

    let json_str = serde_json::to_string(&result).unwrap();
    let deserialized: PaginatedResult<User> = serde_json::from_str(&json_str).unwrap();

    assert_eq!(result.data, deserialized.data);
    assert_eq!(result.total, deserialized.total);
    assert_eq!(result.page, deserialized.page);
    assert_eq!(result.page_size, deserialized.page_size);
    assert_eq!(result.total_pages, deserialized.total_pages);
}
