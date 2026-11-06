//! `docx` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Get 获取群公告基本信息;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_get(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取群公告基本信息;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_get_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取群公告所有块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_list(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取群公告所有块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_list_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取群公告块的内容;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_get(
        &self,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取群公告块的内容;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_get_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取所有子块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_children_get(
        &self,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取所有子块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_children_get_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 在群公告中创建块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_children_create(
        &self,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 在群公告中创建块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_children_create_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children");
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

    /// BatchDelete 删除群公告中的块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_children_batch_delete(
        &self,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// BatchDelete 删除群公告中的块;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=docx&resource=chat.announcement.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_children_batch_delete_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/{block_id}/children/batch_delete");
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

    /// BatchUpdate 批量更新群公告块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_chats_announcement_blocks_batch_update(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 批量更新群公告块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=docx&resource=chat.announcement.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_chats_announcement_blocks_batch_update_user(
        &self,
        bearer_token: &str,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/chats/{chat_id}/announcement/blocks/batch_update");
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

    /// Create 创建文档
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/docx/v1/documents".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建文档
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/docx/v1/documents".to_string();
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

    /// Get 获取文档基本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_get(
        &self,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取文档基本信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_get_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文档所有块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=docx&resource=document.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_list(
        &self,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取文档所有块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=docx&resource=document.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_list_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_get(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_get_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=docx&resource=document.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_patch(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=docx&resource=document.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_patch_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}");
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

    /// Get 获取所有子块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_children_get(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取所有子块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=docx&resource=document.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_children_get_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_children_create(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_children_create_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children");
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

    /// BatchDelete 删除块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=docx&resource=document.block.children&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_children_batch_delete(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children/batch_delete"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// BatchDelete 删除块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=docx&resource=document.block.children&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_children_batch_delete_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/children/batch_delete"
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

    /// Create 创建嵌套块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document.block.descendant&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_descendant_create(
        &self,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/descendant");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建嵌套块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=docx&resource=document.block.descendant&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_descendant_create_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/docx/v1/documents/{document_id}/blocks/{block_id}/descendant");
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

    /// BatchUpdate 批量更新块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=docx&resource=document.block&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_batch_update(
        &self,
        document_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 批量更新块的内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=docx&resource=document.block&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_batch_update_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/blocks/batch_update");
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

    /// RawContent 获取文档纯文本内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=raw_content&project=docx&resource=document&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_raw_content(
        &self,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/raw_content");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// RawContent 获取文档纯文本内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=raw_content&project=docx&resource=document&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_raw_content_user(
        &self,
        bearer_token: &str,
        document_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("document_id", document_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/docx/v1/documents/{document_id}/raw_content");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Convert Markdown/HTML 内容转换为文档块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=convert&project=docx&resource=document&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn docx_documents_blocks_convert(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/docx/v1/documents/blocks/convert".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Convert Markdown/HTML 内容转换为文档块
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=convert&project=docx&resource=document&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn docx_documents_blocks_convert_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/docx/v1/documents/blocks/convert".to_string();
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
