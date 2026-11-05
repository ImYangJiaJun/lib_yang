//! 审批官方 v4 端点薄封装（自动生成，路径以官方文档为准）。

use serde_json::Value;

use crate::error::Result;
use crate::transport::Method;

use super::ApprovalClient;

impl ApprovalClient {
    /// 撤回审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/instance/cancel.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:instance`
    pub async fn cancel_instance(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/cancel".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 抄送审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/instance/cc.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:instance`
    pub async fn cc_instance(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/cc".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 按条件查询审批实例列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/approval-search/query-2.md>
    ///
    /// Scope: `approval:approval.list:readonly`
    pub async fn query_instances(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 获取用户已发起的审批列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/approval-v4/instance/initiated.md>
    ///
    /// Scope: `approval:instance:read`
    pub async fn list_initiated_instances(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/initiated".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 搜索当前用户可发起的审批定义
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/approval-v4/approval/search_launchable.md>
    ///
    /// Scope: `approval:approval:read`
    pub async fn search_launchable_approvals(
        &self,
        bearer_token: &str,
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/approval/v4/approvals/search_launchable".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 同意审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/task/approve.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:task`
    pub async fn approve_task(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/tasks/approve".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 拒绝审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/task/reject.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:task`
    pub async fn reject_task(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/tasks/reject".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 审批任务加签
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/task/approval-task-addsign.md>
    ///
    /// Scope: `approval:approval:readonly`
    pub async fn add_sign_task(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/add_sign".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 退回审批任务至已审批节点
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/task/specified_rollback.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:task`
    pub async fn rollback_task(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/specified_rollback".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 转交审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/task/transfer.md>
    ///
    /// Scope: `approval:approval, approval:approval:readonly, approval:task`
    pub async fn transfer_task(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/tasks/transfer".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 按条件查询审批任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/approval-search/search.md>
    ///
    /// Scope: `approval:approval:readonly, approval:approval.list:readonly`
    pub async fn search_tasks(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/tasks/search".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 查询用户的任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/approval-v4/approval-search/query.md>
    ///
    /// Scope: `approval:approval:readonly`
    pub async fn query_user_tasks(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/approval/v4/tasks/query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 发送审批催办消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/approval-v4/instance/remind.md>
    ///
    /// Scope: `approval:instance:write`
    pub async fn remind_instance(&self, bearer_token: &str, body: &Value) -> Result<Value> {
        let path = "/open-apis/approval/v4/instances/remind".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }
}
