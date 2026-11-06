//! 飞书事件入站：challenge 应答与事件回调解码。
//!
//! 只做平台通用的事：解密、`verification_token` 校验、可信信封提取。事件类型白名单、
//! 审批字段解析与落库语义留在宿主。
//!
//! **X-Lark-Signature 不在本期**：它需要对原始请求体计算签名，当前宿主传输层没有
//! 保留原始字节。`EventConfig::require_signature` 只能为 `false`，置 `true` 会在
//! 构造时显式失败，避免「看起来启用了签名」的静默缺口。

use serde_json::Value;

use crate::crypto::{decrypt_bytes, derive_key, tokens_equal};

/// 事件配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventConfig {
    /// 事件订阅的 verification_token。
    pub verification_token: String,
    /// Encrypt Key（配置后所有推送都应加密）。
    pub encrypt_key: Option<String>,
    /// 是否强制要求 `X-Lark-Signature`；当前必须为 `false`。
    pub require_signature: bool,
}

/// 事件解码失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
    /// 配置不可用（缺 Encrypt Key、要求未实现的签名等）。
    #[error("{0}")]
    Config(String),
    /// 密文解密失败。
    #[error("{0}")]
    Decrypt(String),
    /// 请求体缺字段或形态不对。
    #[error("飞书事件请求体无效（{field}）：{message}")]
    InvalidBody {
        /// 出错的字段。
        field: &'static str,
        /// 可读原因。
        message: String,
    },
    /// 来源不可信（401）。
    #[error("{0}")]
    Unauthorized(String),
    /// challenge 的 token 不匹配（403）。
    #[error("{0}")]
    Forbidden(String),
}

/// 事件回调的可信信封。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackEnvelope {
    /// 事件 id（v1 取顶层 `uuid`，v2 取 `header.event_id`）。
    pub event_id: String,
    /// 事件类型（v1 取 `event.type`，v2 取 `header.event_type`）。
    pub event_type: String,
    /// `event` 嵌套层的原始 JSON。
    pub event: Value,
    /// 剥掉 token 后的完整体（供落 raw_payload）。
    pub payload: Value,
}

/// challenge 端点的解码结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inbound {
    /// challenge 回显文案。
    Challenge(String),
    /// 共用 URL 的 event_callback 委托结果。
    Callback(CallbackEnvelope),
}

/// 事件验证器。
#[derive(Debug, Clone)]
pub struct EventVerifier {
    config: EventConfig,
}

impl EventVerifier {
    /// 构造验证器。配置了未实现的签名要求时直接失败。
    pub fn new(config: EventConfig) -> Result<Self, EventError> {
        if config.require_signature {
            return Err(EventError::Config(
                "X-Lark-Signature 尚未实现：请将 require_signature 设为 false".to_string(),
            ));
        }
        if config.verification_token.trim().is_empty() {
            return Err(EventError::Config(
                "飞书事件 verification_token 未配置".to_string(),
            ));
        }
        Ok(Self { config })
    }

    /// 解码 challenge 端点请求体。
    pub fn decode_challenge(&self, body: &[u8]) -> Result<Inbound, EventError> {
        let body = self.decode_body(body)?;
        let r#type = body.get("type").and_then(Value::as_str).unwrap_or("");
        let challenge = body.get("challenge").and_then(Value::as_str).unwrap_or("");

        if r#type == "event_callback" || body.get("schema").and_then(Value::as_str) == Some("2.0") {
            // 共用订阅 URL：body 已在本函数解密，委托给回调提取时不能再按
            // 「已配置 Encrypt Key 却收到明文」拒绝。
            return decode_callback_body(&body, &self.config.verification_token)
                .map(Inbound::Callback);
        }

        if challenge.is_empty() {
            return Err(EventError::InvalidBody {
                field: "challenge",
                message: "缺少 challenge".to_string(),
            });
        }
        if r#type != "url_verification" {
            return Err(EventError::InvalidBody {
                field: "type",
                message: "不支持的飞书回调类型".to_string(),
            });
        }
        let token = body.get("token").and_then(Value::as_str).unwrap_or("");
        if token.is_empty() || !tokens_equal(token, &self.config.verification_token) {
            return Err(EventError::Forbidden(
                "飞书 challenge verification_token 无效".to_string(),
            ));
        }
        Ok(Inbound::Challenge(challenge.to_string()))
    }

    /// 解码事件回调端点请求体。
    pub fn decode_callback(&self, body: &[u8]) -> Result<CallbackEnvelope, EventError> {
        let body = self.decode_body(body)?;
        decode_callback_body(&body, &self.config.verification_token)
    }

    fn decode_body(&self, raw: &[u8]) -> Result<Value, EventError> {
        let value: Value =
            serde_json::from_slice(raw).map_err(|error| EventError::InvalidBody {
                field: "body",
                message: format!("不是合法 JSON: {error}"),
            })?;
        match value.get("encrypt").and_then(Value::as_str) {
            Some(encrypted) => {
                let Some(key_raw) = self.config.encrypt_key.as_deref() else {
                    return Err(EventError::Config(
                        "飞书发送了加密请求，但 event_encrypt_key 未配置".to_string(),
                    ));
                };
                let key = derive_key(key_raw);
                let decrypted = decrypt_bytes(encrypted, &key)
                    .map_err(|error| EventError::Decrypt(error.to_string()))?;
                serde_json::from_slice(&decrypted).map_err(|error| EventError::InvalidBody {
                    field: "decrypted_body",
                    message: format!("飞书事件解密后不是合法 JSON: {error}"),
                })
            }
            None => {
                if self.config.encrypt_key.is_some() {
                    return Err(EventError::Unauthorized(
                        "已配置 event_encrypt_key，拒绝明文请求".to_string(),
                    ));
                }
                Ok(value)
            }
        }
    }
}

fn decode_callback_body(
    body: &Value,
    expected_token: &str,
) -> Result<CallbackEnvelope, EventError> {
    if body.get("schema").and_then(Value::as_str) == Some("2.0") {
        return decode_callback_v2(body, expected_token);
    }

    let actual_token = body
        .get("token")
        .and_then(Value::as_str)
        .ok_or_else(|| EventError::Unauthorized("飞书事件缺少 verification_token".to_string()))?;
    if !tokens_equal(actual_token, expected_token) {
        return Err(EventError::Unauthorized(
            "飞书事件 verification_token 不匹配".to_string(),
        ));
    }

    let mut payload = body.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("token");
    }

    if body.get("type").and_then(Value::as_str) != Some("event_callback") {
        return Err(EventError::InvalidBody {
            field: "type",
            message: "必须为 event_callback".to_string(),
        });
    }
    let event = body
        .get("event")
        .cloned()
        .ok_or_else(|| EventError::InvalidBody {
            field: "event",
            message: "缺少 event 对象".to_string(),
        })?;
    let event_id = body.get("uuid").and_then(Value::as_str).unwrap_or("");
    if event_id.trim().is_empty() || event_id.chars().count() > 128 {
        return Err(EventError::InvalidBody {
            field: "uuid",
            message: "顶层 uuid 必须非空且最多 128 字符".to_string(),
        });
    }
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    Ok(CallbackEnvelope {
        event_id: event_id.to_string(),
        event_type,
        event,
        payload,
    })
}

/// v2.0 事件信封：`schema=2.0`，身份在 `header`，`token` 也在 `header.token`。
fn decode_callback_v2(body: &Value, expected_token: &str) -> Result<CallbackEnvelope, EventError> {
    let header = body
        .get("header")
        .and_then(Value::as_object)
        .ok_or_else(|| EventError::InvalidBody {
            field: "header",
            message: "v2.0 事件缺少 header 对象".to_string(),
        })?;
    let actual_token = header
        .get("token")
        .and_then(Value::as_str)
        .ok_or_else(|| EventError::Unauthorized("飞书 v2.0 事件缺少 header.token".to_string()))?;
    if !tokens_equal(actual_token, expected_token) {
        return Err(EventError::Unauthorized(
            "飞书 v2.0 事件 verification_token 不匹配".to_string(),
        ));
    }
    let event_id = header.get("event_id").and_then(Value::as_str).unwrap_or("");
    if event_id.trim().is_empty() || event_id.chars().count() > 128 {
        return Err(EventError::InvalidBody {
            field: "header.event_id",
            message: "必须非空且最多 128 字符".to_string(),
        });
    }
    let event_type = header
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let event = body
        .get("event")
        .cloned()
        .ok_or_else(|| EventError::InvalidBody {
            field: "event",
            message: "缺少 event 对象".to_string(),
        })?;
    let mut payload = body.clone();
    if let Some(header) = payload.get_mut("header").and_then(Value::as_object_mut) {
        header.remove("token");
    }
    Ok(CallbackEnvelope {
        event_id: event_id.to_string(),
        event_type,
        event,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::encrypt_bytes;

    fn config() -> EventConfig {
        EventConfig {
            verification_token: "token-1".into(),
            encrypt_key: None,
            require_signature: false,
        }
    }

    fn verifier() -> EventVerifier {
        EventVerifier::new(config()).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn challenge_requires_matching_token_and_type() {
        let body = br#"{"type":"url_verification","challenge":"c-1","token":"token-1"}"#;
        assert_eq!(
            verifier().decode_challenge(body),
            Ok(Inbound::Challenge("c-1".into()))
        );

        let wrong = br#"{"type":"url_verification","challenge":"c-1","token":"bad"}"#;
        assert!(matches!(
            verifier().decode_challenge(wrong),
            Err(EventError::Forbidden(_))
        ));

        let missing = br#"{"type":"url_verification","challenge":"c-1"}"#;
        assert!(verifier().decode_challenge(missing).is_err());
    }

    #[test]
    fn plaintext_is_rejected_when_a_key_is_configured() {
        let verifier = EventVerifier::new(EventConfig {
            verification_token: "token-1".into(),
            encrypt_key: Some("key".into()),
            require_signature: false,
        })
        .unwrap();
        let body = br#"{"type":"url_verification","challenge":"c-1","token":"token-1"}"#;
        assert!(matches!(
            verifier.decode_challenge(body),
            Err(EventError::Unauthorized(_))
        ));
    }

    #[test]
    fn encrypted_challenge_round_trips() {
        let key = derive_key("key");
        let plaintext = br#"{"type":"url_verification","challenge":"c-1","token":"token-1"}"#;
        let cipher = encrypt_bytes(plaintext, &key).unwrap();
        let body = serde_json::json!({"encrypt": cipher}).to_string();
        let verifier = EventVerifier::new(EventConfig {
            verification_token: "token-1".into(),
            encrypt_key: Some("key".into()),
            require_signature: false,
        })
        .unwrap();
        assert_eq!(
            verifier.decode_challenge(body.as_bytes()),
            Ok(Inbound::Challenge("c-1".into()))
        );
    }

    #[test]
    fn callback_extracts_top_level_uuid_and_strips_token() {
        let body = serde_json::json!({
            "type": "event_callback",
            "uuid": "evt-1",
            "token": "token-1",
            "event": {"type": "approval_instance", "instance_code": "ABC"}
        });
        let envelope = verifier()
            .decode_callback(body.to_string().as_bytes())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(envelope.event_id, "evt-1");
        assert_eq!(envelope.event_type, "approval_instance");
        assert!(envelope.payload.get("token").is_none());
        assert_eq!(envelope.event["instance_code"], "ABC");
    }

    #[test]
    fn callback_v2_extracts_header_identity_and_strips_token() {
        let body = serde_json::json!({
            "schema": "2.0",
            "header": {
                "event_id": "evt-v2",
                "event_type": "im.message.receive_v1",
                "token": "token-1",
                "app_id": "cli_x"
            },
            "event": {"message": {"message_id": "om_1"}}
        });
        let envelope = verifier()
            .decode_callback(body.to_string().as_bytes())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(envelope.event_id, "evt-v2");
        assert_eq!(envelope.event_type, "im.message.receive_v1");
        assert!(envelope.payload["header"].get("token").is_none());
        assert_eq!(envelope.event["message"]["message_id"], "om_1");

        let wrong = serde_json::json!({
            "schema": "2.0",
            "header": {"event_id": "evt-v2", "event_type": "x", "token": "bad"},
            "event": {}
        });
        assert!(matches!(
            verifier().decode_callback(wrong.to_string().as_bytes()),
            Err(EventError::Unauthorized(_))
        ));
    }

    #[test]
    fn callback_rejects_bad_token_and_missing_identity() {
        let wrong = serde_json::json!({
            "type": "event_callback",
            "uuid": "evt-1",
            "token": "bad",
            "event": {"type": "approval_instance"}
        });
        assert!(matches!(
            verifier().decode_callback(wrong.to_string().as_bytes()),
            Err(EventError::Unauthorized(_))
        ));

        let missing = serde_json::json!({
            "type": "event_callback",
            "token": "token-1",
            "event": {"type": "approval_instance"}
        });
        assert!(matches!(
            verifier().decode_callback(missing.to_string().as_bytes()),
            Err(EventError::InvalidBody { field: "uuid", .. })
        ));
    }

    #[test]
    fn shared_url_callback_is_delegated_after_decryption() {
        let key = derive_key("key");
        let plaintext = serde_json::json!({
            "type": "event_callback",
            "uuid": "evt-2",
            "token": "token-1",
            "event": {"type": "approval_task"}
        })
        .to_string();
        let cipher = encrypt_bytes(plaintext.as_bytes(), &key).unwrap();
        let body = serde_json::json!({"encrypt": cipher}).to_string();
        let verifier = EventVerifier::new(EventConfig {
            verification_token: "token-1".into(),
            encrypt_key: Some("key".into()),
            require_signature: false,
        })
        .unwrap();
        match verifier
            .decode_challenge(body.as_bytes())
            .unwrap_or_else(|error| panic!("{error}"))
        {
            Inbound::Callback(envelope) => assert_eq!(envelope.event_id, "evt-2"),
            Inbound::Challenge(_) => panic!("应为 Callback"),
        }
    }

    #[test]
    fn signature_requirement_fails_closed_until_implemented() {
        let error = EventVerifier::new(EventConfig {
            verification_token: "token-1".into(),
            encrypt_key: None,
            require_signature: true,
        })
        .err()
        .unwrap_or_else(|| panic!("应失败"));
        assert!(error.to_string().contains("X-Lark-Signature"));
    }
}
