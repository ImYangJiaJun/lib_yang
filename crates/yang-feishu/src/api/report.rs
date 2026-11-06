//! `report` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

impl ApiClient {
    /// Remove 移除规则看板
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=remove&project=report&resource=rule.view&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn report_rules_views_remove(
        &self,
        rule_id: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        validate_opaque_path_segment("rule_id", rule_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/report/v1/rules/{rule_id}/views/remove");
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 查询规则
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=report&resource=rule&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn report_rules_query(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/report/v1/rules/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 查询任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=report&resource=task&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn report_tasks_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/report/v1/tasks/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 查询任务
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=report&resource=task&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn report_tasks_query_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/report/v1/tasks/query".to_string();
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
