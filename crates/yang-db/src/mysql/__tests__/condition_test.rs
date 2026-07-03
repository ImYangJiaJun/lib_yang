//! `SqlValue` 转换单元测试

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::mysql::SqlValue;

/// NEW-11：u64 > i64::MAX 不再静默截断环绕成负数，而是走十进制字符串。
#[test]
fn test_from_u64_top_half_no_truncation() {
    // 小于等于 i64::MAX：仍为 Int
    let small: SqlValue = 42u64.into();
    assert!(matches!(small, SqlValue::Int(42)));

    let max_i64: SqlValue = (i64::MAX as u64).into();
    assert!(matches!(max_i64, SqlValue::Int(v) if v == i64::MAX));

    // i64::MAX + 1：必须走 String，不能环绕成负数
    let over: SqlValue = (i64::MAX as u64 + 1).into();
    match over {
        SqlValue::String(s) => assert_eq!(s, "9223372036854775808"),
        other => panic!("u64 顶半区应转为 String，实得 {:?}", other),
    }

    // u64::MAX：同样走 String
    let umax: SqlValue = u64::MAX.into();
    match umax {
        SqlValue::String(s) => assert_eq!(s, u64::MAX.to_string()),
        other => panic!("u64::MAX 应转为 String，实得 {:?}", other),
    }
}
