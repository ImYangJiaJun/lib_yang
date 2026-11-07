//! 公开面契约：只用 `yang_feishu::...` 的公开 API 完成装配与纯函数调用。
//!
//! 本文件证明宿主可以像独立 crate 一样使用 yang-feishu：不启用真实网络、
//! 不碰 Redis/MySQL，Transport 与 Sleeper 全部用测试替身。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use yang_feishu::auth::MemoryTokenCache;
use yang_feishu::crypto::{
    decrypt_bytes, derive_key, encrypt_bytes, hash_token, tokens_equal, verify_token,
};
use yang_feishu::drive::{
    image_content_type, multipart, parse_attachments, AttachmentError, UploadSpec,
};
use yang_feishu::im::{valid_message_id, MessageRecipient};
use yang_feishu::options::{
    build_result_body, decode_cursor, encode_cursor, normalize_linkage_value, FeishuEnvelope,
    FeishuResultBody, DEFAULT_LOCALE,
};
use yang_feishu::path::{validate_opaque_path_segment, validate_path_segment};
use yang_feishu::{
    Endpoints, Error, FailureKind, Method, OpenApiClient, OpenApiConfig, Request, Response, Result,
    RetryPolicy, Sleeper, Transport,
};

/// 不会真正出站的传输替身：一旦被调用即视为测试失败路径。
struct FakeTransport;

#[async_trait::async_trait]
impl Transport for FakeTransport {
    async fn send(&self, _request: Request) -> Result<Response> {
        Err(Error::fatal("公开面测试不应触发出站"))
    }
}

/// 不真正睡眠的睡眠器。
struct NoSleep;

#[async_trait::async_trait]
impl Sleeper for NoSleep {
    async fn sleep(&self, _duration: Duration) {}
}

#[test]
fn builder_assembles_client_without_real_transport() {
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_public_api", "app-secret")
            .endpoints(Endpoints::feishu())
            .cache_namespace("public-api"),
    )
    .transport(Arc::new(FakeTransport))
    .sleeper(Arc::new(NoSleep))
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()
    .unwrap_or_else(|error| panic!("公开面装配失败: {error}"));

    assert_eq!(client.app_id(), "cli_public_api");
    assert_eq!(client.endpoints(), &Endpoints::feishu());
    assert_eq!(
        client.tokens().token_key(),
        "public-api:app:cli_public_api:tenant_token"
    );
}

#[test]
fn endpoints_presets_match_platform_domains() {
    let feishu = Endpoints::feishu();
    assert_eq!(feishu.open.as_str(), "https://open.feishu.cn");
    assert_eq!(feishu.accounts.as_str(), "https://accounts.feishu.cn");
    assert_eq!(feishu.upload.as_str(), "https://www.feishu.cn");

    let lark = Endpoints::lark();
    assert_eq!(lark.open.as_str(), "https://open.larksuite.com");
    assert_eq!(lark.accounts.as_str(), "https://accounts.larksuite.com");
    assert_eq!(lark.upload.as_str(), "https://www.larksuite.com");

    let custom = Endpoints::custom(
        "https://open.internal.example",
        "https://accounts.internal.example",
        "https://upload.internal.example",
    );
    assert_eq!(custom.open.to_string(), "https://open.internal.example");
    assert_eq!(
        custom.accounts.to_string(),
        "https://accounts.internal.example"
    );
    assert_eq!(custom.upload.to_string(), "https://upload.internal.example");

    assert_eq!(Endpoints::default(), Endpoints::feishu());
}

#[test]
fn strict_and_opaque_path_segments_keep_different_character_sets() {
    // 宽松版放行审批历史值里的 `.` `:` `@`，严格版（多维表格）一律拒绝。
    for value in ["4202AD96-9EC1.2026", "user:123", "a@b"] {
        assert!(
            validate_opaque_path_segment("id", value).is_ok(),
            "宽松版应放行 {value:?}"
        );
        assert!(
            validate_path_segment("id", value).is_err(),
            "严格版应拒绝 {value:?}"
        );
    }

    // 严格版白名单：ASCII 字母数字与 `-` `_`。
    assert!(validate_path_segment("app_token", "bascnCMII2ORej2RItqpZZUNMIe").is_ok());
    assert!(validate_path_segment("record_id", "recABC_123-x").is_ok());

    // 两套语义都必须挡住路径穿越与其他分隔符。
    for value in ["../evil", "a/b", "a?b", "a#b", "a b", "a\\b", ""] {
        assert!(
            validate_path_segment("id", value).is_err(),
            "严格版应拒绝 {value:?}"
        );
        assert!(
            validate_opaque_path_segment("id", value).is_err(),
            "宽松版应拒绝 {value:?}"
        );
    }
}

#[test]
fn crypto_derives_sha256_keys_and_round_trips_bytes() {
    // sha256("") 的已知摘要：derive_key 直接取原文的 SHA-256，不做 hex/base64 猜测。
    let expected_empty = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(derive_key(""), expected_empty);

    let key = derive_key("public-api-secret");
    let plaintext = "飞书外部选项".as_bytes();
    let cipher = encrypt_bytes(plaintext, &key).unwrap_or_else(|error| panic!("{error}"));
    assert_ne!(cipher.as_bytes(), plaintext);
    let decoded = decrypt_bytes(&cipher, &key).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(decoded, plaintext);

    // 换钥匙必须 fail-closed，而不是返回垃圾明文。
    assert!(decrypt_bytes(&cipher, &derive_key("other-secret")).is_err());
    assert!(decrypt_bytes("not-base64!!", &key).is_err());
}

#[test]
fn token_digests_are_lowercase_sha256_and_fail_closed() {
    assert_eq!(
        hash_token("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let stored = hash_token("feishu-token");
    assert!(verify_token("feishu-token", &stored));
    assert!(verify_token("feishu-token", &stored.to_uppercase()));
    assert!(!verify_token("other", &stored));
    assert!(!verify_token("feishu-token", "not-a-hash"));

    assert!(tokens_equal("feishu-token", "feishu-token"));
    assert!(!tokens_equal("feishu-token", "feishu-tokeN"));
    assert!(!tokens_equal("short", "longer"));
}

#[test]
fn envelope_serializes_msg_key_and_result_shapes() {
    let ok_body = build_result_body(&[], DEFAULT_LOCALE, None);
    let ok: serde_json::Value =
        serde_json::from_str(&FeishuEnvelope::ok(ok_body).to_json().unwrap_or_default())
            .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(ok["code"], 0);
    assert_eq!(ok["msg"], "success!");
    assert!(ok.get("message").is_none(), "键必须是 msg，不是 message");
    assert!(ok["data"]["result"]["options"].is_array());
    assert!(ok["data"]["result"]["i18nResources"].is_array());

    let fail: serde_json::Value = serde_json::from_str(
        &FeishuEnvelope::fail(60012, "uuid 已创建")
            .to_json()
            .unwrap_or_default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(fail["code"], 60012);
    assert_eq!(fail["msg"], "uuid 已创建");
    assert!(fail.get("message").is_none());
    assert!(fail["data"].is_null());

    let encrypted: serde_json::Value = serde_json::from_str(
        &FeishuEnvelope::encrypted("base64-cipher".into())
            .to_json()
            .unwrap_or_default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(encrypted["data"]["result"], "base64-cipher");
    assert!(encrypted.get("message").is_none());
}

#[test]
fn build_result_body_always_emits_the_default_locale() {
    let body = build_result_body(&[], DEFAULT_LOCALE, Some("next-page".into()));
    assert_eq!(body.i18n_resources.len(), 1, "即使没有选项也要有默认语言");
    assert_eq!(body.i18n_resources[0].locale, DEFAULT_LOCALE);
    assert!(body.i18n_resources[0].is_default);
    assert!(body.i18n_resources[0].texts.is_empty());
    assert!(body.options.is_empty());
    assert!(body.has_more);
    assert_eq!(body.next_page_token.as_deref(), Some("next-page"));

    // 默认语言由调用方指定；它同样必须恒存在并标记 isDefault。
    let english = build_result_body(&[], "en_us", None);
    assert_eq!(english.i18n_resources.len(), 1);
    assert_eq!(english.i18n_resources[0].locale, "en_us");
    assert!(english.i18n_resources[0].is_default);
    assert_eq!(body.has_more, body.next_page_token.is_some());
}

#[test]
fn empty_result_body_still_carries_the_default_locale() {
    let body = FeishuResultBody::empty(DEFAULT_LOCALE);
    assert!(body.options.is_empty());
    assert!(body.next_page_token.is_none());
    assert!(!body.has_more);
    assert_eq!(body.i18n_resources.len(), 1);
    assert_eq!(body.i18n_resources[0].locale, DEFAULT_LOCALE);
    assert!(body.i18n_resources[0].is_default);
}

#[test]
fn cursor_round_trips_and_malformed_cursors_fail_closed() {
    let encoded = encode_cursor(42, "dept_sales");
    assert!(!encoded.contains("dept_sales"), "游标不应明文暴露业务键");
    assert_eq!(
        decode_cursor(&encoded).unwrap_or_else(|error| panic!("{error}")),
        Some((42, "dept_sales".to_string()))
    );

    // 空串与全空白 = 请求第一页。
    assert_eq!(
        decode_cursor("").unwrap_or_else(|error| panic!("{error}")),
        None
    );
    assert_eq!(
        decode_cursor("   ").unwrap_or_else(|error| panic!("{error}")),
        None
    );

    // 非法 base64、合法 base64 但缺分隔符、分隔符在但排序键不是整数。
    assert!(decode_cursor("not-base64!!").is_err());
    assert!(
        decode_cursor("MTIzNDU").is_err(),
        "base64(\"12345\") 缺 US 分隔符"
    );
    assert!(
        decode_cursor("YWJjH3g").is_err(),
        "含分隔符但排序键不是整数的游标必须失败"
    );
}

#[test]
fn normalize_linkage_value_strips_prefix_and_trims_both_ends() {
    assert_eq!(normalize_linkage_value(" @i18n@ 成都 "), "成都");
    assert_eq!(normalize_linkage_value("成都"), "成都");
    assert_eq!(normalize_linkage_value("  @i18n@ "), "");
    assert_eq!(normalize_linkage_value(""), "");
}

/// 断言四类接收人都按各自形态校验。
fn recipient(receive_id_type: &str, receive_id: &str) -> MessageRecipient {
    serde_json::from_value(serde_json::json!({
        "receive_id_type": receive_id_type,
        "receive_id": receive_id,
    }))
    .unwrap_or_else(|error| panic!("构造接收人失败: {error}"))
}

#[test]
fn message_recipient_validates_all_four_id_types() {
    // open_id：必须是 ASCII 且带 ou_ 前缀。
    assert!(recipient("open_id", "ou_123").validate().is_ok());
    assert!(recipient("open_id", "on_123").validate().is_err());
    assert!(recipient("open_id", "张三").validate().is_err());

    // union_id：必须是 ASCII 且带 on_ 前缀。
    assert!(recipient("union_id", "on_123").validate().is_ok());
    assert!(recipient("union_id", "ou_123").validate().is_err());

    // user_id：只要求 ASCII，不能传姓名或带空白。
    assert!(recipient("user_id", "u_123").validate().is_ok());
    assert!(recipient("user_id", "张三").validate().is_err());
    assert!(recipient("user_id", "u 123").validate().is_err());

    // email：local@domain，domain 必须含点且不能再有 @。
    assert!(recipient("email", "a@b.com").validate().is_ok());
    assert!(recipient("email", "a@b").validate().is_err());
    assert!(recipient("email", "a@@b.com").validate().is_err());
    assert!(recipient("email", "").validate().is_err());
}

#[test]
fn message_id_rejects_path_injection() {
    assert!(valid_message_id("om_123-abc"));
    assert!(!valid_message_id("../evil"));
    assert!(!valid_message_id("om/123"));
    assert!(!valid_message_id(""));
    assert!(!valid_message_id(&"a".repeat(129)));
}

#[test]
fn parse_attachments_enforces_the_file_contract() {
    let spec = UploadSpec {
        label: "附件",
        file_type: "attachment",
        max_bytes: 50 * 1024 * 1024,
    };

    let files = parse_attachments(
        &serde_json::json!([
            {"file_token": "boxcn123", "name": "a.pdf", "size": 3},
            {"file_token": "boxcn456", "name": "b.png", "size": 10},
        ]),
        spec,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_token, "boxcn123");
    assert_eq!(files[0].name, "a.pdf");
    assert_eq!(files[0].size, 3);

    // null 单元格等同空附件列表。
    assert!(parse_attachments(&serde_json::Value::Null, spec)
        .unwrap_or_else(|error| panic!("{error}"))
        .is_empty());

    for bad in [
        // 不是附件数组
        serde_json::json!({"file_token": "boxcn123", "name": "a.pdf", "size": 3}),
        // file_token 含路径字符
        serde_json::json!([{"file_token": "../x", "name": "a.pdf", "size": 3}]),
        // 文件名没有扩展名
        serde_json::json!([{"file_token": "boxcn123", "name": "a", "size": 3}]),
        // 文件名含路径分隔符
        serde_json::json!([{"file_token": "boxcn123", "name": "a/b.pdf", "size": 3}]),
        // size 为 0
        serde_json::json!([{"file_token": "boxcn123", "name": "a.pdf", "size": 0}]),
        // size 超过上限
        serde_json::json!([{"file_token": "boxcn123", "name": "a.pdf", "size": 50 * 1024 * 1024 + 1}]),
    ] {
        assert!(parse_attachments(&bad, spec).is_err(), "应拒绝 {bad}");
    }

    assert!(matches!(
        parse_attachments(&serde_json::json!({"x": 1}), spec),
        Err(AttachmentError::NotArray { .. })
    ));
}

#[test]
fn multipart_carries_boundary_and_guessed_part_content_type() {
    let (content_type, body) =
        multipart("a.png", b"abc", "image").unwrap_or_else(|error| panic!("{error}"));
    assert!(
        content_type.starts_with("multipart/form-data; boundary=feishu-"),
        "实际: {content_type}"
    );
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"type\"\r\n\r\nimage"));
    assert!(text.contains("filename=\"a.png\""));
    assert!(text.contains("Content-Type: image/png"));
    assert!(text.contains("abc"));

    // 非图片按二进制附件走。
    let (_, attachment_body) =
        multipart("a.zip", b"xyz", "attachment").unwrap_or_else(|error| panic!("{error}"));
    assert!(String::from_utf8_lossy(&attachment_body)
        .contains("Content-Type: application/octet-stream"));

    // 文件名在 multipart 内部再次校验，挡住 CRLF/路径注入。
    assert!(multipart("a\nb.png", b"abc", "image").is_err());
    assert!(multipart("a/b.png", b"abc", "image").is_err());
}

#[test]
fn image_content_type_guesses_by_extension_and_falls_back() {
    assert_eq!(image_content_type("x.png"), "image/png");
    assert_eq!(image_content_type("x.JPG"), "image/jpeg");
    assert_eq!(image_content_type("x.jpeg"), "image/jpeg");
    assert_eq!(image_content_type("x.gif"), "image/gif");
    assert_eq!(image_content_type("x.webp"), "image/webp");
    assert_eq!(image_content_type("x.bmp"), "image/bmp");
    assert_eq!(image_content_type("x.heic"), "image/heic");
    assert_eq!(image_content_type("x.unknown"), "application/octet-stream");
    assert_eq!(
        image_content_type("no-extension"),
        "application/octet-stream"
    );
}

#[test]
fn error_display_is_fixed_and_all_failure_kinds_are_constructible() {
    let kinds = [
        FailureKind::Retry {
            retry_after_seconds: Some(3),
        },
        FailureKind::Retry {
            retry_after_seconds: None,
        },
        FailureKind::TokenExpired,
        FailureKind::UuidConflict,
        FailureKind::InstanceNotFound,
        FailureKind::Fatal { code: 1254003 },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        let error = Error::new(kind, format!("msg-{index}"));
        assert_eq!(
            error.to_string(),
            format!("飞书出站调用失败: msg-{index}"),
            "Display 文案固定，不拼接运维提示"
        );
        assert_eq!(error.kind, kind);
    }
}

#[test]
fn root_reexports_keep_the_public_surface_intact() {
    let error = Error::new(FailureKind::Fatal { code: 1254003 }, "权限不足");
    let result: Result<()> = Err(error.clone());
    assert_eq!(result, Err(error));

    let request = Request {
        method: Method::Get,
        url: "https://open.feishu.cn/open-apis/ping".to_string(),
        query: Vec::new(),
        bearer_token: None,
        json_body: None,
        raw_body: None,
        binary_limit: None,
        timeout_secs: None,
        idempotent: true,
    };
    assert_eq!(request.method, Method::Get);

    let response = Response {
        status: 200,
        body: r#"{"code":0}"#.to_string(),
        bytes: None,
        headers: BTreeMap::new(),
    };
    assert_eq!(response.status, 200);

    let policy = RetryPolicy {
        max_attempts: 1,
        base_backoff_ms: 0,
        max_backoff_ms: 0,
        max_retry_after_ms: 0,
    };
    assert_eq!(policy.worst_case_seconds(20), 20);
}
