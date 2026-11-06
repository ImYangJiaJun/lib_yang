//! `approval` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// 发送审批 Bot 消息
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_message_send(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v1/message/send".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 更新审批 Bot 消息
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_message_update(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v1/message/update".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建审批定义
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=approval&resource=approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_approvals_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/approvals".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 查看指定审批定义
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=approval&resource=approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_approvals_get(
        &self,
        approval_code: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_code", approval_code)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/approvals/{approval_code}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Subscribe 订阅审批事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=approval&resource=approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_approvals_subscribe(
        &self,
        approval_code: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_code", approval_code)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/approvals/{approval_code}/subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscribe 取消订阅审批事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscribe&project=approval&resource=approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_approvals_unsubscribe(
        &self,
        approval_code: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_code", approval_code)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/approvals/{approval_code}/unsubscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 查询地理库信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=district&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_districts_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/districts".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询地理库信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=district&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_districts_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/districts".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Search 搜索地理库信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=approval&resource=district&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_districts_search(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/districts/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 搜索地理库信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=approval&resource=district&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_districts_search_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/districts/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Create 同步三方审批定义
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=approval&resource=external_approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_external_approvals_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/external_approvals".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 查看指定三方审批定义
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=approval&resource=external_approval&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_external_approvals_get(
        &self,
        approval_code: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_code", approval_code)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/external_approvals/{approval_code}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 同步三方审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=approval&resource=external_instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_external_instances_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/external_instances".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Check 校验三方审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=check&project=approval&resource=external_instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_external_instances_check(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/external_instances/check".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取三方审批任务状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=external_task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_external_tasks_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/external_tasks".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 批量获取审批实例 ID
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 获取单个审批实例详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_get(
        &self,
        instance_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("instance_id", instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/instances/{instance_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=instance.comment&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_comments_list(
        &self,
        instance_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("instance_id", instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/instances/{instance_id}/comments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=approval&resource=instance.comment&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_comments_create(
        &self,
        instance_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("instance_id", instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/instances/{instance_id}/comments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=approval&resource=instance.comment&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_comments_delete(
        &self,
        instance_id: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("instance_id", instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/instances/{instance_id}/comments/{comment_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Remove 清空评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remove&project=approval&resource=instance.comment&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_comments_remove(
        &self,
        instance_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("instance_id", instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/approval/v4/instances/{instance_id}/comments/remove");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// AddCc 抄送审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=add_cc&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_add_cc_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/add_cc".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// AddSign
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=add_sign&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_add_sign(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/add_sign".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cancel 撤回审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_cancel(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/cancel".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cc 抄送审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cc&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_cc(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/cc".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Detail 获取单个审批实例详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=detail&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_detail_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/detail".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Initiated 获取用户已发起审批列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=initiated&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_initiated_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/initiated".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Preview
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=preview&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_preview(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/preview".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 查询实例列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Recall 撤回审批实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=recall&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_recall_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/recall".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Remind 发送催办消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remind&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_remind_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/remind".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// SearchCc 查询抄送列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search_cc&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_search_cc(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/search_cc".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// SpecifiedRollback 退回审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=specified_rollback&project=approval&resource=instance&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_instances_specified_rollback(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/specified_rollback".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscription 退订审批实例状态变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_subscription_unsubscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Delete,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// Subscription 订阅审批实例状态变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=approval&resource=instance&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_instances_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/instances/subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// List 获取审批任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// AddSign 加签审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=add_sign&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_add_sign_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/add_sign".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Approve 同意审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=approve&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_approve(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/approve".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Forward 转交审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=forward&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_forward_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/forward".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Pass 同意审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=pass&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_pass_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/pass".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Query 查询用户的任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_query(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询用户的任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_query_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Refuse 拒绝审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=refuse&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_refuse_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/refuse".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Reject 拒绝审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reject&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_reject(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/reject".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Resubmit 重新提交审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=resubmit&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_resubmit(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/resubmit".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Rollback 退回审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=rollback&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_rollback_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/rollback".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Search 查询任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_search(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscription 退订审批任务状态变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_subscription_unsubscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Delete,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// Subscription 订阅审批任务状态变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=approval&resource=task&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn approval_tasks_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
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

    /// Transfer 转交审批任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=transfer&project=approval&resource=task&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn approval_tasks_transfer(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/approval/v4/tasks/transfer".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
