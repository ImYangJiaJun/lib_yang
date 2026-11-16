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
            Method::Patch => self.client.patch(&request.url),
            Method::Delete => self.client.delete(&request.url),
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

        let mut response = builder.send().await.map_err(transport_error)?;
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

        // 二进制分支：binary_limit 且 200 且非 JSON 响应。
        let binary_limit = request
            .binary_limit
            .filter(|_| status == 200 && !json_response);
        let (body, bytes) = if let Some(limit) = binary_limit {
            // Content-Length 预检只做快速失败：工作区 reqwest 未启用自动解压，
            // `content_length()` 是原始 body 的精确长度；chunked 时为 None，
            // 回退到下面的逐块累计。上游可能省略或谎报长度，因此头部值不能
            // 作为唯一判据。
            if let Some(length) = response.content_length() {
                if length > limit as u64 {
                    return Err(Error::new(
                        FailureKind::Fatal {
                            code: super::codes::CODE_TOO_LARGE_RESPONSE,
                        },
                        format!("响应体 {length} 字节超过上限 {limit}"),
                    ));
                }
            }
            // 硬上限：逐块累计、先判后拷，超限立即中止，不把整个响应体读进内存。
            let mut collected: Vec<u8> = Vec::with_capacity(limit.min(64 * 1024));
            while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
                if collected.len().saturating_add(chunk.len()) > limit {
                    return Err(Error::new(
                        FailureKind::Fatal {
                            code: super::codes::CODE_TOO_LARGE_RESPONSE,
                        },
                        // 中止时拿不到实际总长度，省略字节总数（非契约差异）。
                        format!("响应体超过上限 {limit} 字节"),
                    ));
                }
                collected.extend_from_slice(&chunk);
            }
            (String::new(), Some(collected))
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

#[cfg(test)]
mod tests {
    //! 二进制响应上限的本地 TCP 覆盖。
    //!
    //! 修复前（RED）：二进制分支用 `response.bytes()` 全量收集后才比较上限。
    //! - Content-Length 已超限但 body 未发完时，旧实现会一直等待整个 body；
    //! - chunked 超限响应永远等不到结束块，旧实现会挂到测试超时。
    //!
    //! 修复后：Content-Length 只做快速失败，`chunk()` 累计先判后拷、超限提前中止。

    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;
    use crate::transport::codes::CODE_TOO_LARGE_RESPONSE;

    /// 一次性本地服务：读到请求头后写出 `response`。
    ///
    /// `close_after_write = false` 时保持连接不关，用来验证客户端在 body 未结束时
    /// 就应中止；`true` 时 shutdown 写端，构造无 Content-Length 的 close 定界响应。
    async fn serve_once(response: Vec<u8>, close_after_write: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("绑定本地端口");
        let address = listener.local_addr().expect("读取本地地址");
        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = [0u8; 1024];
            let mut received = Vec::new();
            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) | Err(_) => return,
                    Ok(read) => {
                        received.extend_from_slice(&buffer[..read]);
                        if received.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }
            if socket.write_all(&response).await.is_err() {
                return;
            }
            if socket.flush().await.is_err() {
                return;
            }
            if close_after_write && socket.shutdown().await.is_err() {
                return;
            }
            // 保持连接：提前中止类测试要求服务端“还没写完”。
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        format!("http://{address}")
    }

    fn binary_request(base: &str, limit: usize) -> Request {
        Request {
            method: Method::Get,
            url: format!("{base}/file"),
            query: Vec::new(),
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: Some(limit),
            timeout_secs: None,
            idempotent: true,
        }
    }

    fn transport() -> ReqwestTransport {
        ReqwestTransport::new().expect("构造 ReqwestTransport")
    }

    fn assert_too_large(error: Error) {
        assert_eq!(
            error.kind,
            FailureKind::Fatal {
                code: CODE_TOO_LARGE_RESPONSE
            },
            "超限必须是 Fatal TOO_LARGE（不重试），实际: {error}"
        );
    }

    #[tokio::test]
    async fn binary_response_over_limit_from_content_length_fails_fast() {
        // RED：旧实现先 `bytes().await`，服务端不发送声明的 body 时会一直等待。
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 1048576\r\n\r\n".to_vec();
        let base = serve_once(response, false).await;
        let transport = transport();
        let error = tokio::time::timeout(
            Duration::from_secs(2),
            transport.send(binary_request(&base, 1024)),
        )
        .await
        .expect("Content-Length 超限必须快速失败，不能等待 body 读完")
        .expect_err("超限响应必须被拒绝");
        assert_too_large(error);
    }

    #[tokio::test]
    async fn binary_response_exactly_at_limit_succeeds() {
        // 上限判断保持 `>`：恰好等于 limit 必须成功，且字节逐一相同。
        let payload: Vec<u8> = (0..8).collect();
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        )
        .into_bytes();
        response.extend_from_slice(&payload);
        let base = serve_once(response, false).await;
        let transport = transport();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            transport.send(binary_request(&base, payload.len())),
        )
        .await
        .expect("恰好等于上限必须成功")
        .expect("恰好等于上限必须成功");
        assert_eq!(result.status, 200);
        assert_eq!(result.body, "");
        assert_eq!(result.bytes.as_deref(), Some(payload.as_slice()));
    }

    #[tokio::test]
    async fn binary_response_chunked_over_limit_aborts_before_end() {
        // RED：旧实现要等 chunked 结束块才比较上限；这里永远不发送 0 长度结束块。
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n".to_vec();
        let base = serve_once(response, false).await;
        let transport = transport();
        let error = tokio::time::timeout(
            Duration::from_secs(2),
            transport.send(binary_request(&base, 4)),
        )
        .await
        .expect("chunked 超限必须在累计超限时立即中止")
        .expect_err("chunked 超限响应必须被拒绝");
        assert_too_large(error);
    }

    #[tokio::test]
    async fn binary_response_without_content_length_is_still_bounded() {
        // close 定界（无 Content-Length、非 chunked）：硬上限由 chunk() 累计保证。
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n".to_vec();
        response.extend_from_slice(&[9u8; 16]);
        let base = serve_once(response, true).await;
        let transport = transport();
        let error = tokio::time::timeout(
            Duration::from_secs(2),
            transport.send(binary_request(&base, 4)),
        )
        .await
        .expect("无 Content-Length 的超限响应也必须中止")
        .expect_err("超限响应必须被拒绝");
        assert_too_large(error);
    }
}
