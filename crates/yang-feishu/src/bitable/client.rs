//! 多维表格读写客户端。
//!
//! 失败语义与宿主保持一致：
//! - `list_all_*` / `list_all_records`：任何一页失败或未收敛都返回 Err；
//! - `search_all_records`：不套收敛断言，只靠 `max_pages` 防失控；
//! - `record_row_numbers`：尽力而为，翻页结束/超上限/缺游标都返回已找到的部分。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use serde::Deserialize;

use crate::client::ClientCore;
use crate::error::{Error, FailureKind, Result};
use crate::path::validate_path_segment;
use crate::transport::{Method, Request, Response, PULL_REQUEST_TIMEOUT_SECS, PULL_RETRY};

use super::dto::{
    BatchGetRecordsData, BitableTableItem, BitableTablesData, BitableViewItem, BitableViewsData,
    FieldItem, ListFieldsData, ListRecordsData, RecordItem, SearchRecordsData,
};
use super::link::BitableCoordinates;
use super::pagination::{PaginationState, RecordsSnapshot};
use super::{MAX_PAGES, PAGE_SIZE, PAGE_SIZE_FIELDS, SEARCH_PAGE_SIZE};

/// 一条待回写的记录：记录 id + 要写的单元格。
pub type UpdateRow = (String, BTreeMap<String, serde_json::Value>);

fn path_error(name: &str, value: &str) -> Result<()> {
    validate_path_segment(name, value).map_err(|error| Error::fatal(error.to_string()))
}

/// 组装记录列表 URL。
pub fn records_url(app_token: &str, table_id: &str) -> Result<String> {
    path_error("bitable_base_token", app_token)?;
    path_error("bitable_table_id", table_id)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 组装字段列表 URL。
pub fn fields_url(app_token: &str, table_id: &str) -> Result<String> {
    path_error("bitable_base_token", app_token)?;
    path_error("bitable_table_id", table_id)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/fields",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 组装视图列表 URL。
pub fn views_url(app_token: &str, table_id: &str) -> Result<String> {
    path_error("bitable_base_token", app_token)?;
    path_error("bitable_table_id", table_id)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 组装数据表列表 URL。
pub fn tables_url(app_token: &str) -> Result<String> {
    path_error("bitable_base_token", app_token)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 组装查询记录 URL。
pub fn search_records_url(app_token: &str, table_id: &str) -> Result<String> {
    path_error("app_token", app_token)?;
    path_error("table_id", table_id)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/search",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 组装批量回写 URL。
pub fn batch_update_records_url(app_token: &str, table_id: &str) -> Result<String> {
    path_error("app_token", app_token)?;
    path_error("table_id", table_id)?;
    Ok(format!(
        "{}/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_update",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 《列出记录》的查询参数。`field_names` 是单个 JSON 数组字符串，不是重复参数。
pub fn list_records_query(
    field_names: &[String],
    view_id: Option<&str>,
    page_token: Option<&str>,
    page_size: u32,
) -> Result<Vec<(String, String)>> {
    if page_size == 0 {
        return Err(Error::fatal("page_size 必须为正数"));
    }
    let mut query = vec![
        ("page_size".to_string(), page_size.to_string()),
        // 多行文本保持字符串返回；置 true 会变成 `[{text,type}]`。
        ("text_field_as_array".to_string(), "false".to_string()),
    ];
    if !field_names.is_empty() {
        let encoded = serde_json::to_string(field_names)
            .map_err(|error| Error::fatal(format!("序列化 field_names 失败: {error}")))?;
        query.push(("field_names".to_string(), encoded));
    }
    if let Some(view_id) = view_id {
        path_error("bitable_view_id", view_id)?;
        query.push(("view_id".to_string(), view_id.trim().to_string()));
    }
    if let Some(page_token) = page_token {
        query.push(("page_token".to_string(), page_token.to_string()));
    }
    Ok(query)
}

/// 每页 100 条的列表查询参数（字段/数据表/视图共用）。
pub fn list_page_query(page_token: Option<&str>) -> Vec<(String, String)> {
    let mut query = vec![("page_size".to_string(), PAGE_SIZE_FIELDS.to_string())];
    if let Some(page_token) = page_token {
        query.push(("page_token".to_string(), page_token.to_string()));
    }
    query
}

/// 信封：`data` 用 `Value` 承载，避免 `#[serde(default)]` 引入 `T: Default` 约束。
#[derive(Debug, Deserialize)]
struct ApiEnvelope {
    code: i32,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

/// 解析信封并取出 `data`。
fn decode_envelope<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T> {
    let envelope: ApiEnvelope = serde_json::from_str(&response.body).map_err(|error| {
        Error::fatal(format!(
            "解析飞书响应失败: {error}；原文: {}",
            crate::transport::summarize(response.status, &response.body)
        ))
    })?;
    let data = envelope.data.ok_or_else(|| {
        Error::new(
            FailureKind::Fatal {
                code: envelope.code,
            },
            format!("飞书返回 code=0 但缺少 data（msg={}）", envelope.msg),
        )
    })?;
    serde_json::from_value(data)
        .map_err(|error| Error::fatal(format!("解析飞书 data 失败: {error}")))
}

fn with_base(mut url: String, base: &str) -> String {
    if let Some(rest) = url.strip_prefix(crate::FEISHU_OPEN_BASE) {
        url = format!("{base}{rest}");
    }
    url
}

/// 多维表格客户端。
#[derive(Clone)]
pub struct BitableClient {
    pub(crate) core: Arc<ClientCore>,
}

impl BitableClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }

    fn open_base(&self) -> String {
        self.core.endpoints().open.as_str().to_string()
    }

    /// 通用：带 tenant token 的请求 + 信封解码。
    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        url: String,
        query: Vec<(String, String)>,
        body: Option<serde_json::Value>,
        timeout_secs: u64,
    ) -> Result<T> {
        let response = self
            .core
            .send_tenant(
                Request {
                    method,
                    url: with_base(url, &self.open_base()),
                    query,
                    bearer_token: None,
                    json_body: body,
                    raw_body: None,
                    binary_limit: None,
                    timeout_secs: Some(timeout_secs),
                    // 幂等：写是回填同值覆盖，读是只读；真正的幂等在 create 的 uuid 上。
                    idempotent: true,
                },
                &PULL_RETRY,
            )
            .await?;
        decode_envelope(response)
    }

    async fn fetch_page<T: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        query: Vec<(String, String)>,
    ) -> Result<T> {
        self.call(Method::Get, url, query, None, PULL_REQUEST_TIMEOUT_SECS)
            .await
    }

    /// 读取 Base 展示名称（不读记录内容）。
    pub async fn get_base_name(&self, app_token: &str) -> Result<Option<String>> {
        if let Err(error) = validate_path_segment("bitable_base_token", app_token) {
            return Err(Error::new(
                FailureKind::Fatal { code: -1 },
                error.to_string(),
            ));
        }
        let url = format!("{}/open-apis/bitable/v1/apps/{app_token}", self.open_base());
        let data: serde_json::Value = self.fetch_page(url, Vec::new()).await?;
        Ok(data
            .get("app")
            .and_then(|app| app.get("name"))
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string))
    }

    /// 只取第一页记录（诊断用，不做收敛断言）。
    pub async fn first_records_page(
        &self,
        app_token: &str,
        table_id: &str,
        view_id: Option<&str>,
        field_names: Option<&[&str]>,
        page_size: u32,
    ) -> Result<ListRecordsData> {
        let url = records_url(app_token, table_id)?;
        let names = owned_names(field_names);
        let query = list_records_query(&names, view_id, None, page_size)?;
        self.fetch_page(url, query).await
    }

    /// 拉取全量记录；成功即返回**已收敛**的快照。
    pub async fn list_all_records(
        &self,
        app_token: &str,
        table_id: &str,
        view_id: Option<&str>,
        field_names: Option<&[&str]>,
    ) -> Result<RecordsSnapshot> {
        let coordinates = BitableCoordinates {
            app_token: app_token.to_string(),
            table_id: table_id.to_string(),
            view_id: view_id.map(str::to_string),
        };
        self.list_all_records_at(&coordinates, &owned_names(field_names), MAX_PAGES)
            .await
    }

    /// 以坐标 + 显式页数上限拉取全量记录（宿主按轮次预算调用）。
    pub async fn list_all_records_at(
        &self,
        coordinates: &BitableCoordinates,
        field_names: &[String],
        max_pages: u32,
    ) -> Result<RecordsSnapshot> {
        let url = records_url(&coordinates.app_token, &coordinates.table_id)?;
        let view_id = coordinates.view_id.as_deref();

        let mut state = PaginationState::new(max_pages);
        let mut cursor: Option<String> = None;
        let mut items: Vec<RecordItem> = Vec::new();

        loop {
            let query = list_records_query(field_names, view_id, cursor.as_deref(), PAGE_SIZE)?;
            let page: ListRecordsData = self.fetch_page(url.clone(), query).await?;
            let next = state.accept(
                page.has_more,
                page.page_token.clone(),
                page.total,
                page.items.len(),
            )?;
            items.extend(page.items);
            match next {
                Some(value) => cursor = Some(value),
                None => break,
            }
        }

        state.assert_converged()?;
        Ok(RecordsSnapshot {
            items,
            total: state.first_total().unwrap_or_default(),
        })
    }

    /// 一次扫描取多条记录在当前视图里的行号（1 起，尽力而为）。
    pub async fn record_row_numbers(
        &self,
        app_token: &str,
        table_id: &str,
        record_ids: &[String],
        max_pages: u32,
    ) -> Result<Vec<(String, u64)>> {
        let coordinates = BitableCoordinates {
            app_token: app_token.to_string(),
            table_id: table_id.to_string(),
            view_id: None,
        };
        let found = self
            .record_row_numbers_at(&coordinates, record_ids, max_pages)
            .await?;
        let mut rows: Vec<(String, u64)> = found
            .into_iter()
            .map(|(id, row)| (id, row as u64))
            .collect();
        rows.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        Ok(rows)
    }

    /// 以坐标扫描行号（宿主可直接复用视图筛选）。
    pub async fn record_row_numbers_at(
        &self,
        coordinates: &BitableCoordinates,
        record_ids: &[String],
        max_pages: u32,
    ) -> Result<HashMap<String, usize>> {
        let mut found = HashMap::new();
        if record_ids.is_empty() {
            return Ok(found);
        }
        let targets: HashSet<&str> = record_ids.iter().map(String::as_str).collect();
        let url = records_url(&coordinates.app_token, &coordinates.table_id)?;
        let mut cursor: Option<String> = None;
        let mut row = 0usize;
        for _ in 0..max_pages {
            let query = list_records_query(
                &[],
                coordinates.view_id.as_deref(),
                cursor.as_deref(),
                PAGE_SIZE,
            )?;
            let page: ListRecordsData = self.fetch_page(url.clone(), query).await?;
            for item in &page.items {
                row += 1;
                if targets.contains(item.record_id.as_str()) {
                    found.insert(item.record_id.clone(), row);
                }
            }
            if found.len() == targets.len() {
                return Ok(found);
            }
            if !page.has_more {
                return Ok(found);
            }
            match page.page_token.filter(|token| !token.is_empty()) {
                Some(token) => cursor = Some(token),
                None => return Ok(found),
            }
        }
        Ok(found)
    }

    /// 拉取某张数据表下的全部视图。
    pub async fn list_all_views(
        &self,
        app_token: &str,
        table_id: &str,
    ) -> Result<Vec<BitableViewItem>> {
        let url = views_url(app_token, table_id)?;
        let mut state = PaginationState::new(MAX_PAGES);
        let mut cursor: Option<String> = None;
        let mut items: Vec<BitableViewItem> = Vec::new();
        loop {
            let page: BitableViewsData = self
                .fetch_page(url.clone(), list_page_query(cursor.as_deref()))
                .await?;
            let next = state.accept(
                page.has_more,
                page.page_token.clone(),
                page.total,
                page.items.len(),
            )?;
            items.extend(page.items);
            match next {
                Some(value) => cursor = Some(value),
                None => break,
            }
        }
        state.assert_converged()?;
        Ok(items)
    }

    /// 拉取某张 Base 下的全部数据表。
    pub async fn list_all_tables(&self, app_token: &str) -> Result<Vec<BitableTableItem>> {
        let url = tables_url(app_token)?;
        let mut state = PaginationState::new(MAX_PAGES);
        let mut cursor: Option<String> = None;
        let mut items: Vec<BitableTableItem> = Vec::new();
        loop {
            let page: BitableTablesData = self
                .fetch_page(url.clone(), list_page_query(cursor.as_deref()))
                .await?;
            let next = state.accept(
                page.has_more,
                page.page_token.clone(),
                page.total,
                page.items.len(),
            )?;
            items.extend(page.items);
            match next {
                Some(value) => cursor = Some(value),
                None => break,
            }
        }
        state.assert_converged()?;
        Ok(items)
    }

    /// 拉取全量字段定义。
    pub async fn list_all_fields(&self, app_token: &str, table_id: &str) -> Result<Vec<FieldItem>> {
        let url = fields_url(app_token, table_id)?;
        let mut state = PaginationState::new(MAX_PAGES);
        let mut cursor: Option<String> = None;
        let mut items: Vec<FieldItem> = Vec::new();
        loop {
            let page: ListFieldsData = self
                .fetch_page(url.clone(), list_page_query(cursor.as_deref()))
                .await?;
            let next = state.accept(
                page.has_more,
                page.page_token.clone(),
                page.total,
                page.items.len(),
            )?;
            items.extend(page.items);
            match next {
                Some(value) => cursor = Some(value),
                None => break,
            }
        }
        state.assert_converged()?;
        Ok(items)
    }

    /// 按条件查询记录（单页，不做收敛断言）。
    pub async fn search_records(
        &self,
        app_token: &str,
        table_id: &str,
        body: serde_json::Value,
        page_token: Option<&str>,
    ) -> Result<SearchRecordsData> {
        let url = search_records_url(app_token, table_id)?;
        let mut query = vec![("page_size".to_string(), SEARCH_PAGE_SIZE.to_string())];
        if let Some(page_token) = page_token.filter(|token| !token.is_empty()) {
            query.push(("page_token".to_string(), page_token.to_string()));
        }
        self.call(
            Method::Post,
            url,
            query,
            Some(body),
            PULL_REQUEST_TIMEOUT_SECS,
        )
        .await
    }

    /// 翻页取**全部**命中的记录。`max_pages` 是防失控上限，不是分页参数。
    pub async fn search_all_records(
        &self,
        app_token: &str,
        table_id: &str,
        body: serde_json::Value,
        max_pages: u32,
    ) -> Result<Vec<RecordItem>> {
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..max_pages {
            let page = self
                .search_records(app_token, table_id, body.clone(), cursor.as_deref())
                .await?;
            items.extend(page.items);
            if !page.has_more {
                return Ok(items);
            }
            match page.page_token.filter(|token| !token.is_empty()) {
                Some(token) => cursor = Some(token),
                None => return Err(Error::fatal("查询记录返回 has_more=true 却没有 page_token")),
            }
        }
        Err(Error::fatal(format!(
            "查询记录翻页超过 {max_pages} 页仍未收敛"
        )))
    }

    /// 批量回写记录的单元格（全有全无；拆批与折半重试留在调用方）。
    pub async fn batch_update_records(
        &self,
        app_token: &str,
        table_id: &str,
        rows: &[UpdateRow],
    ) -> Result<()> {
        let url = batch_update_records_url(app_token, table_id)?;
        let records: Vec<serde_json::Value> = rows
            .iter()
            .map(|(record_id, fields)| {
                serde_json::json!({
                    "record_id": record_id,
                    "fields": fields,
                })
            })
            .collect();
        let body = serde_json::json!({ "records": records });
        let _: serde_json::Value = self
            .call(
                Method::Post,
                url,
                Vec::new(),
                Some(body),
                PULL_REQUEST_TIMEOUT_SECS,
            )
            .await?;
        Ok(())
    }

    /// 按 `record_id` 列表取记录（`records/batch_get`，单次 1~100 条）。
    pub async fn get_records_by_ids(
        &self,
        app_token: &str,
        table_id: &str,
        record_ids: &[String],
    ) -> Result<Vec<RecordItem>> {
        if record_ids.is_empty() {
            return Err(Error::fatal("record_ids 不能为空"));
        }
        if record_ids.len() > 100 {
            return Err(Error::fatal(format!(
                "record_ids 单次最多 100 条，实际 {}",
                record_ids.len()
            )));
        }
        if record_ids.iter().any(|id| id.trim().is_empty()) {
            return Err(Error::fatal("record_ids 不得含空值"));
        }

        let url = format!("{}/batch_get", records_url(app_token, table_id)?);
        let body = serde_json::json!({
            "record_ids": record_ids,
            "user_id_type": "open_id",
            "automatic_fields": true,
        });
        let data: BatchGetRecordsData = self
            .call(
                Method::Post,
                url,
                Vec::new(),
                Some(body),
                PULL_REQUEST_TIMEOUT_SECS,
            )
            .await?;

        if let Some(message) =
            describe_unfetched(&data.forbidden_record_ids, &data.absent_record_ids)
        {
            return Err(Error::fatal(message));
        }
        Ok(data.records)
    }

    /// 只为消息补充原记录入口；不修改记录与分享设置。
    ///
    /// 超时固定 2 秒：与其它路径的 20 秒不同，改它会改变线上行为。
    pub async fn get_record_shared_url(
        &self,
        coordinates: &BitableCoordinates,
        record_id: &str,
    ) -> Result<Option<String>> {
        let url = (|| -> Result<String> {
            validate_path_segment("record_id", record_id)
                .map_err(|error| Error::fatal(error.to_string()))?;
            Ok(format!(
                "{}/{}",
                records_url(&coordinates.app_token, &coordinates.table_id)?,
                record_id
            ))
        })()
        .map_err(|_| Error::fatal("原记录链接坐标无效"))?;
        let data: serde_json::Value = self
            .call(
                Method::Get,
                url,
                vec![
                    ("with_shared_url".into(), "true".into()),
                    ("automatic_fields".into(), "false".into()),
                ],
                None,
                2,
            )
            .await?;
        let record = &data["record"];
        if record["record_id"].as_str() != Some(record_id) {
            return Ok(None);
        }
        Ok(record["record_url"]
            .as_str()
            .filter(|url| super::link::is_document_link(url))
            .map(str::to_owned))
    }
}

fn owned_names(field_names: Option<&[&str]>) -> Vec<String> {
    field_names
        .map(|names| names.iter().map(|name| (*name).to_string()).collect())
        .unwrap_or_default()
}

/// `absent` 与 `forbidden` 的处置完全不同，必须分开写文案。
fn describe_unfetched(forbidden: &[String], absent: &[String]) -> Option<String> {
    let mut parts = Vec::new();
    if !forbidden.is_empty() {
        parts.push(format!("应用无权读取该记录（高级权限拒绝）：{forbidden:?}"));
    }
    if !absent.is_empty() {
        parts.push(format!("记录不存在（可能已被删除）：{absent:?}"));
    }
    (!parts.is_empty()).then(|| parts.join("；"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_builders_keep_the_documented_shapes() {
        let query = list_records_query(&["A".into(), "B".into()], Some("view1"), None, 500)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(query[0], ("page_size".into(), "500".into()));
        assert_eq!(query[1], ("text_field_as_array".into(), "false".into()));
        assert_eq!(query[2].0, "field_names");
        assert_eq!(query[2].1, r#"["A","B"]"#);
        assert_eq!(query[3], ("view_id".into(), "view1".into()));
        assert!(!query.iter().any(|(key, _)| key == "offset"));
    }

    #[test]
    fn page_size_zero_is_rejected() {
        assert!(list_records_query(&[], None, None, 0).is_err());
    }

    #[test]
    fn urls_validate_path_segments() {
        assert!(records_url("app", "../evil")
            .unwrap_err()
            .to_string()
            .contains("只能包含"));
        assert!(tables_url("app").is_ok());
        assert!(views_url("app", "tbl").is_ok());
        assert!(search_records_url("app", "tbl").is_ok());
        assert!(batch_update_records_url("app", "tbl").is_ok());
    }

    #[test]
    fn unfetched_reasons_stay_separate() {
        assert_eq!(describe_unfetched(&[], &[]), None);
        let message = describe_unfetched(&[], &["recGone".into()]).unwrap_or_default();
        assert!(message.contains("记录不存在") && message.contains("recGone"));
        let message = describe_unfetched(&["recDenied".into()], &[]).unwrap_or_default();
        assert!(message.contains("无权读取") && message.contains("recDenied"));
    }
}
