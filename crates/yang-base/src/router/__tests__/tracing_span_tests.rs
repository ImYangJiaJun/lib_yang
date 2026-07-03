//! Tracing span 字段验证测试（TEST-2）
//!
//! 验证 dispatch/handle span 的创建和基本字段存在性。
//! 使用 `tracing_subscriber::fmt()` + `try_init()` 确保并行测试安全。
//!
//! 注意: fmt subscriber 无法直接读取 span 的字段键值对（仅输出文本），
//! 完整字段值断言需自定义 `tracing::Subscriber` + `Visit` 实现。
//! 参见 docs/yang-base-db-optimization-guide.md TEST-2 修正方案。

fn try_init_subscriber() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

/// 验证 dispatch span 创建和 request_id record 不 panic
#[test]
fn test_dispatch_span_fields() {
    try_init_subscriber();

    let span = tracing::info_span!(
        "dispatch",
        module = "test_module",
        action = "test_action",
        request_id = tracing::field::Empty,
    );
    let _enter = span.enter();
    span.record("request_id", "req-0000000000000000");
}

/// 验证 handle span 创建不 panic
#[test]
fn test_handle_span_fields() {
    try_init_subscriber();

    let span = tracing::info_span!("handle", action = "get_user");
    let _enter = span.enter();
}

/// 验证 dispatch 嵌套 handle span 不互相干扰
#[test]
fn test_dispatch_nested_handle() {
    try_init_subscriber();

    let dispatch = tracing::info_span!(
        "dispatch",
        module = "users",
        action = "get",
        request_id = tracing::field::Empty,
    );
    let _d_enter = dispatch.enter();
    dispatch.record("request_id", "req-1234567890abcdef");

    let handle = tracing::info_span!("handle", action = "get_user");
    let _h_enter = handle.enter();
}

/// 验证多次 record 同一字段不 panic
#[test]
fn test_multiple_records_same_field() {
    try_init_subscriber();

    let span = tracing::info_span!(
        "dispatch",
        module = "test",
        action = "update",
        request_id = tracing::field::Empty,
    );
    let _enter = span.enter();
    span.record("request_id", "req-aaa");
    span.record("request_id", "req-bbb");
}
