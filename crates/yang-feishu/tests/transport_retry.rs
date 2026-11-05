//! 出站传输与令牌契约的集成测试。
//!
//! 全部使用脚本化 [`Transport`] 与记录型 [`Sleeper`] 替身：零真实网络、零 Redis/MySQL，
//! 只通过 `yang_feishu::...` 的公开 API 钉住行为契约。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use yang_feishu::auth::{
    cache_ttl_seconds, parse_token_grant, MemoryTokenCache, TenantTokenGrant, TokenCache,
    LOCK_TTL_SECONDS,
};
use yang_feishu::error::FailureKind;
use yang_feishu::transport::{
    classify, dedup_key, disposition, summarize, Disposition, Method, Request, Response, Sleeper,
    Transport, DEPARTMENT_RETRY, PULL_RETRY, TOKEN_REQUEST_TIMEOUT_SECS, TOKEN_RETRY,
};
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig, Result};

const TOKEN_URL: &str = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";
const BUSINESS_URL: &str = "https://open.feishu.cn/open-apis/test/v1/items";

fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

fn response(status: u16, body: &str, header_pairs: &[(&str, &str)]) -> Response {
    Response {
        status,
        body: body.to_string(),
        bytes: None,
        headers: headers(header_pairs),
    }
}

fn grant_body(token: &str, expire: i64) -> String {
    format!(r#"{{"code":0,"msg":"ok","tenant_access_token":"{token}","expire":{expire}}}"#)
}

fn business_request(idempotent: bool) -> Request {
    Request {
        method: Method::Get,
        url: BUSINESS_URL.to_string(),
        query: Vec::new(),
        bearer_token: None,
        json_body: None,
        raw_body: None,
        binary_limit: None,
        timeout_secs: None,
        idempotent,
    }
}

/// 按脚本逐次返回响应的传输替身，同时记录收到的每个请求。
#[derive(Default)]
struct FakeTransport {
    script: Mutex<VecDeque<Response>>,
    requests: Mutex<Vec<Request>>,
}

impl FakeTransport {
    fn new(script: Vec<Response>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("请求记录锁未中毒").clone()
    }

    fn request_count(&self) -> usize {
        self.requests.lock().expect("请求记录锁未中毒").len()
    }

    fn token_exchange_count(&self) -> usize {
        self.requests()
            .iter()
            .filter(|request| request.url == TOKEN_URL)
            .count()
    }
}

#[async_trait::async_trait]
impl Transport for FakeTransport {
    async fn send(&self, request: Request) -> Result<Response> {
        self.requests
            .lock()
            .expect("请求记录锁未中毒")
            .push(request);
        self.script
            .lock()
            .expect("脚本锁未中毒")
            .pop_front()
            .ok_or_else(|| yang_feishu::Error::fatal("FakeTransport 脚本已耗尽"))
    }
}

/// 记录每次睡眠时长的睡眠替身。
#[derive(Default)]
struct RecordingSleeper {
    sleeps: Mutex<Vec<Duration>>,
}

impl RecordingSleeper {
    fn durations(&self) -> Vec<Duration> {
        self.sleeps.lock().expect("睡眠记录锁未中毒").clone()
    }
}

#[async_trait::async_trait]
impl Sleeper for RecordingSleeper {
    async fn sleep(&self, duration: Duration) {
        self.sleeps.lock().expect("睡眠记录锁未中毒").push(duration);
    }
}

fn build_client(
    transport: Arc<FakeTransport>,
    sleeper: Arc<RecordingSleeper>,
) -> (OpenApiClient, Arc<MemoryTokenCache>) {
    let cache = Arc::new(MemoryTokenCache::default());
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_test", "secret-9")
            .endpoints(Endpoints::feishu())
            .cache_namespace("test-ns"),
    )
    .transport(transport)
    .sleeper(sleeper)
    .token_cache(cache.clone())
    .build()
    .expect("客户端应能装配");
    (client, cache)
}

#[tokio::test]
async fn 首次429按重置头退避一千毫秒后重试成功() {
    let transport = Arc::new(FakeTransport::new(vec![
        response(429, "", &[("x-ogw-ratelimit-reset", "1")]),
        response(200, r#"{"code":0,"msg":"ok"}"#, &[]),
    ]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let (client, _) = build_client(transport.clone(), sleeper.clone());

    let result = client
        .send_raw(business_request(true), &PULL_RETRY)
        .await
        .expect("第二次尝试成功应返回响应");

    assert_eq!(result.status, 200);
    assert_eq!(result.body, r#"{"code":0,"msg":"ok"}"#);
    assert_eq!(
        transport.request_count(),
        2,
        "429 之后应重发一次，共 2 次请求"
    );
    assert_eq!(
        sleeper.durations(),
        vec![Duration::from_millis(1_000)],
        "应采信 x-ogw-ratelimit-reset 的秒数"
    );
}

#[tokio::test]
async fn 非幂等请求遇可重试失败只发一次() {
    let transport = Arc::new(FakeTransport::new(vec![response(
        429,
        r#"{"msg":"rate limited"}"#,
        &[("x-ogw-ratelimit-reset", "1")],
    )]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let (client, _) = build_client(transport.clone(), sleeper.clone());

    let error = client
        .send_raw(business_request(false), &PULL_RETRY)
        .await
        .expect_err("非幂等请求不得重试");

    assert_eq!(
        error.kind,
        FailureKind::Retry {
            retry_after_seconds: Some(1)
        }
    );
    assert_eq!(transport.request_count(), 1, "非幂等请求只能发一次");
    assert!(sleeper.durations().is_empty(), "不重试就不该 sleep");
}

#[tokio::test]
async fn 幂等请求在最大尝试次数内重试并返回最后一次失败() {
    let transport = Arc::new(FakeTransport::new(vec![
        response(200, r#"{"code":1254290,"msg":"attempt-1"}"#, &[]),
        response(200, r#"{"code":1254290,"msg":"attempt-2"}"#, &[]),
        response(200, r#"{"code":1254290,"msg":"attempt-3"}"#, &[]),
    ]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let (client, _) = build_client(transport.clone(), sleeper.clone());

    let error = client
        .send_raw(business_request(true), &PULL_RETRY)
        .await
        .expect_err("三次尝试全部失败应上抛");

    assert_eq!(
        error.kind,
        FailureKind::Retry {
            retry_after_seconds: None
        }
    );
    assert_eq!(
        transport.request_count(),
        3,
        "PULL_RETRY.max_attempts 为 3，共发 3 次"
    );
    assert!(
        error.message.contains("attempt-3"),
        "错误应来自最后一次响应，实际: {}",
        error.message
    );
    assert_eq!(
        sleeper.durations(),
        vec![Duration::from_millis(500), Duration::from_millis(1_000)],
        "退避应为 500ms 起、翻倍封顶"
    );
}

#[test]
fn 业务码优先于http状态() {
    let empty = headers(&[]);

    // HTTP 200 + 权限业务码：Fatal，且不因状态 200 被误判成功。
    assert_eq!(
        classify(200, &empty, r#"{"code":1254302,"msg":"permission denied"}"#),
        Some(FailureKind::Fatal { code: 1254302 })
    );
    // HTTP 400 + 99991663：按 TokenExpired 处理，而不是 4xx 终态。
    assert_eq!(
        classify(400, &empty, r#"{"code":99991663,"msg":"token expired"}"#),
        Some(FailureKind::TokenExpired)
    );
    // 非 JSON 的 5xx 只能按状态兜底为可重试。
    assert_eq!(
        classify(500, &empty, "<html>bad gateway</html>"),
        Some(FailureKind::Retry {
            retry_after_seconds: None
        })
    );
    // HTTP 200 + code=0 才是成功。
    assert_eq!(classify(200, &empty, r#"{"code":0,"msg":"ok"}"#), None);
}

#[tokio::test]
async fn token失效时清缓存刷新一次并用新token重试() {
    let transport = Arc::new(FakeTransport::new(vec![
        response(400, r#"{"code":99991663,"msg":"token expired"}"#, &[]),
        response(200, &grant_body("new-token", 7200), &[]),
        response(200, r#"{"code":0,"msg":"ok"}"#, &[]),
    ]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let (client, cache) = build_client(transport.clone(), sleeper.clone());

    let key = client.tokens().token_key();
    assert_eq!(key, "test-ns:app:cli_test:tenant_token");
    cache
        .put(&key, "old-token", 600)
        .await
        .expect("预置 token 缓存");

    let result = client
        .send_tenant(business_request(true), &PULL_RETRY)
        .await
        .expect("刷新 token 后重试应成功");
    assert_eq!(result.status, 200);

    let requests = transport.requests();
    assert_eq!(
        requests.len(),
        3,
        "序列应为：业务请求 → token 换取 → 业务重试"
    );
    assert_eq!(
        requests[0].bearer_token.as_deref(),
        Some("old-token"),
        "第一次业务请求应使用缓存里的旧 token"
    );

    let exchange = &requests[1];
    assert_eq!(exchange.url, TOKEN_URL, "第二次应为 token 换取请求");
    assert_eq!(exchange.method, Method::Post);
    assert!(
        exchange.bearer_token.is_none(),
        "换取 token 的请求不能带 Authorization"
    );
    let body = exchange
        .json_body
        .as_ref()
        .expect("换取 token 必须带 JSON body");
    assert_eq!(
        body.get("app_id").and_then(|value| value.as_str()),
        Some("cli_test")
    );
    assert_eq!(
        body.get("app_secret").and_then(|value| value.as_str()),
        Some("secret-9")
    );

    assert_eq!(
        requests[2].bearer_token.as_deref(),
        Some("new-token"),
        "重试的业务请求必须使用刷新后的 token"
    );
    assert_eq!(
        cache.get(&key).await.expect("读取缓存").as_deref(),
        Some("new-token"),
        "刷新后的 token 应写回缓存"
    );
    assert!(
        sleeper.durations().is_empty(),
        "token 失效走清缓存强刷，而不是退避重试"
    );
}

#[tokio::test]
async fn 同一请求第二次仍失效时不再无限刷新() {
    let transport = Arc::new(FakeTransport::new(vec![
        response(400, r#"{"code":99991663,"msg":"expired-1"}"#, &[]),
        response(200, &grant_body("new-token", 7200), &[]),
        response(400, r#"{"code":99991663,"msg":"expired-2"}"#, &[]),
    ]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let (client, cache) = build_client(transport.clone(), sleeper.clone());

    let key = client.tokens().token_key();
    cache
        .put(&key, "old-token", 600)
        .await
        .expect("预置 token 缓存");

    let error = client
        .send_tenant(business_request(true), &PULL_RETRY)
        .await
        .expect_err("第二次仍失效时应直接上抛 TokenExpired");

    assert_eq!(error.kind, FailureKind::TokenExpired);
    assert!(
        error.message.contains("expired-2"),
        "错误应来自刷新后的那次失败，实际: {}",
        error.message
    );
    assert_eq!(
        transport.request_count(),
        3,
        "业务请求 → 换取 token → 业务重试，之后不得再刷新"
    );
    assert_eq!(
        transport.token_exchange_count(),
        1,
        "单次 send_tenant 只允许强制刷新一次"
    );
}

#[test]
fn token授予与缓存ttl的公开契约() {
    let grant = parse_token_grant(&grant_body("t-1", 7200)).expect("扁平信封应可解析");
    assert_eq!(
        grant,
        TenantTokenGrant {
            tenant_access_token: "t-1".to_string(),
            expire: 7200
        }
    );

    let error = parse_token_grant("not json").expect_err("非 JSON 必须失败");
    assert!(
        error.message.contains("解析 tenant_access_token 响应失败"),
        "实际: {}",
        error.message
    );
    let error =
        parse_token_grant(r#"{"code":10003,"msg":"bad"}"#).expect_err("业务码非 0 必须失败");
    assert!(
        error.message.contains("code=10003"),
        "实际: {}",
        error.message
    );
    let error = parse_token_grant(r#"{"code":0,"msg":"ok","expire":7200}"#)
        .expect_err("缺 tenant_access_token 必须失败");
    assert!(
        error.message.contains("缺少 tenant_access_token"),
        "实际: {}",
        error.message
    );
    let error = parse_token_grant(r#"{"code":0,"tenant_access_token":"   ","expire":7200}"#)
        .expect_err("空白 token 视同缺失");
    assert!(error.message.contains("缺少 tenant_access_token"));
    let error = parse_token_grant(r#"{"code":0,"tenant_access_token":"t"}"#)
        .expect_err("缺 expire 必须失败");
    assert!(
        error.message.contains("缺少 expire"),
        "实际: {}",
        error.message
    );
    let error = parse_token_grant(r#"{"code":0,"tenant_access_token":"t","expire":0}"#)
        .expect_err("非正 expire 必须失败");
    assert!(
        error.message.contains("必须为正数"),
        "实际: {}",
        error.message
    );

    assert_eq!(cache_ttl_seconds(7200).expect("7200 合法"), 6900);
    assert_eq!(cache_ttl_seconds(301).expect("301 合法"), 1);
    assert!(cache_ttl_seconds(300).is_err(), "300 不留余量必须拒绝");
    assert!(cache_ttl_seconds(299).is_err());
    assert!(cache_ttl_seconds(0).is_err());

    assert_eq!(LOCK_TTL_SECONDS, 30);
    assert_eq!(
        TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS),
        19
    );
    assert!(
        TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS) < LOCK_TTL_SECONDS as u64,
        "刷新耗时上界必须小于锁 TTL"
    );
}

#[test]
fn 分类边界覆盖重置头与状态兜底() {
    let empty = headers(&[]);

    // 429 没有建议头：可重试但无建议秒数。
    assert_eq!(
        classify(429, &empty, ""),
        Some(FailureKind::Retry {
            retry_after_seconds: None
        })
    );
    // Retry-After 是第二顺位来源。
    assert_eq!(
        classify(429, &headers(&[("retry-after", "3")]), ""),
        Some(FailureKind::Retry {
            retry_after_seconds: Some(3)
        })
    );
    // 建议秒数会 trim 后解析；解析失败视为无建议。
    assert_eq!(
        classify(429, &headers(&[("x-ogw-ratelimit-reset", " 7 ")]), ""),
        Some(FailureKind::Retry {
            retry_after_seconds: Some(7)
        })
    );
    assert_eq!(
        classify(429, &headers(&[("x-ogw-ratelimit-reset", "soon")]), ""),
        Some(FailureKind::Retry {
            retry_after_seconds: None
        })
    );
    // 无业务码时状态码兜底：2xx 成功、5xx 可重试、其余 Fatal{0}。
    assert_eq!(classify(204, &empty, ""), None);
    assert_eq!(classify(200, &empty, "[]"), None);
    assert_eq!(
        classify(404, &empty, "not json"),
        Some(FailureKind::Fatal { code: 0 })
    );
    // token 格式错不可通过刷新修复。
    assert_eq!(
        classify(400, &empty, r#"{"code":99991661}"#),
        Some(FailureKind::Fatal { code: 99991661 })
    );
    // 审批语义码单独成档。
    assert_eq!(
        classify(200, &empty, r#"{"code":60012}"#),
        Some(FailureKind::UuidConflict)
    );
    assert_eq!(
        classify(200, &empty, r#"{"code":1390003}"#),
        Some(FailureKind::InstanceNotFound)
    );
}

#[test]
fn 处置函数不重试终态与非幂等() {
    let retry = FailureKind::Retry {
        retry_after_seconds: None,
    };

    assert_eq!(
        disposition(&retry, 1, true, &PULL_RETRY),
        Disposition::RetryAfter(500)
    );
    assert_eq!(
        disposition(&retry, 2, true, &PULL_RETRY),
        Disposition::RetryAfter(1_000)
    );
    assert_eq!(
        disposition(&retry, PULL_RETRY.max_attempts, true, &PULL_RETRY),
        Disposition::Give,
        "达到 max_attempts 后必须放弃"
    );
    assert_eq!(
        disposition(&retry, 1, false, &PULL_RETRY),
        Disposition::Give,
        "非幂等请求一律不重试"
    );
    assert_eq!(
        disposition(
            &FailureKind::Retry {
                retry_after_seconds: Some(1)
            },
            1,
            true,
            &PULL_RETRY
        ),
        Disposition::RetryAfter(1_000),
        "服务端建议秒数优先于指数退避"
    );
    assert_eq!(
        disposition(&FailureKind::TokenExpired, 1, true, &PULL_RETRY),
        Disposition::Give,
        "TokenExpired 由 send_tenant 强刷，不走退避"
    );
    assert_eq!(
        disposition(&FailureKind::Fatal { code: 1254302 }, 1, true, &PULL_RETRY),
        Disposition::Give
    );
    assert_eq!(
        disposition(&FailureKind::UuidConflict, 1, true, &PULL_RETRY),
        Disposition::Give
    );
    assert_eq!(
        disposition(&FailureKind::InstanceNotFound, 1, true, &PULL_RETRY),
        Disposition::Give
    );
    assert_eq!(
        disposition(&retry, 1, true, &DEPARTMENT_RETRY),
        Disposition::Give,
        "max_attempts=1 的策略第一次就放弃"
    );
}

#[test]
fn 摘要按字符截断且去重键抹掉易变log_id() {
    assert_eq!(summarize(500, "boom"), "HTTP 500: boom");

    let exactly = "中".repeat(500);
    assert_eq!(summarize(200, &exactly), format!("HTTP 200: {exactly}"));

    let over = "中".repeat(501);
    let summary = summarize(200, &over);
    assert!(summary.starts_with("HTTP 200: 中"));
    assert!(summary.ends_with("…（共 501 字符，已截断）"));
    assert_eq!(
        summary.chars().filter(|ch| *ch == '中').count(),
        500,
        "按字符截断，不能切碎 UTF-8"
    );

    assert_eq!(dedup_key("no volatile id"), "no volatile id");
    let left = r#"{"code":1,"log_id":"aaaa","msg":"x"}"#;
    let right = r#"{"code":1,"log_id":"bbbb","msg":"x"}"#;
    assert_eq!(dedup_key(left), dedup_key(right), "log_id 值不应影响去重键");
    assert!(!dedup_key(left).contains("log_id"));
    assert!(!dedup_key(left).contains("aaaa"));
    assert_eq!(dedup_key(r#"{"log_id" : "cccc"}"#), "{}");
    assert_eq!(
        dedup_key("tail \"log_id\""),
        "tail \"log_id\"",
        "畸形 log_id 片段保持原样"
    );
}
