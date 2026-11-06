//! `spark` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 批量获取妙搭应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/spark/v1/apps".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建妙搭应用
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/spark/v1/apps".to_string();
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

    /// Patch 更新妙搭应用信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_patch_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}");
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

    /// GetAppVisibility 获取妙搭应用可用范围;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_app_visibility&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_access_scope_get_app_visibility_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/access-scope");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UpdateAppVisibility 更新妙搭应用可用范围
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_app_visibility&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_access_scope_update_app_visibility_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/access-scope");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(
                bearer_token,
                Method::Put,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// GetEnumList 获取自定义枚举列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_enum_list&project=spark&resource=app.enum&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_enums_get_enum_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/enums");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetEnumDetail 获取自定义枚举详细信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_enum_detail&project=spark&resource=app.enum&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_enums_get_enum_detail_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        enum_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("enum_name", enum_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/enums/{enum_name}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// SqlCommands 执行 SQL
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=sql_commands&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_sql_commands_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/sql_commands");
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

    /// Download 下载文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=spark&resource=app.storage&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn spark_apps_storage_download_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/storage");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Get,
                url,
                query,
                content_type,
                body,
                20,
                true,
            )
            .await
    }

    /// Upload 上传文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload&project=spark&resource=app.storage&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn spark_apps_storage_upload_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/storage/upload");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }

    /// UploadComplete 分片上传文件 - 完成上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_complete&project=spark&resource=app.storage&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_storage_upload_complete_upload_complete_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/storage/upload/complete");
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

    /// UploadInitialize 分片上传文件 - 创建上传请求
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_initialize&project=spark&resource=app.storage&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_storage_upload_initialize_upload_initialize_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/storage/upload/initialize");
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

    /// UploadPart 分片上传文件 - 上传分片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_part&project=spark&resource=app.storage&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn spark_apps_storage_upload_part_upload_part_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/storage/upload/part");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }

    /// GetTableList 获取数据表列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_table_list&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_get_table_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetTableDetail 获取数据表详细信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_table_detail&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_get_table_detail_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// DeleteTableRecords 删除数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete_table_records&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_records_delete_table_records_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}/records");
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

    /// GetTableRecordList 查询数据表数据记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_table_record_list&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_records_get_table_record_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}/records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// PatchTableRecords 按条件更新数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch_table_records&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_records_patch_table_records_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}/records");
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

    /// PostTableRecords 向数据表中添加或更新记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=post_table_records&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_records_post_table_records_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}/records");
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

    /// BatchUpdateTableRecords 批量更新数据表中的记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update_table_records&project=spark&resource=app.table&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_tables_records_batch_update_batch_update_table_records_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        table_name: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_name", table_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/spark/v1/apps/{app_id}/tables/{table_name}/records_batch_update");
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

    /// UploadHtmlCodeAndRelease 上传 HTML 代码并发布;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_html_code_and_release&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn spark_apps_upload_and_release_html_code_upload_html_code_and_release_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/upload_and_release_html_code");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }

    /// GetViewRecordList 查询视图数据记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_view_record_list&project=spark&resource=app.view&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_apps_views_records_get_view_record_list_user(
        &self,
        bearer_token: &str,
        app_id: &str,
        view_name: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("app_id", app_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_name", view_name)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/spark/v1/apps/{app_id}/views/{view_name}/records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// IdConvert 转换飞书妙搭和飞书开放平台用户 ID
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=id_convert&project=spark&resource=directory.user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn spark_directory_user_id_convert(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/spark/v1/directory/user/id_convert".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// IdConvert 转换飞书妙搭和飞书开放平台用户 ID
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=id_convert&project=spark&resource=directory.user&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn spark_directory_user_id_convert_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/spark/v1/directory/user/id_convert".to_string();
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

    /// Icon 上传妙搭应用图标
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=icon&project=spark&resource=app&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn spark_icon_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/spark/v1/icon".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                content_type,
                body,
                20,
                false,
            )
            .await
    }
}
