//! `translation` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// Detect 识别文本语种
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=detect&project=translation&resource=text&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn translation_text_detect(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/translation/v1/text/detect".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Translate 翻译文本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=translate&project=translation&resource=text&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn translation_text_translate(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/translation/v1/text/translate".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
