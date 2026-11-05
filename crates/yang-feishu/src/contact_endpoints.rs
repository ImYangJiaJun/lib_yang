//! 通讯录官方端点薄封装（自动生成，路径以官方文档为准）。

use serde_json::Value;

use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

use crate::contact::ContactClient;

impl ContactClient {
    /// 获取通讯录中单个用户的信息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/contact-v3/user/get.md>
    ///
    /// Scope: `contact:contact.base:readonly, contact:contact:access_as_app, contact:contact:readonly, contact:contact:readonly_as_app`
    pub async fn get_user(&self, user_id: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("user_id", user_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/contact/v3/users/{user_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 按关键词搜索用户
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/contact-v3/user/search-users.md>
    ///
    /// Scope: `contact:user:search`
    pub async fn search_users(&self, bearer_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/search/v1/user".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 按关键词搜索可见的机器人/智能体
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/bot-v4/bot/search.md>
    ///
    /// Scope: `search:bot`
    pub async fn search_bots(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/bot/v4/bot/search".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 批量获取用户个人资料（个人状态、个性签名）
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=profile&resource=user_profile&version=v2>
    ///
    /// Scope: `profile:user_profile:read`
    pub async fn batch_query_user_profiles(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/profile/v2/user_profiles/batch_query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 获取单个部门信息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/contact-v3/department/get.md>
    ///
    /// Scope: `contact:contact.base:readonly, contact:contact:access_as_app, contact:contact:readonly, contact:contact:readonly_as_app`
    pub async fn get_department(
        &self,
        department_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("department_id", department_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/contact/v3/departments/{department_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
