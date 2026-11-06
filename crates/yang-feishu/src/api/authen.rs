//! `authen` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// Create 获取 user_access_token（v1 版本）
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=authen&resource=access_token&version=v1>
    /// 需要 `app_access_token`，由调用方传入 `app_access_token`。
    pub async fn authen_access_token_create_app(
        &self,
        app_access_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v1/access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                app_access_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Create 获取 user_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=authen&resource=oidc.access_token&version=v1>
    /// 需要 `app_access_token`，由调用方传入 `app_access_token`。
    pub async fn authen_oidc_access_token_create_app(
        &self,
        app_access_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v1/oidc/access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                app_access_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Create 刷新 user_access_token
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=authen&resource=oidc.refresh_access_token&version=v1>
    /// 需要 `app_access_token`，由调用方传入 `app_access_token`。
    pub async fn authen_oidc_refresh_access_token_create_app(
        &self,
        app_access_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v1/oidc/refresh_access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                app_access_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Create 刷新 user_access_token（v1 版本）
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=authen&resource=refresh_access_token&version=v1>
    /// 需要 `app_access_token`，由调用方传入 `app_access_token`。
    pub async fn authen_refresh_access_token_create_app(
        &self,
        app_access_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v1/refresh_access_token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                app_access_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// Get 获取用户信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=authen&resource=user_info&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn authen_user_info_get_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v1/user_info".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取 user_access_token（v2）
    ///
    /// 需要 `app_access_token`，由调用方传入 `app_access_token`。
    pub async fn authen_oauth_token_app(
        &self,
        app_access_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/authen/v2/oauth/token".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                app_access_token,
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
