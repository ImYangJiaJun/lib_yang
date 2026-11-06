//! `helpdesk` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// AgentEmail 获取客服邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=agent_email&project=helpdesk&resource=agent&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agent_emails_agent_email(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_emails".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询全部客服工作日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=agent_schedule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agent_schedules_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_schedules".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建客服工作日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=agent_schedule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agent_schedules_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_schedules".to_string();
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

    /// List 获取客服技能列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=agent_skill_rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agent_skill_rules_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_skill_rules".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询全部客服技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=agent_skill&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agent_skills_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_skills".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建客服技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=agent_skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agent_skills_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/agent_skills".to_string();
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

    /// Delete 删除客服技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=helpdesk&resource=agent_skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agent_skills_delete_user(
        &self,
        bearer_token: &str,
        agent_skill_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_skill_id", agent_skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agent_skills/{agent_skill_id}");
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

    /// Get 查询指定客服技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=agent_skill&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agent_skills_get(
        &self,
        agent_skill_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_skill_id", agent_skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agent_skills/{agent_skill_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新客服技能
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=agent_skill&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agent_skills_patch_user(
        &self,
        bearer_token: &str,
        agent_skill_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_skill_id", agent_skill_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agent_skills/{agent_skill_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Patch 更新客服信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=agent&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agents_patch_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agents/{agent_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Delete 删除客服工作日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=helpdesk&resource=agent.schedules&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agents_schedules_delete_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agents/{agent_id}/schedules");
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

    /// Get 查询指定客服工作日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=agent.schedules&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_agents_schedules_get(
        &self,
        agent_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agents/{agent_id}/schedules");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新客服工作日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=agent.schedules&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_agents_schedules_patch_user(
        &self,
        bearer_token: &str,
        agent_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("agent_id", agent_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/agents/{agent_id}/schedules");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// List 获取全部知识库分类 - meta
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=category&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_categories_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/categories".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建知识库分类
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=category&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_categories_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/categories".to_string();
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

    /// Delete 删除知识库分类详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=helpdesk&resource=category&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_categories_delete_user(
        &self,
        bearer_token: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/categories/{id}");
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

    /// Get 获取知识库分类
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=category&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_categories_get(
        &self,
        id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/categories/{id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新知识库分类详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=category&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_categories_patch_user(
        &self,
        bearer_token: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/categories/{id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// CustomizedFields 获取服务台自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=customized_fields&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_customized_fields(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/customized_fields".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Subscribe 订阅服务台事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=helpdesk&resource=event&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_events_subscribe(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/events/subscribe".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscribe 取消订阅服务台事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscribe&project=helpdesk&resource=event&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_events_unsubscribe(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/events/unsubscribe".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取全部知识库详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=faq&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_faqs_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/faqs".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建知识库
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=faq&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_faqs_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/faqs".to_string();
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

    /// Delete 删除知识库
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=helpdesk&resource=faq&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_faqs_delete_user(
        &self,
        bearer_token: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/faqs/{id}");
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

    /// Get 获取指定知识库详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=faq&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_faqs_get(
        &self,
        id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/faqs/{id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改知识库
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=faq&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_faqs_patch_user(
        &self,
        bearer_token: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/faqs/{id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// FaqImage 获取知识库图像
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=faq_image&project=helpdesk&resource=faq&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn helpdesk_faqs_image_faq_image(
        &self,
        id: &str,
        image_key: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("image_key", image_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/faqs/{id}/image/{image_key}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        let response = self
            .core
            .send_tenant(
                crate::transport::Request {
                    method: Method::Get,
                    url,
                    query,
                    bearer_token: None,
                    json_body: None,
                    raw_body: None,
                    binary_limit: Some(crate::api::MAX_BINARY_BYTES),
                    timeout_secs: Some(20),
                    idempotent: true,
                },
                &crate::transport::PULL_RETRY,
            )
            .await?;
        Ok(response)
    }

    /// Search 搜索知识库
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=helpdesk&resource=faq&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_faqs_search(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/faqs/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 服务台机器人向工单绑定的群内发送消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=bot.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_message_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/message".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/notifications".to_string();
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

    /// Get 查询推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_get_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_patch_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// CancelApprove 取消审核
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel_approve&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_cancel_approve_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}/cancel_approve");
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

    /// CancelSend 取消推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel_send&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_cancel_send_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}/cancel_send");
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

    /// ExecuteSend 执行推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=execute_send&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_execute_send_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}/execute_send");
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

    /// Preview 预览推送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=preview&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_preview_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}/preview");
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

    /// SubmitApprove 提交审核
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=submit_approve&project=helpdesk&resource=notification&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_notifications_submit_approve_user(
        &self,
        bearer_token: &str,
        notification_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("notification_id", notification_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/notifications/{notification_id}/submit_approve");
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

    /// StartService 创建服务台对话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=start_service&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_start_service(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/start_service".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取全部工单自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=ticket_customized_field&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_ticket_customized_fields_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/ticket_customized_fields".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建工单自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=ticket_customized_field&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_ticket_customized_fields_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/ticket_customized_fields".to_string();
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

    /// Delete 删除工单自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=helpdesk&resource=ticket_customized_field&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_ticket_customized_fields_delete_user(
        &self,
        bearer_token: &str,
        ticket_customized_field_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_customized_field_id", ticket_customized_field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/helpdesk/v1/ticket_customized_fields/{ticket_customized_field_id}");
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

    /// Get 获取工单自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=ticket_customized_field&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_ticket_customized_fields_get(
        &self,
        ticket_customized_field_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_customized_field_id", ticket_customized_field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/helpdesk/v1/ticket_customized_fields/{ticket_customized_field_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新工单自定义字段
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=helpdesk&resource=ticket_customized_field&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_ticket_customized_fields_patch_user(
        &self,
        bearer_token: &str,
        ticket_customized_field_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_customized_field_id", ticket_customized_field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/helpdesk/v1/ticket_customized_fields/{ticket_customized_field_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// TicketImage 获取工单内图像
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=ticket_image&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn helpdesk_ticket_images_ticket_image(
        &self,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        let path = "/open-apis/helpdesk/v1/ticket_images".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        let response = self
            .core
            .send_tenant(
                crate::transport::Request {
                    method: Method::Get,
                    url,
                    query,
                    bearer_token: None,
                    json_body: None,
                    raw_body: None,
                    binary_limit: Some(crate::api::MAX_BINARY_BYTES),
                    timeout_secs: Some(20),
                    idempotent: true,
                },
                &crate::transport::PULL_RETRY,
            )
            .await?;
        Ok(response)
    }

    /// List 查询全部工单详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_tickets_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/helpdesk/v1/tickets".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询指定工单详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_tickets_get(
        &self,
        ticket_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_id", ticket_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/tickets/{ticket_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新工单详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn helpdesk_tickets_update_user(
        &self,
        bearer_token: &str,
        ticket_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_id", ticket_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/tickets/{ticket_id}");
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

    /// AnswerUserQuery 回复用户在工单里的提问
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=answer_user_query&project=helpdesk&resource=ticket&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_tickets_answer_user_query(
        &self,
        ticket_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_id", ticket_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/tickets/{ticket_id}/answer_user_query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取工单消息详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=helpdesk&resource=ticket.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_tickets_messages_list(
        &self,
        ticket_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_id", ticket_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/tickets/{ticket_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 发送工单消息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=helpdesk&resource=ticket.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn helpdesk_tickets_messages_create(
        &self,
        ticket_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket_id", ticket_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/helpdesk/v1/tickets/{ticket_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
