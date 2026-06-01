//! compile_fail: where 条件传入与字段类型不匹配的值应编译失败。
//!
//! 派生为 `id: i64` 生成的 `UWhere::Id` 持有 `WhereOp<i64>`；
//! 这里塞入 `&str`，类型不匹配，编译期拒绝。

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
    let _: UWhere = UWhere::Id(WhereOp::Eq("not an integer"));
}
