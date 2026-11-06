//! `pay` 官方端点薄封装（自动生成）。
//!
//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；
//! 重新生成见 `scripts/generate_openapi_endpoints.py`。

use crate::api::ApiClient;
use crate::error::Result;
use crate::transport::Method;

impl ApiClient {
    /// 查询订单信息
    ///
    /// 需要 `tenant_access_token`（客户端自动获取）。
    pub async fn pay_order_get(&self, query: &[(&str, &str)]) -> Result<serde_json::Value> {
        let path = "/open-apis/pay/v1/order/get".to_string();
        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);
        let query = crate::api::query_vec(query);
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
