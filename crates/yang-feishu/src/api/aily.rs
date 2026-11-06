//! `aily` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Check 获取智能体可见性
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=check&project=aily&resource=agent.agent_visibility&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_agent_visibility_check_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/agent_visibility/check");
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

    /// Get 下载智能体产物
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_artifact&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_artifacts_get(
        &self,
        agent_id: &str,
        agent_artifact_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_artifact_id", agent_artifact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/artifacts/{agent_artifact_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 下载智能体产物
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_artifact&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_artifacts_get_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        agent_artifact_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_artifact_id", agent_artifact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/artifacts/{agent_artifact_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 上传附件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_attachment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn aily_agents_attachments_create(
        &self,
        agent_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/attachments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// Create 上传附件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_attachment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn aily_agents_attachments_create_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/attachments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }

    /// Create 发起智能体对话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_chat&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_chats_create(
        &self,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/chats");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 发起智能体对话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_chat&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_chats_create_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/chats");
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

    /// Get 获取对话结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_chat&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_chats_get(
        &self,
        agent_id: &str,
        agent_chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_id", agent_chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/chats/{agent_chat_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取对话结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_chat&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_chats_get_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        agent_chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_id", agent_chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/chats/{agent_chat_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询会话列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_sessions_list(
        &self,
        agent_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询会话列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_sessions_list_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_sessions_create(
        &self,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_sessions_create_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions");
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

    /// Delete 删除会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_sessions_delete(
        &self,
        agent_id: &str,
        agent_chat_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_session_id", agent_chat_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions/{agent_chat_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_sessions_delete_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        agent_chat_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_session_id", agent_chat_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions/{agent_chat_session_id}");
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

    /// Get 获取指定会话信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_agents_sessions_get(
        &self,
        agent_id: &str,
        agent_chat_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_session_id", agent_chat_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions/{agent_chat_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取指定会话信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=agent.agent_chat_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_agents_sessions_get_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        agent_chat_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("agent_chat_session_id", agent_chat_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/agents/{agent_id}/sessions/{agent_chat_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询应用统计数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=tenant.app_stat&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_app_stats_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/aily/v1/app_stats".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取数据知识分类列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.data_asset_tag&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_data_asset_tags_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_asset_tags");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取数据知识分类列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.data_asset_tag&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_data_asset_tags_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_asset_tags");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取数据知识列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_data_assets_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取数据知识列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_data_assets_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_data_assets_create(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_data_assets_create_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets");
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

    /// Delete 删除数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_data_assets_delete(
        &self,
        app_id: &str,
        data_asset_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("data_asset_id", data_asset_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/{data_asset_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_data_assets_delete_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        data_asset_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("data_asset_id", data_asset_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/{data_asset_id}");
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

    /// Get 获取数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_data_assets_get(
        &self,
        app_id: &str,
        data_asset_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("data_asset_id", data_asset_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/{data_asset_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取数据知识
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_data_assets_get_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        data_asset_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("data_asset_id", data_asset_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/{data_asset_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UploadFile 上传文件用于 Aily 的数据知识管理;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_file&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn aily_apps_data_assets_upload_file(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/upload_file");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// UploadFile 上传文件用于 Aily 的数据知识管理;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_file&project=aily&resource=app.data_asset&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn aily_apps_data_assets_upload_file_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/data_assets/upload_file");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }

    /// Ask 执行数据知识问答
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=ask&project=aily&resource=app.knowledge&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_knowledges_ask(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/knowledges/ask");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Ask 执行数据知识问答
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=ask&project=aily&resource=app.knowledge&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_knowledges_ask_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/knowledges/ask");
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

    /// List 查询技能列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.skill&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_skills_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询技能列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=app.skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_skills_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取技能信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=app.skill&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_skills_get(
        &self,
        app_id: &str,
        skill_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("skill_id", skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills/{skill_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取技能信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=app.skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_skills_get_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        skill_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("skill_id", skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills/{skill_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Start 调用技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=start&project=aily&resource=app.skill&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_apps_skills_start(
        &self,
        app_id: &str,
        skill_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("skill_id", skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills/{skill_id}/start");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Start 调用技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=start&project=aily&resource=app.skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_apps_skills_start_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        skill_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("skill_id", skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/apps/{app_id}/skills/{skill_id}/start");
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

    /// Create 创建会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/aily/v1/sessions".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/aily/v1/sessions".to_string();
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

    /// Delete 删除会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=aily_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_delete(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=aily&resource=aily_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_delete_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
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

    /// Get 获取会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_get(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_get_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=aily&resource=aily_session&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_update(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=aily&resource=aily_session&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_update_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Put,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// List 列出 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_messages_list(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_messages_list_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 发送 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_messages_create(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 发送 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_messages_create_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages");
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

    /// Get 获取 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_messages_get(
        &self,
        aily_session_id: &str,
        aily_message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("aily_message_id", aily_message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages/{aily_message_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取 Aily 消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session.aily_message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_messages_get_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        aily_message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("aily_message_id", aily_message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/aily/v1/sessions/{aily_session_id}/messages/{aily_message_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_runs_list(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_runs_list_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_runs_create(
        &self,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_runs_create_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs");
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

    /// Get 获取运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_runs_get(
        &self,
        aily_session_id: &str,
        run_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("run_id", run_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs/{run_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_runs_get_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        run_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("run_id", run_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs/{run_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Cancel 中止一次运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn aily_sessions_runs_cancel(
        &self,
        aily_session_id: &str,
        run_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("run_id", run_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs/{run_id}/cancel");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cancel 中止一次运行
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=aily&resource=aily_session.run&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn aily_sessions_runs_cancel_user(
        &self,
        bearer_token: &str,
        aily_session_id: &str,
        run_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("aily_session_id", aily_session_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("run_id", run_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/aily/v1/sessions/{aily_session_id}/runs/{run_id}/cancel");
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
}
