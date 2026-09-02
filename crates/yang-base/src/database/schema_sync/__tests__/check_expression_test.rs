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
