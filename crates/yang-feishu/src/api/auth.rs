//! `auth` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// Create 商店应用获取 app_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=auth&resource=app_access_token&version=v3>
    /// 无鉴权；请求体需自带应用凭证。
    pub async fn auth_app_access_token_create_public(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/auth/v3/app_access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_public(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Internal 自建应用获取 app_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=internal&project=auth&resource=app_access_token&version=v3>
    /// 无鉴权；请求体需自带应用凭证。
    pub async fn auth_app_access_token_internal_public(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/auth/v3/app_access_token/internal".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_public(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Resend 重新获取 app_ticket
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=resend&project=auth&resource=app_ticket&version=v3>
    /// 无鉴权；请求体需自带应用凭证。
    pub async fn auth_app_ticket_resend_public(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/auth/v3/app_ticket/resend".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_public(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 商店应用获取 tenant_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=auth&resource=tenant_access_token&version=v3>
    /// 无鉴权；请求体需自带应用凭证。
    pub async fn auth_tenant_access_token_create_public(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/auth/v3/tenant_access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_public(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Internal 自建应用获取 tenant_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=internal&project=auth&resource=tenant_access_token&version=v3>
    /// 无鉴权；请求体需自带应用凭证。
    pub async fn auth_tenant_access_token_internal_public(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/auth/v3/tenant_access_token/internal".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_public(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
