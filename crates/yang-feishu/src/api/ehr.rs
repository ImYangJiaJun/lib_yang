//! `ehr` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Get 下载人员的附件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=ehr&resource=attachment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn ehr_attachments_get(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/ehr/v1/attachments/{path_token}");
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

    /// List 批量获取员工花名册信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=ehr&resource=employee&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn ehr_employees_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/ehr/v1/employees".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
