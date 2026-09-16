// JSON 数字的绑定分类
//
// M29/NEW-11：raw-SQL 绑定路径此前 `as_i64() -> else as_f64()`，超过 `i64::MAX` 的
// u64（雪花 ID 等）会静默落进 f64 丢失精度。本模块提供唯一判定点，供 MySQL/PG 两侧
// 共 6 处绑定宏/助手统一使用，与 `SqlValue::from(u64)` 同口径。

/// JSON 数字的绑定分类：优先 i64，其次 u64 顶半区（> i64::MAX），最后 f64。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum JsonNumber {
    Int(i64),
    Uint(u64),
    Float(f64),
}

/// 按 `i64 → u64 顶半区 → f64` 的顺序分类一个 JSON 数字。
///
/// u64 顶半区若落进 f64 会静默丢精度（`u64::MAX` → `"18446744073709552000"`），
/// 必须先于 f64 判定。
pub(crate) fn classify_json_number(n: &serde_json::Number) -> Option<JsonNumber> {
    if let Some(i) = n.as_i64() {
        Some(JsonNumber::Int(i))
    } else if let Some(u) = n.as_u64() {
        Some(JsonNumber::Uint(u))
    } else {
        n.as_f64().map(JsonNumber::Float)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn u64_top_half_is_not_float() {
        let n = serde_json::json!(u64::MAX);
        assert_eq!(
            classify_json_number(n.as_number().expect("number")),
            Some(JsonNumber::Uint(u64::MAX))
        );
        assert_eq!(
            classify_json_number(serde_json::json!(i64::MAX).as_number().expect("n")),
            Some(JsonNumber::Int(i64::MAX))
        );
        assert_eq!(
            classify_json_number(serde_json::json!(1.5).as_number().expect("n")),
            Some(JsonNumber::Float(1.5))
        );
    }
}
