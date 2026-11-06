//! `cardkit` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Create 创建卡片实体
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=cardkit&resource=card&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/cardkit/v1/cards".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 全量更新卡片实体
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=cardkit&resource=card&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_update(
        &self,
        card_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 局部更新卡片实体
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=cardkit&resource=card&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_batch_update(
        &self,
        card_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 新增组件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=cardkit&resource=card.element&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_elements_create(
        &self,
        card_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/elements");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除组件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=cardkit&resource=card.element&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_elements_delete(
        &self,
        card_id: &str,
        element_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("element_id", element_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/elements/{element_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Patch 更新组件属性
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=cardkit&resource=card.element&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_elements_patch(
        &self,
        card_id: &str,
        element_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("element_id", element_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/elements/{element_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新组件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=cardkit&resource=card.element&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_elements_update(
        &self,
        card_id: &str,
        element_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("element_id", element_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/elements/{element_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Content 流式更新文本
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=content&project=cardkit&resource=card.element&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_elements_content(
        &self,
        card_id: &str,
        element_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("element_id", element_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/elements/{element_id}/content");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Settings 更新卡片配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=settings&project=cardkit&resource=card&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_settings(
        &self,
        card_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("card_id", card_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/cardkit/v1/cards/{card_id}/settings");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// IdConvert 转换 ID
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=id_convert&project=cardkit&resource=card&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn cardkit_cards_id_convert(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/cardkit/v1/cards/id_convert".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
