//! `apaas` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Get 飞书低代码平台-查询审计日志详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=apaas&resource=application.audit_log&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_applications_audit_log_get_user(
        &self,
        bearer_token: &str,
        namespace: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/audit_log");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// AuditLogList 查询审计日志列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=audit_log_list&project=apaas&resource=application.audit_log&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_applications_audit_log_audit_log_list_user(
        &self,
        bearer_token: &str,
        namespace: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/audit_log/audit_log_list");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// DataChangeLogDetail 飞书低代码平台-查询数据变更日志详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=data_change_log_detail&project=apaas&resource=application.audit_log&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_applications_audit_log_data_change_log_detail_user(
        &self,
        bearer_token: &str,
        namespace: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/apaas/v1/applications/{namespace}/audit_log/data_change_log_detail"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// DataChangeLogsList 飞书低代码平台-查询数据变更日志列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=data_change_logs_list&project=apaas&resource=application.audit_log&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_applications_audit_log_data_change_logs_list_user(
        &self,
        bearer_token: &str,
        namespace: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/applications/{namespace}/audit_log/data_change_logs_list");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询环境变量详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=apaas&resource=application.environment_variable&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_environment_variables_get(
        &self,
        namespace: &str,
        environment_variable_api_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment(
            "environment_variable_api_name",
            environment_variable_api_name,
        )
        .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/environment_variables/{environment_variable_api_name}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询环境变量列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=apaas&resource=application.environment_variable&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_environment_variables_query(
        &self,
        namespace: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/applications/{namespace}/environment_variables/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Execute 发起流程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=execute&project=apaas&resource=application.flow&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_flows_execute(
        &self,
        namespace: &str,
        flow_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("flow_id", flow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/flows/{flow_id}/execute");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Invoke 执行函数
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=invoke&project=apaas&resource=application.function&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_functions_invoke(
        &self,
        namespace: &str,
        function_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("function_api_name", function_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/apaas/v1/applications/{namespace}/functions/{function_api_name}/invoke"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 新建记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_create(
        &self,
        namespace: &str,
        object_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_delete(
        &self,
        namespace: &str,
        object_api_name: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/{id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Patch 编辑记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_patch(
        &self,
        namespace: &str,
        object_api_name: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/{id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 获取记录详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_query(
        &self,
        namespace: &str,
        object_api_name: &str,
        id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("id", id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/{id}/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchCreate 批量新建记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_batch_create(
        &self,
        namespace: &str,
        object_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 批量删除记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_batch_delete(
        &self,
        namespace: &str,
        object_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// BatchQuery 查询记录列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_batch_query(
        &self,
        namespace: &str,
        object_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/batch_query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 批量编辑记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=apaas&resource=application.object.record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_records_batch_update(
        &self,
        namespace: &str,
        object_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("object_api_name", object_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/{object_api_name}/records/batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// OqlQuery 执行 OQL
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=oql_query&project=apaas&resource=application.object&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_oql_query(
        &self,
        namespace: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/oql_query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 搜索记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=apaas&resource=application.object&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_objects_search(
        &self,
        namespace: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/objects/search");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchCreateAuthorization 批量创建记录权限授权
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create_authorization&project=apaas&resource=application.record_permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_record_permissions_member_batch_create_authorization(
        &self,
        namespace: &str,
        record_permission_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("record_permission_api_name", record_permission_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/record_permissions/{record_permission_api_name}/member/batch_create_authorization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchRemoveAuthorization 批量删除记录权限授权
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_remove_authorization&project=apaas&resource=application.record_permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_record_permissions_member_batch_remove_authorization(
        &self,
        namespace: &str,
        record_permission_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("record_permission_api_name", record_permission_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/record_permissions/{record_permission_api_name}/member/batch_remove_authorization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get 获取角色成员详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=apaas&resource=application.role.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_roles_member_get(
        &self,
        namespace: &str,
        role_api_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_api_name", role_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/applications/{namespace}/roles/{role_api_name}/member");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreateAuthorization 批量创建角色成员授权
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create_authorization&project=apaas&resource=application.role.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_roles_member_batch_create_authorization(
        &self,
        namespace: &str,
        role_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_api_name", role_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/roles/{role_api_name}/member/batch_create_authorization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchRemoveAuthorization 批量删除角色成员授权
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_remove_authorization&project=apaas&resource=application.role.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_applications_roles_member_batch_remove_authorization(
        &self,
        namespace: &str,
        role_api_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("namespace", namespace)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_api_name", role_api_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/applications/{namespace}/roles/{role_api_name}/member/batch_remove_authorization");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cancel 人工任务 - 撤销
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel&project=apaas&resource=approval_instance&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_approval_instances_cancel(
        &self,
        approval_instance_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_instance_id", approval_instance_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/approval_instances/{approval_instance_id}/cancel");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// AddAssignee 人工任务加签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=add_assignee&project=apaas&resource=approval_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_approval_tasks_add_assignee(
        &self,
        approval_task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_task_id", approval_task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/approval_tasks/{approval_task_id}/add_assignee");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Agree 同意人工任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=agree&project=apaas&resource=approval_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_approval_tasks_agree(
        &self,
        approval_task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_task_id", approval_task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/approval_tasks/{approval_task_id}/agree");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Reject 拒绝人工任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reject&project=apaas&resource=approval_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_approval_tasks_reject(
        &self,
        approval_task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_task_id", approval_task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/approval_tasks/{approval_task_id}/reject");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Transfer 转交人工任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=transfer&project=apaas&resource=approval_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_approval_tasks_transfer(
        &self,
        approval_task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("approval_task_id", approval_task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/approval_tasks/{approval_task_id}/transfer");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 查看应用基本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=apaas&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_apps_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/apaas/v1/apps".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询席位活跃详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=apaas&resource=seat_activity&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_seat_activities_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/apaas/v1/seat_activities".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询席位分配详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=apaas&resource=seat_assignment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_seat_assignments_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/apaas/v1/seat_assignments".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 获取应用运营数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=apaas&resource=tenant_app_metrics&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_tenant_app_metrics_query_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/apaas/v1/tenant_app_metrics/query".to_string();
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

    /// Query 查询人工任务列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_task_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/apaas/v1/user_task/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Cc 人工任务 - 抄送任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cc&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_tasks_cc(
        &self,
        task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("task_id", task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/user_tasks/{task_id}/cc");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// ChatGroup 人工任务 - 发起群聊
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=chat_group&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_tasks_chat_group(
        &self,
        task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("task_id", task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/user_tasks/{task_id}/chat_group");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Expediting 人工任务 - 催办任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=expediting&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_tasks_expediting(
        &self,
        task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("task_id", task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/user_tasks/{task_id}/expediting");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Rollback 人工任务 - 退回
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=rollback&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_tasks_rollback(
        &self,
        task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("task_id", task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/user_tasks/{task_id}/rollback");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// RollbackPoints 人工任务 - 查询可退回的位置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=rollback_points&project=apaas&resource=user_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn apaas_user_tasks_rollback_points(
        &self,
        task_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("task_id", task_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/user_tasks/{task_id}/rollback_points");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取工作空间下的自定义枚举列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=apaas&resource=workspace.enum&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_enums_list_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/enums");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// EnumGet 获取自定义枚举详细信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=enum_get&project=apaas&resource=workspace.enum&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_enums_enum_get_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        enum_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("enum_name", enum_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/enums/{enum_name}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// SqlCommands 执行 SQL
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=sql_commands&project=apaas&resource=workspace&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_sql_commands_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/sql_commands");
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

    /// List 获取工作空间下的数据表列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_list_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// TableGet 获取数据表详细信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=table_get&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_table_get_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// RecordsDelete 删除数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=records_delete&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_records_records_delete_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}/records");
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

    /// RecordsGet 查询数据表数据记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=records_get&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_records_records_get_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}/records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// RecordsPatch 按条件更新数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=records_patch&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_records_records_patch_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}/records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// RecordsPost 向数据表中添加或更新记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=records_post&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_records_records_post_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}/records");
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

    /// RecordsBatchUpdate 批量更新数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=records_batch_update&project=apaas&resource=workspace.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_tables_records_batch_update_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/apaas/v1/workspaces/{workspace_id}/tables/{table_name}/records_batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Patch,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// ViewsGet 查询视图数据记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=views_get&project=apaas&resource=workspace.view&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn apaas_workspaces_views_records_views_get_user(
        &self,
        bearer_token: &str,
        workspace_id: &str,
        view_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_name", view_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/apaas/v1/workspaces/{workspace_id}/views/{view_name}/records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }
}
