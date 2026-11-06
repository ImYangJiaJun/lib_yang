//! `minutes` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Get 获取单个妙记信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn minutes_minutes_get(
        &self,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取单个妙记信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_get_user(
        &self,
        bearer_token: &str,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Artifacts 获取妙记AI产物
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=artifacts&project=minutes&resource=minute&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn minutes_minutes_artifacts(
        &self,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/artifacts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Artifacts 获取妙记AI产物
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=artifacts&project=minutes&resource=minute&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_artifacts_user(
        &self,
        bearer_token: &str,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/artifacts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 下载妙记音视频文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn minutes_minutes_media_get(
        &self,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/media");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 下载妙记音视频文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_media_get_user(
        &self,
        bearer_token: &str,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/media");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取妙记统计数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.statistics&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn minutes_minutes_statistics_get(
        &self,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/statistics");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取妙记统计数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.statistics&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_statistics_get_user(
        &self,
        bearer_token: &str,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/statistics");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 导出妙记文字记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.transcript&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn minutes_minutes_transcript_get(
        &self,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/transcript");
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

    /// Get 导出妙记文字记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=minutes&resource=minute.transcript&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn minutes_minutes_transcript_get_user(
        &self,
        bearer_token: &str,
        minute_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("minute_token", minute_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/minutes/v1/minutes/{minute_token}/transcript");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        let response = self
            .core
            .send_raw(
                crate::transport::Request {
                    method: Method::Get,
                    url,
                    query,
                    bearer_token: Some(bearer_token.to_string()),
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

    /// Search 搜索妙记
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=minutes&resource=minute&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn minutes_minutes_search(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/minutes/v1/minutes/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 搜索妙记
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=minutes&resource=minute&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_search_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/minutes/v1/minutes/search".to_string();
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

    /// Subscription 订阅妙记变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=minutes&resource=minute&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/minutes/v1/minutes/subscription".to_string();
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

    /// Unsubscription 取消订阅妙记变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=minutes&resource=minute&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn minutes_minutes_unsubscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/minutes/v1/minutes/unsubscription".to_string();
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
