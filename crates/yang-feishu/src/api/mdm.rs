//! `mdm` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// Bind 用户数据维度绑定
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=bind&project=mdm&resource=user_auth_data_relation&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mdm_user_auth_data_relations_bind(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mdm/v1/user_auth_data_relations/bind".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unbind 用户数据维度解绑
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unbind&project=mdm&resource=user_auth_data_relation&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mdm_user_auth_data_relations_unbind(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mdm/v1/user_auth_data_relations/unbind".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 通过mdmcode批量查询国家/地区信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mdm&resource=batch_country_region&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mdm_batch_country_region_get(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mdm/v3/batch_country_region".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 分页批量查询国家/地区
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mdm&resource=country_region&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mdm_country_regions_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mdm/v3/country_regions".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
