//! `acs` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 获取门禁记录列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=acs&resource=access_record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn acs_access_records_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/access_records".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 下载开门时的人脸识别图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=acs&resource=access_record.access_photo&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn acs_access_records_access_photo_get(
        &self,
        access_record_id: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("access_record_id", access_record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/access_records/{access_record_id}/access_photo");
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

    /// List 获取门禁设备列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=acs&resource=device&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn acs_devices_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/devices".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Delete 删除权限组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=acs&resource=rule_external&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_rule_external_delete_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/rule_external".to_string();
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

    /// Get 获取权限组信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=acs&resource=rule_external&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_rule_external_get_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/rule_external".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建或更新权限组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=acs&resource=rule_external&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_rule_external_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/rule_external".to_string();
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

    /// DeviceBind 设备绑定权限组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=device_bind&project=acs&resource=rule_external&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_rule_external_device_bind_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/rule_external/device_bind".to_string();
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

    /// List 获取用户列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=acs&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn acs_users_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/users".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取单个用户信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=acs&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn acs_users_get(
        &self,
        user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/users/{user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改用户部分信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=acs&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn acs_users_patch(
        &self,
        user_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/users/{user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 下载人脸图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=acs&resource=user.face&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn acs_users_face_get(
        &self,
        user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/users/{user_id}/face");
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

    /// Update 上传人脸图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=acs&resource=user.face&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn acs_users_face_update(
        &self,
        user_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/users/{user_id}/face");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Put, url, query, content_type, body, 20, false)
            .await
    }

    /// Create 添加访客
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=acs&resource=visitor&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_visitors_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/acs/v1/visitors".to_string();
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

    /// Delete 删除访客
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=acs&resource=visitor&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn acs_visitors_delete_user(
        &self,
        bearer_token: &str,
        visitor_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("visitor_id", visitor_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/acs/v1/visitors/{visitor_id}");
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
}
