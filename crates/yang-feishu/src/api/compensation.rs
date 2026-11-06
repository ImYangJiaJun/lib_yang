//! `compensation` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// Create 创建薪资档案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=compensation&resource=archive&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_archives_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/archives".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Create 创建薪资档案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=compensation&resource=archive&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn compensation_archives_create_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/archives".to_string();
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

    /// Query 批量查询员工薪资档案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=archive&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_archives_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/archives/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 批量查询员工薪资档案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=archive&version=v1>
    /// 需要 `user_access_token`，由调用方传入 `bearer_token`。
    pub async fn compensation_archives_query_user(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/archives/query".to_string();
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

    /// List 批量查询定调薪原因
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=change_reason&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_change_reasons_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/change_reasons".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 批量查询薪资统计指标
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=indicator&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_indicators_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/indicators".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 批量获取薪资项分类信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=item_category&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_item_categories_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/item_categories".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 批量查询薪资项
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=item&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_items_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/items".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量创建一次性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=compensation&resource=lump_sum_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_lump_sum_payment_batch_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/lump_sum_payment/batch_create".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchRemove 批量删除一次性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_remove&project=compensation&resource=lump_sum_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_lump_sum_payment_batch_remove(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/lump_sum_payment/batch_remove".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 批量更正一次性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=compensation&resource=lump_sum_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_lump_sum_payment_batch_update(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/lump_sum_payment/batch_update".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 查询一次性支付授予记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=lump_sum_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_lump_sum_payment_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/lump_sum_payment/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// QueryDetail 查询一次性授予明细
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query_detail&project=compensation&resource=lump_sum_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_lump_sum_payment_query_detail(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/lump_sum_payment/query_detail".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 批量查询薪资方案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=plan&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_plans_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/plans".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// BatchCreate 批量创建经常性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_create&project=compensation&resource=recurring_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_recurring_payment_batch_create(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/recurring_payment/batch_create".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchRemove 批量删除经常性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_remove&project=compensation&resource=recurring_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_recurring_payment_batch_remove(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/recurring_payment/batch_remove".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// BatchUpdate 批量更正经常性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=compensation&resource=recurring_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_recurring_payment_batch_update(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/recurring_payment/batch_update".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 查询经常性支付记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=recurring_payment&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_recurring_payment_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/recurring_payment/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 批量获取员工参保档案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=social_archive&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_social_archive_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/social_archive/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// Query 批量获取社保增减员记录
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=social_archive_adjust_record&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_social_archive_adjust_record_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/social_archive_adjust_record/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// List 获取险种配置列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=social_insurance&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_social_insurances_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/social_insurances".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// List 根据生效日期分页查询参保方案
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=compensation&resource=social_plan&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_social_plans_list(
        &self,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/social_plans".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Query 根据方案ID和生效日期批量查询参保方案;
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=query&project=compensation&resource=social_plan&version=v1>
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn compensation_social_plans_query(
        &self,
        query: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = "/open-apis/compensation/v1/social_plans/query".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }
}
