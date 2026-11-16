//! HTTP 客户端核心实现
//!
//! 提供 HTTP 客户端和请求构建能力；实例在启动期经
//! [`ToolsBuilder::http`](crate::tools::ToolsBuilder) 注册进应用资源，
//! 运行期通过 `Tools::http()` 或 `ActionContext::http()` 获取。

use crate::error::BaseError;
use crate::http::request::RequestBuilder;
use reqwest::{Client, Method};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 出站响应体默认上限（10 MiB，与出站场景对齐；服务端侧为 DEFAULT_MAX_BODY_BYTES = 2 MiB）
const DEFAULT_MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// HTTP 重定向跟随策略。
///
/// 通过 [`HttpClientConfig::redirect`] 配置。默认
/// `HttpRedirectPolicy::Limited(10)`，与 reqwest 0.12 的默认策略
/// `reqwest::redirect::Policy::limited(10)` 等价：最多跟随 10 次重定向。
///
/// 该策略只控制 client 级重定向跟随行为，**不逐跳校验目标 host，也不会额外
/// 剥离自定义凭证头**。需要 SSRF 白名单的调用方应显式设为
/// [`HttpRedirectPolicy::None`]，自行检查 3xx 响应的 `Location` 后再发起受控请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HttpRedirectPolicy {
    /// 不跟随任何重定向：3xx 原样返回，`Location` 响应头保留给调用方判断。
    None,
    /// 最多跟随 `n` 次重定向；超过上限时 reqwest 返回 `TooManyRedirects` 请求错误。
    ///
    /// `Limited(0)` 与 [`HttpRedirectPolicy::None`] 的区别：前者在遇到第一个 3xx 时
    /// 返回错误，后者把 3xx 响应原样返回。
    Limited(usize),
}

impl Default for HttpRedirectPolicy {
    fn default() -> Self {
        // 显式对齐 reqwest 0.12.16 的默认策略，而非依赖 reqwest 未来的默认值。
        Self::Limited(10)
    }
}

impl HttpRedirectPolicy {
    /// 映射为 reqwest 的 client 级重定向策略。
    fn to_reqwest(self) -> reqwest::redirect::Policy {
        match self {
            Self::None => reqwest::redirect::Policy::none(),
            Self::Limited(max) => reqwest::redirect::Policy::limited(max),
        }
    }
}

/// HTTP 客户端配置
///
/// 用于通过 `HttpClient::with_config` 创建自定义配置的客户端
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::http::{HttpClient, HttpClientConfig};
///
/// let config = HttpClientConfig {
///     timeout_secs: 60,
///     pool_max_idle_per_host: 10,
///     user_agent: Some("MyApp/1.0".to_string()),
///     ..Default::default()
/// };
/// let client = HttpClient::with_config(config)?;
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HttpClientConfig {
    /// 请求超时时间（秒），默认 30 秒
    pub timeout_secs: u64,

    /// 每个主机最大空闲连接数，默认 10
    pub pool_max_idle_per_host: usize,

    /// 连接池空闲超时时间（秒），默认 90 秒
    pub pool_idle_timeout_secs: u64,

    /// 自定义 User-Agent，默认为 None（使用 reqwest 默认值）
    pub user_agent: Option<String>,

    /// 是否接受无效的 TLS 证书，默认 false（生产环境不应设为 true）
    pub accept_invalid_certs: bool,

    /// 代理 URL，默认为 None（不使用代理）
    pub proxy_url: Option<String>,

    /// 熔断器策略，默认 None（不启用）。设为 `Some(..)` 后，对连续失败的目标
    /// host 快速失败（返回 `BaseError::HttpCircuitBreakerOpen`），按 host 分键。
    pub circuit_breaker: Option<crate::http::CircuitBreakerConfig>,

    /// 出站响应体最大允许字节数，默认 10 MiB（`DEFAULT_MAX_RESPONSE_BYTES`）。
    ///
    /// 读取响应体（`bytes`/`json`/`text`）时先查 Content-Length 做快速失败，
    /// 再流式累计校验运行中上限，超限返回 `BaseError::HttpResponseTooLarge`。
    pub max_response_bytes: usize,

    /// 重定向跟随策略，默认 `HttpRedirectPolicy::Limited(10)`（reqwest 默认：
    /// 最多跟随 10 跳）。
    ///
    /// 默认策略不逐跳校验目标 host，也不会额外剥离自定义凭证头；需要 SSRF
    /// 白名单或严格同源校验的调用方应显式设为 [`HttpRedirectPolicy::None`]，
    /// 自行校验 3xx 的 `Location`。
    pub redirect: HttpRedirectPolicy,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            pool_max_idle_per_host: 10,
            pool_idle_timeout_secs: 90,
            user_agent: None,
            accept_invalid_certs: false,
            proxy_url: None,
            circuit_breaker: None,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            redirect: HttpRedirectPolicy::default(),
        }
    }
}

impl HttpClientConfig {
    /// 验证 HTTP 客户端配置。
    ///
    /// 拒绝会导致请求立即失败或连接池配置退化的零值参数。
    pub fn validate(&self) -> Result<(), BaseError> {
        if self.timeout_secs == 0 {
            return Err(BaseError::ParamInvalid(
                "http.timeout_secs".to_string(),
                "HTTP 请求超时时间必须大于 0 秒".to_string(),
            ));
        }
        if self.pool_max_idle_per_host == 0 {
            return Err(BaseError::ParamInvalid(
                "http.pool_max_idle_per_host".to_string(),
                "每个主机最大空闲连接数必须大于 0".to_string(),
            ));
        }
        if self.pool_idle_timeout_secs == 0 {
            return Err(BaseError::ParamInvalid(
                "http.pool_idle_timeout_secs".to_string(),
                "连接池空闲超时时间必须大于 0 秒".to_string(),
            ));
        }
        if self.max_response_bytes == 0 {
            return Err(BaseError::ParamInvalid(
                "http.max_response_bytes".to_string(),
                "HTTP 响应体上限必须大于 0 字节".to_string(),
            ));
        }
        if let Some(circuit_breaker) = &self.circuit_breaker {
            circuit_breaker.validate()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_http_client_config_validate_rejects_zero_values() {
        let invalid_configs = [
            HttpClientConfig {
                timeout_secs: 0,
                ..HttpClientConfig::default()
            },
            HttpClientConfig {
                pool_max_idle_per_host: 0,
                ..HttpClientConfig::default()
            },
            HttpClientConfig {
                pool_idle_timeout_secs: 0,
                ..HttpClientConfig::default()
            },
            HttpClientConfig {
                max_response_bytes: 0,
                ..HttpClientConfig::default()
            },
        ];

        for config in invalid_configs {
            let err = config.validate().expect_err("HTTP 客户端零值配置应被拒绝");

            assert!(matches!(err, BaseError::ParamInvalid(_, _)));
        }
    }

    #[test]
    fn test_with_config_rejects_invalid_config_before_building_client() {
        let config = HttpClientConfig {
            timeout_secs: 0,
            ..HttpClientConfig::default()
        };

        let err = HttpClient::with_config(config)
            .expect_err("无效 HTTP 配置应在构建 reqwest client 前被拒绝");

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "http.timeout_secs"));
    }

    #[test]
    fn test_http_client_config_validate_rejects_invalid_circuit_breaker_config() {
        let config = HttpClientConfig {
            circuit_breaker: Some(crate::http::CircuitBreakerConfig {
                failure_threshold: 0,
                ..crate::http::CircuitBreakerConfig::default()
            }),
            ..HttpClientConfig::default()
        };

        let err = config
            .validate()
            .expect_err("HTTP 客户端应拒绝非法熔断器配置");

        assert!(matches!(
            err,
            BaseError::ParamInvalid(field, _) if field == "http.circuit_breaker.failure_threshold"
        ));
    }

    #[test]
    fn test_default_redirect_policy_stays_backward_compatible_and_validates() {
        // 条件：新增 redirect 配置的 Default 必须等价于 reqwest 0.12.16 的默认
        // `Policy::limited(10)`，且 validate() 不得新增会拒绝既有合法配置的校验。
        assert_eq!(
            HttpClientConfig::default().redirect,
            HttpRedirectPolicy::Limited(10)
        );
        assert_eq!(
            HttpRedirectPolicy::default(),
            HttpRedirectPolicy::Limited(10)
        );
        assert!(HttpClientConfig::default().validate().is_ok());
        assert!(HttpClientConfig {
            redirect: HttpRedirectPolicy::None,
            ..HttpClientConfig::default()
        }
        .validate()
        .is_ok());
        assert!(HttpClientConfig {
            redirect: HttpRedirectPolicy::Limited(0),
            ..HttpClientConfig::default()
        }
        .validate()
        .is_ok());
    }
}

#[cfg(test)]
mod redirect_policy_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 启动一次性 HTTP 服务：读走一个请求后返回固定响应字节。
    async fn serve_once(response: Vec<u8>) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("本地监听端口应可绑定");
        let addr = listener.local_addr().expect("应能取得监听地址");

        tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            // 读走请求，避免客户端写请求时收到 EPIPE
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).await;
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });

        addr
    }

    fn found_response(location: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes()
    }

    fn ok_response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    /// 回归护栏：默认配置必须保持 reqwest 默认的 `limited(10)`，合法 302 链路
    /// （同一 127.0.0.1、仅端口不同）仍应自动跟随，行为与修复前一致。
    #[tokio::test]
    async fn test_default_redirect_policy_follows_302_as_before() {
        let target = serve_once(ok_response("ok")).await;
        let target_url = format!("http://{target}/final");
        let origin = serve_once(found_response(&target_url)).await;

        let client = HttpClient::new(5).expect("默认客户端应可创建");
        let response = client
            .get(&format!("http://{origin}/start"))
            .send()
            .await
            .expect("默认策略下请求应成功");

        assert_eq!(response.status(), 200, "默认策略应跟随 302");
        assert_eq!(
            response.url(),
            target_url,
            "Response::url() 应返回跟随重定向后的最终 URL"
        );
        assert_eq!(
            response.text().await.expect("应读到最终响应体"),
            "ok",
            "默认策略应落到重定向目标"
        );
    }

    /// RED（修复前）：`HttpClientConfig` 没有 redirect 配置，调用方无法关闭自动
    /// 跟随，只能被动落到重定向目标，拿不到原始 302 与 Location。
    #[tokio::test]
    async fn test_redirect_policy_none_returns_3xx_without_following() {
        let target = serve_once(ok_response("must-not-be-read")).await;
        let target_url = format!("http://{target}/final");
        let origin = serve_once(found_response(&target_url)).await;

        let client = HttpClient::with_config(HttpClientConfig {
            redirect: HttpRedirectPolicy::None,
            ..HttpClientConfig::default()
        })
        .expect("合法配置应创建客户端");

        let response = client
            .get(&format!("http://{origin}/start"))
            .send()
            .await
            .expect("None 策略下 3xx 应作为成功响应返回");

        assert_eq!(response.status(), 302, "None 应原样返回 3xx");
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        assert_eq!(
            location.as_deref(),
            Some(target_url.as_str()),
            "Location 必须保留给调用方做逐跳校验"
        );
    }

    /// `Limited(n)` 必须真正生效：配置 1 跳时，默认的 10 跳策略会跟随的第二跳
    /// 应触发 reqwest 的 TooManyRedirects 错误。
    #[tokio::test]
    async fn test_redirect_policy_limited_stops_after_configured_hops() {
        let final_server = serve_once(ok_response("too-far")).await;
        let second_url = format!("http://{final_server}/second");
        let second = serve_once(found_response(&second_url)).await;
        let first_url = format!("http://{second}/first");
        let origin = serve_once(found_response(&first_url)).await;

        let client = HttpClient::with_config(HttpClientConfig {
            redirect: HttpRedirectPolicy::Limited(1),
            ..HttpClientConfig::default()
        })
        .expect("合法配置应创建客户端");

        // `Response` 未实现 Debug，不能对 `Result<Response, _>` 用 expect_err。
        let err = match client.get(&format!("http://{origin}/start")).send().await {
            Ok(_) => panic!("超过跳数上限应返回请求错误"),
            Err(err) => err,
        };

        assert!(
            matches!(&err, BaseError::HttpRequestFailed(_)),
            "跳数超限应映射为 HttpRequestFailed，实际: {err:?}"
        );
    }
}

/// HTTP 客户端
///
/// 提供 HTTP 请求构建和发送能力。
///
/// 内部持有 `Arc<reqwest::Client>`，`clone()` 时复用同一连接池（即 `Arc::clone`），
/// 不会创建新的底层连接池。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::http::HttpClient;
/// use yang_base::tools::ToolsBuilder;
///
/// // 启动期：把客户端注册进应用资源并冻结
/// let tools = ToolsBuilder::new().http(HttpClient::new(30)?).build()?;
///
/// // 运行期：从 Tools 获取客户端发起请求（Action 内为 ctx.http()?）
/// let response = tools.http()?
///     .get("https://api.example.com/users")
///     .send()
///     .await?;
/// ```
#[derive(Clone, Debug)]
pub struct HttpClient {
    /// reqwest 客户端（Arc 包装，clone 时复用同一连接池）
    client: Client,

    /// 默认超时时间
    default_timeout: Duration,

    /// 出站响应体最大允许字节数
    max_response_bytes: usize,

    /// 默认 Token（可选）
    default_token: Arc<RwLock<Option<String>>>,

    /// 熔断器（可选，默认 None）。clone 时共享同一份状态。
    circuit_breaker: Option<crate::http::CircuitBreaker>,
}

impl HttpClient {
    /// 使用结构化配置创建 HTTP 客户端
    ///
    /// # 参数
    ///
    /// - `cfg`: HTTP 客户端配置
    ///
    /// # 返回
    ///
    /// - `Ok(HttpClient)`: 客户端实例
    /// - `Err(BaseError::HttpClientCreateFailed)`: 创建失败（如代理 URL 无效）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::http::{HttpClient, HttpClientConfig};
    ///
    /// let config = HttpClientConfig {
    ///     timeout_secs: 60,
    ///     pool_max_idle_per_host: 20,
    ///     ..Default::default()
    /// };
    /// let client = HttpClient::with_config(config)?;
    /// ```
    pub fn with_config(cfg: HttpClientConfig) -> Result<Self, BaseError> {
        cfg.validate()?;

        // 安装 rustls 的进程级 CryptoProvider。
        //
        // 本 crate 走的是 reqwest 的 `rustls-tls-webpki-roots-no-provider`（原因见
        // 工作区 `Cargo.toml` 里 reqwest 那段注释：带 provider 的变体会经
        // `__rustls-ring` 的 `quinn?/ring` 把 quinn 及其 `rand 0.10` 链锁进
        // `Cargo.lock`，而那条链需要 edition2024，直接击穿 MSRV 1.80）。
        // `-no-provider` 不替我们选 provider，因此必须在这里显式安装：
        // 不装的话**第一次 TLS 握手**才会失败（"no process-level CryptoProvider
        // available"），编译期与构造期都看不出来。
        //
        // provider 是进程级的，重复安装返回 `Err`，属预期，忽略即可。
        let _ = rustls::crypto::ring::default_provider().install_default();

        // 构建 reqwest 客户端
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .pool_max_idle_per_host(cfg.pool_max_idle_per_host)
            .pool_idle_timeout(Duration::from_secs(cfg.pool_idle_timeout_secs))
            .danger_accept_invalid_certs(cfg.accept_invalid_certs);

        // 设置重定向策略：默认显式落到 reqwest 默认的 limited(10)，未配置新策略的
        // 调用方与修复前行为完全一致；重定向仍由 reqwest client 级策略处理。
        builder = builder.redirect(cfg.redirect.to_reqwest());

        // 设置自定义 User-Agent
        if let Some(ua) = cfg.user_agent {
            builder = builder.user_agent(ua);
        }

        // 设置代理
        if let Some(proxy_url) = cfg.proxy_url {
            let proxy =
                reqwest::Proxy::all(&proxy_url).map_err(BaseError::HttpClientCreateFailed)?;
            builder = builder.proxy(proxy);
        }

        let client = builder.build().map_err(BaseError::HttpClientCreateFailed)?;

        let circuit_breaker = cfg
            .circuit_breaker
            .map(crate::http::CircuitBreaker::new)
            .transpose()?;

        Ok(Self {
            client,
            default_timeout: Duration::from_secs(cfg.timeout_secs),
            max_response_bytes: cfg.max_response_bytes,
            default_token: Arc::new(RwLock::new(None)),
            circuit_breaker,
        })
    }

    /// 创建新的 HTTP 客户端
    ///
    /// 委托给 `with_config`，使用默认配置并仅覆盖超时时间。
    ///
    /// # 参数
    ///
    /// - `timeout_secs`: 默认超时时间（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(HttpClient)`: 客户端实例
    /// - `Err(BaseError)`: 创建失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let client = HttpClient::new(30)?;
    /// ```
    pub fn new(timeout_secs: u64) -> Result<Self, BaseError> {
        // 委托给 with_config，仅覆盖超时时间
        Self::with_config(HttpClientConfig {
            timeout_secs,
            ..Default::default()
        })
    }

    /// 设置默认 Token
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut client = HttpClient::new(30)?;
    /// client.set_default_token("your_token".to_string());
    /// ```
    pub fn set_default_token(&self, token: String) {
        // 使用 unwrap_or_else 处理锁中毒：即使锁中毒也能恢复数据并继续写入
        let mut default_token = self
            .default_token
            .write()
            .unwrap_or_else(|p| p.into_inner());
        *default_token = Some(token);
        log::debug!("已设置默认 Token");
    }

    /// 获取默认 Token
    fn get_default_token(&self) -> Option<String> {
        // 使用 unwrap_or_else 处理锁中毒：即使锁中毒也能恢复数据并继续读取
        let default_token = self.default_token.read().unwrap_or_else(|p| p.into_inner());
        default_token.clone()
    }

    /// 创建请求构建器
    ///
    /// 注意：`self.client.clone()` 是 `Arc::clone`，复用同一底层连接池，
    /// 不会创建新的 TCP 连接池，多个请求共享连接。
    ///
    /// # 参数
    ///
    /// - `method`: HTTP 方法
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use reqwest::Method;
    ///
    /// let response = client
    ///     .request(Method::GET, "https://api.example.com/users")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        // self.client.clone() 是 Arc::clone，复用同一连接池
        RequestBuilder::new(
            self.client.clone(),
            method,
            url.to_string(),
            self.default_timeout,
            self.max_response_bytes,
            self.get_default_token(),
            self.circuit_breaker.clone(),
        )
    }

    /// GET 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn get(&self, url: &str) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// POST 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/users")
    ///     .json(&user)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn post(&self, url: &str) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// PUT 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .put("https://api.example.com/users/1")
    ///     .json(&user)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn put(&self, url: &str) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    /// DELETE 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .delete("https://api.example.com/users/1")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// PATCH 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .patch("https://api.example.com/users/1")
    ///     .json(&updates)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn patch(&self, url: &str) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }
}
