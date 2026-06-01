//! compile_fail: 在数值字段上使用 Like 操作符应编译失败。
//!
//! 通用 `WhereOp<V>` 没有 `Like` 变体——只有字符串字段对应的 `StringWhereOp` 才有。
//! 因此对 `WhereOp<i64>` 构造 `Like` 会找不到该变体，编译期拒绝。

use yang_base::table::WhereOp;

fn main() {
    let _: WhereOp<i64> = WhereOp::Like("%foo%".into());
}
