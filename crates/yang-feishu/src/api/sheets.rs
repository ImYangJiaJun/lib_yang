//! `sheets` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// 批量获取条件格式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_condition_formats_get(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 批量获取条件格式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_condition_formats_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 批量创建条件格式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_condition_formats_batch_create(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_create"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量创建条件格式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_condition_formats_batch_create_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_create"
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

    /// 批量删除条件格式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_condition_formats_batch_delete(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_delete"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 批量删除条件格式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_condition_formats_batch_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_delete"
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

    /// 批量更新条件格式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_condition_formats_batch_update(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_update"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量更新条件格式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_condition_formats_batch_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/condition_formats/batch_update"
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

    /// 删除下拉列表设置
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_data_validation_delete(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 删除下拉列表设置
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_data_validation_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
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

    /// 查询下拉列表设置
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_data_validation_get(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 查询下拉列表设置
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_data_validation_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 设置下拉列表
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_data_validation_set(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 设置下拉列表
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_data_validation_set_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation");
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

    /// 更新下拉列表设置
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_data_validation_update(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        data_validation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheetId", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dataValidationId", data_validation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation/{sheet_id}/{data_validation_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 更新下拉列表设置
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_data_validation_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        data_validation_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheetId", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dataValidationId", data_validation_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dataValidation/{sheet_id}/{data_validation_id}");
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

    /// 删除行列
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_dimension_range_delete(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 删除行列
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_dimension_range_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
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

    /// 增加行列
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_dimension_range_add(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 增加行列
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_dimension_range_add_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
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

    /// 更新行列
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_dimension_range_update(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 更新行列
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_dimension_range_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/dimension_range");
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

    /// 插入行列
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_insert_dimension_range_insert(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/insert_dimension_range");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 插入行列
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_insert_dimension_range_insert_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/insert_dimension_range");
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

    /// 合并单元格
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_merge_cells_merge(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/merge_cells");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 合并单元格
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_merge_cells_merge_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/merge_cells");
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

    /// 增加保护范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_protected_dimension_add(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_dimension");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 增加保护范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_protected_dimension_add_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_dimension");
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

    /// 删除保护范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_protected_range_batch_del_delete(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_del"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 删除保护范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_protected_range_batch_del_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_del"
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

    /// 获取保护范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_get(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_get"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取保护范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_get"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 修改保护范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_update(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_update"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 修改保护范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/protected_range_batch_update"
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

    /// 操作工作表（含更新工作表属性）
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_batch_update(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/sheets_batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 操作工作表（含更新工作表属性）
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_batch_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/sheets_batch_update");
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

    /// 设置单元格样式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_style_set(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/style");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 设置单元格样式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_style_set_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/style");
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

    /// 批量设置单元格样式
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_batch_update_1(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/styles_batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量设置单元格样式
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_batch_update_1_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/styles_batch_update");
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

    /// 拆分单元格
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_unmerge_cells_unmerge(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/unmerge_cells");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 拆分单元格
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_unmerge_cells_unmerge_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/unmerge_cells");
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

    /// 向单个范围写入数据
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_values_write(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 向单个范围写入数据
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_values_write_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values");
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

    /// 读取单个范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_values_get(
        &self,
        spreadsheet_token: &str,
        range: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("range", range)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values/{range}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 读取单个范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_values_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        range: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("range", range)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values/{range}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 追加数据
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_append(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_append");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 追加数据
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_append_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_append");
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

    /// 读取多个范围
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_batch_get(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_batch_get");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 读取多个范围
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_batch_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_batch_get");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 向多个范围写入数据
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_batch_update_2(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_batch_update");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 向多个范围写入数据
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_batch_update_2_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_batch_update");
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

    /// 写入图片
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_values_image_write(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_image");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 写入图片
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_values_image_write_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_image");
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

    /// 插入数据
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_prepend(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_prepend");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 插入数据
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_prepend_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v2/spreadsheets/{spreadsheet_token}/values_prepend");
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

    /// Create 创建电子表格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/sheets/v3/spreadsheets".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建电子表格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/sheets/v3/spreadsheets".to_string();
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

    /// Get 获取电子表格信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_get_1(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取电子表格信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_get_1_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改电子表格属性
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_patch(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 修改电子表格属性
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_patch_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
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

    /// 修改电子表格属性
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_update_1(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 修改电子表格属性
    ///
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_update_1_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}");
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

    /// Get 查询工作表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_get(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询工作表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Delete 删除筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_delete(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
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

    /// Get 获取筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_get(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_create(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_create_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
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

    /// Update 更新筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_update(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新筛选
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=sheets&resource=spreadsheet.sheet.filter&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter"
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

    /// Create 创建筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_create(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_create_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views"
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

    /// Delete 删除筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_delete(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
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

    /// Get 获取筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_get(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_patch(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_patch_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}");
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

    /// Create 创建筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_create(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_create_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions");
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

    /// Delete 删除筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_delete(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    #[allow(clippy::too_many_arguments)]
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
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

    /// Get 获取筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_get(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_update(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    #[allow(clippy::too_many_arguments)]
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_update_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        condition_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("condition_id", condition_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/{condition_id}");
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

    /// Query 查询筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_query(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询筛选条件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.filter_view.condition&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_conditions_query_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        filter_view_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("filter_view_id", filter_view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/{filter_view_id}/conditions/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_filter_views_query(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询筛选视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.filter_view&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_filter_views_query_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/filter_views/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Find 查找单元格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=find&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_find(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/find");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Find 查找单元格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=find&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_find_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/find");
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

    /// Create 创建浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_float_images_create(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_float_images_create_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images"
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

    /// Delete 删除浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_float_images_delete(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_float_images_delete_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
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

    /// Get 获取浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_float_images_get(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_float_images_get_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_float_images_patch(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_float_images_patch_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        float_image_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("float_image_id", float_image_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/{float_image_id}");
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

    /// Query 查询浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_float_images_query(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询浮动图片
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet.float_image&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_float_images_query_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/float_images/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// MoveDimension 移动行列
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=move_dimension&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_move_dimension(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/move_dimension");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// MoveDimension 移动行列
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=move_dimension&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_move_dimension_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/move_dimension");
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

    /// Replace 替换单元格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=replace&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_replace(
        &self,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/replace"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Replace 替换单元格
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=replace&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_replace_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        sheet_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("sheet_id", sheet_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/{sheet_id}/replace"
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

    /// Query 获取工作表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn sheets_spreadsheets_sheets_query(
        &self,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 获取工作表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=sheets&resource=spreadsheet.sheet&version=v3>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn sheets_spreadsheets_sheets_query_user(
        &self,
        bearer_token: &str,
        spreadsheet_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("spreadsheet_token", spreadsheet_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/sheets/v3/spreadsheets/{spreadsheet_token}/sheets/query");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }
}
