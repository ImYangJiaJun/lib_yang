//! 审批客户端与表单值编码的集成测试。
//!
//! 全部使用脚本化 [`Transport`] + 记录型 [`Sleeper`] + 预置 [`MemoryTokenCache`]：
//! 零真实网络、零 Redis/MySQL，只通过 `yang_feishu::...` 的公开 API 钉住行为契约。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{json, Map, Value};
use yang_feishu::approval::{
    amount_currency, build_form, derive_uuid, parse_form, ConvertError, Converter, CreateOutcome,
    FixedOffset, FormError, FormInput, GetOutcome, SubscribeOutcome, WidgetMap,
};
use yang_feishu::auth::{MemoryTokenCache, TokenCache};
use yang_feishu::transport::{Method, Request, Response, Sleeper, Transport};
use yang_feishu::{Error, FailureKind, OpenApiClient, OpenApiConfig, Result};

const APP_ID: &str = "cli_approval_contract";
const NAMESPACE: &str = "approval-contract";
const TENANT_TOKEN: &str = "cached-tenant-token";
const OPEN_BASE: &str = "https://open.feishu.cn";

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 按脚本逐次返回响应的传输替身，同时记录收到的每个请求。
struct FakeTransport {
    script: Mutex<VecDeque<Response>>,
    requests: Mutex<Vec<Request>>,
}

impl FakeTransport {
    fn new(script: Vec<Response>) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(script.into()),
            requests: Mutex::new(Vec::new()),
        })
    }

    fn requests(&self) -> Vec<Request> {
        locked(&self.requests).clone()
    }
}

#[async_trait::async_trait]
impl Transport for FakeTransport {
    async fn send(&self, request: Request) -> Result<Response> {
        locked(&self.requests).push(request);
        locked(&self.script)
            .pop_front()
            .ok_or_else(|| Error::fatal("FakeTransport 脚本已耗尽"))
    }
}

/// 记录型睡眠替身：所有退避立即返回，测试不等待真实时间。
struct NoopSleeper;

#[async_trait::async_trait]
impl Sleeper for NoopSleeper {
    async fn sleep(&self, _duration: Duration) {}
}

fn response(body: Value) -> Response {
    Response {
        status: 200,
        body: body.to_string(),
        bytes: None,
        headers: BTreeMap::new(),
    }
}

fn ok_data(data: Value) -> Response {
    response(json!({"code": 0, "msg": "ok", "data": data}))
}

fn business_failure(code: i32, message: &str) -> Response {
    response(json!({"code": code, "msg": message}))
}

/// 装配客户端并预置 tenant token：业务请求不应再访问 token 端点。
async fn client_with(script: Vec<Response>) -> (OpenApiClient, Arc<FakeTransport>) {
    let transport = FakeTransport::new(script);
    let cache = Arc::new(MemoryTokenCache::default());
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials(APP_ID, "secret").cache_namespace(NAMESPACE),
    )
    .transport(transport.clone())
    .sleeper(Arc::new(NoopSleeper))
    .token_cache(cache.clone())
    .build()
    .unwrap_or_else(|error| panic!("客户端装配失败: {error}"));
    cache
        .put(&client.tokens().token_key(), TENANT_TOKEN, 600)
        .await
        .unwrap_or_else(|error| panic!("预置 tenant token 失败: {error}"));
    (client, transport)
}

fn assert_cached_token_used(requests: &[Request]) {
    assert!(
        requests
            .iter()
            .all(|request| !request.url.contains("tenant_access_token")),
        "预置缓存后不应再访问 token 端点"
    );
    assert!(
        requests
            .iter()
            .all(|request| request.bearer_token.as_deref() == Some(TENANT_TOKEN)),
        "业务请求必须携带预置的 tenant token"
    );
}

// ---------------------------------------------------------------------------
// 1. create_instance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_instance_成功时发送官方_post_并携带幂等_uuid() {
    let (client, transport) = client_with(vec![ok_data(json!({"instance_code": "INS-1"}))]).await;
    let form = r#"[{"id":"w_text","type":"input","value":"出差"}]"#;

    let outcome = client
        .approval()
        .create_instance("APPROVAL-1", form, "ou_user", "uuid-1")
        .await
        .expect("创建审批实例应成功");

    match outcome {
        CreateOutcome::Created { instance_code } => assert_eq!(instance_code, "INS-1"),
        other => panic!("期望 Created，实际 {other:?}"),
    }

    let requests = transport.requests();
    assert_cached_token_used(&requests);
    assert_eq!(requests.len(), 1, "成功创建只应有一次业务请求");
    let request = &requests[0];
    assert_eq!(request.method, Method::Post);
    assert_eq!(
        request.url,
        format!("{OPEN_BASE}/open-apis/approval/v4/instances")
    );
    assert!(request.query.is_empty());
    assert!(
        request.idempotent,
        "uuid 是服务端幂等唯一依据，必须 idempotent=true"
    );

    let body = request
        .json_body
        .as_ref()
        .expect("create 必须携带 JSON body");
    assert_eq!(body["approval_code"], "APPROVAL-1");
    assert_eq!(body["open_id"], "ou_user");
    assert_eq!(body["form"], form, "form 必须原样透传压缩数组字符串");
    assert_eq!(body["uuid"], "uuid-1");
}

#[tokio::test]
async fn create_instance_的_60012_返回_uuid_conflict_而不是_err() {
    let (client, transport) = client_with(vec![business_failure(60012, "uuid already used")]).await;

    let outcome = client
        .approval()
        .create_instance("APPROVAL-1", "[]", "ou_user", "uuid-1")
        .await
        .expect("60012 不是失败，必须返回 UuidConflict 供按 uuid 反查");

    assert!(matches!(outcome, CreateOutcome::UuidConflict));
    assert_eq!(transport.requests().len(), 1, "60012 是终态语义，不得重试");
}

// ---------------------------------------------------------------------------
// 2. get_instance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_instance_把_1390003_映射为_not_found() {
    let (client, transport) =
        client_with(vec![business_failure(1390003, "instance not found")]).await;

    let outcome = client
        .approval()
        .get_instance("INS-MISSING")
        .await
        .expect("1390003 不是调用错误，必须交给上层区分并发窗口");

    assert!(matches!(outcome, GetOutcome::NotFound));
    let requests = transport.requests();
    assert_cached_token_used(&requests);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::Get);
    assert_eq!(
        requests[0].url,
        format!("{OPEN_BASE}/open-apis/approval/v4/instances/INS-MISSING")
    );
    assert_eq!(
        requests[0].query,
        vec![("user_id_type".to_string(), "open_id".to_string())]
    );
}

#[tokio::test]
async fn get_instance_解析_bot_形态的_task_list_与_timeline() {
    let data = json!({
        "instance_code": "INS-BOT",
        "approval_code": "APPROVAL-1",
        "serial_number": "202609280001",
        "status": "PENDING",
        "start_time": "1790750409857",
        "end_time": "0",
        "open_id": "ou_initiator",
        "user_id": "E1001",
        "department_id": "db59a45c851464gf",
        "department_name": "研发部",
        "uuid": "57801135-b6d4-8a98-949c-12de67e20ca8",
        "reverted": false,
        "task_list": [{
            "id": "T1",
            "node_id": "N1",
            "node_name": "直属主管",
            "user_id": "E2002",
            "open_id": "ou_approver",
            "status": "DONE",
            "type": "AND",
            "start_time": "1790750410000",
            "end_time": "1790750420000"
        }],
        "timeline": [{
            "type": "APPROVE",
            "create_time": "1790750420000",
            "user_id": "E2002",
            "open_id": "ou_approver",
            "ext": "同意"
        }]
    });
    let (client, _) = client_with(vec![ok_data(data)]).await;

    let detail = match client
        .approval()
        .get_instance("INS-BOT")
        .await
        .expect("取实例详情应成功")
    {
        GetOutcome::Found(detail) => detail,
        other => panic!("期望 Found，实际 {other:?}"),
    };

    assert_eq!(detail.instance_code, "INS-BOT");
    assert_eq!(detail.approval_code.as_deref(), Some("APPROVAL-1"));
    assert_eq!(detail.serial_number.as_deref(), Some("202609280001"));
    assert_eq!(detail.status.as_deref(), Some("PENDING"));
    assert_eq!(detail.start_time.as_deref(), Some("1790750409857"));
    // `"0"` 是 end_time 的「未完成」哨兵，不是 serial_number 的空值。
    assert_eq!(detail.end_time.as_deref(), Some("0"));
    assert_eq!(detail.open_id.as_deref(), Some("ou_initiator"));
    assert_eq!(detail.user_id.as_deref(), Some("E1001"));
    assert_eq!(detail.department_id.as_deref(), Some("db59a45c851464gf"));
    assert_eq!(
        detail.uuid.as_deref(),
        Some("57801135-b6d4-8a98-949c-12de67e20ca8")
    );
    assert_eq!(detail.reverted, Some(false));
    // 部门名是宿主富化值，不属于 API 响应；即使响应里混入同名键也不能采信。
    assert_eq!(detail.department_name, None);

    let tasks = detail
        .task_list
        .as_ref()
        .expect("bot 形态必须解析 task_list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id.as_deref(), Some("T1"));
    assert_eq!(tasks[0].node_id.as_deref(), Some("N1"));
    assert_eq!(tasks[0].node_name.as_deref(), Some("直属主管"));
    assert_eq!(tasks[0].user_id.as_deref(), Some("E2002"));
    assert_eq!(tasks[0].open_id.as_deref(), Some("ou_approver"));
    assert_eq!(tasks[0].status.as_deref(), Some("DONE"));
    assert_eq!(tasks[0].r#type.as_deref(), Some("AND"));

    let timeline = detail.timeline.as_ref().expect("bot 形态必须解析 timeline");
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].r#type.as_deref(), Some("APPROVE"));
    assert_eq!(timeline[0].create_time.as_deref(), Some("1790750420000"));
    assert_eq!(timeline[0].ext.as_deref(), Some("同意"));
}

#[tokio::test]
async fn get_instance_解析_user_形态并归一空串单号() {
    let data = json!({
        "instance_code": "INS-USER",
        "definition_code": "APPROVAL-2",
        "serial_number": "",
        "status": "APPROVED",
        "start_time": "1790750409857",
        "user_id": "ou_user_shape",
        "department_id": "od-123",
        "tasks": [{"id": "T1"}],
        "current_nodes": [{"node_id": "N1"}],
        "operation_records": [{"type": "APPROVE"}]
    });
    let (client, _) = client_with(vec![ok_data(data)]).await;

    let detail = match client
        .approval()
        .get_instance("INS-USER")
        .await
        .expect("取实例详情应成功")
    {
        GetOutcome::Found(detail) => detail,
        other => panic!("期望 Found，实际 {other:?}"),
    };

    assert_eq!(detail.instance_code, "INS-USER");
    // user 形态的 definition_code / tasks / current_nodes / operation_records 不在本契约内。
    assert_eq!(detail.approval_code, None);
    assert_eq!(detail.task_list, None);
    assert_eq!(detail.timeline, None);
    // 空串与缺失同义，统一归一为 None。
    assert_eq!(detail.serial_number, None);
    // user 形态的 user_id 是 ou_ 前缀的 open_id，语义与 bot 形态不同但键名相同。
    assert_eq!(detail.user_id.as_deref(), Some("ou_user_shape"));
    assert_eq!(detail.department_id.as_deref(), Some("od-123"));
    assert_eq!(detail.department_name, None);
}

// ---------------------------------------------------------------------------
// 3. subscribe / definition
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_approval_把_1390007_当幂等成功_其它失败冒泡() {
    let (client, transport) = client_with(vec![
        business_failure(1390007, "subscription existed"),
        business_failure(1390001, "approval definition invalid"),
        ok_data(json!({})),
    ])
    .await;
    let approval = client.approval();

    let first = approval
        .subscribe_approval("APPROVAL-1")
        .await
        .expect("1390007 应按幂等成功处理");
    assert!(matches!(first, SubscribeOutcome::AlreadySubscribed));

    let error = approval
        .subscribe_approval("APPROVAL-1")
        .await
        .expect_err("其它订阅失败必须冒泡");
    assert!(matches!(error.kind, FailureKind::Fatal { code: 1390001 }));

    let third = approval
        .subscribe_approval("APPROVAL-1")
        .await
        .expect("首次订阅应成功");
    assert!(matches!(third, SubscribeOutcome::Subscribed));

    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|request| {
        request.method == Method::Post
            && request
                .url
                .ends_with("/open-apis/approval/v4/approvals/APPROVAL-1/subscribe")
    }));
}

#[tokio::test]
async fn get_approval_definition_保留字符串_form_且_is_external_缺省_false() {
    let raw_form = json!([{
        "id": "w_text",
        "name": "事由",
        "type": "input",
        "required": true
    }])
    .to_string();
    let (client, transport) = client_with(vec![
        ok_data(json!({
            "approval_name": "请假申请",
            "form": raw_form,
            "is_external": true
        })),
        ok_data(json!({
            "approval_name": "报销申请",
            "form": [{"id": "w_amount", "name": "金额", "type": "amount"}]
        })),
    ])
    .await;

    let definition = client
        .approval()
        .get_approval_definition("APPROVAL-1")
        .await
        .expect("取审批定义应成功");
    assert_eq!(definition.approval_name, "请假申请");
    assert!(definition.is_external);
    let form = definition.form.as_ref().expect("form 必须原样保留");
    assert!(form.is_string(), "JSON 字符串形态的 form 必须保留字符串");
    let widgets = parse_form(form).expect("字符串形态的 form 应可解析");
    assert_eq!(widgets.len(), 1);
    assert_eq!(widgets[0].id, "w_text");

    let second = client
        .approval()
        .get_approval_definition("APPROVAL-2")
        .await
        .expect("取审批定义应成功");
    assert_eq!(second.approval_name, "报销申请");
    assert!(!second.is_external, "is_external 缺省必须是 false");
    assert!(second.form.as_ref().is_some_and(Value::is_array));

    let requests = transport.requests();
    assert!(requests.iter().all(|request| {
        request.method == Method::Get
            && request
                .url
                .starts_with(&format!("{OPEN_BASE}/open-apis/approval/v4/approvals/"))
    }));
}

// ---------------------------------------------------------------------------
// 4. 幂等键派生
// ---------------------------------------------------------------------------

#[test]
fn derive_uuid_与存量黄金向量逐字节一致() {
    // 这行输出已持久化在宿主任务表与组表里；一旦变化，存量任务按 uuid 回捞会全部 1390003。
    assert_eq!(
        derive_uuid("appbcbWCzen6", "tblsRc9GRRX", "4202AD96-9EC1", "recqwIwhc6"),
        "57801135-b6d4-8a98-949c-12de67e20ca8"
    );
}

#[test]
fn derive_uuid_四元组任一变化都产生不同输出() {
    let baseline = derive_uuid("base", "table", "code", "record");
    assert_eq!(baseline.len(), 36);
    assert_ne!(baseline, derive_uuid("base2", "table", "code", "record"));
    assert_ne!(baseline, derive_uuid("base", "table2", "code", "record"));
    assert_ne!(baseline, derive_uuid("base", "table", "code2", "record"));
    assert_ne!(baseline, derive_uuid("base", "table", "code", "record2"));
}

// ---------------------------------------------------------------------------
// 5. form 解析与币种
// ---------------------------------------------------------------------------

#[test]
fn parse_form_接受数组与_json_字符串() {
    let form = json!([{
        "id": "w_text",
        "name": "事由",
        "type": "input",
        "required": true
    }]);

    let from_array = parse_form(&form).expect("数组形态应可解析");
    assert_eq!(from_array.len(), 1);
    assert_eq!(from_array[0].id, "w_text");
    assert_eq!(from_array[0].name, "事由");
    assert_eq!(from_array[0].r#type, "input");
    assert!(from_array[0].required);

    let from_string =
        parse_form(&Value::String(form.to_string())).expect("JSON 字符串形态应可解析");
    assert_eq!(from_string.len(), 1);
    assert_eq!(from_string[0].id, "w_text");
}

#[test]
fn parse_form_拒绝_null_空数组与空字符串() {
    assert!(matches!(parse_form(&Value::Null), Err(FormError::Missing)));
    assert!(matches!(parse_form(&json!([])), Err(FormError::Empty)));
    assert!(matches!(
        parse_form(&Value::String(String::new())),
        Err(FormError::Parse(_))
    ));
    assert!(matches!(
        parse_form(&json!({})),
        Err(FormError::UnexpectedType("object"))
    ));
}

#[test]
fn amount_currency_取_currency_range_首值() {
    let snapshot = json!([
        {
            "id": "w_amount",
            "name": "金额",
            "type": "amount",
            "option": {"currencyRange": ["CNY", "USD"]}
        },
        {"id": "w_text", "name": "事由", "type": "input"}
    ])
    .to_string();

    assert_eq!(
        amount_currency(&snapshot, "w_amount").as_deref(),
        Some("CNY")
    );
    assert_eq!(
        amount_currency(&snapshot, "w_text"),
        None,
        "非金额控件没有币种"
    );
    assert_eq!(amount_currency(&snapshot, "w_missing"), None);
    assert_eq!(amount_currency("不是 JSON", "w_amount"), None);
}

// ---------------------------------------------------------------------------
// 6. build_form
// ---------------------------------------------------------------------------

fn widget_map(
    widget_id: &str,
    widget_type: &str,
    source_field: &str,
    converter: Converter,
) -> WidgetMap {
    WidgetMap {
        widget_id: widget_id.to_string(),
        widget_type: widget_type.to_string(),
        required: true,
        source_field: source_field.to_string(),
        interval_end_field: None,
        converter,
        option_map: BTreeMap::new(),
        option_source_key: None,
        currency: None,
    }
}

fn form_input<'a>(
    widgets: &'a [WidgetMap],
    form_snapshot: &'a str,
    cells: &'a Map<String, Value>,
    attachment_codes: &'a BTreeMap<String, Vec<String>>,
    timezone_offset: FixedOffset,
) -> FormInput<'a> {
    FormInput {
        widgets,
        form_snapshot,
        cells,
        attachment_codes,
        timezone_offset,
        approval_app_id: None,
    }
}

#[test]
fn build_form_生成压缩数组并覆盖数字_多选_日期区间_金额币种与人员() {
    let snapshot = json!([
        {"id": "w_text", "name": "事由", "type": "input", "required": true},
        {"id": "w_number", "name": "数量", "type": "number", "required": true},
        {
            "id": "w_multi",
            "name": "多选",
            "type": "checkboxV2",
            "required": true,
            "option": [
                {"text": "A", "value": "opt-a"},
                {"text": "B", "value": "opt-b"}
            ]
        },
        {"id": "w_interval", "name": "区间", "type": "dateInterval", "required": true},
        {
            "id": "w_amount",
            "name": "金额",
            "type": "amount",
            "required": true,
            "option": {"currencyRange": ["CNY", "USD"]}
        },
        {"id": "w_contact", "name": "成员", "type": "contact", "required": true}
    ])
    .to_string();

    let mut multi = widget_map("w_multi", "checkboxV2", "f_multi", Converter::Option);
    multi
        .option_map
        .insert("A".to_string(), "opt-a".to_string());
    multi
        .option_map
        .insert("B".to_string(), "opt-b".to_string());

    let mut interval = widget_map("w_interval", "dateInterval", "f_start", Converter::Date);
    interval.interval_end_field = Some("f_end".to_string());

    let mut amount = widget_map("w_amount", "amount", "f_amount", Converter::Direct);
    amount.currency = Some("CNY".to_string());

    let widgets = vec![
        widget_map("w_text", "input", "f_text", Converter::Direct),
        widget_map("w_number", "number", "f_number", Converter::Direct),
        multi,
        interval,
        amount,
        widget_map("w_contact", "contact", "f_contact", Converter::Direct),
    ];

    let mut cells = Map::new();
    cells.insert("f_text".into(), json!("出差"));
    cells.insert("f_number".into(), json!(42));
    cells.insert("f_multi".into(), json!(["A", "B"]));
    cells.insert("f_start".into(), json!(1_790_750_400_000i64));
    cells.insert("f_end".into(), json!(1_790_836_800_000i64));
    cells.insert("f_amount".into(), json!(12.34));
    cells.insert("f_contact".into(), json!(["ou_a", "ou_b"]));

    let attachments = BTreeMap::new();
    let input = form_input(
        &widgets,
        &snapshot,
        &cells,
        &attachments,
        FixedOffset::from_seconds(0).unwrap_or_else(|error| panic!("{error}")),
    );
    let form = build_form(&input).unwrap_or_else(|error| panic!("build_form 失败: {error}"));

    assert!(form.starts_with('['), "form 必须是 JSON 数组字符串: {form}");
    assert!(
        !form.contains('\n'),
        "form 必须是压缩 JSON，不允许换行: {form}"
    );
    assert!(
        !form.contains(' '),
        "form 必须是压缩 JSON，不允许空白: {form}"
    );

    let parsed: Vec<Value> =
        serde_json::from_str(&form).unwrap_or_else(|error| panic!("form 不是合法 JSON: {error}"));
    let by_id = |id: &str| {
        parsed
            .iter()
            .find(|item| item["id"] == id)
            .unwrap_or_else(|| panic!("form 缺少控件「{id}」"))
    };

    assert_eq!(by_id("w_text")["value"], "出差");

    let number = by_id("w_number");
    assert!(
        number["value"].is_number(),
        "数字控件的 value 必须是 JSON number，实际是 {}",
        number["value"]
    );
    assert_eq!(number["value"].as_i64(), Some(42));

    assert_eq!(
        by_id("w_multi")["value"],
        json!(["opt-a", "opt-b"]),
        "多选按 option_map 映射为 value 数组"
    );

    let interval = &by_id("w_interval")["value"];
    assert_eq!(interval["start"], "2026-09-30T06:40:00+00:00");
    assert_eq!(interval["end"], "2026-10-01T06:40:00+00:00");
    assert_eq!(interval["interval"].as_f64(), Some(1.0));

    let amount = by_id("w_amount");
    assert!(
        amount["value"].is_number(),
        "金额控件的 value 必须是 JSON number，实际是 {}",
        amount["value"]
    );
    assert_eq!(amount["value"].as_f64(), Some(12.34));
    assert_eq!(amount["currency"], "CNY", "币种是 value 的兄弟字段");
    let amount_object = amount.as_object().expect("金额控件必须是对象");
    assert!(amount_object.contains_key("value") && amount_object.contains_key("currency"));

    let contact = by_id("w_contact");
    assert_eq!(
        contact["open_ids"],
        json!(["ou_a", "ou_b"]),
        "人员控件用 open_ids 携带人员 id"
    );
    assert_eq!(contact["value"], json!([]), "人员控件的 value 必须是空数组");
}

#[test]
fn build_form_对无法提单的控件返回_unsupported() {
    let snapshot = json!([
        {"id": "w_text", "name": "说明", "type": "text", "required": true}
    ])
    .to_string();
    let widgets = vec![widget_map("w_text", "text", "f_text", Converter::Direct)];
    let mut cells = Map::new();
    cells.insert("f_text".into(), json!("文本"));
    let attachments = BTreeMap::new();
    let input = form_input(
        &widgets,
        &snapshot,
        &cells,
        &attachments,
        FixedOffset::from_seconds(0).unwrap_or_else(|error| panic!("{error}")),
    );

    let error = build_form(&input).expect_err("text 控件不能通过 API 提单");
    match error {
        ConvertError::Unsupported {
            widget_id,
            widget_type,
        } => {
            assert_eq!(widget_id, "w_text");
            assert_eq!(widget_type, "text");
        }
        other => panic!("期望 Unsupported，实际 {other:?}"),
    }
}

#[test]
fn build_form_对转换器不匹配返回_converter_mismatch() {
    let snapshot = json!([
        {"id": "w_date", "name": "日期", "type": "date", "required": true}
    ])
    .to_string();
    let widgets = vec![widget_map("w_date", "date", "f_date", Converter::Direct)];
    let mut cells = Map::new();
    cells.insert("f_date".into(), json!(1_790_750_400_000i64));
    let attachments = BTreeMap::new();
    let input = form_input(
        &widgets,
        &snapshot,
        &cells,
        &attachments,
        FixedOffset::from_seconds(0).unwrap_or_else(|error| panic!("{error}")),
    );

    let error = build_form(&input).expect_err("日期控件必须配 Date 转换器");
    match error {
        ConvertError::ConverterMismatch {
            widget_id,
            widget_type,
            converter,
        } => {
            assert_eq!(widget_id, "w_date");
            assert_eq!(widget_type, "date");
            assert_eq!(converter, "direct");
        }
        other => panic!("期望 ConverterMismatch，实际 {other:?}"),
    }
}

#[test]
fn build_form_对未映射选项返回_unmapped_option() {
    let snapshot = json!([{
        "id": "w_multi",
        "name": "多选",
        "type": "checkboxV2",
        "required": true,
        "option": [{"text": "A", "value": "opt-a"}]
    }])
    .to_string();
    let widgets = vec![widget_map(
        "w_multi",
        "checkboxV2",
        "f_multi",
        Converter::Option,
    )];
    let mut cells = Map::new();
    cells.insert("f_multi".into(), json!(["未映射"]));
    let attachments = BTreeMap::new();
    let input = form_input(
        &widgets,
        &snapshot,
        &cells,
        &attachments,
        FixedOffset::from_seconds(0).unwrap_or_else(|error| panic!("{error}")),
    );

    let error = build_form(&input).expect_err("未配置映射的选项必须失败");
    match error {
        ConvertError::UnmappedOption { widget_id, label } => {
            assert_eq!(widget_id, "w_multi");
            assert_eq!(label, "未映射");
        }
        other => panic!("期望 UnmappedOption，实际 {other:?}"),
    }
}
