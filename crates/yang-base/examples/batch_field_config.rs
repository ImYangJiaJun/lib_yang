//! Schema-first 批量定义表字段示例
//!
//! 展示如何使用 `Table::fields` 一次声明字段，并在 `build` 时生成不可变的
//! `TableDefinition`。查询和 Action 的动态行数据统一使用 `Record`。

use serde_json::json;
use yang_base::router::ModuleRouter;
use yang_base::table::{col, Field, Record, Table, TableDefinition};

fn users_table() -> Result<TableDefinition, yang_base::BaseError> {
    Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id").label("ID"),
            Field::string("username", 50)
                .label("用户名")
                .required()
                .length(3..=50)
                .unique()
                .filterable()
                .sortable(),
            Field::string("email", 100)
                .label("邮箱")
                .required()
                .email()
                .unique(),
            Field::integer("age").label("年龄").min(18.0).max(100.0),
            Field::enumeration("status", ["active", "inactive"])
                .label("状态")
                .default(json!("active")),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("created_at").desc())
        .build()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 数组适合字段集合固定的表。
    let users = users_table()?;
    println!("表名: {}", users.name());
    println!("显示名称: {}", users.label());
    println!("主键: {}", users.primary_key());
    println!("字段数量: {}", users.field_count());

    if let Some(username) = users.field("username") {
        println!(
            "用户名字段: {} ({:?}), 必填: {}",
            username.label(),
            username.field_type(),
            username.is_required()
        );
    }

    // Vec 和其他迭代器同样可以直接传给 fields。
    let product_fields = vec![
        Field::id("id"),
        Field::string("name", 50).required(),
        Field::text("description").nullable(),
    ];
    let products = Table::new("products")
        .label("产品表")
        .fields(product_fields)
        .build()?;
    println!("\n产品表字段数量: {}", products.field_count());

    // 先在 Vec 中组合基础字段和特殊字段，再一次性交给 Table 构建器。
    let mut order_fields = vec![
        Field::id("id"),
        Field::string("order_no", 50).required().unique(),
        Field::bigint("user_id").required().index(),
        Field::float("amount").required().min(0.0).max(999_999.0),
    ];
    order_fields.push(Field::json("metadata").label("元数据").nullable());
    let orders = Table::new("orders")
        .label("订单表")
        .fields(order_fields)
        .build()?;
    println!("订单表字段数量: {}", orders.field_count());

    // Record 是动态查询结果和 CRUD 输入使用的统一行类型。
    let input = Record::new()
        .set("username", "alice")
        .set("email", "alice@example.com")
        .set("status", "active");
    let username: String = input.require("username")?;
    println!("Record 中的用户名: {username}");

    // 绑定表定义后，crud() 一次注册标准增删改查与 schema API。
    let _router = ModuleRouter::new("user", "用户管理").table(users).crud()?;

    Ok(())
}
