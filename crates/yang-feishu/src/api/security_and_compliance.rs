//! `security_and_compliance` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Get 获取数据驻留地理位置列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=security_and_compliance&resource=multi_geo_entity.tenant&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_multi_geo_entity_tenant_get(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/multi_geo_entity/tenant".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取数据驻留地理位置列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=security_and_compliance&resource=multi_geo_entity.tenant&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_multi_geo_entity_tenant_get_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/multi_geo_entity/tenant".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// ListData 获取OpenAPI审计日志数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list_data&project=security_and_compliance&resource=openapi_log&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_openapi_logs_list_data(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/openapi_logs/list_data".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 迁移用户数据驻留位置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_user_migrations_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 迁移用户数据驻留位置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_user_migrations_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations".to_string();
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

    /// Get 获取单个用户迁移状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_user_migrations_get(
        &self,
        user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/security_and_compliance/v1/user_migrations/{user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取单个用户迁移状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_user_migrations_get_user(
        &self,
        bearer_token: &str,
        user_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/security_and_compliance/v1/user_migrations/{user_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Cancel 取消用户迁移任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_user_migrations_cancel(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations/cancel".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cancel 取消用户迁移任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_user_migrations_cancel_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations/cancel".to_string();
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

    /// Search 批量获取用户迁移状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_user_migrations_search(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 批量获取用户迁移状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=security_and_compliance&resource=user_migration&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_user_migrations_search_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v1/user_migrations/search".to_string();
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

    /// Update 审批设备申报
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=security_and_compliance&resource=device_apply_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_apply_records_update(
        &self,
        device_apply_record_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("device_apply_record_id", device_apply_record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/security_and_compliance/v2/device_apply_records/{device_apply_record_id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 查询设备信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_records_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v2/device_records".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 新增设备
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_records_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v2/device_records".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除设备
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_records_delete(
        &self,
        device_record_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("device_record_id", device_record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/security_and_compliance/v2/device_records/{device_record_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 获取设备信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_records_get(
        &self,
        device_record_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("device_record_id", device_record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/security_and_compliance/v2/device_records/{device_record_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新设备
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn security_and_compliance_device_records_update(
        &self,
        device_record_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("device_record_id", device_record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/security_and_compliance/v2/device_records/{device_record_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Mine 获取设备认证信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=mine&project=security_and_compliance&resource=device_record&version=v2>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn security_and_compliance_device_records_mine_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/security_and_compliance/v2/device_records/mine".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }
}
