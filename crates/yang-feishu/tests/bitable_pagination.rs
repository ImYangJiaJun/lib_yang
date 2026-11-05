//! 多维表格分页、字段解析、链接与筛选契约的集成测试。
//!
//! 全部用脚本化 [`Transport`] + 预置 [`MemoryTokenCache`]：零真实网络、
//! 零真实 Redis/MySQL，只通过 `yang_feishu` 的公开 API 断言行为契约。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{json, Value};
use yang_feishu::auth::{MemoryTokenCache, TokenCache};
use yang_feishu::bitable::{
    cell_label, parse_bitable_link, pending_backfill_filter, rekey_cells_by_field_id,
    resolve_bitable_link, resolve_field_names, BitableCoordinates, CellValue, FieldItem, MAX_PAGES,
};
use yang_feishu::transport::{Method, Request, Response, Sleeper, Transport};
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig};

const APP: &str = "appBitableTest";
const TABLE: &str = "tblBitableTest";

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 按 URL 分流的脚本化传输：token 端点恒返回 grant，其余按顺序吐预置响应。
struct FakeTransport {
    scripted: Mutex<VecDeque<Response>>,
    requests: Mutex<Vec<Request>>,
}

impl FakeTransport {
    fn new(scripted: Vec<Response>) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<Request> {
        locked(&self.requests).clone()
    }
}

#[async_trait::async_trait]
impl Transport for FakeTransport {
    async fn send(&self, request: Request) -> yang_feishu::Result<Response> {
        locked(&self.requests).push(request.clone());
        if request.url.contains("tenant_access_token") {
            return Ok(json_response(json!({
                "code": 0,
                "msg": "ok",
                "tenant_access_token": "granted-tenant-token",
                "expire": 7200,
            })));
        }
        let response = locked(&self.scripted)
            .pop_front()
            .unwrap_or_else(|| panic!("脚本耗尽：{}", request.url));
        Ok(response)
    }
}

struct NoSleep;

#[async_trait::async_trait]
impl Sleeper for NoSleep {
    async fn sleep(&self, _duration: Duration) {}
}

fn json_response(body: Value) -> Response {
    Response {
        status: 200,
        body: body.to_string(),
        bytes: None,
        headers: BTreeMap::new(),
    }
}

fn data_response(data: Value) -> Response {
    json_response(json!({ "code": 0, "msg": "ok", "data": data }))
}

fn records_page(
    ids: &[&str],
    has_more: bool,
    page_token: Option<&str>,
    total: Option<i64>,
) -> Response {
    let items: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "record_id": id, "fields": {} }))
        .collect();
    let mut data = json!({ "has_more": has_more, "items": items });
    if let Some(token) = page_token {
        data["page_token"] = json!(token);
    }
    if let Some(total) = total {
        data["total"] = json!(total);
    }
    data_response(data)
}

/// 装配客户端，并预置 token 缓存，证明分页请求不会再走 token 端点。
async fn client_with(transport: Arc<FakeTransport>) -> OpenApiClient {
    let cache = Arc::new(MemoryTokenCache::default());
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials("cli_test", "secret")
            .cache_namespace("it")
            .endpoints(Endpoints::feishu()),
    )
    .transport(transport)
    .sleeper(Arc::new(NoSleep))
    .token_cache(cache.clone())
    .build()
    .unwrap_or_else(|error| panic!("客户端装配失败: {error}"));
    cache
        .put(&client.tokens().token_key(), "cached-tenant-token", 600)
        .await
        .unwrap_or_else(|error| panic!("预置 token 缓存失败: {error}"));
    client
}

fn coordinates() -> BitableCoordinates {
    BitableCoordinates {
        app_token: APP.to_string(),
        table_id: TABLE.to_string(),
        view_id: None,
    }
}

// ---- 1. 两页收敛与分页参数 ----

#[tokio::test]
async fn 两页记录收敛且每页查询参数正确() {
    let transport = Arc::new(FakeTransport::new(vec![
        records_page(&["rec1"], true, Some("p2"), Some(2)),
        records_page(&["rec2"], false, Some("p2"), Some(2)),
    ]));
    let client = client_with(transport.clone()).await;

    let snapshot = client
        .bitable()
        .list_all_records(APP, TABLE, None, None)
        .await
        .unwrap_or_else(|error| panic!("全量拉取应成功: {error}"));

    assert_eq!(snapshot.items.len(), 2, "两页各一条，必须全部返回");
    assert_eq!(snapshot.total, 2, "total 取首屏值");
    assert_eq!(snapshot.items[0].record_id, "rec1");
    assert_eq!(snapshot.items[1].record_id, "rec2");

    let requests = transport.requests();
    assert!(
        !requests
            .iter()
            .any(|request| request.url.contains("tenant_access_token")),
        "缓存已预置，不该再请求 token"
    );
    let bitable: Vec<&Request> = requests
        .iter()
        .filter(|request| request.url.contains("/bitable/"))
        .collect();
    assert_eq!(bitable.len(), 2, "两页应产生两次 bitable 请求");

    assert!(
        bitable[0]
            .query
            .iter()
            .any(|(key, value)| key == "page_size" && value == "500"),
        "第一页必须带 page_size=500"
    );
    assert!(
        !bitable[0].query.iter().any(|(key, _)| key == "page_token"),
        "第一页没有游标"
    );
    assert!(
        bitable[1]
            .query
            .iter()
            .any(|(key, value)| key == "page_size" && value == "500"),
        "第二页仍带 page_size=500"
    );
    assert!(
        bitable[1]
            .query
            .iter()
            .any(|(key, value)| key == "page_token" && value == "p2"),
        "第二页必须带上第一页返回的游标"
    );
}

// ---- 2. 三类硬失败 + 页数上限 ----

#[tokio::test]
async fn has_more_为真但缺游标必须报错() {
    let transport = Arc::new(FakeTransport::new(vec![records_page(
        &["rec1"],
        true,
        None,
        Some(2),
    )]));
    let client = client_with(transport).await;

    let error = client
        .bitable()
        .list_all_records(APP, TABLE, None, None)
        .await
        .expect_err("has_more=true 却没有 page_token 必须失败，不能把残页当全量");
    assert!(
        error.to_string().contains("page_token"),
        "报错要点名缺失的游标，实际: {error}"
    );
}

#[tokio::test]
async fn 重复游标必须报错而不是死循环() {
    let transport = Arc::new(FakeTransport::new(vec![
        records_page(&["rec1"], true, Some("p2"), Some(2)),
        records_page(&["rec2"], true, Some("p2"), Some(2)),
    ]));
    let client = client_with(transport).await;

    let error = client
        .bitable()
        .list_all_records(APP, TABLE, None, None)
        .await
        .expect_err("游标不推进必须硬失败");
    assert!(
        error.to_string().contains("重复"),
        "报错要说明游标重复，实际: {error}"
    );
}

#[tokio::test]
async fn 首屏_total_大于实际累计时必须报错() {
    let transport = Arc::new(FakeTransport::new(vec![records_page(
        &["rec1"],
        false,
        None,
        Some(5),
    )]));
    let client = client_with(transport).await;

    let error = client
        .bitable()
        .list_all_records(APP, TABLE, None, None)
        .await
        .expect_err("累计 1 条少于 total=5，不能当作完整快照");
    assert!(
        error.to_string().contains("不完整快照"),
        "报错要明确指出快照不完整，实际: {error}"
    );
}

#[tokio::test]
async fn 页数上限在第二页触发() {
    let transport = Arc::new(FakeTransport::new(vec![
        records_page(&["rec1"], true, Some("p2"), Some(2)),
        records_page(&["rec2"], false, None, Some(2)),
    ]));
    let client = client_with(transport).await;

    let error = client
        .bitable()
        .list_all_records_at(&coordinates(), &[], 1)
        .await
        .expect_err("max_pages=1 时第二页必须触发上限");
    assert!(
        error.to_string().contains("上限 1"),
        "报错要带上实际页数上限，实际: {error}"
    );
}

// ---- 3. 首屏只取一页，不做收敛断言 ----

#[tokio::test]
async fn 首屏拉取不套收敛断言() {
    let transport = Arc::new(FakeTransport::new(vec![records_page(
        &["rec1"],
        true,
        Some("p2"),
        Some(9),
    )]));
    let client = client_with(transport.clone()).await;

    let page = client
        .bitable()
        .first_records_page(APP, TABLE, None, Some(&["字段A"]), 100)
        .await
        .unwrap_or_else(|error| panic!("首屏诊断不应因 has_more=true 失败: {error}"));

    assert!(page.has_more, "原样返回 has_more");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.total, Some(9));

    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "首屏只发一页");
    assert!(
        requests[0]
            .query
            .iter()
            .any(|(key, value)| key == "page_size" && value == "100"),
        "调用方给的 page_size 必须透传"
    );
    assert!(
        !requests[0].query.iter().any(|(key, _)| key == "page_token"),
        "首屏没有游标"
    );
    assert!(
        requests[0]
            .query
            .iter()
            .any(|(key, value)| key == "field_names" && value == r#"["字段A"]"#),
        "field_names 必须是单个 JSON 数组字符串"
    );
}

// ---- 4. 行号扫描：尽力而为 ----

#[tokio::test]
async fn 行号扫描缺游标时返回已找到的部分() {
    let transport = Arc::new(FakeTransport::new(vec![
        records_page(&["recX", "recA", "recY"], true, Some("p2"), None),
        // 第二页 has_more=true 却没有游标：尽力而为语义应返回已找到的 recA。
        records_page(&["recZ"], true, None, None),
    ]));
    let client = client_with(transport.clone()).await;
    let ids = vec!["recA".to_string(), "recB".to_string()];

    let rows = client
        .bitable()
        .record_row_numbers(APP, TABLE, &ids, MAX_PAGES)
        .await
        .unwrap_or_else(|error| panic!("行号扫描应返回部分结果而不是 Err: {error}"));

    assert_eq!(
        rows,
        vec![("recA".to_string(), 2u64)],
        "recA 在第一页第 2 行"
    );
    let requests = transport.requests();
    assert_eq!(requests.len(), 2, "两页各一次请求");
    assert!(
        requests[1]
            .query
            .iter()
            .any(|(key, value)| key == "page_token" && value == "p2"),
        "第二页必须带游标"
    );
}

#[tokio::test]
async fn 行号扫描全部命中后提前返回() {
    // 只脚本一页：若实现继续翻页，FakeTransport 会因脚本耗尽而 panic。
    let transport = Arc::new(FakeTransport::new(vec![records_page(
        &["recX", "recA"],
        true,
        Some("p2"),
        None,
    )]));
    let client = client_with(transport.clone()).await;
    let ids = vec!["recA".to_string()];

    let rows = client
        .bitable()
        .record_row_numbers(APP, TABLE, &ids, MAX_PAGES)
        .await
        .unwrap_or_else(|error| panic!("行号扫描应成功: {error}"));

    assert_eq!(rows, vec![("recA".to_string(), 2u64)]);
    assert_eq!(transport.requests().len(), 1, "全部目标已找到就不能再翻页");
}

// ---- 5. 查询记录：分页参数在 query，不在 body ----

#[tokio::test]
async fn 查询记录的分页参数走_query_而不是_body() {
    let transport = Arc::new(FakeTransport::new(vec![data_response(json!({
        "items": [{ "record_id": "recS", "fields": {} }],
        "has_more": false,
        "total": 1,
    }))]));
    let client = client_with(transport.clone()).await;
    let body = json!({
        "filter": { "conjunction": "and", "conditions": [] },
        "automatic_fields": false,
    });

    let page = client
        .bitable()
        .search_records(APP, TABLE, body.clone(), Some("s2"))
        .await
        .unwrap_or_else(|error| panic!("查询记录应成功: {error}"));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.total, Some(1));

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, Method::Post);
    assert!(
        request.url.ends_with("/records/search"),
        "实际 URL: {}",
        request.url
    );
    assert!(
        request
            .query
            .iter()
            .any(|(key, value)| key == "page_size" && value == "500"),
        "page_size 必须是 query 参数"
    );
    assert!(
        request
            .query
            .iter()
            .any(|(key, value)| key == "page_token" && value == "s2"),
        "page_token 必须是 query 参数"
    );
    assert_eq!(request.json_body.as_ref(), Some(&body), "body 原样透传");
    let sent = request.json_body.as_ref().expect("应有 JSON body");
    assert!(sent.get("page_token").is_none(), "page_token 不能进 body");
    assert!(sent.get("page_size").is_none(), "page_size 不能进 body");
}

#[tokio::test]
async fn 查询全量缺游标必须报错() {
    let transport = Arc::new(FakeTransport::new(vec![data_response(json!({
        "items": [{ "record_id": "recS", "fields": {} }],
        "has_more": true,
        "total": 2,
    }))]));
    let client = client_with(transport.clone()).await;

    let error = client
        .bitable()
        .search_all_records(APP, TABLE, json!({}), 3)
        .await
        .expect_err("查询翻页缺 page_token 必须失败");
    assert!(error.to_string().contains("page_token"), "实际: {error}");
    assert_eq!(transport.requests().len(), 1, "缺游标应立即失败，不能重发");
}

// ---- 6. 批量回写请求体形状 ----

#[tokio::test]
async fn 批量回写请求体只有_records_数组() {
    let transport = Arc::new(FakeTransport::new(vec![data_response(json!({}))]));
    let client = client_with(transport.clone()).await;
    let fields = BTreeMap::from([("fldA".to_string(), json!("值A"))]);
    let rows = vec![("rec1".to_string(), fields)];

    client
        .bitable()
        .batch_update_records(APP, TABLE, &rows)
        .await
        .unwrap_or_else(|error| panic!("批量回写应成功: {error}"));

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, Method::Post);
    assert!(
        request.url.ends_with("/records/batch_update"),
        "实际 URL: {}",
        request.url
    );
    assert!(request.query.is_empty(), "批量回写没有 query 参数");
    assert_eq!(
        request.json_body,
        Some(json!({
            "records": [{ "record_id": "rec1", "fields": { "fldA": "值A" } }]
        })),
        "请求体必须固定为 records 数组，每项含 record_id 与 fields"
    );
}

// ---- 7. 单元格、字段、链接与筛选的纯函数契约 ----

#[test]
fn 单元格取值区分文本_空值与不支持() {
    assert_eq!(
        cell_label(&json!(["A", "B"])),
        CellValue::Text(vec!["A".into(), "B".into()]),
        "多选必须逐元素给出标签"
    );
    assert_eq!(
        cell_label(&json!(["A", "  ", "B"])),
        CellValue::Text(vec!["A".into(), "B".into()]),
        "空白元素跳过但保留全部有效项"
    );
    assert_eq!(
        cell_label(&json!([{ "text": "富文本" }])),
        CellValue::Text(vec!["富文本".into()])
    );
    assert_eq!(
        cell_label(&json!(42)),
        CellValue::Text(vec!["42".into()]),
        "数字在列表接口里可能是字符串，但 number 也要支持"
    );

    assert_eq!(cell_label(&Value::Null), CellValue::Empty);
    assert_eq!(cell_label(&json!([])), CellValue::Empty);
    assert_eq!(cell_label(&json!("   ")), CellValue::Empty);

    assert_eq!(
        cell_label(&json!([{ "id": "ou_1", "name": "张三" }])),
        CellValue::Unsupported,
        "人员对象没有 text 键，不能静默当空"
    );
    assert_eq!(
        cell_label(&json!({ "address": "东长安街" })),
        CellValue::Unsupported,
        "地理位置对象同理"
    );
    assert_eq!(cell_label(&json!(true)), CellValue::Unsupported);
}

fn field_item(field_id: &str, field_name: &str) -> FieldItem {
    serde_json::from_value(json!({
        "field_id": field_id,
        "field_name": field_name,
    }))
    .unwrap_or_else(|error| panic!("构造 FieldItem 失败: {error}"))
}

#[test]
fn 字段名解析保持请求顺序并一次点名全部缺失() {
    let fields = vec![
        field_item("fldB", "汇率"),
        field_item("fldA", "币种"),
        field_item("fldC", "费用类型"),
    ];

    let resolved = resolve_field_names(
        &fields,
        &["fldC".to_string(), "fldA".to_string(), "fldB".to_string()],
    )
    .unwrap_or_else(|error| panic!("应可解析: {error}"));
    assert_eq!(
        resolved,
        vec![
            ("fldC".to_string(), "费用类型".to_string()),
            ("fldA".to_string(), "币种".to_string()),
            ("fldB".to_string(), "汇率".to_string()),
        ],
        "输出顺序必须与请求一致"
    );

    let error = resolve_field_names(
        &fields,
        &[
            "fldA".to_string(),
            "fldGONE".to_string(),
            "fldMISSING".to_string(),
        ],
    )
    .expect_err("缺失字段必须报错");
    let message = error.to_string();
    assert!(message.contains("fldGONE"), "实际: {message}");
    assert!(
        message.contains("fldMISSING"),
        "要一次点名全部缺失，实际: {message}"
    );
    assert!(
        !message.contains("fldA"),
        "存在的字段不该被点名，实际: {message}"
    );
}

#[test]
fn 单元格重映射按_field_id_作键并豁免系统字段() {
    let fields = vec![field_item("fldA", "申请人"), field_item("fldB", "金额")];
    let mut cells = BTreeMap::new();
    cells.insert("申请人".to_string(), json!("ou_1"));
    cells.insert("fldB".to_string(), json!(42));
    cells.insert("created_by".to_string(), json!({ "id": "ou_creator" }));
    cells.insert("created_time".to_string(), json!(1690000000000i64));

    let out = rekey_cells_by_field_id(&fields, &cells)
        .unwrap_or_else(|error| panic!("重映射应成功: {error}"));
    assert_eq!(out.get("fldA"), Some(&json!("ou_1")), "列名要换成 field_id");
    assert_eq!(
        out.get("fldB"),
        Some(&json!(42)),
        "已是 field_id 的原样保留"
    );
    assert!(!out.contains_key("申请人"), "输出不能保留列名键");
    assert!(!out.contains_key("created_by"), "系统字段必须豁免");
    assert!(!out.contains_key("created_time"), "系统字段必须豁免");

    let mut unknown = BTreeMap::new();
    unknown.insert("已删除的列".to_string(), json!("x"));
    let error = rekey_cells_by_field_id(&fields, &unknown).expect_err("未知键必须点名报错");
    assert!(error.to_string().contains("已删除的列"), "实际: {error}");
}

#[test]
fn 多维表格链接解析与显式字段合并() {
    let parsed = parse_bitable_link("https://tenant.feishu.cn/base/appToken?table=tblA&view=vew1")
        .unwrap_or_else(|error| panic!("应可解析: {error}"));
    assert_eq!(parsed.app_token, "appToken");
    assert_eq!(parsed.table_id.as_deref(), Some("tblA"));
    assert_eq!(parsed.view_id.as_deref(), Some("vew1"));

    let legacy = parse_bitable_link("appTokenOnly")
        .unwrap_or_else(|error| panic!("纯 Base Token 应兼容: {error}"));
    assert_eq!(legacy.app_token, "appTokenOnly");
    assert_eq!(legacy.table_id, None);
    assert_eq!(legacy.view_id, None);

    assert!(
        parse_bitable_link("https://tenant.feishu.cn/wiki/appToken").is_err(),
        "路径必须是 /base/<token>"
    );

    let merged = resolve_bitable_link(
        "https://tenant.feishu.cn/base/appToken?table=tblA",
        Some("tblA"),
        Some("vewExplicit"),
    )
    .unwrap_or_else(|error| panic!("一致时应合并成功: {error}"));
    assert_eq!(merged.table_id.as_deref(), Some("tblA"));
    assert_eq!(merged.view_id.as_deref(), Some("vewExplicit"));

    let conflict = resolve_bitable_link(
        "https://tenant.feishu.cn/base/appToken?table=tblA",
        Some("tblB"),
        None,
    )
    .expect_err("链接内 table 与单独字段不一致必须报错");
    assert!(conflict.to_string().contains("不一致"), "实际: {conflict}");
}

#[test]
fn 回填筛选_is_empty_用空数组且复选框用字符串布尔() {
    let filter = pending_backfill_filter("回填列", None);
    assert_eq!(filter["filter"]["conjunction"], "and");
    assert_eq!(filter["filter"]["conditions"][0]["field_name"], "回填列");
    assert_eq!(filter["filter"]["conditions"][0]["operator"], "isEmpty");
    let value = filter["filter"]["conditions"][0]["value"]
        .as_array()
        .expect("isEmpty 的 value 必须是数组");
    assert!(value.is_empty(), "isEmpty 必须传空数组，不能是 null");
    assert_eq!(filter["automatic_fields"], false);

    let with_scope = pending_backfill_filter("回填列", Some("批量范围"));
    let conditions = with_scope["filter"]["conditions"]
        .as_array()
        .expect("conditions 必须是数组");
    assert_eq!(conditions.len(), 2, "复选框条件与 isEmpty 同时成立");
    assert_eq!(conditions[1]["field_name"], "批量范围");
    assert_eq!(conditions[1]["operator"], "is");
    assert_eq!(conditions[1]["value"], json!(["true"]));
    assert_eq!(
        conditions[1]["value"][0].as_str(),
        Some("true"),
        "复选框值必须是字符串布尔而不是 JSON 布尔"
    );
    assert_eq!(with_scope["filter"]["conjunction"], "and");
    assert_eq!(with_scope["automatic_fields"], false);
}
