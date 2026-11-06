//! `board` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// DownloadAsImage 下载画板为图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_as_image&project=board&resource=whiteboard&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn board_whiteboards_download_as_image(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/download_as_image");
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

    /// DownloadAsImage 下载画板为图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_as_image&project=board&resource=whiteboard&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。
    pub async fn board_whiteboards_download_as_image_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<crate::transport::Response> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/download_as_image");
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

    /// List 获取所有节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_nodes_list(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取所有节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_nodes_list_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建画板节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_nodes_create(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建画板节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_nodes_create_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes");
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

    /// BatchDelete 批量删除节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_nodes_batch_delete(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// BatchDelete 批量删除节点
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_nodes_batch_delete_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes/batch_delete");
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

    /// CreatePlantuml 解析画板语法
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_plantuml&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_nodes_plantuml_create_plantuml(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes/plantuml");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// CreatePlantuml 解析画板语法
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_plantuml&project=board&resource=whiteboard.node&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_nodes_plantuml_create_plantuml_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/nodes/plantuml");
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

    /// Theme 获取画板主题
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=theme&project=board&resource=whiteboard&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_theme(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/theme");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Theme 获取画板主题
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=theme&project=board&resource=whiteboard&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_theme_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/theme");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// UpdateTheme 更新主题
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_theme&project=board&resource=whiteboard&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn board_whiteboards_update_theme(
        &self,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/update_theme");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// UpdateTheme 更新主题
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_theme&project=board&resource=whiteboard&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn board_whiteboards_update_theme_user(
        &self,
        bearer_token: &str,
        whiteboard_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("whiteboard_id", whiteboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/board/v1/whiteboards/{whiteboard_id}/update_theme");
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
