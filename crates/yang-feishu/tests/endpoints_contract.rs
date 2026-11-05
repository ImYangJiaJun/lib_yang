//! 长尾官方端点薄封装的请求形状契约。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use yang_feishu::auth::{MemoryTokenCache, TokenCache};
use yang_feishu::transport::{Request, Response, Sleeper, Transport};
use yang_feishu::{Method, OpenApiClient, OpenApiConfig};

#[derive(Clone, Default)]
struct FakeTransport {
    requests: Arc<Mutex<Vec<Request>>>,
    responses: Arc<Mutex<VecDeque<String>>>,
}

#[async_trait::async_trait]
impl Transport for FakeTransport {
    async fn send(&self, request: Request) -> yang_feishu::Result<Response> {
        self.requests.lock().unwrap().push(request);
        Ok(Response {
            status: 200,
            body: self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| r#"{"code":0,"data":{}}"#.to_string()),
            bytes: None,
            headers: Default::default(),
        })
    }
}

struct NoSleep;

#[async_trait::async_trait]
impl Sleeper for NoSleep {
    async fn sleep(&self, _duration: std::time::Duration) {}
}

async fn client() -> (OpenApiClient, Arc<FakeTransport>) {
    let transport = Arc::new(FakeTransport::default());
    let cache = Arc::new(MemoryTokenCache::default());
    cache
        .put("demo:app:cli_test:tenant_token", "t-fake", 600)
        .await
        .unwrap();
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_test", "secret").cache_namespace("demo"),
    )
    .transport(transport.clone())
    .sleeper(Arc::new(NoSleep))
    .token_cache(cache)
    .build()
    .unwrap();
    (client, transport)
}

fn last_request(transport: &FakeTransport) -> Request {
    transport.requests.lock().unwrap().last().cloned().unwrap()
}

#[tokio::test]
async fn bitable_create_record_uses_post_and_official_path() {
    let (client, transport) = client().await;
    client
        .bitable()
        .create_record(
            "app",
            "tbl",
            &[("user_id_type", "open_id")],
            &serde_json::json!({"fields": {}}),
        )
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Post);
    assert_eq!(
        request.url,
        "https://open.feishu.cn/open-apis/bitable/v1/apps/app/tables/tbl/records"
    );
    assert_eq!(request.json_body.unwrap()["fields"], serde_json::json!({}));
}

#[tokio::test]
async fn bitable_update_view_uses_patch() {
    let (client, transport) = client().await;
    client
        .bitable()
        .update_view("app", "tbl", "vew", &serde_json::json!({"view_name": "x"}))
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Patch);
    assert!(request.url.ends_with("/tables/tbl/views/vew"));
}

#[tokio::test]
async fn approval_task_approve_uses_official_path() {
    let (client, transport) = client().await;
    client
        .approval()
        .approve_task(&[("user_id_type", "open_id")], &serde_json::json!({}))
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Post);
    assert_eq!(
        request.url,
        "https://open.feishu.cn/open-apis/approval/v4/tasks/approve"
    );
    assert_eq!(
        request.query,
        vec![("user_id_type".into(), "open_id".into())]
    );
}

#[tokio::test]
async fn contact_get_user_uses_get() {
    let (client, transport) = client().await;
    client
        .contact()
        .get_user("ou_1", &[("user_id_type", "open_id")])
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Get);
    assert!(request.url.ends_with("/open-apis/contact/v3/users/ou_1"));
}

#[tokio::test]
async fn im_recall_message_uses_delete() {
    let (client, transport) = client().await;
    client.im().recall_message("om_1").await.unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Delete);
    assert_eq!(
        request.url,
        "https://open.feishu.cn/open-apis/im/v1/messages/om_1"
    );
}

#[tokio::test]
async fn drive_list_files_uses_get_with_query() {
    let (client, transport) = client().await;
    client
        .drive()
        .list_files(&[("folder_token", "fld_1"), ("page_size", "100")])
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Get);
    assert_eq!(
        request.url,
        "https://open.feishu.cn/open-apis/drive/v1/files"
    );
    assert_eq!(request.query.len(), 2);
}

#[tokio::test]
async fn im_upload_file_builds_multipart_body() {
    let (client, transport) = client().await;
    transport
        .responses
        .lock()
        .unwrap()
        .push_back(r#"{"code":0,"data":{"file_key":"file_v3_1"}}"#.to_string());
    let key = client
        .im()
        .upload_file("a.txt", "stream", b"hello")
        .await
        .unwrap();
    assert_eq!(key, "file_v3_1");
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Post);
    let (content_type, body) = request.raw_body.unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary=feishu-"));
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"file_type\""));
    assert!(text.contains("name=\"file_name\""));
    assert!(text.contains("hello"));
}

#[tokio::test]
async fn base_v3_table_list_uses_cli_path() {
    let (client, transport) = client().await;
    client
        .bitable()
        .v3_table_list("bascn", &[("limit", "300")])
        .await
        .unwrap();
    let request = last_request(&transport);
    assert_eq!(request.method, Method::Get);
    assert_eq!(
        request.url,
        "https://open.feishu.cn/open-apis/base/v3/bases/bascn/tables"
    );
}

#[tokio::test]
async fn drive_upload_file_builds_multipart_and_returns_token() {
    let (client, transport) = client().await;
    transport
        .responses
        .lock()
        .unwrap()
        .push_back(r#"{"code":0,"data":{"file_token":"boxcn1"}}"#.to_string());
    let token = client
        .drive()
        .upload_file("a.txt", "explorer", "fld_1", b"hello")
        .await
        .unwrap();
    assert_eq!(token, "boxcn1");
    let request = last_request(&transport);
    let (content_type, body) = request.raw_body.unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary=feishu-"));
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"parent_type\""));
    assert!(text.contains("name=\"size\""));
    assert!(text.contains("hello"));
}
