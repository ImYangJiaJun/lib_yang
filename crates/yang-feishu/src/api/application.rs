//! `application` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// 获取用户可用应用
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_user_visible_apps(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v1/user/visible_apps".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取应用可用范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_app_visibility(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v2/app/visibility".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 判断用户是否为应用管理员
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_is_user_admin(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v3/is_user_admin".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Favourite 获取用户自定义常用的应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=favourite&project=application&resource=application&version=v5>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_favourite_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v5/applications/favourite".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Recommend 获取管理员推荐的应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=recommend&project=application&resource=application&version=v5>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_recommend_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v5/applications/recommend".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Set 更新应用红点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=set&project=application&resource=app_badge&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_app_badge_set(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/app_badge/set".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取当前设置的推荐规则列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=application&resource=app_recommend_rule&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_app_recommend_rules_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/app_recommend_rules".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取企业安装的应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=application&resource=application&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/applications".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取应用信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=application&resource=application&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_get(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新应用分组信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// DepartmentOverview 获取多部门应用使用概览
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=department_overview&project=application&resource=application.app_usage&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_usage_department_overview(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/application/v6/applications/{app_id}/app_usage/department_overview"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// MessagePushOverview 获取消息推送概览
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=message_push_overview&project=application&resource=application.app_usage&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_usage_message_push_overview(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/application/v6/applications/{app_id}/app_usage/message_push_overview"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Overview 获取应用使用概览
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=overview&project=application&resource=application.app_usage&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_usage_overview(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/app_usage/overview");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取应用版本列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=application&resource=application.app_version&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_versions_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/app_versions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取应用版本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=application&resource=application.app_version&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_versions_get(
        &self,
        app_id: &str,
        version_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/application/v6/applications/{app_id}/app_versions/{version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新应用审核状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.app_version&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_versions_patch(
        &self,
        app_id: &str,
        version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/application/v6/applications/{app_id}/app_versions/{version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// ContactsRangeSuggest 获取应用版本中开发者申请的通讯录权限范围
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=contacts_range_suggest&project=application&resource=application.app_version&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_app_versions_contacts_range_suggest(
        &self,
        app_id: &str,
        version_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/app_versions/{version_id}/contacts_range_suggest");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取应用协作者列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=application&resource=application.collaborators&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_collaborators_get(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/collaborators");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取应用协作者列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=application&resource=application.collaborators&version=v6>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_collaborators_get_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/collaborators");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新应用协作者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=application&resource=application.collaborators&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_collaborators_update(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/collaborators");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新应用协作者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=application&resource=application.collaborators&version=v6>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_collaborators_update_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/collaborators");
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

    /// Patch 更新应用通讯录权限范围配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.contacts_range&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_contacts_range_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/contacts_range");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// ContactsRangeConfiguration 获取应用通讯录权限范围配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=contacts_range_configuration&project=application&resource=application&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_contacts_range_configuration(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/application/v6/applications/{app_id}/contacts_range_configuration");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取应用反馈列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=application&resource=application.feedback&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_feedbacks_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/feedbacks");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新应用反馈
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.feedback&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_feedbacks_patch(
        &self,
        app_id: &str,
        feedback_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("feedback_id", feedback_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/application/v6/applications/{app_id}/feedbacks/{feedback_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 启停用应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=application&resource=application.management&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_management_update(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/management");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 转移应用所有者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=application&resource=application.owner&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_owner_update(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/owner");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 转移应用所有者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=application&resource=application.owner&version=v6>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_owner_update_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/owner");
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

    /// Patch 更新应用可用范围
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.visibility&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_visibility_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v6/applications/{app_id}/visibility");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// CheckWhiteBlackList 查询用户或部门是否在应用的可用或禁用名单
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=check_white_black_list&project=application&resource=application.visibility&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_visibility_check_white_black_list(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/application/v6/applications/{app_id}/visibility/check_white_black_list"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Underauditlist 查看待审核的应用列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=underauditlist&project=application&resource=application&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_underauditlist(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/applications/underauditlist".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询租户授权状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=application&resource=scope&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_scopes_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/scopes".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Apply 向管理员申请授权
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=apply&project=application&resource=scope&version=v6>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_scopes_apply(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v6/scopes/apply".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 上传应用图标
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=application&resource=app_avatar.upload&version=v7>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn application_app_avatar_upload_create(
        &self,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v7/app_avatar/upload".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// Create 上传应用图标
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=application&resource=app_avatar.upload&version=v7>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn application_app_avatar_upload_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/application/v7/app_avatar/upload".to_string();
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

    /// Patch 更新应用能力
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.ability&version=v7>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_ability_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/ability");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新应用能力
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.ability&version=v7>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_ability_patch_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/ability");
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

    /// Patch 更新应用基础信息配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.base&version=v7>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_base_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/base");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新应用基础信息配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.base&version=v7>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_base_patch_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/base");
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

    /// Patch 更新应用开发配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.config&version=v7>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_config_patch(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/config");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新应用开发配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=application&resource=application.config&version=v7>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_config_patch_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/config");
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

    /// Create 提交发布自建应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=application&resource=application.publish&version=v7>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn application_applications_publish_create(
        &self,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/publish");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 提交发布自建应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=application&resource=application.publish&version=v7>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn application_applications_publish_create_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/application/v7/applications/{app_id}/publish");
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
