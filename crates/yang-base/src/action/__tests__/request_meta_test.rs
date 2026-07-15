//! RequestMeta 传输元数据契约测试。

use crate::action::{Request, RequestMeta};
use serde_json::json;
use std::net::SocketAddr;

#[test]
fn request_meta_defaults_to_missing_transport_fields() {
    let meta = RequestMeta::default();

    assert_eq!(meta.method, None);
    assert_eq!(meta.original_uri, None);
    assert_eq!(meta.scheme, None);
    assert_eq!(meta.peer_addr, None);
    assert_eq!(meta.local_addr, None);
    assert!(meta.extensions.is_empty());
}

#[test]
fn request_meta_builders_preserve_present_transport_fields() {
    let peer: SocketAddr = "203.0.113.10:43120".parse().expect("peer 地址应合法");
    let local: SocketAddr = "10.0.0.8:443".parse().expect("local 地址应合法");
    let meta = RequestMeta::new()
        .with_method("POST")
        .with_original_uri("https://api.example.test/users?token=uri-secret")
        .with_scheme("https")
        .with_peer_addr(peer)
        .with_local_addr(local)
        .extension("transport.request_target", "/users?token=extension-secret");

    assert_eq!(meta.method.as_deref(), Some("POST"));
    assert_eq!(
        meta.original_uri.as_deref(),
        Some("https://api.example.test/users?token=uri-secret")
    );
    assert_eq!(meta.scheme.as_deref(), Some("https"));
    assert_eq!(meta.peer_addr, Some(peer));
    assert_eq!(meta.local_addr, Some(local));
    assert_eq!(
        meta.extensions
            .get("transport.request_target")
            .map(String::as_str),
        Some("/users?token=extension-secret")
    );
}

#[test]
fn request_and_meta_debug_redact_transport_secrets() {
    let request = Request::new(json!({ "safe": true }))
        .header("Authorization", "Bearer header-secret")
        .header("Cookie", "session=cookie-secret")
        .header("User-Agent", "PrivateClient/1.0");
    let meta = RequestMeta::new()
        .with_method("GET")
        .with_original_uri("https://user:password@example.test/private?token=uri-secret")
        .with_scheme("https")
        .with_peer_addr("203.0.113.10:43120".parse().expect("peer 地址应合法"))
        .with_local_addr("10.0.0.8:443".parse().expect("local 地址应合法"))
        .extension("transport.authorization", "extension-secret");

    let request_debug = format!("{request:?}");
    let meta_debug = format!("{meta:?}");

    assert_eq!(request.user_agent(), Some("PrivateClient/1.0"));
    assert_eq!(request.cookie(), Some("session=cookie-secret"));
    for secret in [
        "header-secret",
        "cookie-secret",
        "PrivateClient/1.0",
        "user:password",
        "uri-secret",
        "203.0.113.10",
        "10.0.0.8",
        "extension-secret",
    ] {
        assert!(
            !request_debug.contains(secret),
            "Request Debug 泄露: {secret}"
        );
        assert!(
            !meta_debug.contains(secret),
            "RequestMeta Debug 泄露: {secret}"
        );
    }
    assert!(request_debug.contains("authorization"));
    assert!(meta_debug.contains("GET"));
    assert!(meta_debug.contains("https"));
    assert!(meta_debug.contains("[REDACTED]"));
}
