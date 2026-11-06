//! `trust_party` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 获取可见关联组织的列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn trust_party_collaboration_tenants_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/trust_party/v1/collaboration_tenants".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取可见关联组织的列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn trust_party_collaboration_tenants_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/trust_party/v1/collaboration_tenants".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn trust_party_collaboration_tenants_get(
        &self,
        target_tenant_key: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn trust_party_collaboration_tenants_get_user(
        &self,
        bearer_token: &str,
        target_tenant_key: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织部门详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant.collaboration_department&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn trust_party_collaboration_tenants_collaboration_departments_get(
        &self,
        target_tenant_key: &str,
        target_department_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("target_department_id", target_department_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/collaboration_departments/{target_department_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织部门详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant.collaboration_department&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn trust_party_collaboration_tenants_collaboration_departments_get_user(
        &self,
        bearer_token: &str,
        target_tenant_key: &str,
        target_department_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("target_department_id", target_department_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/collaboration_departments/{target_department_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织成员详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant.collaboration_user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn trust_party_collaboration_tenants_collaboration_users_get(
        &self,
        target_tenant_key: &str,
        target_user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("target_user_id", target_user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/collaboration_users/{target_user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取关联组织成员详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=trust_party&resource=collaboration_tenant.collaboration_user&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn trust_party_collaboration_tenants_collaboration_users_get_user(
        &self,
        bearer_token: &str,
        target_tenant_key: &str,
        target_user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("target_user_id", target_user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/collaboration_users/{target_user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// VisibleOrganization 获取关联组织的成员信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=visible_organization&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn trust_party_collaboration_tenants_visible_organization(
        &self,
        target_tenant_key: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/visible_organization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// VisibleOrganization 获取关联组织的成员信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=visible_organization&project=trust_party&resource=collaboration_tenant&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn trust_party_collaboration_tenants_visible_organization_user(
        &self,
        bearer_token: &str,
        target_tenant_key: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("target_tenant_key", target_tenant_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/trust_party/v1/collaboration_tenants/{target_tenant_key}/visible_organization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }
}
