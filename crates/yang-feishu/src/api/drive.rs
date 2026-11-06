//! `drive` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// 获取文件夹元信息
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_explorer_folder_meta(
        &self,
        folder_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("folderToken", folder_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/explorer/v2/folder/{folder_token}/meta");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取文件夹元信息
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_explorer_folder_meta_user(
        &self,
        bearer_token: &str,
        folder_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("folderToken", folder_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/explorer/v2/folder/{folder_token}/meta");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取根文件夹元信息
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_explorer_root_folder_meta(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/explorer/v2/root_folder/meta".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取根文件夹元信息
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_explorer_root_folder_meta_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/explorer/v2/root_folder/meta".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建导出任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=export_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_export_tasks_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/export_tasks".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建导出任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=export_task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_export_tasks_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/export_tasks".to_string();
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

    /// Get 查询导出任务结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=export_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_export_tasks_get(
        &self,
        ticket: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket", ticket)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/export_tasks/{ticket}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询导出任务结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=export_task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_export_tasks_get_user(
        &self,
        bearer_token: &str,
        ticket: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket", ticket)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/export_tasks/{ticket}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Download 下载导出文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=export_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_export_tasks_file_download(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/export_tasks/file/{file_token}/download");
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

    /// Download 下载导出文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=export_task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_export_tasks_file_download_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/export_tasks/file/{file_token}/download");
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

    /// List 获取文件夹中的文件清单
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文件夹中的文件清单
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Delete 删除文件或文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_delete(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除文件或文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_delete_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}");
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

    /// List 获取云文档所有评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.comment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_list(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取云文档所有评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.comment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_list_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 添加全文评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.comment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_create(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 添加全文评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.comment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_create_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments");
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

    /// Get 获取全文评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.comment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_get(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取全文评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.comment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_get_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 解决/恢复评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=file.comment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_patch(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 解决/恢复评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=file.comment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_patch_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}");
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

    /// List 获取回复信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_replies_list(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取回复信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_replies_list_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 添加回复
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_replies_create(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 添加回复
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_replies_create_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
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

    /// Delete 删除回复
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_replies_delete(
        &self,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除回复
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_replies_delete_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
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

    /// Update 更新回复的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_replies_update(
        &self,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新回复的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=file.comment.reply&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_replies_update_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
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

    /// BatchQuery 批量获取评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=drive&resource=file.comment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_batch_query(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/batch_query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchQuery 批量获取评论
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=drive&resource=file.comment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_batch_query_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/batch_query");
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

    /// Copy 复制文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=copy&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_copy(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/copy");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Copy 复制文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=copy&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_copy_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/copy");
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

    /// DeleteSubscribe 取消云文档事件订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete_subscribe&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_delete_subscribe(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/delete_subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// DeleteSubscribe 取消云文档事件订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete_subscribe&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_delete_subscribe_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/delete_subscribe");
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

    /// Download 下载文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_files_download(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/download");
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

    /// Download 下载文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_files_download_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/download");
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

    /// GetSubscribe 查询云文档事件订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_subscribe&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_get_subscribe(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/get_subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetSubscribe 查询云文档事件订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_subscribe&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_get_subscribe_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/get_subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Move 移动文件或文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=move&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_move(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/move");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Move 移动文件或文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=move&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_move_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/move");
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

    /// Get 获取文件统计信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.statistics&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_statistics_get(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/statistics");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取文件统计信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.statistics&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_statistics_get_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/statistics");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Subscribe 订阅云文档事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_subscribe(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Subscribe 订阅云文档事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_subscribe_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/subscribe");
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

    /// Create 创建订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.subscription&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_subscriptions_create_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/subscriptions");
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

    /// Get 获取订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.subscription&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_subscriptions_get_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        subscription_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("subscription_id", subscription_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/drive/v1/files/{file_token}/subscriptions/{subscription_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=file.subscription&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_subscriptions_patch_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        subscription_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("subscription_id", subscription_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/drive/v1/files/{file_token}/subscriptions/{subscription_id}");
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

    /// List 获取文档版本列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.version&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_versions_list(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文档版本列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.version&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_versions_list_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建文档版本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.version&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_versions_create(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建文档版本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=file.version&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_versions_create_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions");
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

    /// Delete 删除文档版本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file.version&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_versions_delete(
        &self,
        file_token: &str,
        version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions/{version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除文档版本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=file.version&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_versions_delete_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        version_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions/{version_id}");
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

    /// Get 获取文档版本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.version&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_versions_get(
        &self,
        file_token: &str,
        version_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions/{version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取文档版本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=file.version&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_versions_get_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        version_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("version_id", version_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/versions/{version_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文件访问记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.view_record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_view_records_list(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/view_records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文件访问记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.view_record&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_view_records_list_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/view_records");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// CreateFolder 新建文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_folder&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_create_folder(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/create_folder".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// CreateFolder 新建文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_folder&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_create_folder_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/create_folder".to_string();
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

    /// CreateShortcut 创建文件快捷方式
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_shortcut&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_create_shortcut(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/create_shortcut".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// CreateShortcut 创建文件快捷方式
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_shortcut&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_create_shortcut_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/create_shortcut".to_string();
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

    /// TaskCheck 查询异步任务状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=task_check&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_task_check(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/task_check".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// TaskCheck 查询异步任务状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=task_check&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_task_check_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/task_check".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UploadAll 上传文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_all&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_files_upload_all(
        &self,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_all".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// UploadAll 上传文件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_all&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_files_upload_all_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_all".to_string();
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

    /// UploadFinish 分片上传文件-完成上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_finish&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_upload_finish(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_finish".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UploadFinish 分片上传文件-完成上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_finish&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_upload_finish_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_finish".to_string();
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

    /// UploadPart 分片上传文件-上传分片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_part&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_files_upload_part(
        &self,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_part".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// UploadPart 分片上传文件-上传分片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_part&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_files_upload_part_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_part".to_string();
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

    /// UploadPrepare 分片上传文件-预上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_prepare&project=drive&resource=file&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_upload_prepare(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_prepare".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UploadPrepare 分片上传文件-预上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_prepare&project=drive&resource=file&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_upload_prepare_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/files/upload_prepare".to_string();
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

    /// Create 创建导入任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=import_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_import_tasks_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/import_tasks".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建导入任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=import_task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_import_tasks_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/import_tasks".to_string();
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

    /// Get 查询导入结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=import_task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_import_tasks_get(
        &self,
        ticket: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket", ticket)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/import_tasks/{ticket}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询导入结果
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=import_task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_import_tasks_get_user(
        &self,
        bearer_token: &str,
        ticket: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("ticket", ticket)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/import_tasks/{ticket}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Download 下载素材
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_medias_download(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/medias/{file_token}/download");
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

    /// Download 下载素材
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn drive_medias_download_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/medias/{file_token}/download");
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

    /// BatchGetTmpDownloadUrl 获取素材临时下载链接
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_get_tmp_download_url&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_medias_batch_get_tmp_download_url(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/batch_get_tmp_download_url".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchGetTmpDownloadUrl 获取素材临时下载链接
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_get_tmp_download_url&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_medias_batch_get_tmp_download_url_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/batch_get_tmp_download_url".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UploadAll 上传素材
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_all&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_medias_upload_all(
        &self,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_all".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// UploadAll 上传素材
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_all&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_medias_upload_all_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_all".to_string();
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

    /// UploadFinish 分片上传素材-完成上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_finish&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_medias_upload_finish(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_finish".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UploadFinish 分片上传素材-完成上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_finish&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_medias_upload_finish_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_finish".to_string();
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

    /// UploadPart 分片上传素材-上传分片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_part&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_medias_upload_part(
        &self,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_part".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_raw_json(Method::Post, url, query, content_type, body, 20, false)
            .await
    }

    /// UploadPart 分片上传素材-上传分片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_part&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。
    pub async fn drive_medias_upload_part_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        content_type: &str,
        body: &[u8],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_part".to_string();
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

    /// UploadPrepare 分片上传素材-预上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_prepare&project=drive&resource=media&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_medias_upload_prepare(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_prepare".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UploadPrepare 分片上传素材-预上传
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=upload_prepare&project=drive&resource=media&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_medias_upload_prepare_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/medias/upload_prepare".to_string();
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

    /// BatchQuery 获取文件元数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=drive&resource=meta&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_metas_batch_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/metas/batch_query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchQuery 获取文件元数据
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=drive&resource=meta&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_metas_batch_query_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/metas/batch_query".to_string();
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

    /// List 获取云文档协作者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_list(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取云文档协作者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_list_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 增加协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_create(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 增加协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_create_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members");
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

    /// Delete 移除协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_delete(
        &self,
        path_token: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 移除协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_delete_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/{member_id}");
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

    /// Update 更新协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_update(
        &self,
        path_token: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_update_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/{member_id}");
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

    /// Auth 判断用户权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=auth&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_auth(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/auth");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Auth 判断用户权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=auth&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_auth_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/auth");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量增加协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_batch_create(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchCreate 批量增加协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_batch_create_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/batch_create");
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

    /// TransferOwner 转移云文档所有者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=transfer_owner&project=drive&resource=permission.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_members_transfer_owner(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/transfer_owner");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// TransferOwner 转移云文档所有者
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=transfer_owner&project=drive&resource=permission.member&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_members_transfer_owner_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/members/transfer_owner");
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

    /// Get 获取云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=permission.public&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_get(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=permission.public&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_get_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=permission.public&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_patch(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=permission.public&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_patch_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public");
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

    /// Delete 停用云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_password_delete(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 停用云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_password_delete_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
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

    /// Create 启用云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_password_create(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 启用云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_password_create_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
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

    /// Update 刷新云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_password_update(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 刷新云文档密码
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=drive&resource=permission.public.password&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_password_update_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{path_token}/public/password");
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

    /// RemoveSubscription 取消用户云文档事件订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remove_subscription&project=drive&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_user_remove_subscription(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/remove_subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// RemoveSubscription 取消用户云文档事件订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remove_subscription&project=drive&resource=user&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_user_remove_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/remove_subscription".to_string();
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

    /// Subscription 订阅用户云文档事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=drive&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_user_subscription(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/subscription".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Subscription 订阅用户云文档事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=drive&resource=user&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_user_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/subscription".to_string();
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

    /// SubscriptionStatus 查询用户云文档事件订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription_status&project=drive&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_user_subscription_status(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/subscription_status".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// SubscriptionStatus 查询用户云文档事件订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription_status&project=drive&resource=user&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_user_subscription_status_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/drive/v1/user/subscription_status".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UpdateReaction 添加/取消表情回应
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_reaction&project=drive&resource=comment_reaction&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_comments_reaction_update_reaction(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/files/{file_token}/comments/reaction");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UpdateReaction 添加/取消表情回应
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_reaction&project=drive&resource=comment_reaction&version=v2>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_comments_reaction_update_reaction_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/files/{file_token}/comments/reaction");
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

    /// List 获取云文档的点赞者列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.like&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_files_likes_list(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/files/{file_token}/likes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取云文档的点赞者列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=drive&resource=file.like&version=v2>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_files_likes_list_user(
        &self,
        bearer_token: &str,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/files/{file_token}/likes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=permission.public&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_get_1(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=permission.public&version=v2>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_get_1_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=permission.public&version=v2>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn drive_permissions_public_patch_1(
        &self,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/permissions/{path_token}/public");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新云文档权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=permission.public&version=v2>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn drive_permissions_public_patch_1_user(
        &self,
        bearer_token: &str,
        path_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("token", path_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/permissions/{path_token}/public");
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
}
