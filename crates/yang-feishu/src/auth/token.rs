//! `tenant_access_token` 的换取、缓存与短锁去重。
//!
//! # 官方语义
//!
//! - `POST /open-apis/auth/v3/tenant_access_token/internal`，请求体只有
//!   `app_id` / `app_secret`，**不带 Authorization 头**；
//! - 响应是**扁平**的 `{code, msg, tenant_access_token, expire}`，不套 `data`；
//! - 最大有效期 2 小时。剩余有效期 ≥ 30 分钟时调用返回原有 token，因此缓存 TTL
//!   取 `expire - 300` 是安全的。
//!
//! # 缓存键
//!
//! `{namespace}:app:{app_id}:tenant_token`。`namespace` 由宿主传入（通常是部署名），
//! `app_id` 保证多应用不会互相拿到对方的 token。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;

use crate::error::{Error, FailureKind, Result};
use crate::transport::{
    send_with_retry, Method, Request, Sleeper, Transport, TOKEN_REQUEST_TIMEOUT_SECS, TOKEN_RETRY,
};

use super::Credentials;

/// 刷新锁 TTL（秒）。
///
/// 不变式：`TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS) = 19 < 30`。
pub const LOCK_TTL_SECONDS: i64 = 30;

/// 缓存 TTL 相对 `expire` 留出的余量（秒）。
const CACHE_TTL_MARGIN_SECONDS: i64 = 300;

/// 缓存 TTL 的宽松上界（秒）。
///
/// 官方 `expire` 固定为 7200（模块文档：最大有效期 2 小时），正常响应算出的
/// `expire - 300 = 6900` 远低于本上界。只有非契约/异常的服务端响应（`expire`
/// 超过 `MAX_CACHE_TTL_SECONDS + CACHE_TTL_MARGIN_SECONDS`）才会被限幅：既避免
/// 把近乎永久的 TTL 写进缓存，也保证 `Instant` 过期点计算不会溢出。
const MAX_CACHE_TTL_SECONDS: i64 = 86_400;

/// 未持锁时的等待轮次与间隔。
const LOCK_WAIT_ATTEMPTS: u8 = 3;
const LOCK_WAIT_INTERVAL: Duration = Duration::from_millis(200);

/// 换取 token 的响应体（扁平信封，刻意不套 `data`）。
#[derive(Deserialize)]
struct TenantTokenResponse {
    code: i32,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    tenant_access_token: Option<String>,
    #[serde(default)]
    expire: Option<i64>,
}

impl std::fmt::Debug for TenantTokenResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TenantTokenResponse")
            .field("code", &self.code)
            .field("msg", &self.msg)
            .field("tenant_access_token", &"<已隐藏>")
            .field("expire", &self.expire)
            .finish()
    }
}

/// 一次成功的换取结果。
#[derive(Clone, PartialEq, Eq)]
pub struct TenantTokenGrant {
    /// 换到的 token。
    pub tenant_access_token: String,
    /// 官方给出的有效期（秒）。
    pub expire: i64,
}

impl std::fmt::Debug for TenantTokenGrant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TenantTokenGrant")
            .field("tenant_access_token", &"<已隐藏>")
            .field("expire", &self.expire)
            .finish()
    }
}

/// 把响应体折成 [`TenantTokenGrant`]。
///
/// 「业务码非 0」与「业务码为 0 但缺字段」分开报：前者是飞书拒绝，后者是我们对
/// 契约的理解错了，排查方向完全不同。
pub fn parse_token_grant(body: &str) -> Result<TenantTokenGrant> {
    let response: TenantTokenResponse = serde_json::from_str(body)
        .map_err(|error| Error::fatal(format!("解析 tenant_access_token 响应失败: {error}")))?;
    if response.code != 0 {
        return Err(Error::new(
            FailureKind::Fatal {
                code: response.code,
            },
            format!(
                "飞书拒绝签发 tenant_access_token: code={}, msg={}",
                response.code, response.msg
            ),
        ));
    }
    let tenant_access_token = response
        .tenant_access_token
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| Error::fatal("飞书返回 code=0 但缺少 tenant_access_token"))?;
    let expire = response
        .expire
        .ok_or_else(|| Error::fatal("飞书返回 code=0 但缺少 expire"))?;
    if expire <= 0 {
        return Err(Error::fatal(format!(
            "飞书返回的 expire 必须为正数，收到 {expire}"
        )));
    }
    Ok(TenantTokenGrant {
        tenant_access_token,
        expire,
    })
}

/// 缓存 TTL = `expire - 300`（秒），并以 `MAX_CACHE_TTL_SECONDS` 限幅。
///
/// 下界必须自己守：负的 TTL 会让缓存写入变成运行期错误，症状是「锁拿到了、token
/// 也换到了，却永远写不进缓存」。上界同样自守，但只作用于远超官方 2 小时有效期的
/// 非契约响应；官方 7200 与合规私有化部署的正常取值不受影响。
pub fn cache_ttl_seconds(expire: i64) -> Result<i64> {
    if expire <= CACHE_TTL_MARGIN_SECONDS {
        return Err(Error::fatal(format!(
            "tenant_access_token 的 expire 必须大于 {CACHE_TTL_MARGIN_SECONDS} 秒，收到 {expire}"
        )));
    }
    Ok((expire - CACHE_TTL_MARGIN_SECONDS).min(MAX_CACHE_TTL_SECONDS))
}

/// token 缓存与锁。
///
/// 键作用域由实现的构造参数决定，crate 不知道 deployment/app_id。
#[async_trait]
pub trait TokenCache: Send + Sync {
    /// 读取缓存值。
    async fn get(&self, key: &str) -> Result<Option<String>>;
    /// 写入缓存值（带 TTL）。
    async fn put(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()>;
    /// 删除缓存值。
    async fn delete(&self, key: &str) -> Result<()>;
    /// 抢锁；`owner` 用于放锁时做比较。
    async fn acquire_lock(&self, key: &str, owner: &str, ttl_seconds: i64) -> Result<bool>;
    /// 释放锁（仅当 owner 匹配）。
    async fn release_lock(&self, key: &str, owner: &str) -> Result<()>;
}

/// 进程内缓存（独立使用者与测试用；多实例部署请用 Redis 实现）。
#[derive(Default)]
pub struct MemoryTokenCache {
    values: Mutex<HashMap<String, (String, Instant)>>,
    locks: Mutex<HashMap<String, (String, Instant)>>,
}

impl std::fmt::Debug for MemoryTokenCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // token 与锁 owner 都可能是凭证/秘密，Debug 不输出任何缓存内容。
        formatter
            .debug_struct("MemoryTokenCache")
            .field("values", &"<已隐藏>")
            .field("locks", &"<已隐藏>")
            .finish()
    }
}

fn poisoned<T>(error: std::sync::PoisonError<T>) -> T {
    error.into_inner()
}

/// 计算 `Instant` 过期点；溢出返回 [`Error::fatal`]，绝不 panic。
///
/// `Instant + Duration` 在 Unix 上以 `i64` 秒表示，极端大的 TTL（例如异常响应
/// 直接透传的 `i64::MAX`）会溢出 panic。调用方已先把 TTL 压到正常量级，这里是
/// 公开 trait 被直接调用时的最后一道防线。
fn expiry_after(now: Instant, ttl_seconds: i64) -> Result<Instant> {
    now.checked_add(Duration::from_secs(ttl_seconds as u64))
        .ok_or_else(|| {
            Error::fatal(format!(
                "tenant_access_token 的 TTL 过大，无法计算过期时间: {ttl_seconds} 秒"
            ))
        })
}

#[async_trait]
impl TokenCache for MemoryTokenCache {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let now = Instant::now();
        let mut values = self.values.lock().unwrap_or_else(poisoned);
        match values.get(key) {
            Some((value, expires_at)) if *expires_at > now => Ok(Some(value.clone())),
            Some(_) => {
                values.remove(key);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()> {
        if ttl_seconds <= 0 {
            return Err(Error::fatal("tenant_access_token 缓存 TTL 必须为正数"));
        }
        let expires_at = expiry_after(Instant::now(), ttl_seconds)?;
        self.values
            .lock()
            .unwrap_or_else(poisoned)
            .insert(key.to_string(), (value.to_string(), expires_at));
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.values.lock().unwrap_or_else(poisoned).remove(key);
        Ok(())
    }

    async fn acquire_lock(&self, key: &str, owner: &str, ttl_seconds: i64) -> Result<bool> {
        if ttl_seconds <= 0 {
            return Err(Error::fatal("tenant_access_token 锁 TTL 必须为正数"));
        }
        let now = Instant::now();
        let expires_at = expiry_after(now, ttl_seconds)?;
        let mut locks = self.locks.lock().unwrap_or_else(poisoned);
        match locks.get(key) {
            Some((_, held_until)) if *held_until > now => Ok(false),
            _ => {
                locks.insert(key.to_string(), (owner.to_string(), expires_at));
                Ok(true)
            }
        }
    }

    async fn release_lock(&self, key: &str, owner: &str) -> Result<()> {
        let mut locks = self.locks.lock().unwrap_or_else(poisoned);
        if locks
            .get(key)
            .is_some_and(|(current, _)| current.as_str() == owner)
        {
            locks.remove(key);
        }
        Ok(())
    }
}

/// 刷新锁的取消守卫。
///
/// 正常路径在 [`TokenProvider::tenant_access_token`] 里显式释放并 `disarm`；只有
/// future 在持锁期间被丢弃（`tokio::time::timeout`、`select!`、worker 优雅关闭）时，
/// `Drop` 才补一次 best-effort 异步释放。两套缓存实现的 `release_lock` 都带 owner
/// 比较，因此迟到或重复释放不会误删后继持有者的锁。没有 tokio 运行时（无法 spawn）
/// 时静默回退锁的 30s TTL，行为与修复前一致。
struct LockGuard {
    cache: Arc<dyn TokenCache>,
    lock_key: String,
    owner: String,
    armed: bool,
}

impl LockGuard {
    fn new(cache: Arc<dyn TokenCache>, lock_key: String, owner: String) -> Self {
        Self {
            cache,
            lock_key,
            owner,
            armed: true,
        }
    }

    /// 正常路径已经显式释放，撤销 `Drop` 里的补偿释放。
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::debug!("没有 tokio 运行时，token 刷新锁交由 TTL 兜底释放");
            return;
        };
        let cache = Arc::clone(&self.cache);
        let lock_key = std::mem::take(&mut self.lock_key);
        let owner = std::mem::take(&mut self.owner);
        drop(handle.spawn(async move {
            if let Err(error) = cache.release_lock(&lock_key, &owner).await {
                tracing::warn!(error = %error, "取消守卫释放 token 刷新锁失败（锁有 TTL 兜底）");
            }
        }));
    }
}

/// 换取并缓存 `tenant_access_token`。
///
/// 由 [`crate::OpenApiClient`] 构造；crate 不提供公开构造函数，避免调用方绕过
/// 客户端的 transport/sleeper 装配。
pub struct TokenProvider {
    cache: Arc<dyn TokenCache>,
    transport: Arc<dyn Transport>,
    sleeper: Arc<dyn Sleeper>,
    credentials: Credentials,
    namespace: String,
    open_base: String,
}

impl TokenProvider {
    pub(crate) fn new(
        cache: Arc<dyn TokenCache>,
        transport: Arc<dyn Transport>,
        sleeper: Arc<dyn Sleeper>,
        credentials: Credentials,
        namespace: impl Into<String>,
        open_base: impl Into<String>,
    ) -> Self {
        Self {
            cache,
            transport,
            sleeper,
            credentials,
            namespace: namespace.into(),
            open_base: open_base.into(),
        }
    }

    /// 缓存键：`{namespace}:app:{app_id}:tenant_token`。
    pub fn token_key(&self) -> String {
        format!(
            "{}:app:{}:tenant_token",
            self.namespace, self.credentials.app_id
        )
    }

    fn lock_key(&self) -> String {
        format!("{}:lock", self.token_key())
    }

    /// 取一个可用的 token。快路径是纯读缓存。
    pub async fn tenant_access_token(&self) -> Result<String> {
        if let Some(token) = self.read_cached().await? {
            return Ok(token);
        }

        let owner = uuid::Uuid::new_v4().to_string();
        let lock_key = self.lock_key();
        let locked = match self
            .cache
            .acquire_lock(&lock_key, &owner, LOCK_TTL_SECONDS)
            .await
        {
            Ok(locked) => locked,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    namespace = %self.namespace,
                    "抢 token 刷新锁失败，降级为直连换取一次"
                );
                metrics_lock_error();
                return self.issue_and_cache(false).await;
            }
        };

        if !locked {
            for _ in 0..LOCK_WAIT_ATTEMPTS {
                self.sleeper.sleep(LOCK_WAIT_INTERVAL).await;
                if let Some(token) = self.read_cached().await? {
                    metrics_token("waited");
                    return Ok(token);
                }
            }
            tracing::warn!(
                namespace = %self.namespace,
                "等待 token 刷新超时，降级为自行换取一次（不写缓存）"
            );
            metrics_token("lock_wait_timeout");
            return self.issue_and_cache(false).await;
        }

        let lock_guard = LockGuard::new(Arc::clone(&self.cache), lock_key, owner.clone());
        let outcome = match self.read_cached().await? {
            Some(token) => Ok(token),
            None => self.issue_and_cache(true).await,
        };
        self.release_lock(&owner).await;
        lock_guard.disarm();
        outcome
    }

    /// 遇到 token 失效业务码时的补救：清缓存 + 强制刷新**一次**。
    pub async fn invalidate_and_refresh(&self) -> Result<String> {
        if let Err(error) = self.cache.delete(&self.token_key()).await {
            tracing::warn!(error = %error, "清除 token 缓存失败，继续强制刷新");
        }
        metrics_token("invalidated");
        self.issue_and_cache(true).await
    }

    async fn read_cached(&self) -> Result<Option<String>> {
        let key = self.token_key();
        match self.cache.get(&key).await {
            // 空串按未命中处理：否则出站会带 `Authorization: Bearer ` 出去。
            Ok(Some(token)) => Ok(Some(token).filter(|token| !token.trim().is_empty())),
            Ok(None) => Ok(None),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    namespace = %self.namespace,
                    "读取 token 缓存失败，降级为直连换取"
                );
                metrics_redis("get_failed");
                Ok(None)
            }
        }
    }

    fn token_request(&self) -> Request {
        Request {
            method: Method::Post,
            url: format!(
                "{}/open-apis/auth/v3/tenant_access_token/internal",
                self.open_base
            ),
            query: Vec::new(),
            bearer_token: None,
            json_body: Some(serde_json::json!({
                "app_id": self.credentials.app_id,
                "app_secret": self.credentials.app_secret.expose_secret(),
            })),
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(TOKEN_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        }
    }

    async fn issue_and_cache(&self, cache: bool) -> Result<String> {
        let response = send_with_retry(
            self.transport.as_ref(),
            self.sleeper.as_ref(),
            &self.token_request(),
            TOKEN_RETRY,
        )
        .await
        .map_err(|error| {
            Error::new(
                error.kind,
                format!("请求 tenant_access_token 失败: {error}"),
            )
        })?;

        let grant = parse_token_grant(&response.body)?;
        if cache {
            let ttl = cache_ttl_seconds(grant.expire)?;
            self.cache
                .put(&self.token_key(), &grant.tenant_access_token, ttl)
                .await
                .map_err(|error| {
                    Error::new(
                        error.kind,
                        format!("写入 tenant_access_token 缓存失败: {error}"),
                    )
                })?;
            metrics_token("refreshed");
        }
        Ok(grant.tenant_access_token)
    }

    async fn release_lock(&self, owner: &str) {
        if let Err(error) = self.cache.release_lock(&self.lock_key(), owner).await {
            tracing::warn!(error = %error, "释放 token 刷新锁失败（锁有 TTL 兜底）");
        }
    }
}

/// 构造一个不会真正出站、也只用于测试的 token 请求体？——不提供。
/// 测试应实现 [`TokenCache`] 并注入脚本化 [`Transport`]。
fn metrics_token(result: &'static str) {
    #[cfg(feature = "metrics")]
    metrics::counter!("feishu_outbound_token_total", "result" => result).increment(1);
    #[cfg(not(feature = "metrics"))]
    let _ = result;
}

fn metrics_lock_error() {
    #[cfg(feature = "metrics")]
    metrics::counter!("feishu_outbound_token_total", "result" => "lock_error").increment(1);
}

fn metrics_redis(reason: &'static str) {
    #[cfg(feature = "metrics")]
    metrics::counter!("feishu_outbound_redis_total", "reason" => reason).increment(1);
    #[cfg(not(feature = "metrics"))]
    let _ = reason;
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

    fn grant_body(token: &str, expire: i64) -> String {
        format!(r#"{{"code":0,"msg":"ok","tenant_access_token":"{token}","expire":{expire}}}"#)
    }

    #[test]
    fn token_types_debug_hide_secrets() {
        let grant = TenantTokenGrant {
            tenant_access_token: "secret-token-value".into(),
            expire: 7200,
        };
        assert!(!format!("{grant:?}").contains("secret-token-value"));
        assert!(format!("{grant:?}").contains("<已隐藏>"));
        assert!(format!("{grant:?}").contains("7200"));

        let response: TenantTokenResponse =
            serde_json::from_str(&grant_body("secret-token-value", 7200)).expect("响应体应可解析");
        assert!(!format!("{response:?}").contains("secret-token-value"));

        let cache = MemoryTokenCache::default();
        let debug = format!("{cache:?}");
        assert!(!debug.contains("secret-token-value"));
        assert!(debug.contains("<已隐藏>"));
    }

    #[test]
    fn parse_token_grant_accepts_flat_envelope() {
        let grant =
            parse_token_grant(&grant_body("t-1", 7200)).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(grant.tenant_access_token, "t-1");
        assert_eq!(grant.expire, 7200);
    }

    #[test]
    fn parse_token_grant_separates_business_failure_from_missing_fields() {
        assert!(parse_token_grant(r#"{"code":10003,"msg":"bad"}"#).is_err());
        let error = parse_token_grant(r#"{"code":0,"msg":"ok","expire":7200}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("缺少 tenant_access_token"));
        let error = parse_token_grant(r#"{"code":0,"tenant_access_token":"t"}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("缺少 expire"));
    }

    #[test]
    fn cache_ttl_keeps_the_official_thirty_minute_margin() {
        assert_eq!(cache_ttl_seconds(7200).unwrap_or(-1), 6900);
        assert!(cache_ttl_seconds(300).is_err());
        assert!(cache_ttl_seconds(299).is_err());
        assert_eq!(cache_ttl_seconds(301).unwrap_or(-1), 1);
    }

    #[test]
    fn token_key_includes_app_and_namespace() {
        let provider = TokenProvider::new(
            Arc::new(MemoryTokenCache::default()),
            Arc::new(EmptyTransport),
            Arc::new(NoSleep),
            test_credentials("cli_a", "secret"),
            "demo",
            "https://open.feishu.cn",
        );
        assert_eq!(provider.token_key(), "demo:app:cli_a:tenant_token");
        assert_eq!(provider.lock_key(), "demo:app:cli_a:tenant_token:lock");
    }

    #[test]
    fn credentials_debug_never_leaks_the_secret() {
        let credentials = test_credentials("cli_a", "super-secret");
        let debug = format!("{credentials:?}");
        assert!(debug.contains("cli_a"));
        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("已隐藏"));
    }

    struct EmptyTransport;

    #[async_trait]
    impl Transport for EmptyTransport {
        async fn send(&self, _request: Request) -> Result<Response> {
            Err(Error::fatal("unexpected send"))
        }
    }

    struct NoSleep;

    #[async_trait]
    impl Sleeper for NoSleep {
        async fn sleep(&self, _duration: Duration) {}
    }

    struct ScriptedTransport {
        bodies: Mutex<VecDeque<String>>,
        requests: Mutex<Vec<Request>>,
    }

    #[async_trait]
    impl Transport for ScriptedTransport {
        async fn send(&self, request: Request) -> Result<Response> {
            self.requests.lock().unwrap_or_else(poisoned).push(request);
            let body = self
                .bodies
                .lock()
                .unwrap_or_else(poisoned)
                .pop_front()
                .unwrap_or_else(|| r#"{"code":0}"#.to_string());
            Ok(Response {
                status: 200,
                body,
                bytes: None,
                headers: BTreeMap::new(),
            })
        }
    }

    fn provider(
        bodies: Vec<String>,
        cache: Arc<MemoryTokenCache>,
    ) -> (TokenProvider, Arc<ScriptedTransport>) {
        let transport = Arc::new(ScriptedTransport {
            bodies: Mutex::new(bodies.into()),
            requests: Mutex::new(Vec::new()),
        });
        let provider = TokenProvider::new(
            cache,
            transport.clone(),
            Arc::new(NoSleep),
            test_credentials("cli_a", "secret"),
            "demo",
            "https://open.feishu.cn",
        );
        (provider, transport)
    }

    #[tokio::test]
    async fn cache_hit_never_touches_the_network() {
        let cache = Arc::new(MemoryTokenCache::default());
        cache
            .put("demo:app:cli_a:tenant_token", "cached", 600)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        let (provider, transport) = provider(Vec::new(), cache);
        assert_eq!(
            provider.tenant_access_token().await.unwrap_or_default(),
            "cached"
        );
        assert!(transport
            .requests
            .lock()
            .unwrap_or_else(poisoned)
            .is_empty());
    }

    #[tokio::test]
    async fn fresh_fetch_caches_with_expire_minus_300() {
        let cache = Arc::new(MemoryTokenCache::default());
        let (provider, transport) = provider(vec![grant_body("t-1", 7200)], cache.clone());
        assert_eq!(
            provider.tenant_access_token().await.unwrap_or_default(),
            "t-1"
        );
        assert_eq!(
            cache
                .get("demo:app:cli_a:tenant_token")
                .await
                .unwrap_or_default()
                .as_deref(),
            Some("t-1")
        );
        assert_eq!(
            transport.requests.lock().unwrap_or_else(poisoned)[0].method,
            Method::Post
        );
        assert!(transport.requests.lock().unwrap_or_else(poisoned)[0]
            .bearer_token
            .is_none());
    }

    #[tokio::test]
    async fn invalidate_and_refresh_clears_the_cache_and_forces_a_fetch() {
        let cache = Arc::new(MemoryTokenCache::default());
        cache
            .put("demo:app:cli_a:tenant_token", "old", 600)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        let (provider, _) = provider(vec![grant_body("new", 7200)], cache.clone());
        assert_eq!(
            provider.invalidate_and_refresh().await.unwrap_or_default(),
            "new"
        );
        assert_eq!(
            cache
                .get("demo:app:cli_a:tenant_token")
                .await
                .unwrap_or_default()
                .as_deref(),
            Some("new")
        );
    }

    #[test]
    fn cache_ttl_clamps_only_abnormal_expire_values() {
        // 官方 7200 与常见合法范围完全不受上界影响。
        assert_eq!(cache_ttl_seconds(7200).unwrap_or(-1), 6900);
        assert_eq!(cache_ttl_seconds(301).unwrap_or(-1), 1);
        assert_eq!(cache_ttl_seconds(86_400).unwrap_or(-1), 86_100);
        // 非契约/异常服务端响应（expire 远超 2 小时）被限幅到宽松上界，
        // 而不是把近乎永久的 TTL 写进缓存或让 Instant 计算溢出。
        assert_eq!(cache_ttl_seconds(100_000).unwrap_or(-1), 86_400);
        assert_eq!(cache_ttl_seconds(i64::MAX).unwrap_or(-1), 86_400);
    }

    #[tokio::test]
    async fn memory_cache_never_panics_on_overflowing_ttl() {
        let cache = MemoryTokenCache::default();
        // RED（Unix）：修复前 `Instant::now() + Duration::from_secs(i64::MAX as u64)`
        // 会在 Timespec 的 i64 秒上溢出 panic；修复后 checked_add 返回 Error::fatal。
        let put_result = cache.put("k", "v", i64::MAX).await;
        let lock_result = cache.acquire_lock("lock", "owner", i64::MAX).await;
        #[cfg(unix)]
        {
            assert!(put_result.is_err(), "put 的 checked_add 溢出必须返回 Error");
            assert!(
                lock_result.is_err(),
                "acquire_lock 的 checked_add 溢出必须返回 Error"
            );
        }
        #[cfg(not(unix))]
        {
            // Windows 的 Instant 以 u64 秒表示，同样输入不溢出；只验证不 panic。
            let _ = (put_result, lock_result);
        }
    }

    /// 永不返回的传输：模拟换取 token 期间 future 被挂起。
    struct PendingTransport;

    #[async_trait]
    impl Transport for PendingTransport {
        async fn send(&self, _request: Request) -> Result<Response> {
            std::future::pending::<Result<Response>>().await
        }
    }

    #[tokio::test]
    async fn cancelled_refresh_releases_the_lock_via_guard() {
        let cache = Arc::new(MemoryTokenCache::default());
        let provider = TokenProvider::new(
            cache.clone(),
            Arc::new(PendingTransport),
            Arc::new(NoSleep),
            test_credentials("cli_a", "secret"),
            "demo",
            "https://open.feishu.cn",
        );
        let lock_key = provider.lock_key();

        // RED：修复前 future 在 issue_and_cache 期间被取消，锁会一直占到 30s TTL，
        // 其他并发调用方只能反复等锁超时后降级直连换取。
        let outcome =
            tokio::time::timeout(Duration::from_millis(50), provider.tenant_access_token()).await;
        assert!(outcome.is_err(), "挂起的换取应被超时取消");

        let mut released = false;
        for _ in 0..50 {
            if cache
                .acquire_lock(&lock_key, "probe", LOCK_TTL_SECONDS)
                .await
                .unwrap_or(false)
            {
                released = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(released, "取消后守卫应异步释放刷新锁，而不是等 30s TTL");
    }
}
