//! HTTP 客户端核心实现
//!
//! 提供全局 HTTP 客户端和请求构建能力。

use crate::error::BaseError;
use crate::http::request::RequestBuilder;
use reqwest::{Client, Method};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

/// 全局 HTTP 客户端实例
static GLOBAL_HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();

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
        ];

        for config in invalid_configs {
            let err = config
                .validate()
                .expect_err("HTTP 客户端零值配置应被拒绝");

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
///
/// // 初始化全局客户端
/// HttpClient::init_global(30)?;
///
/// // 使用全局客户端发起请求
/// let response = HttpClient::global()?
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

        // 构建 reqwest 客户端
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .pool_max_idle_per_host(cfg.pool_max_idle_per_host)
            .pool_idle_timeout(Duration::from_secs(cfg.pool_idle_timeout_secs))
            .danger_accept_invalid_certs(cfg.accept_invalid_certs);

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

    /// 使用完整配置初始化全局 HTTP 客户端
    ///
    /// 允许配置连接池大小、User-Agent、代理等高级选项。
    /// 重复调用返回 `BaseError::HttpClientAlreadyInitialized`。
    ///
    /// # 参数
    ///
    /// - `config`: HTTP 客户端配置
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError::HttpClientAlreadyInitialized)`: 已初始化（重复调用）
    /// - `Err(BaseError::HttpClientCreateFailed)`: 客户端创建失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::http::HttpClientConfig;
    ///
    /// let config = HttpClientConfig {
    ///     timeout_secs: 60,
    ///     pool_max_idle_per_host: 20,
    ///     user_agent: Some("MyApp/1.0".to_string()),
    ///     ..Default::default()
    /// };
    /// HttpClient::init_global_with_config(config)?;
    /// ```
    pub fn init_global_with_config(config: HttpClientConfig) -> Result<(), BaseError> {
        let timeout = config.timeout_secs;
        let client = Self::with_config(config)?;

        GLOBAL_HTTP_CLIENT
            .set(client)
            .map_err(|_| BaseError::HttpClientAlreadyInitialized)?;

        log::info!("全局 HTTP 客户端已初始化，超时时间: {} 秒", timeout);
        Ok(())
    }

    /// 初始化全局 HTTP 客户端（仅超时时间）
    ///
    /// 使用默认配置，仅覆盖超时时间。如需完整配置（连接池、UA、代理等），
    /// 请使用 [`init_global_with_config`](Self::init_global_with_config)。
    ///
    /// # 参数
    ///
    /// - `timeout_secs`: 默认超时时间（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError::HttpClientAlreadyInitialized)`: 已初始化（重复调用）
    /// - `Err(BaseError::HttpClientCreateFailed)`: 客户端创建失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// HttpClient::init_global(30)?;
    /// ```
    pub fn init_global(timeout_secs: u64) -> Result<(), BaseError> {
        Self::init_global_with_config(HttpClientConfig {
            timeout_secs,
            ..Default::default()
        })
    }

    /// 获取全局 HTTP 客户端
    ///
    /// # 返回
    ///
    /// - `Ok(&HttpClient)`: 客户端实例
    /// - `Err(BaseError::HttpClientNotInitialized)`: 客户端未初始化
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let client = HttpClient::global()?;
    /// ```
    pub fn global() -> Result<&'static HttpClient, BaseError> {
        // 未初始化返回结构化错误
        GLOBAL_HTTP_CLIENT
            .get()
            .ok_or(BaseError::HttpClientNotInitialized)
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
