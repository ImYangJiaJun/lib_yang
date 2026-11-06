//! `slides_ai` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// 渲染页面截图为图片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_slide_image_render(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/slides_ai/v1/slide_image/render".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 渲染页面截图为图片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_slide_image_render_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/slides_ai/v1/slide_image/render".to_string();
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

    /// 创建演示文稿
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/slides_ai/v1/xml_presentations".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 创建演示文稿
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/slides_ai/v1/xml_presentations".to_string();
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

    /// 获取演示文稿
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_get(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取演示文稿
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_get_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 历史版本列表
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_histories_list(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/histories");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 历史版本列表
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_histories_list_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/histories");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 历史版本回滚
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_history_revert(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/history/revert"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 历史版本回滚
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_history_revert_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/history/revert"
        );
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

    /// 回滚状态查询
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_history_revert_status_get(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/history/revert_status"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 回滚状态查询
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_history_revert_status_get_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/history/revert_status"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 删除幻灯片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_slide_delete(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 删除幻灯片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_slide_delete_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
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

    /// 获取幻灯片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_slide_get(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取幻灯片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_slide_get_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 创建幻灯片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_slide_create(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 创建幻灯片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_slide_create_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide");
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

    /// 替换幻灯片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_slide_replace(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide/replace"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 替换幻灯片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_slide_replace_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide/replace"
        );
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

    /// 获取幻灯片截图
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn slides_ai_xml_presentations_slide_images_render(
        &self,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide_images");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取幻灯片截图
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn slides_ai_xml_presentations_slide_images_render_user(
        &self,
        bearer_token: &str,
        xml_presentation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("xml_presentation_id", xml_presentation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/slides_ai/v1/xml_presentations/{xml_presentation_id}/slide_images");
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
