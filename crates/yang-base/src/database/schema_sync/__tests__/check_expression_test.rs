use super::super::render::normalize_check_expression;

#[test]
fn mysql_metadata_decorations_do_not_break_check_idempotency() {
    assert_eq!(
        normalize_check_expression("(`status` in (_utf8mb4\\'active\\',_utf8mb4\\'disabled\\'))"),
        normalize_check_expression("`status` IN ('active', 'disabled')")
    );
}

#[test]
fn internal_parentheses_remain_semantically_significant() {
    assert_ne!(
        normalize_check_expression("`a` AND (`b` OR `c`)"),
        normalize_check_expression("(`a` AND `b`) OR `c`")
    );
}

#[test]
fn multi_layer_backslash_escapes_converge() {
    // MySQL 各元数据路径对 CHECK 字符串字面量叠加不同层数的反斜杠转义，
    // 声明侧（2 层）与 information_schema 读回侧（4/8 层）必须归一相等。
    let declared = normalize_check_expression(
        "regexp_like(`permission`, '^[a-z][a-z0-9_]*(\\\\.[a-z][a-z0-9_]*)+$')",
    );
    let read_back_4 = normalize_check_expression(
        "regexp_like(`permission`,_utf8mb4\\'^[a-z][a-z0-9_]*(\\\\\\\\.[a-z][a-z0-9_]*)+$\\')",
    );
    let read_back_8 = normalize_check_expression(
        "regexp_like(`permission`,_utf8mb4\\\\'^[a-z][a-z0-9_]*(\\\\\\\\\\\\\\\\.[a-z][a-z0-9_]*)+$\\\\')",
    );
    assert_eq!(declared, read_back_4);
    assert_eq!(declared, read_back_8);
}
