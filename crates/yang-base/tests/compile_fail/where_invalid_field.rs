//! compile_fail: where 条件引用派生枚举上不存在的字段变体应编译失败。
//!
//! 派生为 `struct U` 生成的 `UWhere` 只有 `Id` / `Name` 两个变体；
//! `NoSuchField` 不存在，封闭枚举杜绝了任意字段名。

use yang_base::table::WhereOp;
use yang_base_derive::TableEntity as DeriveTableEntity;

#[derive(
    serde::Deserialize,
    serde::Serialize,
    schemars::JsonSchema,
    sqlx::FromRow,
    DeriveTableEntity,
)]
#[table(name = "u")]
struct U {
    #[entity(primary_key)]
    id: i64,
    name: String,
}

fn main() {
    let _ = UWhere::NoSuchField(WhereOp::Eq(1));
}
