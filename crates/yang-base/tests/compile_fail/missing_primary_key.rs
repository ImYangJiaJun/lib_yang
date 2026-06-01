//! compile_fail: 派生 TableEntity 时缺少 #[entity(primary_key)] 应编译失败。
//!
//! 错误来自 yang-base-derive 的 `abort!`，文案稳定（不依赖 rustc 内部）。

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
    id: i64,
    name: String,
}

fn main() {}
