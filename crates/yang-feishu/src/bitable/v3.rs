//! Base v3 端点薄封装（由本地 lark-cli dry-run 目录生成）。
//!
//! 说明：Base v3 与官方 llms 文档中的 bitable v1 是两套路径；本模块对齐
//! lark-cli 当前使用的 `/open-apis/base/v3/...`。未在官方 llms 索引中出现的
//! 路径以 CLI 为准，升级 CLI 后应重新生成本文件。

use serde_json::Value;

use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

use super::BitableClient;

impl BitableClient {
    /// Disable advanced permissions for a Base
    ///
    /// 来源: `lark-cli base +advperm-disable`（Base v3）。
    pub async fn v3_advperm_disable(&self, base_token: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/advperm/enable?enable=false");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, None, 20, true)
            .await
    }

    /// Enable advanced permissions for a Base
    ///
    /// 来源: `lark-cli base +advperm-enable`（Base v3）。
    pub async fn v3_advperm_enable(&self, base_token: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/advperm/enable?enable=true");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, None, 20, true)
            .await
    }

    /// Get a BaseApp page block by ID
    ///
    /// 来源: `lark-cli base +app-block-get`（Base v3）。
    pub async fn v3_app_block_get(
        &self,
        app_token: &str,
        page_id: &str,
        block_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}/blocks/{block_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get computed data for a BaseApp page chart block
    ///
    /// 来源: `lark-cli base +app-block-get-data`（Base v3）。
    pub async fn v3_app_block_get_data(
        &self,
        app_token: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/blocks/{block_id}/data");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List blocks on a BaseApp page
    ///
    /// 来源: `lark-cli base +app-block-list`（Base v3）。
    pub async fn v3_app_block_list(
        &self,
        app_token: &str,
        page_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}/blocks");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update a block on a BaseApp page
    ///
    /// 来源: `lark-cli base +app-block-update`（Base v3）。
    pub async fn v3_app_block_update(
        &self,
        app_token: &str,
        page_id: &str,
        block_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}/blocks/{block_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a new BaseApp in a Workspace (not a copy)
    ///
    /// 来源: `lark-cli base +app-create`（Base v3）。
    pub async fn v3_app_create(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/base/v3/base_apps".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get BaseApp info, page summaries, and the referenced Base/Table map
    ///
    /// 来源: `lark-cli base +app-get`（Base v3）。
    pub async fn v3_app_get(&self, app_token: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create a page in a BaseApp
    ///
    /// 来源: `lark-cli base +app-page-create`（Base v3）。
    pub async fn v3_app_page_create(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a BaseApp page
    ///
    /// 来源: `lark-cli base +app-page-delete`（Base v3）。
    pub async fn v3_app_page_delete(&self, app_token: &str, page_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a BaseApp page by ID
    ///
    /// 来源: `lark-cli base +app-page-get`（Base v3）。
    pub async fn v3_app_page_get(&self, app_token: &str, page_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List pages in a BaseApp
    ///
    /// 来源: `lark-cli base +app-page-list`（Base v3）。
    pub async fn v3_app_page_list(&self, app_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Rename a BaseApp page
    ///
    /// 来源: `lark-cli base +app-page-update`（Base v3）。
    pub async fn v3_app_page_update(
        &self,
        app_token: &str,
        page_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("page_id", page_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/base_apps/{app_token}/pages/{page_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a block
    ///
    /// 来源: `lark-cli base +base-block-create`（Base v3）。
    pub async fn v3_base_block_create(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/blocks");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a block
    ///
    /// 来源: `lark-cli base +base-block-delete`（Base v3）。
    pub async fn v3_base_block_delete(&self, base_token: &str, block_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/blocks/{block_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// List blocks in a base
    ///
    /// 来源: `lark-cli base +base-block-list`（Base v3）。
    pub async fn v3_base_block_list(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/blocks/list");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Move a block
    ///
    /// 来源: `lark-cli base +base-block-move`（Base v3）。
    pub async fn v3_base_block_move(
        &self,
        base_token: &str,
        block_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/blocks/{block_id}/move");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Rename a block
    ///
    /// 来源: `lark-cli base +base-block-rename`（Base v3）。
    pub async fn v3_base_block_rename(
        &self,
        base_token: &str,
        block_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/blocks/{block_id}/rename");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Copy a Base resource (not a BaseApp)
    ///
    /// 来源: `lark-cli base +base-copy`（Base v3）。
    pub async fn v3_base_copy(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/copy");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create a new base resource
    ///
    /// 来源: `lark-cli base +base-create`（Base v3）。
    pub async fn v3_base_create(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/base/v3/bases".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get a base resource
    ///
    /// 来源: `lark-cli base +base-get`（Base v3）。
    pub async fn v3_base_get(&self, base_token: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Auto-arrange dashboard blocks layout (server-side smart layout)
    ///
    /// 来源: `lark-cli base +dashboard-arrange`（Base v3）。
    pub async fn v3_dashboard_arrange(
        &self,
        base_token: &str,
        dashboard_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/arrange");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a dashboard block
    ///
    /// 来源: `lark-cli base +dashboard-block-delete`（Base v3）。
    pub async fn v3_dashboard_block_delete(
        &self,
        base_token: &str,
        dashboard_id: &str,
        block_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/blocks/{block_id}"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a dashboard block by ID
    ///
    /// 来源: `lark-cli base +dashboard-block-get`（Base v3）。
    pub async fn v3_dashboard_block_get(
        &self,
        base_token: &str,
        dashboard_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/blocks/{block_id}"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get computed data for a dashboard chart block
    ///
    /// 来源: `lark-cli base +dashboard-block-get-data`（Base v3）。
    pub async fn v3_dashboard_block_get_data(
        &self,
        base_token: &str,
        block_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/dashboards/blocks/{block_id}/data");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List blocks in a dashboard
    ///
    /// 来源: `lark-cli base +dashboard-block-list`（Base v3）。
    pub async fn v3_dashboard_block_list(
        &self,
        base_token: &str,
        dashboard_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/blocks");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update a dashboard block
    ///
    /// 来源: `lark-cli base +dashboard-block-update`（Base v3）。
    pub async fn v3_dashboard_block_update(
        &self,
        base_token: &str,
        dashboard_id: &str,
        block_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/blocks/{block_id}"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a dashboard in a base
    ///
    /// 来源: `lark-cli base +dashboard-create`（Base v3）。
    pub async fn v3_dashboard_create(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a dashboard
    ///
    /// 来源: `lark-cli base +dashboard-delete`（Base v3）。
    pub async fn v3_dashboard_delete(&self, base_token: &str, dashboard_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a dashboard by ID
    ///
    /// 来源: `lark-cli base +dashboard-get`（Base v3）。
    pub async fn v3_dashboard_get(&self, base_token: &str, dashboard_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List dashboards in a base
    ///
    /// 来源: `lark-cli base +dashboard-list`（Base v3）。
    pub async fn v3_dashboard_list(
        &self,
        base_token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get dashboard share status and settings
    ///
    /// 来源: `lark-cli base +dashboard-share-get`（Base v3）。
    pub async fn v3_dashboard_share_get(
        &self,
        base_token: &str,
        dashboard_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/share");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update dashboard share status and settings
    ///
    /// 来源: `lark-cli base +dashboard-share-update`（Base v3）。
    pub async fn v3_dashboard_share_update(
        &self,
        base_token: &str,
        dashboard_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}/share");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Update a dashboard
    ///
    /// 来源: `lark-cli base +dashboard-update`（Base v3）。
    pub async fn v3_dashboard_update(
        &self,
        base_token: &str,
        dashboard_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("dashboard_id", dashboard_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/dashboards/{dashboard_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Query and analyze Base data with JSON DSL (aggregation, filter, sort)
    ///
    /// 来源: `lark-cli base +data-query`（Base v3）。
    pub async fn v3_data_query(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/data/query");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create one or more fields
    ///
    /// 来源: `lark-cli base +field-create`（Base v3）。
    pub async fn v3_field_create(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a field by ID or name
    ///
    /// 来源: `lark-cli base +field-delete`（Base v3）。
    pub async fn v3_field_delete(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a field extension configuration
    ///
    /// 来源: `lark-cli base +field-extension-get`（Base v3）。
    pub async fn v3_field_extension_get(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}/field_extensions");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Install, update, or clear a field extension
    ///
    /// 来源: `lark-cli base +field-extension-update`（Base v3）。
    pub async fn v3_field_extension_update(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}/field_extensions");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get a field by ID or name
    ///
    /// 来源: `lark-cli base +field-get`（Base v3）。
    pub async fn v3_field_get(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List all fields in a table
    ///
    /// 来源: `lark-cli base +field-list`（Base v3）。
    pub async fn v3_field_list(
        &self,
        base_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Search select options of a field
    ///
    /// 来源: `lark-cli base +field-search-options`（Base v3）。
    pub async fn v3_field_search_options(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}/options"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update a field by ID or name
    ///
    /// 来源: `lark-cli base +field-update`（Base v3）。
    pub async fn v3_field_update(
        &self,
        base_token: &str,
        table_id: &str,
        field_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a form in a Base table
    ///
    /// 来源: `lark-cli base +form-create`（Base v3）。
    pub async fn v3_form_create(&self, base_token: &str, table_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, None, 20, false)
            .await
    }

    /// Delete a form in a Base table
    ///
    /// 来源: `lark-cli base +form-delete`（Base v3）。
    pub async fn v3_form_delete(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get form detail by share token
    ///
    /// 来源: `lark-cli base +form-detail`（Base v3）。
    pub async fn v3_form_detail(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/base/v3/bases/tables/forms/detail".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get a form in a Base table
    ///
    /// 来源: `lark-cli base +form-get`（Base v3）。
    pub async fn v3_form_get(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List all forms in a Base table (auto-paginated)
    ///
    /// 来源: `lark-cli base +form-list`（Base v3）。
    pub async fn v3_form_list(
        &self,
        base_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create questions for a form in a Base table
    ///
    /// 来源: `lark-cli base +form-questions-create`（Base v3）。
    pub async fn v3_form_questions_create(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/questions"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete questions from a form in a Base table
    ///
    /// 来源: `lark-cli base +form-questions-delete`（Base v3）。
    pub async fn v3_form_questions_delete(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/questions"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List questions of a form in a Base table
    ///
    /// 来源: `lark-cli base +form-questions-list`（Base v3）。
    pub async fn v3_form_questions_list(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/questions"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update questions of a form in a Base table
    ///
    /// 来源: `lark-cli base +form-questions-update`（Base v3）。
    pub async fn v3_form_questions_update(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/questions"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, None, 20, true)
            .await
    }

    /// Get form share status and settings
    ///
    /// 来源: `lark-cli base +form-share-get`（Base v3）。
    pub async fn v3_form_share_get(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/share"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update form share status and settings
    ///
    /// 来源: `lark-cli base +form-share-update`（Base v3）。
    pub async fn v3_form_share_update(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}/share"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Submit a form (fill and submit form data)
    ///
    /// 来源: `lark-cli base +form-submit`（Base v3）。
    pub async fn v3_form_submit(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/base/v3/bases/tables/forms/submit".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update a form in a Base table
    ///
    /// 来源: `lark-cli base +form-update`（Base v3）。
    pub async fn v3_form_update(
        &self,
        base_token: &str,
        table_id: &str,
        form_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/forms/{form_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, None, 20, true)
            .await
    }

    /// Batch create records
    ///
    /// 来源: `lark-cli base +record-batch-create`（Base v3）。
    pub async fn v3_record_batch_create(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/batch_create");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Batch update records with record-specific fields
    ///
    /// 来源: `lark-cli base +record-batch-update`（Base v3）。
    pub async fn v3_record_batch_update(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/batch_update");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete one or more records by ID
    ///
    /// 来源: `lark-cli base +record-delete`（Base v3）。
    pub async fn v3_record_delete(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/batch_delete");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get one or more records by ID
    ///
    /// 来源: `lark-cli base +record-get`（Base v3）。
    pub async fn v3_record_get(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/batch_get");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List record change history
    ///
    /// 来源: `lark-cli base +record-history-list`（Base v3）。
    pub async fn v3_record_history_list(
        &self,
        base_token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/record_history");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List records in a table
    ///
    /// 来源: `lark-cli base +record-list`（Base v3）。
    pub async fn v3_record_list(&self, base_token: &str, table_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records?field_id=TEST&filter=%7B%7D&limit=1&offset=1&view_id=TEST");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Remove one or more file_token values from a Base record attachment cell
    ///
    /// 来源: `lark-cli base +record-remove-attachment`（Base v3）。
    pub async fn v3_record_remove_attachment(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/remove_attachments");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search records in a table
    ///
    /// 来源: `lark-cli base +record-search`（Base v3）。
    pub async fn v3_record_search(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/search");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Generate share links for one or more records (max 100 per request)
    ///
    /// 来源: `lark-cli base +record-share-link-create`（Base v3）。
    pub async fn v3_record_share_link_create(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/share_links/batch"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create or update a record
    ///
    /// 来源: `lark-cli base +record-upsert`（Base v3）。
    pub async fn v3_record_upsert(
        &self,
        base_token: &str,
        table_id: &str,
        record_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("record_id", record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/records/{record_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a custom role in a Base
    ///
    /// 来源: `lark-cli base +role-create`（Base v3）。
    pub async fn v3_role_create(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/roles");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a custom role (system roles cannot be deleted)
    ///
    /// 来源: `lark-cli base +role-delete`（Base v3）。
    pub async fn v3_role_delete(&self, base_token: &str, role_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/roles/{role_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get full config of a role
    ///
    /// 来源: `lark-cli base +role-get`（Base v3）。
    pub async fn v3_role_get(&self, base_token: &str, role_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/roles/{role_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List all roles in a Base
    ///
    /// 来源: `lark-cli base +role-list`（Base v3）。
    pub async fn v3_role_list(&self, base_token: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/roles");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update a role config (delta merge, only changed fields needed)
    ///
    /// 来源: `lark-cli base +role-update`（Base v3）。
    pub async fn v3_role_update(
        &self,
        base_token: &str,
        role_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/roles/{role_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Copy a table by ID or name; structure only by default
    ///
    /// 来源: `lark-cli base +table-copy`（Base v3）。
    pub async fn v3_table_copy(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/copy");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Get one table copy task status
    ///
    /// 来源: `lark-cli base +table-copy-status`（Base v3）。
    pub async fn v3_table_copy_status(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/copy_table_state");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create a table with an explicit field schema, plus optional views
    ///
    /// 来源: `lark-cli base +table-create`（Base v3）。
    pub async fn v3_table_create(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a table by ID or name
    ///
    /// 来源: `lark-cli base +table-delete`（Base v3）。
    pub async fn v3_table_delete(&self, base_token: &str, table_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a table by ID or name
    ///
    /// 来源: `lark-cli base +table-get`（Base v3）。
    pub async fn v3_table_get(&self, base_token: &str, table_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List all tables in a base
    ///
    /// 来源: `lark-cli base +table-list`（Base v3）。
    pub async fn v3_table_list(&self, base_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Rename a table by ID or name
    ///
    /// 来源: `lark-cli base +table-update`（Base v3）。
    pub async fn v3_table_update(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List Base template center categories
    ///
    /// 来源: `lark-cli base +template-categories`（Base v3）。
    pub async fn v3_template_categories(&self) -> Result<Value> {
        let path = "/open-apis/base/v3/bases/templates/category".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List Base templates by category
    ///
    /// 来源: `lark-cli base +template-list`（Base v3）。
    pub async fn v3_template_list(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/base/v3/bases/templates".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Search Base templates by keyword
    ///
    /// 来源: `lark-cli base +template-search`（Base v3）。
    pub async fn v3_template_search(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/base/v3/bases/templates/search".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create one or more views
    ///
    /// 来源: `lark-cli base +view-create`（Base v3）。
    pub async fn v3_view_create(
        &self,
        base_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete a view by ID or name
    ///
    /// 来源: `lark-cli base +view-delete`（Base v3）。
    pub async fn v3_view_delete(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// Get a view by ID or name
    ///
    /// 来源: `lark-cli base +view-get`（Base v3）。
    pub async fn v3_view_get(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view card configuration
    ///
    /// 来源: `lark-cli base +view-get-card`（Base v3）。
    pub async fn v3_view_get_card(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/card");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view filter configuration
    ///
    /// 来源: `lark-cli base +view-get-filter`（Base v3）。
    pub async fn v3_view_get_filter(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/filter"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view group configuration
    ///
    /// 来源: `lark-cli base +view-get-group`（Base v3）。
    pub async fn v3_view_get_group(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/group"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view sort configuration
    ///
    /// 来源: `lark-cli base +view-get-sort`（Base v3）。
    pub async fn v3_view_get_sort(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/sort");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view timebar configuration
    ///
    /// 来源: `lark-cli base +view-get-timebar`（Base v3）。
    pub async fn v3_view_get_timebar(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/timebar"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get view visible fields configuration
    ///
    /// 来源: `lark-cli base +view-get-visible-fields`（Base v3）。
    pub async fn v3_view_get_visible_fields(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/visible_fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List views in a table
    ///
    /// 来源: `lark-cli base +view-list`（Base v3）。
    pub async fn v3_view_list(
        &self,
        base_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Rename a view by ID or name
    ///
    /// 来源: `lark-cli base +view-rename`（Base v3）。
    pub async fn v3_view_rename(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view card configuration
    ///
    /// 来源: `lark-cli base +view-set-card`（Base v3）。
    pub async fn v3_view_set_card(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/card");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view filter configuration
    ///
    /// 来源: `lark-cli base +view-set-filter`（Base v3）。
    pub async fn v3_view_set_filter(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/filter"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view group configuration
    ///
    /// 来源: `lark-cli base +view-set-group`（Base v3）。
    pub async fn v3_view_set_group(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/group"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view sort configuration
    ///
    /// 来源: `lark-cli base +view-set-sort`（Base v3）。
    pub async fn v3_view_set_sort(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/sort");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view timebar configuration
    ///
    /// 来源: `lark-cli base +view-set-timebar`（Base v3）。
    pub async fn v3_view_set_timebar(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/timebar"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Set view visible fields
    ///
    /// 来源: `lark-cli base +view-set-visible-fields`（Base v3）。
    pub async fn v3_view_set_visible_fields(
        &self,
        base_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/tables/{table_id}/views/{view_id}/visible_fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a new workflow in a base
    ///
    /// 来源: `lark-cli base +workflow-create`（Base v3）。
    pub async fn v3_workflow_create(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Disable a workflow in a base
    ///
    /// 来源: `lark-cli base +workflow-disable`（Base v3）。
    pub async fn v3_workflow_disable(&self, base_token: &str, workflow_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("workflow_id", workflow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows/{workflow_id}/disable");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, None, 20, true)
            .await
    }

    /// Enable a workflow in a base
    ///
    /// 来源: `lark-cli base +workflow-enable`（Base v3）。
    pub async fn v3_workflow_enable(&self, base_token: &str, workflow_id: &str) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("workflow_id", workflow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows/{workflow_id}/enable");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, None, 20, true)
            .await
    }

    /// Get a single workflow definition (including steps) from a base
    ///
    /// 来源: `lark-cli base +workflow-get`（Base v3）。
    pub async fn v3_workflow_get(
        &self,
        base_token: &str,
        workflow_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("workflow_id", workflow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows/{workflow_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List all workflows in a base (auto-paginated)
    ///
    /// 来源: `lark-cli base +workflow-list`（Base v3）。
    pub async fn v3_workflow_list(&self, base_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows/list");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Replace a workflow's full definition (title and/or steps) in a base
    ///
    /// 来源: `lark-cli base +workflow-update`（Base v3）。
    pub async fn v3_workflow_update(
        &self,
        base_token: &str,
        workflow_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("base_token", base_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("workflow_id", workflow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/bases/{base_token}/workflows/{workflow_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Create a workspace
    ///
    /// 来源: `lark-cli base +workspace-create`（Base v3）。
    pub async fn v3_workspace_create(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/base/v3/workspaces".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List bases and BaseApps in a workspace
    ///
    /// 来源: `lark-cli base +workspace-entity-list`（Base v3）。
    pub async fn v3_workspace_entity_list(
        &self,
        workspace_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/workspaces/{workspace_id}/entities");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Move an existing Base or BaseApp into a workspace
    ///
    /// 来源: `lark-cli base +workspace-move-in`（Base v3）。
    pub async fn v3_workspace_move_in(&self, workspace_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("workspace_id", workspace_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v3/workspaces/{workspace_id}/move_in");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
