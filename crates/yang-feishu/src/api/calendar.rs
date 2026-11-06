//! `calendar` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// List 查询日历列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 查询日历列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建共享日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建共享日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars".to_string();
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

    /// Delete 删除共享日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_delete(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除共享日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_delete_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
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

    /// Get 查询日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_get(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 查询日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_get_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_patch(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_patch_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}");
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

    /// List 获取访问控制列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_acls_list(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取访问控制列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_acls_list_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建访问控制
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_acls_create(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建访问控制
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_acls_create_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls");
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

    /// Delete 删除访问控制
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_acls_delete(
        &self,
        calendar_id: &str,
        acl_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("acl_id", acl_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls/{acl_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除访问控制
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_acls_delete_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        acl_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("acl_id", acl_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls/{acl_id}");
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

    /// Subscription 订阅日历访问控制变更事件;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_acls_subscription_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls/subscription");
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

    /// Unsubscription 取消订阅日历访问控制变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=calendar&resource=calendar.acl&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_acls_unsubscription_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/acls/unsubscription");
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

    /// List 获取日程列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_list(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取日程列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_list_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 创建日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_create(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_create_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events");
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

    /// Delete 删除日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_delete(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 删除日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_delete_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
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

    /// Get 获取日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_get(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Get 获取日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_get_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Patch 更新日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_patch(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Patch 更新日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_patch_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}");
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

    /// List 获取日程参与人列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_attendees_list(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取日程参与人列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_attendees_list_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Create 添加日程参与人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_attendees_create(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 添加日程参与人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_attendees_create_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees");
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

    /// List 获取日程参与群成员列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event.attendee.chat_member&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_attendees_chat_members_list(
        &self,
        calendar_id: &str,
        event_id: &str,
        attendee_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("attendee_id", attendee_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees/{attendee_id}/chat_members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 获取日程参与群成员列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=calendar.event.attendee.chat_member&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_attendees_chat_members_list_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        attendee_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("attendee_id", attendee_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees/{attendee_id}/chat_members");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchDelete 删除日程参与人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_attendees_batch_delete(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees/batch_delete");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchDelete 删除日程参与人
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_delete&project=calendar&resource=calendar.event.attendee&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_attendees_batch_delete_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/attendees/batch_delete");
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

    /// Instances 获取重复日程实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=instances&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_instances(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/instances");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Instances 获取重复日程实例
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=instances&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_instances_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/instances");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Delete 解绑会议群
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.event.meeting_chat&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_meeting_chat_delete(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_chat"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// Delete 解绑会议群
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=calendar.event.meeting_chat&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_meeting_chat_delete_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_chat"
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

    /// Create 创建会议群
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.meeting_chat&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_meeting_chat_create(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_chat"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建会议群
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.meeting_chat&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_meeting_chat_create_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_chat"
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

    /// Create 创建会议纪要
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.meeting_minute&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_meeting_minute_create(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_minute"
        );
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建会议纪要
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=calendar.event.meeting_minute&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_meeting_minute_create_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/meeting_minute"
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

    /// Reply 回复日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reply&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_reply(
        &self,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/reply");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Reply 回复日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=reply&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_reply_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("event_id", event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path =
            format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/{event_id}/reply");
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

    /// InstanceView 查询日程视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=instance_view&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_instance_view(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/instance_view");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// InstanceView 查询日程视图
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=instance_view&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_instance_view_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/instance_view");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Search 搜索日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_search(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/search");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 搜索日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_search_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/search");
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

    /// Subscription 订阅日程变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_subscription(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/subscription");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Subscription 订阅日程变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_subscription_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/subscription");
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

    /// Unsubscription 取消订阅日程变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_events_unsubscription(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/unsubscription");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscription 取消订阅日程变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=calendar&resource=calendar.event&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_events_unsubscription_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/events/unsubscription");
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

    /// Subscribe 订阅日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_subscribe(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/subscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Subscribe 订阅日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscribe&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_subscribe_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/subscribe");
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

    /// Unsubscribe 取消订阅日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscribe&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_unsubscribe(
        &self,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/unsubscribe");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Unsubscribe 取消订阅日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscribe&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_unsubscribe_user(
        &self,
        bearer_token: &str,
        calendar_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("calendar_id", calendar_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/calendars/{calendar_id}/unsubscribe");
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

    /// Mget 批量查询日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=mget&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_mget(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/mget".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Mget 批量查询日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=mget&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_mget_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/mget".to_string();
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

    /// Primary 查询主日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=primary&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_primary(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/primary".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Primary 查询主日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=primary&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_primary_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/primary".to_string();
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

    /// Primarys 批量获取主日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=primarys&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_primarys(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/primarys".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Primarys 批量获取主日历信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=primarys&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_primarys_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/primarys".to_string();
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

    /// Search 搜索日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=calendar&resource=calendar&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_calendars_search(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/search".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Search 搜索日历
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=search&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_search_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/search".to_string();
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

    /// Subscription 订阅日历变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=subscription&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_subscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/subscription".to_string();
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

    /// Unsubscription 取消订阅日历变更事件
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=unsubscription&project=calendar&resource=calendar&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_calendars_unsubscription_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/calendars/unsubscription".to_string();
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

    /// Create 将 Exchange 账户绑定到飞书账户
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=exchange_binding&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_exchange_bindings_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/exchange_bindings".to_string();
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

    /// Delete 解除 Exchange 账户绑定
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=exchange_binding&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_exchange_bindings_delete_user(
        &self,
        bearer_token: &str,
        exchange_binding_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("exchange_binding_id", exchange_binding_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/exchange_bindings/{exchange_binding_id}");
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

    /// Get 查询 Exchange 账户的绑定状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=calendar&resource=exchange_binding&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_exchange_bindings_get_user(
        &self,
        bearer_token: &str,
        exchange_binding_id: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("exchange_binding_id", exchange_binding_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/exchange_bindings/{exchange_binding_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// Batch 批量查询主日历日程忙闲信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch&project=calendar&resource=freebusy&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_freebusy_batch(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/freebusy/batch".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Batch 批量查询主日历日程忙闲信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch&project=calendar&resource=freebusy&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_freebusy_batch_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/freebusy/batch".to_string();
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

    /// List 查询主日历日程忙闲信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=freebusy&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_freebusy_list(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/freebusy/list".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 查询主日历日程忙闲信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=calendar&resource=freebusy&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_freebusy_list_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/freebusy/list".to_string();
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

    /// GenerateCaldavConf 生成 CalDAV 配置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=generate_caldav_conf&project=calendar&resource=setting&version=v4>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn calendar_settings_generate_caldav_conf_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/settings/generate_caldav_conf".to_string();
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

    /// Create 创建请假日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=calendar&resource=timeoff_event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_timeoff_events_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/calendar/v4/timeoff_events".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Delete 删除请假日程
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=calendar&resource=timeoff_event&version=v4>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn calendar_timeoff_events_delete(
        &self,
        timeoff_event_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("timeoff_event_id", timeoff_event_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/calendar/v4/timeoff_events/{timeoff_event_id}");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }
}
