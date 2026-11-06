//! `unified_kms` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 批量获取密钥信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=unified_kms&resource=autonomous_key&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/unified_kms/v1/autonomous_keys".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 导入与轮转密钥
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=unified_kms&resource=autonomous_key&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/unified_kms/v1/autonomous_keys".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 立即删除密钥
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=unified_kms&resource=autonomous_key&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_delete(
        &self,
        key_version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("key_version_id", key_version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/unified_kms/v1/autonomous_keys/{key_version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 获取密钥信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=unified_kms&resource=autonomous_key&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_get(
        &self,
        key_version_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("key_version_id", key_version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/unified_kms/v1/autonomous_keys/{key_version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Delete 取消密钥删除计划
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=unified_kms&resource=autonomous_key.deletion_plan&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_deletion_plan_delete(
        &self,
        key_version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("key_version_id", key_version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/unified_kms/v1/autonomous_keys/{key_version_id}/deletion_plan");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create 创建密钥删除计划
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=unified_kms&resource=autonomous_key.deletion_plan&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_deletion_plan_create(
        &self,
        key_version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("key_version_id", key_version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/unified_kms/v1/autonomous_keys/{key_version_id}/deletion_plan");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 恢复已删除的主密钥
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=unified_kms&resource=autonomous_key.recover&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_autonomous_keys_recover_create(
        &self,
        key_version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("key_version_id", key_version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/unified_kms/v1/autonomous_keys/{key_version_id}/recover");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 获取密钥导入材料
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=unified_kms&resource=key_import_material&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn unified_kms_key_import_material_get(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/unified_kms/v1/key_import_material".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
