//! 事件入站公开 API 的契约测试：challenge 应答、明文/密文闸门、回调信封与失败语义。
//!
//! 全部离线：密文用 crate 自身的 `derive_key` + `encrypt_bytes` 现场生成，不依赖真实租户。

use yang_feishu::crypto::{decrypt_bytes, derive_key, encrypt_bytes};
use yang_feishu::event::{EventConfig, EventError, EventVerifier, Inbound};

const TOKEN: &str = "verify-token-1";

fn 明文配置() -> EventConfig {
    EventConfig {
        verification_token: TOKEN.into(),
        encrypt_key: None,
        require_signature: false,
    }
}

fn 加密配置(encrypt_key: &str) -> EventConfig {
    EventConfig {
        verification_token: TOKEN.into(),
        encrypt_key: Some(encrypt_key.into()),
        require_signature: false,
    }
}

fn 构造(config: EventConfig) -> EventVerifier {
    EventVerifier::new(config).unwrap_or_else(|error| panic!("构造 EventVerifier 失败: {error}"))
}

fn 加密请求体(明文: &serde_json::Value, encrypt_key: &str) -> Vec<u8> {
    let 密文 = encrypt_bytes(明文.to_string().as_bytes(), &derive_key(encrypt_key))
        .unwrap_or_else(|error| panic!("加密失败: {error}"));
    serde_json::json!({ "encrypt": 密文 })
        .to_string()
        .into_bytes()
}

/// 用另一把密钥加密 challenge，并确认目标密钥解密时填充校验必定失败。
///
/// CBC 无认证：错误密钥只有约 1/256 的概率碰巧通过 PKCS#7 填充，若不排除，
/// `Decrypt` 断言会偶发退化成 `InvalidBody`。这里显式循环排除该分支。
fn 密钥不匹配的密文(目标密钥: &str) -> String {
    let 目标密钥 = derive_key(目标密钥);
    for _ in 0..64 {
        let 密文 = encrypt_bytes(
            br#"{"type":"url_verification","challenge":"c-9","token":"verify-token-1"}"#,
            &derive_key("ek-other"),
        )
        .unwrap_or_else(|error| panic!("加密失败: {error}"));
        if decrypt_bytes(&密文, &目标密钥).is_err() {
            return 密文;
        }
    }
    panic!("64 次尝试内未构造出填充非法的密文");
}

fn 回调体() -> serde_json::Value {
    serde_json::json!({
        "type": "event_callback",
        "uuid": "evt-1",
        "token": TOKEN,
        "event": {"type": "approval_instance", "instance_code": "ABC"}
    })
}

#[test]
fn 明文challenge_合法token回显challenge() {
    let body = br#"{"type":"url_verification","challenge":"c-1","token":"verify-token-1"}"#;
    assert_eq!(
        构造(明文配置()).decode_challenge(body),
        Ok(Inbound::Challenge("c-1".into()))
    );
}

#[test]
fn 明文challenge_token错误或缺失返回forbidden() {
    let verifier = 构造(明文配置());

    let 错误 = br#"{"type":"url_verification","challenge":"c-1","token":"bad"}"#;
    assert!(
        matches!(
            verifier.decode_challenge(错误),
            Err(EventError::Forbidden(_))
        ),
        "token 不匹配必须按 403 拒绝"
    );

    let 缺失 = br#"{"type":"url_verification","challenge":"c-1"}"#;
    assert!(
        matches!(
            verifier.decode_challenge(缺失),
            Err(EventError::Forbidden(_))
        ),
        "缺 token 必须 fail-closed 为 403，而不是放行或 401"
    );
}

#[test]
fn 明文challenge_缺challenge或未知类型返回invalidbody() {
    let verifier = 构造(明文配置());

    let 缺challenge = br#"{"type":"url_verification","token":"verify-token-1"}"#;
    match verifier.decode_challenge(缺challenge) {
        Err(EventError::InvalidBody { field, .. }) => assert_eq!(field, "challenge"),
        other => panic!("缺 challenge 应为 InvalidBody{{field:challenge}}，实际 {other:?}"),
    }

    let 未知类型 = br#"{"type":"something_else","challenge":"c-1","token":"verify-token-1"}"#;
    match verifier.decode_challenge(未知类型) {
        Err(EventError::InvalidBody { field, .. }) => assert_eq!(field, "type"),
        other => panic!("未知 type 应为 InvalidBody{{field:type}}，实际 {other:?}"),
    }
}

#[test]
fn 配置加密密钥后明文请求返回unauthorized() {
    let verifier = 构造(加密配置("ek-1"));

    let challenge = br#"{"type":"url_verification","challenge":"c-1","token":"verify-token-1"}"#;
    assert!(
        matches!(
            verifier.decode_challenge(challenge),
            Err(EventError::Unauthorized(_))
        ),
        "已配置 Encrypt Key 时明文 challenge 必须是 401 语义，不是 token 的 403"
    );

    let 回调 = 回调体().to_string();
    assert!(
        matches!(
            verifier.decode_callback(回调.as_bytes()),
            Err(EventError::Unauthorized(_))
        ),
        "已配置 Encrypt Key 时明文回调同样必须是 401 语义"
    );
}

#[test]
fn 加密challenge_密钥匹配时解密回显() {
    let verifier = 构造(加密配置("ek-1"));
    let 明文 = serde_json::json!({
        "type": "url_verification",
        "challenge": "c-9",
        "token": TOKEN
    });
    assert_eq!(
        verifier.decode_challenge(&加密请求体(&明文, "ek-1")),
        Ok(Inbound::Challenge("c-9".into()))
    );
}

#[test]
fn 加密challenge_密钥不匹配返回decrypt() {
    let verifier = 构造(加密配置("ek-1"));
    let body = serde_json::json!({ "encrypt": 密钥不匹配的密文("ek-1") }).to_string();
    match verifier.decode_challenge(body.as_bytes()) {
        Err(EventError::Decrypt(_)) => {}
        other => panic!("密钥不匹配应为 Decrypt，实际 {other:?}"),
    }
}

#[test]
fn 事件回调_提取顶层uuid与事件类型() {
    let body = 回调体().to_string();
    let envelope = 构造(明文配置())
        .decode_callback(body.as_bytes())
        .unwrap_or_else(|error| panic!("解码回调失败: {error}"));

    assert_eq!(envelope.event_id, "evt-1", "事件 id 取顶层 uuid");
    assert_eq!(
        envelope.event_type, "approval_instance",
        "事件类型取 event.type"
    );
    assert_eq!(
        envelope.event["instance_code"], "ABC",
        "event 嵌套层原样保留"
    );
}

#[test]
fn 事件回调_payload剥离verification_token() {
    let body = 回调体().to_string();
    let envelope = 构造(明文配置())
        .decode_callback(body.as_bytes())
        .unwrap_or_else(|error| panic!("解码回调失败: {error}"));

    assert!(
        envelope.payload.get("token").is_none(),
        "认证材料不得进入 payload"
    );
    assert!(
        !envelope.payload.to_string().contains(TOKEN),
        "payload 序列化后不得出现 verification_token"
    );
    assert_eq!(
        envelope.payload["uuid"], "evt-1",
        "其余顶层字段保留供宿主落库"
    );
    assert_eq!(envelope.payload["type"], "event_callback");
    assert_eq!(envelope.payload["event"]["type"], "approval_instance");
}

#[test]
fn 事件回调_缺uuid或类型不符返回invalidbody() {
    let verifier = 构造(明文配置());

    let 缺uuid = serde_json::json!({
        "type": "event_callback",
        "token": TOKEN,
        "event": {"type": "approval_instance"}
    });
    match verifier.decode_callback(缺uuid.to_string().as_bytes()) {
        Err(EventError::InvalidBody { field, .. }) => assert_eq!(field, "uuid"),
        other => panic!("缺顶层 uuid 应为 InvalidBody{{field:uuid}}，实际 {other:?}"),
    }

    let 非回调 = serde_json::json!({
        "type": "url_verification",
        "uuid": "evt-1",
        "challenge": "c-1",
        "token": TOKEN,
        "event": {"type": "approval_instance"}
    });
    match verifier.decode_callback(非回调.to_string().as_bytes()) {
        Err(EventError::InvalidBody { field, .. }) => assert_eq!(field, "type"),
        other => panic!("非 event_callback 应为 InvalidBody{{field:type}}，实际 {other:?}"),
    }
}

#[test]
fn 事件回调_token不匹配返回unauthorized() {
    let mut body = 回调体();
    body["token"] = serde_json::json!("bad");
    assert!(
        matches!(
            构造(明文配置()).decode_callback(body.to_string().as_bytes()),
            Err(EventError::Unauthorized(_))
        ),
        "回调 token 不匹配必须按 401 语义拒绝"
    );
}

#[test]
fn 共用url_加密事件回调经解码后委托为callback() {
    let verifier = 构造(加密配置("ek-1"));
    let 明文 = serde_json::json!({
        "type": "event_callback",
        "uuid": "evt-shared",
        "token": TOKEN,
        "event": {"type": "approval_task"}
    });

    match verifier.decode_challenge(&加密请求体(&明文, "ek-1")) {
        Ok(Inbound::Callback(envelope)) => {
            assert_eq!(envelope.event_id, "evt-shared");
            assert_eq!(envelope.event_type, "approval_task");
            assert!(
                envelope.payload.get("token").is_none(),
                "委托路径同样要剥掉 token"
            );
        }
        other => panic!("共用 URL 的加密回调必须委托为 Inbound::Callback，实际 {other:?}"),
    }
}

#[test]
fn require_signature为true时构造失败() {
    let error = EventVerifier::new(EventConfig {
        verification_token: TOKEN.into(),
        encrypt_key: None,
        require_signature: true,
    })
    .err()
    .unwrap_or_else(|| panic!("未实现的签名要求必须显式失败，不得静默放行"));

    match error {
        EventError::Config(message) => assert!(
            message.contains("X-Lark-Signature"),
            "错误信息应点名未实现的签名缺口: {message}"
        ),
        other => panic!("require_signature=true 应为 EventError::Config，实际 {other:?}"),
    }
}
