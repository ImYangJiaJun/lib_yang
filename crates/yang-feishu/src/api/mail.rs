//! `mail` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 批量获取邮件组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_list(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/mailgroups".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮件组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/mailgroups".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除邮件组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_delete(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 查询指定邮件组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_get(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改邮件组部分信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_patch(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 修改邮件组全部信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=mailgroup&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_update(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取邮件组所有别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=mailgroup.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_aliases_list(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮件组别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=mailgroup.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_aliases_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除邮件组别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=mailgroup.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_aliases_delete(
        &self,
        mailgroup_id: &str,
        alias_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("alias_id", alias_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/aliases/{alias_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List 批量获取邮件组管理员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=mailgroup.manager&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_managers_list(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/managers");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量创建邮件组管理员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=mail&resource=mailgroup.manager&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_managers_batch_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/managers/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 批量删除邮件组管理员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=mail&resource=mailgroup.manager&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_managers_batch_delete(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/managers/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取所有邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_list(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_delete(
        &self,
        mailgroup_id: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 查询指定邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_get(
        &self,
        mailgroup_id: &str,
        member_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量创建邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_batch_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 批量删除邮件组成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=mail&resource=mailgroup.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_members_batch_delete(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/members/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List 批量获取邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_list(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_delete(
        &self,
        mailgroup_id: &str,
        permission_member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("permission_member_id", permission_member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members/{permission_member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 获取邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_get(
        &self,
        mailgroup_id: &str,
        permission_member_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("permission_member_id", permission_member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members/{permission_member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量创建邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_batch_create(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 批量删除邮件组权限成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=mail&resource=mailgroup.permission_member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_mailgroups_permission_members_batch_delete(
        &self,
        mailgroup_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("mailgroup_id", mailgroup_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/mailgroups/{mailgroup_id}/permission_members/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Search 多实体搜索
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=mail&resource=multi_entity&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_multi_entity_search_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/multi_entity/search".to_string();
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

    /// List 查询所有公共邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/public_mailboxes".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询所有公共邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_public_mailboxes_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/public_mailboxes".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建公共邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/public_mailboxes".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 永久删除公共邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_delete(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 查询指定公共邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_get(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改公共邮箱部分信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_patch(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 修改公共邮箱全部信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_update(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 查询公共邮箱的所有别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=public_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_aliases_list(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建公共邮箱别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=public_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_aliases_create(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除公共邮箱别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=public_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_aliases_delete(
        &self,
        public_mailbox_id: &str,
        alias_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("alias_id", alias_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/aliases/{alias_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List 查询所有公共邮箱成员信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_list(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 添加公共邮箱成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_create(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除公共邮箱单个成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_delete(
        &self,
        public_mailbox_id: &str,
        member_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Get 查询指定公共邮箱成员信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_get(
        &self,
        public_mailbox_id: &str,
        member_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("member_id", member_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members/{member_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量添加公共邮箱成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_batch_create(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members/batch_create");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 批量删除公共邮箱成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_batch_delete(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Clear 删除公共邮箱所有成员
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=clear&project=mail&resource=public_mailbox.member&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_members_clear(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/members/clear");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// RemoveToRecycleBin 将公共邮箱移至回收站
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remove_to_recycle_bin&project=mail&resource=public_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_public_mailboxes_remove_to_recycle_bin(
        &self,
        public_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("public_mailbox_id", public_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/public_mailboxes/{public_mailbox_id}/remove_to_recycle_bin"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 从回收站删除用户邮箱地址
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_delete(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// AccessibleMailboxes 列出可访问的邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=accessible_mailboxes&project=mail&resource=user_mailbox&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_accessible_mailboxes(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/accessible_mailboxes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// AccessibleMailboxes 列出可访问的邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=accessible_mailboxes&project=mail&resource=user_mailbox&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_accessible_mailboxes_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/accessible_mailboxes");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取用户邮箱所有别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_aliases_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建用户邮箱别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_aliases_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/aliases");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除用户邮箱别名
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.alias&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_aliases_delete(
        &self,
        user_mailbox_id: &str,
        alias_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("alias_id", alias_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/aliases/{alias_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// List 列出草稿列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建草稿
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts");
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

    /// Delete 删除草稿
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        draft_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("draft_id", draft_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts/{draft_id}");
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

    /// Get 获取草稿内容
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        draft_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("draft_id", draft_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts/{draft_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update 更新草稿
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_update_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        draft_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("draft_id", draft_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts/{draft_id}");
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

    /// Send 发送草稿
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=send&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_drafts_send_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        draft_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("draft_id", draft_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/drafts/{draft_id}/send");
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

    /// Subscribe 订阅事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=mail&resource=user_mailbox.event&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_event_subscribe_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/event/subscribe");
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

    /// Subscription 获取订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=mail&resource=user_mailbox.event&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_event_subscription_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/event/subscription");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Unsubscribe 取消订阅
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscribe&project=mail&resource=user_mailbox.event&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_event_unsubscribe_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/event/unsubscribe");
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

    /// List 列出邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_folders_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_folders_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_folders_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_folders_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders");
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

    /// Delete 删除邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_folders_delete(
        &self,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_folders_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
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

    /// Get 获取邮箱文件夹信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_folders_get(
        &self,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取邮箱文件夹信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_folders_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 修改邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_folders_patch(
        &self,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 修改邮箱文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.folder&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_folders_patch_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        folder_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("folder_id", folder_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/folders/{folder_id}");
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

    /// List 列出标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_labels_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_labels_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_labels_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_labels_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels");
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

    /// Delete 删除标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_labels_delete(
        &self,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_labels_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
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

    /// Get 获取标签信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_labels_get(
        &self,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取标签信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_labels_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_labels_patch(
        &self,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新标签
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.label&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_labels_patch_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        label_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("label_id", label_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/labels/{label_id}");
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

    /// List 列出邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_mail_contacts_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_mail_contacts_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_mail_contacts_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_mail_contacts_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts");
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

    /// Delete 删除邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_mail_contacts_delete(
        &self,
        user_mailbox_id: &str,
        mail_contact_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("mail_contact_id", mail_contact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts/{mail_contact_id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除邮箱联系人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_mail_contacts_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        mail_contact_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("mail_contact_id", mail_contact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts/{mail_contact_id}"
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

    /// Patch 修改邮箱联系人信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_mail_contacts_patch(
        &self,
        user_mailbox_id: &str,
        mail_contact_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("mail_contact_id", mail_contact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts/{mail_contact_id}"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 修改邮箱联系人信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=mail&resource=user_mailbox.mail_contact&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_mail_contacts_patch_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        mail_contact_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("mail_contact_id", mail_contact_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/mail_contacts/{mail_contact_id}"
        );
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

    /// List 列出邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取邮件详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_get(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取邮件详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// DownloadUrl 获取附件下载链接
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_url&project=mail&resource=user_mailbox.message.attachment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_attachments_download_url(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/attachments/download_url");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// DownloadUrl 获取附件下载链接
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_url&project=mail&resource=user_mailbox.message.attachment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_attachments_download_url_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/attachments/download_url");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// CancelScheduledSend 取消定时发送
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=cancel_scheduled_send&project=mail&resource=user_mailbox.draft&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_cancel_scheduled_send_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/cancel_scheduled_send");
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

    /// Modify 修改邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=modify&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_modify(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/modify"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Modify 修改邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=modify&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_modify_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/modify"
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

    /// GetRecallDetail 查询邮件撤回进度
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_recall_detail&project=mail&resource=user_mailbox.sent_message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_recall_get_recall_detail(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/recall"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetRecallDetail 查询邮件撤回进度
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_recall_detail&project=mail&resource=user_mailbox.sent_message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_recall_get_recall_detail_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/recall"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Recall 撤回已发送的邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=recall&project=mail&resource=user_mailbox.sent_message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_recall(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/recall"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Recall 撤回已发送的邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=recall&project=mail&resource=user_mailbox.sent_message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_recall_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/recall"
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

    /// SendStatus
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=send_status&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_send_status_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/send_status"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Trash 删除邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=trash&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_trash(
        &self,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/trash"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Trash 删除邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=trash&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_trash_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        message_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/{message_id}/trash"
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

    /// BatchGet 批量获取邮件详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_get&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_batch_get(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_get");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchGet 批量获取邮件详情
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_get&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_batch_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_get");
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

    /// BatchModify 批量修改邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_modify&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_batch_modify(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_modify");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchModify 批量修改邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_modify&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_batch_modify_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_modify");
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

    /// BatchTrash 批量删除邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_trash&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_batch_trash(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_trash");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchTrash 批量删除邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_trash&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_batch_trash_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/batch_trash");
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

    /// GetByCard 获取邮件卡片的邮件列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_by_card&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_messages_get_by_card(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/get_by_card");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetByCard 获取邮件卡片的邮件列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_by_card&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_get_by_card_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/get_by_card");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Send 发送邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=send&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_messages_send_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/messages/send");
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

    /// Profile 获取用户邮箱信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=profile&project=mail&resource=user_mailbox&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_profile_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/profile");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_rules_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_rules_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_rules_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_rules_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules");
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

    /// Delete 删除收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_rules_delete(
        &self,
        user_mailbox_id: &str,
        rule_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("rule_id", rule_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/{rule_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_rules_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        rule_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("rule_id", rule_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/{rule_id}");
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

    /// Update 更新收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_rules_update(
        &self,
        user_mailbox_id: &str,
        rule_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("rule_id", rule_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/{rule_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update 更新收信规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_rules_update_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        rule_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("rule_id", rule_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/{rule_id}");
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

    /// Reorder 对收信规则进行排序
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reorder&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_rules_reorder(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/reorder");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Reorder 对收信规则进行排序
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reorder&project=mail&resource=user_mailbox.rule&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_rules_reorder_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/rules/reorder");
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

    /// Search 搜索邮件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=mail&resource=user_mailbox&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_search_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/search");
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

    /// SendAs 列出可发信邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=send_as&project=mail&resource=user_mailbox.setting&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_settings_send_as(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/settings/send_as");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// SendAs 列出可发信邮箱
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=send_as&project=mail&resource=user_mailbox.setting&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_settings_send_as_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/settings/send_as");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// GetSignatures 获取用户的签名列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get_signatures&project=mail&resource=user_mailbox.setting&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_settings_signatures_get_signatures_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/settings/signatures");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_create(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_create_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates");
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

    /// Delete
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_delete(
        &self,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_delete_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
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

    /// Get
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_get(
        &self,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Update
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_update(
        &self,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Update
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=mail&resource=user_mailbox.template&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_update_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}");
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

    /// DownloadUrl
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_url&project=mail&resource=user_mailbox.template.attachment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_templates_attachments_download_url(
        &self,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}/attachments/download_url");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// DownloadUrl
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=download_url&project=mail&resource=user_mailbox.template.attachment&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_templates_attachments_download_url_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        template_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("template_id", template_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/templates/{template_id}/attachments/download_url");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_list(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 列出邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_list_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取邮件会话邮件列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_get(
        &self,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取邮件会话邮件列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_get_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// ListThreadMessage 查询会话邮件信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list_thread_message&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_messages_list_thread_message(
        &self,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/messages"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// ListThreadMessage 查询会话邮件信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list_thread_message&project=mail&resource=user_mailbox.message&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_messages_list_thread_message_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/messages"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Modify 修改邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=modify&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_modify(
        &self,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/modify"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Modify 修改邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=modify&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_modify_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/modify"
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

    /// Trash 删除邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=trash&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_trash(
        &self,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/trash"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Trash 删除邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=trash&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_trash_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        thread_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/{thread_id}/trash"
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

    /// BatchModify 批量修改邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_modify&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_batch_modify(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/batch_modify");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchModify 批量修改邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_modify&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_batch_modify_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/batch_modify");
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

    /// BatchTrash 批量删除邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_trash&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_user_mailboxes_threads_batch_trash(
        &self,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/batch_trash");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchTrash 批量删除邮件会话
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_trash&project=mail&resource=user_mailbox.thread&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn mail_user_mailboxes_threads_batch_trash_user(
        &self,
        bearer_token: &str,
        user_mailbox_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("user_mailbox_id", user_mailbox_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/mail/v1/user_mailboxes/{user_mailbox_id}/threads/batch_trash");
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

    /// Query 查询邮箱地址状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=mail&resource=user&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn mail_users_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/mail/v1/users/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
