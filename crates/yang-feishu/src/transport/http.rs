//! 基于 `reqwest` 的独立传输实现（feature `http`）。
//!
//! 宿主若已有自己的 HTTP 客户端与限流装饰器，应实现 [`Transport`] 注入，而不是启用
//! 本 feature——这样审批创建闸门、连接池等宿主能力不会被复制一份。
//!
//! [`Transport`]: super::Transport

use std::collections::BTreeMap;
use std::time::Duration;

use crate::error::{Error, FailureKind, Result};

use super::{Method, Request, Response, Transport};

/// reqwest 传输配置。
#[derive(Debug, Clone, Default)]
pub struct HttpConfig {
    /// 请求未指定 `timeout_secs` 时的默认超时（秒）。
    pub default_timeout_secs: Option<u64>,
    /// 可选的 User-Agent。
    pub user_agent: Option<String>,
}

impl HttpConfig {
    /// 默认配置：不设全局超时与 User-Agent。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置默认超时。
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.default_timeout_secs = Some(seconds);
        self
    }

    /// 设置 User-Agent。
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }
}

/// 生产级 reqwest 传输。
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    config: HttpConfig,
}

impl ReqwestTransport {
    /// 用默认配置构造。
    pub fn new() -> Result<Self> {
        Self::with_config(HttpConfig::default())
    }

    /// 用显式配置构造：无全局超时（每次请求自带 `timeout_secs` 或配置默认值）。
    pub fn with_config(config: HttpConfig) -> Result<Self> {
        // reqwest 的 `rustls-tls-webpki-roots-no-provider` 不自带 CryptoProvider，
        // 必须在构造客户端前安装一次。重复安装返回 Err，忽略即可。
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut builder = reqwest::Client::builder();
        if let Some(seconds) = config.default_timeout_secs {
            builder = builder.timeout(Duration::from_secs(seconds));
        }
        if let Some(user_agent) = config.user_agent.as_deref() {
            builder = builder.user_agent(user_agent);
        }
        let client = builder
            .build()
            .map_err(|error| Error::fatal(format!("构造 reqwest 客户端失败: {error}")))?;
        Ok(Self { client, config })
    }

    /// 用调用方构造好的 `reqwest::Client` 构造。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            config: HttpConfig::default(),
        }
    }
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        match Self::new() {
            Ok(transport) => transport,
            Err(error) => {
                tracing::warn!(error = %error, "构造默认 ReqwestTransport 失败，改用裸客户端");
                Self {
                    client: reqwest::Client::new(),
                    config: HttpConfig::default(),
                }
            }
        }
    }
}

fn transport_error(error: reqwest::Error) -> Error {
    let message = error.to_string();
    if error.is_timeout() || error.is_connect() || error.is_request() {
        Error::retry(message)
    } else {
        Error::new(FailureKind::Fatal { code: 0 }, message)
    }
}

#[async_trait::async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, request: Request) -> Result<Response> {
        let mut builder = match request.method {
            Method::Get => self.client.get(&request.url),
            Method::Post => self.client.post(&request.url),
            Method::Put => self.client.put(&request.url),
        };

        if !request.query.is_empty() {
            builder = builder.query(&request.query);
        }
        if let Some(token) = request.bearer_token.as_deref() {
            builder = builder.bearer_auth(token);
        }
        if let Some(body) = request.json_body.as_ref() {
            builder = builder
                .header("content-type", "application/json; charset=utf-8")
                .json(body);
        }
        if let Some((content_type, body)) = request.raw_body {
            builder = builder.header("content-type", content_type).body(body);
        }
        if let Some(timeout_secs) = request.timeout_secs.or(self.config.default_timeout_secs) {
            builder = builder.timeout(Duration::from_secs(timeout_secs));
        }

        let response = builder.send().await.map_err(transport_error)?;
        let status = response.status().as_u16();
        let headers: BTreeMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect();

        let json_response = !headers.contains_key("content-disposition")
            && headers
                .get("content-type")
                .is_some_and(|value| value.to_ascii_lowercase().contains("application/json"));

        let (body, bytes) = if request.binary_limit.is_some() && status == 200 && !json_response {
            let bytes = response.bytes().await.map_err(transport_error)?;
            if let Some(limit) = request.binary_limit {
                if bytes.len() > limit {
                    return Err(Error::new(
                        FailureKind::Fatal {
                            code: super::codes::CODE_TOO_LARGE_RESPONSE,
                        },
                        format!("响应体 {} 字节超过上限 {limit}", bytes.len()),
                    ));
                }
            }
            (String::new(), Some(bytes.to_vec()))
        } else {
            let body = response.text().await.map_err(transport_error)?;
            if let Some(limit) = request.binary_limit {
                if body.len() > limit {
                    return Err(Error::new(
                        FailureKind::Fatal {
                            code: super::codes::CODE_TOO_LARGE_RESPONSE,
                        },
                        format!("响应体 {} 字节超过上限 {limit}", body.len()),
                    ));
                }
            }
            (body, None)
        };

        Ok(Response {
            status,
            body,
            bytes,
            headers,
        })
    }
}
