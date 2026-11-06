//! 全量官方端点薄封装的运行时契约：method、URL、query、身份与二进制响应。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use yang_feishu::auth::MemoryTokenCache;
use yang_feishu::{
    Endpoints, Error, Method, OpenApiClient, OpenApiConfig, Request, Response, Result, Sleeper,
    Transport,
};

struct ScriptedTransport {
    script: Mutex<VecDeque<Response>>,
    requests: Mutex<Vec<Request>>,
}

impl ScriptedTransport {
    fn new(script: Vec<Response>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("请求记录锁未中毒").clone()
    }
}

#[async_trait::async_trait]
impl Transport for ScriptedTransport {
    async fn send(&self, request: Request) -> Result<Response> {
        self.requests
            .lock()
            .expect("请求记录锁未中毒")
            .push(request);
        self.script
            .lock()
            .expect("脚本锁未中毒")
            .pop_front()
            .ok_or_else(|| Error::fatal("ScriptedTransport 脚本已耗尽"))
    }
}

struct NoSleep;

#[async_trait::async_trait]
impl Sleeper for NoSleep {
    async fn sleep(&self, _duration: Duration) {}
}

fn response(body: &str) -> Response {
    Response {
        status: 200,
        body: body.to_string(),
        bytes: None,
        headers: BTreeMap::new(),
    }
}

fn binary_response(bytes: Vec<u8>) -> Response {
    Response {
        status: 200,
        body: String::new(),
        bytes: Some(bytes),
        headers: BTreeMap::new(),
    }
}

#[tokio::test]
async fn generated_api_surface_sends_expected_requests() {
    let transport = Arc::new(ScriptedTransport::new(vec![
        response(r#"{"code":0,"msg":"ok","tenant_access_token":"tenant-token","expire":7200}"#),
        response(r#"{"code":0,"data":{"document_id":"docx_1"}}"#),
        response(r#"{"code":0,"data":{"items":[]}}"#),
        response(r#"{"code":0,"data":{}}"#),
        response(r#"{"code":0,"data":{"children":[]}}"#),
        binary_response(vec![1, 2, 3]),
        response(r#"{"code":0,"data":{"file_key":"file_1"}}"#),
    ]));
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_api_surface", "secret")
            .endpoints(Endpoints::feishu())
            .cache_namespace("api-surface"),
    )
    .transport(transport.clone())
    .sleeper(Arc::new(NoSleep))
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()
    .unwrap_or_else(|error| panic!("客户端装配失败: {error}"));

    let api = client.api();
    api.docx_documents_create(&[], &json!({"title": "x"}))
        .await
        .expect("tenant JSON 调用失败");
    api.contact_departments_list_user("user-token", &[("page_size", "20")])
        .await
        .expect("user JSON 调用失败");
    api.auth_app_access_token_create_public(&[], &json!({"app_id": "a"}))
        .await
        .expect("public JSON 调用失败");
    api.docx_documents_blocks_children_create("docx_1", "block_1", &[], &json!({}))
        .await
        .expect("路径参数调用失败");
    let binary = api
        .im_files_get("file-key", &[])
        .await
        .expect("二进制调用失败");
    assert_eq!(binary.bytes.as_deref(), Some(&[1u8, 2, 3][..]));
    api.im_files_create(&[], "multipart/form-data; boundary=test", b"multipart-body")
        .await
        .expect("multipart 调用失败");

    let requests = transport.requests();
    assert_eq!(requests.len(), 7, "应当只有一次 token 换取 + 6 次业务调用");

    assert_eq!(
        requests[0].url,
        "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal"
    );
    assert_eq!(requests[0].method, Method::Post);

    assert_eq!(
        requests[1].url,
        "https://open.feishu.cn/open-apis/docx/v1/documents"
    );
    assert_eq!(requests[1].method, Method::Post);
    assert_eq!(requests[1].bearer_token.as_deref(), Some("tenant-token"));
    assert_eq!(requests[1].json_body, Some(json!({"title": "x"})));

    assert_eq!(
        requests[2].url,
        "https://open.feishu.cn/open-apis/contact/v3/departments"
    );
    assert_eq!(requests[2].method, Method::Get);
    assert_eq!(requests[2].bearer_token.as_deref(), Some("user-token"));
    assert_eq!(
        requests[2].query,
        vec![("page_size".to_string(), "20".to_string())]
    );

    assert_eq!(
        requests[3].url,
        "https://open.feishu.cn/open-apis/auth/v3/app_access_token"
    );
    assert_eq!(requests[3].method, Method::Post);
    assert_eq!(requests[3].bearer_token, None);

    assert_eq!(
        requests[4].url,
        "https://open.feishu.cn/open-apis/docx/v1/documents/docx_1/blocks/block_1/children"
    );
    assert_eq!(requests[4].method, Method::Post);
    assert_eq!(requests[4].bearer_token.as_deref(), Some("tenant-token"));

    assert_eq!(
        requests[5].url,
        "https://open.feishu.cn/open-apis/im/v1/files/file-key"
    );
    assert_eq!(requests[5].method, Method::Get);
    assert!(requests[5].binary_limit.is_some());

    assert_eq!(
        requests[6].url,
        "https://open.feishu.cn/open-apis/im/v1/files"
    );
    assert_eq!(requests[6].method, Method::Post);
    let (content_type, raw_body) = requests[6]
        .raw_body
        .as_ref()
        .expect("multipart 请求必须携带 raw_body");
    assert_eq!(content_type, "multipart/form-data; boundary=test");
    assert_eq!(raw_body.as_slice(), b"multipart-body");
}

#[tokio::test]
async fn generated_path_segments_reject_injection() {
    let transport = Arc::new(ScriptedTransport::new(Vec::new()));
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_api_surface", "secret")
            .endpoints(Endpoints::feishu())
            .cache_namespace("api-surface"),
    )
    .transport(transport)
    .sleeper(Arc::new(NoSleep))
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()
    .expect("客户端装配失败");

    let error = client
        .api()
        .docx_documents_blocks_children_create("doc/../x", "block", &[], &json!({}))
        .await
        .expect_err("路径参数必须拒绝斜杠");
    assert!(error.to_string().contains("document_id"));
}
