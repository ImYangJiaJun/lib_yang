//! 客户端装配：凭证、端点、传输、睡眠器、令牌缓存。
//!
//! 独立使用者只装配一次即可拿到全部能力：
//!
//! ```rust,ignore
//! let client = OpenApiClient::builder(
//!     OpenApiConfig::static_credentials("cli_xxx", "secret")
//!         .endpoints(Endpoints::feishu())
//!         .cache_namespace("demo"),
//! )
//! .sleeper(std::sync::Arc::new(TokioSleeper))
//! .token_cache(std::sync::Arc::new(MemoryTokenCache::default()))
//! .build()?;
//! ```
//!
//! 宿主已有 HTTP/Redis 时注入自己的 `Transport` / `TokenCache`，不要启用
//! `http` / `redis` feature。

use std::sync::Arc;

use crate::api::ApiClient;
use crate::approval::ApprovalClient;
use crate::auth::oauth::OAuthClient;
use crate::auth::token::{MemoryTokenCache, TokenCache, TokenProvider};
use crate::auth::{Credentials, CredentialsProvider, SecretString, StaticCredentials};
use crate::bitable::BitableClient;
use crate::contact::ContactClient;
use crate::drive::DriveClient;
use crate::endpoints::Endpoints;
use crate::error::Result;
use crate::im::ImClient;
use crate::transport::{
    send_with_retry, Method, Request, Response, RetryPolicy, Sleeper, TokioSleeper, Transport,
    PULL_RETRY,
};

/// 客户端静态配置。
#[derive(Clone)]
pub struct OpenApiConfig {
    credentials: Arc<dyn CredentialsProvider>,
    endpoints: Endpoints,
    cache_namespace: String,
}

impl std::fmt::Debug for OpenApiConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenApiConfig")
            .field("credentials", &self.credentials.credentials())
            .field("endpoints", &self.endpoints)
            .field("cache_namespace", &self.cache_namespace)
            .finish()
    }
}

impl OpenApiConfig {
    /// 由固定凭证构造。
    pub fn static_credentials(app_id: impl AsRef<str>, app_secret: impl AsRef<str>) -> Self {
        Self {
            credentials: Arc::new(StaticCredentials::new(Credentials {
                app_id: app_id.as_ref().to_string(),
                app_secret: SecretString::new(app_secret.as_ref()),
            })),
            endpoints: Endpoints::feishu(),
            cache_namespace: "yang-feishu".to_string(),
        }
    }

    /// 由自定义凭证来源构造（配置热更新、密钥托管）。
    pub fn with_provider(provider: Arc<dyn CredentialsProvider>) -> Self {
        Self {
            credentials: provider,
            endpoints: Endpoints::feishu(),
            cache_namespace: "yang-feishu".to_string(),
        }
    }

    /// 覆盖端点域名。
    pub fn endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// 覆盖令牌缓存键前缀（通常是部署名）。
    pub fn cache_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.cache_namespace = namespace.into();
        self
    }
}

/// 客户端构造器。
pub struct OpenApiClientBuilder {
    config: OpenApiConfig,
    transport: Option<Arc<dyn Transport>>,
    sleeper: Option<Arc<dyn Sleeper>>,
    token_cache: Option<Arc<dyn TokenCache>>,
}

impl OpenApiClientBuilder {
    /// 注入传输（宿主自己的 HTTP 适配器）。
    pub fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// 注入睡眠器（测试可用记录型替身）。
    pub fn sleeper(mut self, sleeper: Arc<dyn Sleeper>) -> Self {
        self.sleeper = Some(sleeper);
        self
    }

    /// 注入令牌缓存（宿主自己的 Redis 适配器）。
    pub fn token_cache(mut self, cache: Arc<dyn TokenCache>) -> Self {
        self.token_cache = Some(cache);
        self
    }

    /// 完成装配。
    pub fn build(self) -> Result<OpenApiClient> {
        let transport = match self.transport {
            Some(transport) => transport,
            #[cfg(feature = "http")]
            None => Arc::new(crate::transport::http::ReqwestTransport::new()?),
            #[cfg(not(feature = "http"))]
            None => {
                return Err(crate::error::Error::fatal(
                    "未配置 Transport：注入自定义 Transport 或启用 http feature",
                ))
            }
        };
        let sleeper = self.sleeper.unwrap_or_else(|| Arc::new(TokioSleeper));
        let cache = self
            .token_cache
            .unwrap_or_else(|| Arc::new(MemoryTokenCache::default()));
        let credentials = self.config.credentials.credentials().clone();
        let tokens = Arc::new(TokenProvider::new(
            cache,
            transport.clone(),
            sleeper.clone(),
            credentials.clone(),
            self.config.cache_namespace.clone(),
            self.config.endpoints.open.as_str(),
        ));
        Ok(OpenApiClient {
            core: Arc::new(ClientCore {
                config: self.config,
                transport,
                sleeper,
                tokens,
                credentials,
            }),
        })
    }
}

/// 所有子客户端共享的内部状态。
pub(crate) struct ClientCore {
    config: OpenApiConfig,
    transport: Arc<dyn Transport>,
    sleeper: Arc<dyn Sleeper>,
    tokens: Arc<TokenProvider>,
    credentials: Credentials,
}

impl ClientCore {
    pub(crate) fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    pub(crate) fn endpoints(&self) -> &Endpoints {
        &self.config.endpoints
    }

    pub(crate) fn tokens(&self) -> &Arc<TokenProvider> {
        &self.tokens
    }

    /// 发送不带鉴权的请求（OAuth 令牌端点、用户 token 调用）。
    pub(crate) async fn send_raw(
        &self,
        request: Request,
        policy: &RetryPolicy,
    ) -> Result<Response> {
        send_with_retry(
            self.transport.as_ref(),
            self.sleeper.as_ref(),
            &request,
            *policy,
        )
        .await
    }

    /// 取 tenant token 后发送；失效时清缓存强刷一次。
    pub(crate) async fn send_tenant(
        &self,
        mut request: Request,
        policy: &RetryPolicy,
    ) -> Result<Response> {
        let mut refreshed = false;
        loop {
            request.bearer_token = Some(self.tokens.tenant_access_token().await?);
            match send_with_retry(
                self.transport.as_ref(),
                self.sleeper.as_ref(),
                &request,
                *policy,
            )
            .await
            {
                Ok(response) => return Ok(response),
                Err(error)
                    if matches!(error.kind, crate::error::FailureKind::TokenExpired)
                        && !refreshed =>
                {
                    refreshed = true;
                    tracing::warn!("飞书 tenant_access_token 失效，清缓存并强制刷新一次");
                    self.tokens
                        .invalidate_and_refresh()
                        .await
                        .map_err(|refresh_error| {
                            crate::error::Error::new(
                                crate::error::FailureKind::Fatal { code: 0 },
                                format!("tenant_access_token 失效后强制刷新失败: {refresh_error}"),
                            )
                        })?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// 长尾端点：tenant token + JSON 响应。
    pub(crate) async fn call_json(
        &self,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        body: Option<serde_json::Value>,
        timeout_secs: u64,
        idempotent: bool,
    ) -> Result<serde_json::Value> {
        let response = self
            .send_tenant(
                Request {
                    method,
                    url,
                    query,
                    bearer_token: None,
                    json_body: body,
                    raw_body: None,
                    binary_limit: None,
                    timeout_secs: Some(timeout_secs),
                    idempotent,
                },
                &PULL_RETRY,
            )
            .await?;
        parse_json_body(&response.body)
    }

    /// 长尾端点：无鉴权调用（应用凭证放在请求体），返回 JSON 响应。
    pub(crate) async fn call_json_public(
        &self,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        body: Option<serde_json::Value>,
        timeout_secs: u64,
        idempotent: bool,
    ) -> Result<serde_json::Value> {
        let response = self
            .send_raw(
                Request {
                    method,
                    url,
                    query,
                    bearer_token: None,
                    json_body: body,
                    raw_body: None,
                    binary_limit: None,
                    timeout_secs: Some(timeout_secs),
                    idempotent,
                },
                &PULL_RETRY,
            )
            .await?;
        parse_json_body(&response.body)
    }

    /// 长尾端点：tenant token + 原始请求体（multipart/form-data），JSON 响应。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn call_raw_json(
        &self,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        content_type: &str,
        body: &[u8],
        timeout_secs: u64,
        idempotent: bool,
    ) -> Result<serde_json::Value> {
        let response = self
            .send_tenant(
                raw_request(
                    method,
                    url,
                    query,
                    content_type,
                    body,
                    timeout_secs,
                    idempotent,
                    None,
                ),
                &PULL_RETRY,
            )
            .await?;
        parse_json_body(&response.body)
    }

    /// 长尾端点：调用方自带 bearer token + 原始请求体，JSON 响应。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn call_raw_json_user(
        &self,
        bearer_token: &str,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        content_type: &str,
        body: &[u8],
        timeout_secs: u64,
        idempotent: bool,
    ) -> Result<serde_json::Value> {
        let response = self
            .send_raw(
                raw_request(
                    method,
                    url,
                    query,
                    content_type,
                    body,
                    timeout_secs,
                    idempotent,
                    Some(bearer_token),
                ),
                &PULL_RETRY,
            )
            .await?;
        parse_json_body(&response.body)
    }

    /// 长尾端点：调用方自带 user_access_token + JSON 响应。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn call_json_user(
        &self,
        bearer_token: &str,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        body: Option<serde_json::Value>,
        timeout_secs: u64,
        idempotent: bool,
    ) -> Result<serde_json::Value> {
        let response = self
            .send_raw(
                Request {
                    method,
                    url,
                    query,
                    bearer_token: Some(bearer_token.to_string()),
                    json_body: body,
                    raw_body: None,
                    binary_limit: None,
                    timeout_secs: Some(timeout_secs),
                    idempotent,
                },
                &PULL_RETRY,
            )
            .await?;
        parse_json_body(&response.body)
    }
}

#[allow(clippy::too_many_arguments)]
fn raw_request(
    method: Method,
    url: String,
    query: Vec<(String, String)>,
    content_type: &str,
    body: &[u8],
    timeout_secs: u64,
    idempotent: bool,
    bearer_token: Option<&str>,
) -> Request {
    Request {
        method,
        url,
        query,
        bearer_token: bearer_token.map(str::to_string),
        json_body: None,
        raw_body: Some((content_type.to_string(), body.to_vec())),
        binary_limit: None,
        timeout_secs: Some(timeout_secs),
        idempotent,
    }
}

fn parse_json_body(body: &str) -> Result<serde_json::Value> {
    serde_json::from_str(body)
        .map_err(|error| crate::error::Error::fatal(format!("解析飞书响应失败: {error}")))
}

/// 飞书开放平台客户端。
#[derive(Clone)]
pub struct OpenApiClient {
    core: Arc<ClientCore>,
}

impl OpenApiClient {
    /// 开始构造。
    pub fn builder(config: OpenApiConfig) -> OpenApiClientBuilder {
        OpenApiClientBuilder {
            config,
            transport: None,
            sleeper: None,
            token_cache: None,
        }
    }

    /// 应用 app_id。
    pub fn app_id(&self) -> &str {
        &self.core.credentials().app_id
    }

    /// 端点配置。
    pub fn endpoints(&self) -> &Endpoints {
        self.core.endpoints()
    }

    /// 多维表格客户端。
    pub fn bitable(&self) -> BitableClient {
        BitableClient::new(self.core.clone())
    }

    /// 审批客户端。
    pub fn approval(&self) -> ApprovalClient {
        ApprovalClient::new(self.core.clone())
    }

    /// 通讯录客户端。
    pub fn contact(&self) -> ContactClient {
        ContactClient::new(self.core.clone())
    }

    /// IM 客户端。
    pub fn im(&self) -> ImClient {
        ImClient::new(self.core.clone())
    }

    /// 云盘/附件客户端。
    pub fn drive(&self) -> DriveClient {
        DriveClient::new(self.core.clone())
    }

    /// 官方 OpenAPI 全量端点薄封装。
    pub fn api(&self) -> ApiClient {
        ApiClient::new(self.core.clone())
    }

    /// 用户 OAuth 客户端。
    pub fn oauth(&self) -> OAuthClient {
        OAuthClient::new(
            self.core.credentials.clone(),
            self.core.endpoints().accounts.as_str(),
            self.core.endpoints().open.as_str(),
            self.core.transport.clone(),
            self.core.sleeper.clone(),
        )
    }

    /// 令牌提供者（供宿主复用，例如事件 worker）。
    pub fn tokens(&self) -> Arc<TokenProvider> {
        self.core.tokens().clone()
    }

    /// 发送**不带鉴权**的请求，按给定策略重试。
    pub async fn send_raw(&self, request: Request, policy: &RetryPolicy) -> Result<Response> {
        self.core.send_raw(request, policy).await
    }

    /// 发送带 tenant_access_token 的请求，按给定策略重试。
    pub async fn send_tenant(&self, request: Request, policy: &RetryPolicy) -> Result<Response> {
        self.core.send_tenant(request, policy).await
    }
}
