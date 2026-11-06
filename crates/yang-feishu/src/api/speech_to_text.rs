//! `speech_to_text` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// FileRecognize 识别语音文件 (ASR)
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=file_recognize&project=speech_to_text&resource=speech&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn speech_to_text_speech_file_recognize(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/speech_to_text/v1/speech/file_recognize".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// StreamRecognize 识别流式语音 (ASR)
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=stream_recognize&project=speech_to_text&resource=speech&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn speech_to_text_speech_stream_recognize(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/speech_to_text/v1/speech/stream_recognize".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
