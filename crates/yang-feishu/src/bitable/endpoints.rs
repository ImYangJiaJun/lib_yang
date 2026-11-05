//! 多维表格官方 v1 端点薄封装（自动生成，路径以官方文档为准）。

use serde_json::Value;

use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

use super::BitableClient;

impl BitableClient {
    /// 新增自定义角色
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/advanced-permission/app-role/create-2.md>
    ///
    /// Scope: `base:role:create`
    pub async fn create_role(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v2/apps/{app_token}/roles");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 列出自定义角色
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/advanced-permission/app-role/list-2.md>
    ///
    /// Scope: `base:role:read`
    pub async fn list_roles(&self, app_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v2/apps/{app_token}/roles");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新自定义角色
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/advanced-permission/app-role/update-2.md>
    ///
    /// Scope: `base:role:update`
    pub async fn update_role(&self, app_token: &str, role_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/base/v2/apps/{app_token}/roles/{role_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 复制仪表盘
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-dashboard/copy.md>
    ///
    /// Scope: `base:dashboard:copy, bitable:app`
    pub async fn copy_dashboard(
        &self,
        app_token: &str,
        block_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("block_id", block_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/dashboards/{block_id}/copy");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 列出多维表格中的仪表盘
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-dashboard/list.md>
    ///
    /// Scope: `base:dashboard:read, bitable:app, bitable:app:readonly`
    pub async fn list_dashboards(&self, app_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/dashboards");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 批量新增角色协作者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role-member/batch_create.md>
    ///
    /// Scope: `base:collaborator:create, bitable:app`
    pub async fn batch_create_role_members(
        &self,
        app_token: &str,
        role_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}/members/batch_create");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量删除角色协作者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role-member/batch_delete.md>
    ///
    /// Scope: `base:collaborator:delete, bitable:app`
    pub async fn batch_delete_role_members(
        &self,
        app_token: &str,
        role_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}/members/batch_delete");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 新增角色协作者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role-member/create.md>
    ///
    /// Scope: `base:collaborator:create, bitable:app`
    pub async fn create_role_member(
        &self,
        app_token: &str,
        role_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除角色协作者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role-member/delete.md>
    ///
    /// Scope: `base:collaborator:delete, bitable:app`
    pub async fn delete_role_member(
        &self,
        app_token: &str,
        role_id: &str,
        member_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}/members/{member_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 列出角色协作者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role-member/list.md>
    ///
    /// Scope: `base:collaborator:read, bitable:app, bitable:app:readonly`
    pub async fn list_role_members(
        &self,
        app_token: &str,
        role_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 删除自定义角色
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/advanced-permission/app-role/delete.md>
    ///
    /// Scope: `base:role:delete, bitable:app`
    pub async fn delete_role(&self, app_token: &str, role_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("role_id", role_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/roles/{role_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 新增字段
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-field/create.md>
    ///
    /// Scope: `base:field:create, bitable:app`
    pub async fn create_field(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除字段
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-field/delete.md>
    ///
    /// Scope: `base:field:delete, bitable:app`
    pub async fn delete_field(
        &self,
        app_token: &str,
        table_id: &str,
        field_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 列出数据表中的字段
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-field/list.md>
    ///
    /// Scope: `base:field:read, bitable:app, bitable:app:readonly`
    pub async fn list_fields(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/fields");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新字段属性
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-field/update.md>
    ///
    /// Scope: `base:field:update, bitable:app`
    pub async fn update_field(
        &self,
        app_token: &str,
        table_id: &str,
        field_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 新增多条记录，单次最多 1000 条
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-record/batch_create.md>
    ///
    /// Scope: `base:record:create, bitable:app`
    pub async fn batch_create_records(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_create"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除多条记录，单次最多 500 条
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-record/batch_delete.md>
    ///
    /// Scope: `base:record:delete, bitable:app`
    pub async fn batch_delete_records(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_delete"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量获取记录，最多查询 100 条
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/app-table-record/batch_get.md>
    ///
    /// Scope: `base:record:read, bitable:app, bitable:app:readonly`
    pub async fn batch_get_records(
        &self,
        app_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_get");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 新增记录
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-record/create.md>
    ///
    /// Scope: `base:record:create, bitable:app`
    pub async fn create_record(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除记录
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-record/delete.md>
    ///
    /// Scope: `base:record:delete, bitable:app`
    pub async fn delete_record(
        &self,
        app_token: &str,
        table_id: &str,
        record_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("record_id", record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/{record_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 更新记录
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-record/update.md>
    ///
    /// Scope: `base:record:update, bitable:app`
    pub async fn update_record(
        &self,
        app_token: &str,
        table_id: &str,
        record_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("record_id", record_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records/{record_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 新增视图
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-view/create.md>
    ///
    /// Scope: `base:view:write_only, bitable:app`
    pub async fn create_view(
        &self,
        app_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除视图
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-view/delete.md>
    ///
    /// Scope: `base:view:write_only, bitable:app`
    pub async fn delete_view(
        &self,
        app_token: &str,
        table_id: &str,
        view_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 获取指定视图信息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-view/get.md>
    ///
    /// Scope: `base:view:read, bitable:app, bitable:app:readonly`
    pub async fn get_view(&self, app_token: &str, table_id: &str, view_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 列出数据表中的所有视图
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-view/list.md>
    ///
    /// Scope: `base:view:read, bitable:app, bitable:app:readonly`
    pub async fn list_views(
        &self,
        app_token: &str,
        table_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新视图名称和属性
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table-view/patch.md>
    ///
    /// Scope: `base:view:write_only, bitable:app`
    pub async fn update_view(
        &self,
        app_token: &str,
        table_id: &str,
        view_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("view_id", view_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/views/{view_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 新增多个数据表，最多 100 个
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/batch_create.md>
    ///
    /// Scope: `base:table:create, bitable:app`
    pub async fn batch_create_tables(
        &self,
        app_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/batch_create");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除多个数据表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/batch_delete.md>
    ///
    /// Scope: `base:table:delete, bitable:app`
    pub async fn batch_delete_tables(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/batch_delete");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 新增一个数据表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/create.md>
    ///
    /// Scope: `base:table:create, bitable:app`
    pub async fn create_table(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除一个数据表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/delete.md>
    ///
    /// Scope: `base:table:delete, bitable:app`
    pub async fn delete_table(&self, app_token: &str, table_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 列出多维表格中的数据表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/list.md>
    ///
    /// Scope: `base:table:read, bitable:app, bitable:app:readonly`
    pub async fn list_tables(&self, app_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新数据表名称
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app-table/patch.md>
    ///
    /// Scope: `base:table:update, bitable:app`
    pub async fn update_table(
        &self,
        app_token: &str,
        table_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 列出自动化流程（工作流）
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/app-workflow/list.md>
    ///
    /// Scope: `base:workflow:read, bitable:app, bitable:app:readonly`
    pub async fn list_workflows(&self, app_token: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/workflows");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新自动化流程（工作流）状态
    ///
    /// 官方文档: <https://open.feishu.cn/document/docs/bitable-v1/app-workflow/update.md>
    ///
    /// Scope: `base:workflow:write, bitable:app`
    pub async fn update_workflow_status(
        &self,
        app_token: &str,
        workflow_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("workflow_id", workflow_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/workflows/{workflow_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 复制多维表格到指定文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app/copy.md>
    ///
    /// Scope: `base:app:copy, bitable:app`
    pub async fn copy_app(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/copy");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 创建多维表格，在指定文件夹中创建并包含一个空白数据表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app/create.md>
    ///
    /// Scope: `base:app:create, bitable:app`
    pub async fn create_app(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/bitable/v1/apps".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取多维表格元数据，包括名称、版本号、高级权限状态
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app/get.md>
    ///
    /// Scope: `base:app:read, bitable:app, bitable:app:readonly`
    pub async fn get_app(&self, app_token: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新多维表格元数据，包括名称和高级权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/app/update.md>
    ///
    /// Scope: `base:app:update, bitable:app`
    pub async fn update_app(&self, app_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 获取表单元数据
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/form/get.md>
    ///
    /// Scope: `base:form:read, bitable:app, bitable:app:readonly`
    pub async fn get_form(&self, app_token: &str, table_id: &str, form_id: &str) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/forms/{form_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 列出表单问题
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/form/list.md>
    ///
    /// Scope: `base:form:read, bitable:app, bitable:app:readonly`
    pub async fn list_form_questions(
        &self,
        app_token: &str,
        table_id: &str,
        form_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/forms/{form_id}/fields"
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

    /// 更新表单元数据
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/form/patch-2.md>
    ///
    /// Scope: `base:form:update, bitable:app`
    pub async fn update_form(
        &self,
        app_token: &str,
        table_id: &str,
        form_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/forms/{form_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 更新表单问题
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/bitable-v1/form/patch.md>
    ///
    /// Scope: `base:form:update, bitable:app`
    pub async fn update_form_question(
        &self,
        app_token: &str,
        table_id: &str,
        form_id: &str,
        field_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("app_token", app_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("table_id", table_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("form_id", form_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("field_id", field_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/forms/{form_id}/fields/{field_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }
}
