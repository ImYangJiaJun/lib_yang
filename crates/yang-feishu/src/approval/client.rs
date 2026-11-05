//! 飞书审批 v4 客户端：建单、查单、取定义、订阅事件。

use std::sync::Arc;

use serde::Deserialize;

use crate::client::ClientCore;
use crate::error::{Error, FailureKind, Result};
use crate::path::validate_opaque_path_segment;
use crate::transport::{decode_data, Method, Request, APPROVAL_REQUEST_TIMEOUT_SECS, PULL_RETRY};

/// 创建实例的 URL。
pub fn create_instance_url() -> String {
    format!(
        "{}/open-apis/approval/v4/instances",
        crate::FEISHU_OPEN_BASE
    )
}

/// 实例详情 URL；`instance_id` 可以是 `instance_code` 或创建时的 `uuid`。
pub fn instance_detail_url(instance_id: &str) -> Result<String> {
    validate_opaque_path_segment("instance_id", instance_id)
        .map_err(|error| Error::fatal(error.to_string()))?;
    Ok(format!(
        "{}/open-apis/approval/v4/instances/{instance_id}",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 审批定义详情 URL。
pub fn approval_definition_url(approval_code: &str) -> Result<String> {
    validate_opaque_path_segment("approval_code", approval_code)
        .map_err(|error| Error::fatal(error.to_string()))?;
    Ok(format!(
        "{}/open-apis/approval/v4/approvals/{approval_code}",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 审批定义事件订阅 URL。
pub fn subscribe_url(approval_code: &str) -> Result<String> {
    validate_opaque_path_segment("approval_code", approval_code)
        .map_err(|error| Error::fatal(error.to_string()))?;
    Ok(format!(
        "{}/open-apis/approval/v4/approvals/{approval_code}/subscribe",
        crate::FEISHU_OPEN_BASE
    ))
}

fn with_base(mut url: String, base: &str) -> String {
    if let Some(rest) = url.strip_prefix(crate::FEISHU_OPEN_BASE) {
        url = format!("{base}{rest}");
    }
    url
}

/// 创建实例的结果。`UuidConflict` 不是失败，是「按 uuid 反查」的信号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    /// 创建成功。
    Created {
        /// 审批实例 code。
        instance_code: String,
    },
    /// uuid 已被占用（60012）：实例此前已建成功，需按 uuid 反查。
    UuidConflict,
}

/// 实例详情里本模块关心的字段（以 bot 形态为准，另一形态缺省为 None）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstanceDetail {
    /// 实例 code。
    pub instance_code: String,
    /// 审批定义 code。
    pub approval_code: Option<String>,
    /// 审批单编号。
    pub serial_number: Option<String>,
    /// 实例状态。
    pub status: Option<String>,
    /// 发起时间（字符串毫秒时间戳）。
    pub start_time: Option<String>,
    /// 完成时间；未完成是字符串 `"0"`。
    pub end_time: Option<String>,
    /// 发起人 open_id。
    pub open_id: Option<String>,
    /// 发起人工号（user 形态此键语义不同）。
    pub user_id: Option<String>,
    /// 发起人部门 ID。
    pub department_id: Option<String>,
    /// 部门名称；**宿主富化值**，不属于 API 响应。
    pub department_name: Option<String>,
    /// 创建时传入的幂等键。
    pub uuid: Option<String>,
    /// 是否被撤销。
    pub reverted: Option<bool>,
    /// 任务/节点列表（bot 形态 `task_list`）。
    pub task_list: Option<Vec<TaskNode>>,
    /// 流程动态（bot 形态 `timeline`）。
    pub timeline: Option<Vec<TimelineRecord>>,
}

/// `task_list[]` 的一个节点。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TaskNode {
    /// 任务 id。
    #[serde(default)]
    pub id: Option<String>,
    /// 节点 id。
    #[serde(default)]
    pub node_id: Option<String>,
    /// 节点名。
    #[serde(default)]
    pub node_name: Option<String>,
    /// 审批人工号。
    #[serde(default)]
    pub user_id: Option<String>,
    /// 审批人 open_id。
    #[serde(default)]
    pub open_id: Option<String>,
    /// 节点状态。
    #[serde(default)]
    pub status: Option<String>,
    /// 节点类型。
    #[serde(default)]
    pub r#type: Option<String>,
    /// 开始时间。
    #[serde(default)]
    pub start_time: Option<String>,
    /// 结束时间。
    #[serde(default)]
    pub end_time: Option<String>,
}

/// `timeline[]` 的一条流程动态。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TimelineRecord {
    /// 事件类型。
    #[serde(default)]
    pub r#type: Option<String>,
    /// 事件时间。
    #[serde(default)]
    pub create_time: Option<String>,
    /// 操作人工号。
    #[serde(default)]
    pub user_id: Option<String>,
    /// 操作人 open_id。
    #[serde(default)]
    pub open_id: Option<String>,
    /// 扩展信息。
    #[serde(default)]
    pub ext: Option<String>,
}

/// 按 `instance_code` 或 `uuid` 取详情的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetOutcome {
    /// 找到实例。
    Found(Box<InstanceDetail>),
    /// 实例不存在（1390003）：调用方区分并发窗口与真实失败。
    NotFound,
}

/// 订阅结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOutcome {
    /// 本次新订阅。
    Subscribed,
    /// 已订阅或已取消（1390007），按幂等成功处理。
    AlreadySubscribed,
}

/// 审批定义里本模块关心的部分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDefinition {
    /// 审批名称。
    pub approval_name: String,
    /// 控件结构快照（原始 JSON）。
    pub form: Option<serde_json::Value>,
    /// 是否三方审批定义。
    pub is_external: bool,
}

#[derive(Debug, Deserialize)]
struct CreateInstanceData {
    instance_code: String,
}

#[derive(Debug, Deserialize)]
struct InstanceDetailData {
    instance_code: String,
    #[serde(default)]
    approval_code: Option<String>,
    #[serde(default)]
    serial_number: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
    #[serde(default)]
    open_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    department_id: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    reverted: Option<bool>,
    #[serde(default)]
    task_list: Option<Vec<TaskNode>>,
    #[serde(default)]
    timeline: Option<Vec<TimelineRecord>>,
}

#[derive(Debug, Deserialize)]
struct ApprovalDefinitionData {
    approval_name: String,
    #[serde(default)]
    form: Option<serde_json::Value>,
    #[serde(default)]
    is_external: Option<bool>,
}

/// 审批客户端。
#[derive(Clone)]
pub struct ApprovalClient {
    pub(crate) core: Arc<ClientCore>,
}

impl ApprovalClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }

    fn open_base(&self) -> String {
        self.core.endpoints().open.as_str().to_string()
    }

    async fn send(&self, request: Request) -> Result<String> {
        let request = Request {
            url: with_base(request.url, &self.open_base()),
            ..request
        };
        let response = self.core.send_tenant(request, &PULL_RETRY).await?;
        decode_data(&response.body)
    }

    /// 创建审批实例。`form` 必须是压缩后的 JSON 数组字符串。
    pub async fn create_instance(
        &self,
        approval_code: &str,
        form: &str,
        open_id: &str,
        uuid: &str,
    ) -> Result<CreateOutcome> {
        let body = serde_json::json!({
            "approval_code": approval_code,
            "open_id": open_id,
            "form": form,
            "uuid": uuid,
        });
        let request = Request {
            method: Method::Post,
            url: create_instance_url(),
            query: Vec::new(),
            bearer_token: None,
            json_body: Some(body),
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(APPROVAL_REQUEST_TIMEOUT_SECS),
            // uuid 的服务端幂等是唯一依据；不能因「POST 有副作用」改成 false。
            idempotent: true,
        };
        match self.send(request).await {
            Ok(response) => {
                let data: CreateInstanceData = serde_json::from_str(&response)
                    .map_err(|error| Error::fatal(format!("创建审批实例响应解析失败: {error}")))?;
                Ok(CreateOutcome::Created {
                    instance_code: data.instance_code,
                })
            }
            Err(error) if matches!(error.kind, FailureKind::UuidConflict) => {
                Ok(CreateOutcome::UuidConflict)
            }
            Err(error) => Err(error),
        }
    }

    /// 取审批实例详情；`instance_id` 传 uuid 即 60012 冲突后的反查。
    pub async fn get_instance(&self, instance_id: &str) -> Result<GetOutcome> {
        let request = Request {
            method: Method::Get,
            url: instance_detail_url(instance_id)?,
            query: vec![("user_id_type".into(), "open_id".into())],
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(APPROVAL_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        match self.send(request).await {
            Ok(response) => {
                let data: InstanceDetailData = serde_json::from_str(&response)
                    .map_err(|error| Error::fatal(format!("审批实例详情响应解析失败: {error}")))?;
                Ok(GetOutcome::Found(Box::new(InstanceDetail {
                    instance_code: data.instance_code,
                    approval_code: data.approval_code,
                    serial_number: data.serial_number.filter(|value| !value.is_empty()),
                    status: data.status,
                    start_time: data.start_time,
                    end_time: data.end_time,
                    open_id: data.open_id,
                    user_id: data.user_id,
                    department_id: data.department_id,
                    department_name: None,
                    uuid: data.uuid,
                    reverted: data.reverted,
                    task_list: data.task_list,
                    timeline: data.timeline,
                })))
            }
            Err(error) if matches!(error.kind, FailureKind::InstanceNotFound) => {
                Ok(GetOutcome::NotFound)
            }
            Err(error) => Err(error),
        }
    }

    /// 取审批定义详情。
    pub async fn get_approval_definition(&self, approval_code: &str) -> Result<ApprovalDefinition> {
        let request = Request {
            method: Method::Get,
            url: approval_definition_url(approval_code)?,
            query: Vec::new(),
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(APPROVAL_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        let response = self.send(request).await?;
        let data: ApprovalDefinitionData = serde_json::from_str(&response)
            .map_err(|error| Error::fatal(format!("审批定义响应解析失败: {error}")))?;
        Ok(ApprovalDefinition {
            approval_name: data.approval_name,
            form: data.form,
            is_external: data.is_external.unwrap_or(false),
        })
    }

    /// 订阅审批定义事件；1390007 按幂等成功处理。
    pub async fn subscribe_approval(&self, approval_code: &str) -> Result<SubscribeOutcome> {
        let request = Request {
            method: Method::Post,
            url: subscribe_url(approval_code)?,
            query: Vec::new(),
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(APPROVAL_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        match self.send(request).await {
            Ok(_) => Ok(SubscribeOutcome::Subscribed),
            Err(error)
                if matches!(
                    error.kind,
                    FailureKind::Fatal {
                        code: crate::transport::codes::CODE_SUBSCRIPTION_EXISTED
                    }
                ) =>
            {
                tracing::info!(
                    approval_code,
                    code = crate::transport::codes::CODE_SUBSCRIPTION_EXISTED,
                    "审批定义事件订阅已存在或已取消，按幂等成功处理；若订阅实际已被取消，事件会断流"
                );
                Ok(SubscribeOutcome::AlreadySubscribed)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_official_and_validate_segments() {
        assert!(create_instance_url().ends_with("/open-apis/approval/v4/instances"));
        assert!(instance_detail_url("uuid-1")
            .unwrap_or_default()
            .ends_with("/instances/uuid-1"));
        assert!(instance_detail_url("../evil").is_err());
        assert!(approval_definition_url("code")
            .unwrap_or_default()
            .ends_with("/approvals/code"));
        assert!(subscribe_url("code")
            .unwrap_or_default()
            .ends_with("/approvals/code/subscribe"));
    }
}
