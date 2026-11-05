//! 飞书用户身份 OAuth 协议：授权 URL、令牌交换/刷新、用户信息。
//!
//! 只搬协议层：用户 token 的单行表存储、提醒 worker 与配置读取留在宿主。
//!
//! # 三条时钟
//!
//! - `access_token` 约 2 小时（以响应 `expires_in` 为准）；
//! - `refresh_token` 一次性且寿命短；
//! - 用户授权满 365 天后必须重新授权，继续刷新报 20037。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use crate::error::{Error, FailureKind};
use crate::transport::{
    send_with_retry, Method, Request, Sleeper, Transport, TOKEN_REQUEST_TIMEOUT_SECS, TOKEN_RETRY,
};

use super::Credentials;

/// 授权页路径（飞书中国站）。
pub const AUTHORIZE_PATH: &str = "/open-apis/authen/v1/authorize";
/// 令牌端点路径（换 code 与刷新共用，只有 `grant_type` 不同）。
pub const TOKEN_PATH: &str = "/oauth/v3/token";

/// access_token 剩余寿命低于这个秒数就提前刷新。
const ACCESS_TOKEN_REFRESH_MARGIN_SECONDS: i64 = 300;

/// 刷新 `refresh_token` 已过期（含 365 天红线触顶）。
const CODE_REFRESH_TOKEN_EXPIRED: i32 = 20037;
/// 刷新 `refresh_token` 无效。
const CODE_REFRESH_TOKEN_INVALID: i32 = 20026;
/// 刷新 `refresh_token` 已被撤销。
const CODE_REFRESH_TOKEN_REVOKED: i32 = 20064;
/// 刷新 `refresh_token` 已被使用（一次性）。
const CODE_REFRESH_TOKEN_USED: i32 = 20073;

/// 「该重新授权了」的提醒线（天）。飞书硬性红线是 365 天。
pub const REAUTH_REMINDER_DAYS: i64 = 350;

/// 当前 unix 秒。
pub fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// 是否已过重新授权提醒线。
pub fn reauth_reminder_due(authorized_at: i64, now: i64) -> bool {
    now - authorized_at >= REAUTH_REMINDER_DAYS * 86_400
}

/// access_token 是否仍然新鲜（提前 5 分钟刷新）。
pub fn access_token_is_fresh(expires_at: i64, now: i64) -> bool {
    expires_at - now > ACCESS_TOKEN_REFRESH_MARGIN_SECONDS
}

/// 按 RFC 3986 的 unreserved 集合做百分号编码。
fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// 用飞书中国站授权域名拼授权 URL。
pub fn build_authorize_url(app_id: &str, redirect_uri: &str, scope: &str, state: &str) -> String {
    build_authorize_url_with_base(
        "https://accounts.feishu.cn",
        app_id,
        redirect_uri,
        scope,
        state,
    )
}

/// 用指定授权域名拼授权 URL（私有化/Lark 用）。
pub fn build_authorize_url_with_base(
    accounts_base: &str,
    app_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> String {
    format!(
        "{accounts_base}{AUTHORIZE_PATH}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&prompt=consent",
        percent_encode(app_id),
        percent_encode(redirect_uri),
        percent_encode(scope),
        percent_encode(state),
    )
}

/// 令牌端点返回的原始报文（成功与失败共用）。
#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i32,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    refresh_token_expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// 一对凭证 + 已算好的绝对过期时刻（unix 秒）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenBundle {
    /// 用户 access_token。
    pub access_token: String,
    /// access_token 的绝对过期时刻。
    pub access_token_expires_at: i64,
    /// 用户 refresh_token（一次性）。
    pub refresh_token: String,
    /// refresh_token 的绝对过期时刻。
    pub refresh_token_expires_at: i64,
    /// 授权 scope。
    pub scope: Option<String>,
}

/// 令牌端点/用户信息接口的失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEndpointError {
    /// 飞书业务码；拿不到（网关错误页、传输层失败）时为 `0`。
    pub code: i32,
    /// 可读失败描述。
    pub message: String,
}

impl TokenEndpointError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// 只能靠**重新授权**补救的那一族。
    pub fn needs_reauth(&self) -> bool {
        matches!(
            self.code,
            CODE_REFRESH_TOKEN_EXPIRED
                | CODE_REFRESH_TOKEN_INVALID
                | CODE_REFRESH_TOKEN_REVOKED
                | CODE_REFRESH_TOKEN_USED
        )
    }

    /// 已登记码的排查提示。
    pub fn hint(&self) -> Option<&'static str> {
        match self.code {
            CODE_REFRESH_TOKEN_EXPIRED => {
                Some("刷新凭证已过期（用户授权满 365 天后飞书强制重新授权）")
            }
            CODE_REFRESH_TOKEN_INVALID | CODE_REFRESH_TOKEN_REVOKED | CODE_REFRESH_TOKEN_USED => {
                Some("刷新凭证已失效（一次性：用过即换新，旧值立即作废）")
            }
            20074 => {
                Some("开发者后台「安全设置 > 刷新 user_access_token」开关未开启（改动需发版生效）")
            }
            20071 => Some("redirect_uri 与开发者后台注册的重定向 URL 不一致"),
            20027 => Some("授权页 scope 含应用未开通的权限（去开发者后台开通后重试）"),
            20003 | 20065 => Some("授权码已被使用（一次性）"),
            20004 => Some("授权码已过期（有效期 5 分钟）"),
            20002 => Some("client_id 与 client_secret 不匹配"),
            20010 => Some("发起授权的用户没有该应用的使用权限"),
            _ => None,
        }
    }
}

impl std::fmt::Display for TokenEndpointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "飞书 OAuth 调用失败: {}", self.message)
    }
}

impl std::error::Error for TokenEndpointError {}

impl From<&Error> for TokenEndpointError {
    fn from(failure: &Error) -> Self {
        let code = match failure.kind {
            FailureKind::Fatal { code } => code,
            _ => 0,
        };
        Self::new(code, failure.message.clone())
    }
}

/// 解析令牌响应；`code != 0` 折成错误。
fn parse_token_response(body: &str) -> std::result::Result<TokenResponse, TokenEndpointError> {
    let parsed: TokenResponse = serde_json::from_str(body)
        .map_err(|error| TokenEndpointError::new(0, format!("令牌端点响应解析失败: {error}")))?;
    if parsed.code != 0 {
        let message = parsed
            .error_description
            .clone()
            .or_else(|| parsed.error.clone())
            .unwrap_or_else(|| "令牌端点返回失败".to_string());
        return Err(TokenEndpointError::new(parsed.code, message));
    }
    Ok(parsed)
}

/// 令牌响应 → 落库用的凭证快照。
fn to_bundle(
    response: TokenResponse,
    now: i64,
) -> std::result::Result<TokenBundle, TokenEndpointError> {
    let access_token = response
        .access_token
        .ok_or_else(|| TokenEndpointError::new(0, "令牌响应里没有 access_token"))?;
    let expires_in = response
        .expires_in
        .ok_or_else(|| TokenEndpointError::new(0, "令牌响应里没有 expires_in"))?;
    let refresh_token = response.refresh_token.ok_or_else(|| {
        TokenEndpointError::new(
            0,
            "令牌响应里没有 refresh_token：授权链接的 scope 是否漏了 offline_access？",
        )
    })?;
    let refresh_expires_in = response.refresh_token_expires_in.unwrap_or(expires_in);
    Ok(TokenBundle {
        access_token,
        access_token_expires_at: now + expires_in,
        refresh_token,
        refresh_token_expires_at: now + refresh_expires_in,
        scope: response.scope,
    })
}

fn retry_policy() -> crate::transport::RetryPolicy {
    TOKEN_RETRY
}

/// 飞书用户 OAuth 客户端。
pub struct OAuthClient {
    credentials: Credentials,
    accounts_base: String,
    open_base: String,
    transport: Arc<dyn Transport>,
    sleeper: Arc<dyn Sleeper>,
}

impl OAuthClient {
    pub(crate) fn new(
        credentials: Credentials,
        accounts_base: impl Into<String>,
        open_base: impl Into<String>,
        transport: Arc<dyn Transport>,
        sleeper: Arc<dyn Sleeper>,
    ) -> Self {
        Self {
            credentials,
            accounts_base: accounts_base.into(),
            open_base: open_base.into(),
            transport,
            sleeper,
        }
    }

    /// 当前凭证。
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    /// 拼授权页 URL（使用客户端配置的 accounts 域名）。
    pub fn authorize_url(&self, redirect_uri: &str, scope: &str, state: &str) -> String {
        build_authorize_url_with_base(
            &self.accounts_base,
            &self.credentials.app_id,
            redirect_uri,
            scope,
            state,
        )
    }

    async fn request_tokens(
        &self,
        body: serde_json::Value,
    ) -> std::result::Result<TokenResponse, TokenEndpointError> {
        let request = Request {
            method: Method::Post,
            url: format!("{}{TOKEN_PATH}", self.accounts_base),
            query: Vec::new(),
            bearer_token: None,
            json_body: Some(body),
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(TOKEN_REQUEST_TIMEOUT_SECS),
            // code 与 refresh_token 都是一次性的；重发只会拿到「已使用」。
            idempotent: false,
        };
        let response = send_with_retry(
            self.transport.as_ref(),
            self.sleeper.as_ref(),
            &request,
            retry_policy(),
        )
        .await
        .map_err(|failure| TokenEndpointError::from(&failure))?;
        parse_token_response(&response.body)
    }

    /// 用授权码换第一对凭证。
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        now: i64,
    ) -> std::result::Result<TokenBundle, TokenEndpointError> {
        let response = self
            .request_tokens(json!({
                "grant_type": "authorization_code",
                "client_id": self.credentials.app_id,
                "client_secret": self.credentials.app_secret.expose_secret(),
                "code": code,
                "redirect_uri": redirect_uri,
            }))
            .await?;
        to_bundle(response, now)
    }

    /// 用刷新凭证换新的一对（旧的 `refresh_token` 立即作废）。
    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
        now: i64,
    ) -> std::result::Result<TokenBundle, TokenEndpointError> {
        let response = self
            .request_tokens(json!({
                "grant_type": "refresh_token",
                "client_id": self.credentials.app_id,
                "client_secret": self.credentials.app_secret.expose_secret(),
                "refresh_token": refresh_token,
            }))
            .await?;
        to_bundle(response, now)
    }

    /// 取授权人的 open_id 与姓名。
    pub async fn fetch_user_info(
        &self,
        access_token: &str,
    ) -> std::result::Result<(String, Option<String>), TokenEndpointError> {
        #[derive(Deserialize)]
        struct UserInfo {
            open_id: String,
            #[serde(default)]
            name: Option<String>,
        }
        #[derive(Deserialize)]
        struct Envelope {
            code: i32,
            #[serde(default)]
            msg: Option<String>,
            #[serde(default)]
            data: Option<UserInfo>,
        }

        let request = Request {
            method: Method::Get,
            url: format!("{}/open-apis/authen/v1/user_info", self.open_base),
            query: Vec::new(),
            bearer_token: Some(access_token.to_string()),
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(TOKEN_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        let response = send_with_retry(
            self.transport.as_ref(),
            self.sleeper.as_ref(),
            &request,
            retry_policy(),
        )
        .await
        .map_err(|failure| TokenEndpointError::from(&failure))?;
        let envelope: Envelope = serde_json::from_str(&response.body).map_err(|error| {
            TokenEndpointError::new(0, format!("用户信息响应解析失败: {error}"))
        })?;
        if envelope.code != 0 {
            return Err(TokenEndpointError::new(
                envelope.code,
                envelope.msg.unwrap_or_else(|| "取用户信息失败".to_string()),
            ));
        }
        let info = envelope
            .data
            .ok_or_else(|| TokenEndpointError::new(0, "用户信息响应里没有 data"))?;
        Ok((info.open_id, info.name))
    }
}

/// 供测试构造凭证。
#[cfg(test)]
pub(crate) fn test_credentials(app_id: &str, app_secret: &str) -> Credentials {
    Credentials {
        app_id: app_id.to_string(),
        app_secret: super::SecretString::new(app_secret),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Response;
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::Mutex;

    #[test]
    fn authorize_url_carries_offline_access_and_encodes_the_redirect() {
        let url = build_authorize_url(
            "cli_a",
            "https://example.com/cb?x=1",
            "contact:user:search offline_access",
            "state-1",
        );
        assert!(url.starts_with("https://accounts.feishu.cn/open-apis/authen/v1/authorize?"));
        assert!(url.contains("client_id=cli_a"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcb%3Fx%3D1"));
        assert!(url.contains("scope=contact%3Auser%3Asearch%20offline_access"));
        assert!(url.contains("state=state-1"));
        assert!(url.contains("prompt=consent"));
    }

    #[test]
    fn access_token_freshness_keeps_a_margin() {
        assert!(access_token_is_fresh(1_000 + 301, 1_000));
        assert!(!access_token_is_fresh(1_000 + 300, 1_000));
        assert!(!access_token_is_fresh(1_000, 1_000));
    }

    #[test]
    fn reminder_line_starts_at_350_days() {
        let day = 86_400;
        assert!(!reauth_reminder_due(0, 349 * day));
        assert!(reauth_reminder_due(0, 350 * day));
    }

    struct ScriptedTransport {
        bodies: Mutex<VecDeque<String>>,
        requests: Mutex<Vec<Request>>,
    }

    #[async_trait::async_trait]
    impl Transport for ScriptedTransport {
        async fn send(&self, request: Request) -> crate::error::Result<Response> {
            self.requests.lock().unwrap().push(request);
            Ok(Response {
                status: 200,
                body: self.bodies.lock().unwrap().pop_front().unwrap_or_default(),
                bytes: None,
                headers: BTreeMap::new(),
            })
        }
    }

    struct NoSleep;

    #[async_trait::async_trait]
    impl Sleeper for NoSleep {
        async fn sleep(&self, _duration: std::time::Duration) {}
    }

    fn client(bodies: Vec<&str>) -> (OAuthClient, Arc<ScriptedTransport>) {
        let transport = Arc::new(ScriptedTransport {
            bodies: Mutex::new(bodies.into_iter().map(str::to_string).collect()),
            requests: Mutex::new(Vec::new()),
        });
        let client = OAuthClient::new(
            test_credentials("cli_a", "secret"),
            "https://accounts.feishu.cn",
            "https://open.feishu.cn",
            transport.clone(),
            Arc::new(NoSleep),
        );
        (client, transport)
    }

    #[tokio::test]
    async fn exchange_code_uses_the_documented_body_and_expiry() {
        let (client, transport) = client(vec![
            r#"{
            "code": 0,
            "access_token": "u-1",
            "expires_in": 7200,
            "refresh_token": "r-1",
            "refresh_token_expires_in": 604800,
            "scope": "offline_access"
        }"#,
        ]);
        let bundle = client
            .exchange_code("code-1", "https://example.com/cb", 1_000)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bundle.access_token, "u-1");
        assert_eq!(bundle.access_token_expires_at, 8_200);
        assert_eq!(bundle.refresh_token_expires_at, 1_000 + 604_800);
        let request = &transport.requests.lock().unwrap()[0];
        assert_eq!(request.method, Method::Post);
        assert!(request.url.ends_with("/oauth/v3/token"));
        assert!(request.bearer_token.is_none());
        assert_eq!(
            request.json_body.as_ref().unwrap()["grant_type"],
            "authorization_code"
        );
    }

    #[tokio::test]
    async fn refresh_never_retries_and_reports_reauth_codes() {
        let (client, transport) = client(vec![
            r#"{"code":20073,"error":"refresh_token used","error_description":"used"}"#,
        ]);
        let error = client
            .refresh_tokens("r-1", 1_000)
            .await
            .err()
            .unwrap_or_else(|| panic!("应失败"));
        assert_eq!(error.code, 20073);
        assert!(error.needs_reauth());
        let request = &transport.requests.lock().unwrap()[0];
        assert!(!request.idempotent);
    }

    #[tokio::test]
    async fn fetch_user_info_returns_open_id_and_name() {
        let (client, transport) = client(vec![
            r#"{"code":0,"data":{"open_id":"ou_1","name":"张三"}}"#,
        ]);
        let (open_id, name) = client
            .fetch_user_info("u-1")
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(open_id, "ou_1");
        assert_eq!(name.as_deref(), Some("张三"));
        let request = &transport.requests.lock().unwrap()[0];
        assert_eq!(request.bearer_token.as_deref(), Some("u-1"));
    }
}
