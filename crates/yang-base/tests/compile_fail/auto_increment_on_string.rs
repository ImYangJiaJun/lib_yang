//! compile_fail: `auto_increment` 只允许标记整数主键字段。

use yang_base_derive::TableEntity as DeriveTableEntity;

#[derive(DeriveTableEntity)]
#[table(name = "invalid_accounts")]
struct InvalidAccount {
    #[entity(primary_key, auto_increment)]
    id: String,
}

fn main() {}
