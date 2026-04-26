//! Action 响应结构
//!
//! 提供统一的 API 响应格式，包含状态码、消息和数据。

use crate::error::BaseError;
use serde::Serialize;

/// API 响应
///
/// 统一的 API 响应格式，用于所有 Action 的返回值
///
/// # 字段
///
/// - `code`: 状态码（0 表示成功，非零表示失败）
/// - `message`: 响应消息
/// - `data`: 响应数据（可选）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::ApiResponse;
/// use serde_json::json;
///
/// // 创建成功响应
/// let response = ApiResponse::success(
///     json!({ "id": 123, "name": "Alice" }),
///     "操作成功"
/// );
/// assert_eq!(response.code, 0);
///
/// // 创建失败响应
/// let response = ApiResponse::fail(400001, "参数错误");
/// assert_eq!(response.code, 400001);
/// assert!(response.data.is_none());
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse {
    /// 状态码
    ///
    /// - 0: 成功
    /// - 非零: 失败（具体错误码由业务定义）
    pub code: i32,

    /// 响应消息
    ///
    /// 描述操作结果的文本信息
    pub message: String,

    /// 响应数据
    ///
    /// 成功时包含业务数据，失败时通常为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ApiResponse {
    /// 创建成功响应
    ///
    /// 状态码为 0，包含业务数据
    ///
    /// # 参数
    ///
    /// - `data`: 响应数据（任何可序列化的类型）
    /// - `message`: 成功消息
    ///
    /// # 返回
    ///
    /// - ApiResponse 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::ApiResponse;
    /// use serde_json::json;
    ///
    /// // 返回单个对象
    /// let response = ApiResponse::success(
    ///     json!({ "id": 1, "name": "Alice" }),
    ///     "获取成功"
    /// );
    ///
    /// // 返回列表
    /// let response = ApiResponse::success(
    ///     json!([
    ///         { "id": 1, "name": "Alice" },
    ///         { "id": 2, "name": "Bob" }
    ///     ]),
    ///     "查询成功"
    /// );
    ///
    /// // 返回影响行数
    /// let response = ApiResponse::success(
    ///     json!({ "affected": 1 }),
    ///     "更新成功"
    /// );
    /// ```
    pub fn success(data: impl Serialize, message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
        }
    }

    /// 创建失败响应
    ///
    /// 状态码为非零错误码，不包含数据
    ///
    /// # 参数
    ///
    /// - `code`: 错误码（非零整数）
    /// - `message`: 错误消息
    ///
    /// # 返回
    ///
    /// - ApiResponse 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::ApiResponse;
    ///
    /// // 参数错误
    /// let response = ApiResponse::fail(400001, "参数缺失: username");
    ///
    /// // 权限错误
    /// let response = ApiResponse::fail(403001, "权限不足");
    ///
    /// // 业务错误
    /// let response = ApiResponse::fail(500001, "用户名已存在");
    /// ```
    pub fn fail(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// 从 BaseError 创建失败响应
    ///
    /// 自动提取错误码和错误消息
    ///
    /// # 参数
    ///
    /// - `error`: BaseError 实例
    ///
    /// # 返回
    ///
    /// - ApiResponse 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::ApiResponse;
    /// use yang_base::error::BaseError;
    ///
    /// let error = BaseError::FieldRequired("username".to_string());
    /// let response = ApiResponse::from_error(error);
    ///
    /// assert_ne!(response.code, 0);
    /// assert!(response.message.contains("username"));
    /// assert!(response.data.is_none());
    /// ```
    pub fn from_error(error: BaseError) -> Self {
        Self::fail(error.code(), error.to_string())
    }
}

impl Default for ApiResponse {
    /// 创建默认响应
    ///
    /// 默认为成功响应，消息为 "OK"，无数据
    fn default() -> Self {
        Self {
            code: 0,
            message: "OK".to_string(),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_success_response() {
        let response = ApiResponse::success(json!({ "id": 123, "name": "Alice" }), "操作成功");

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "操作成功");
        assert!(response.data.is_some());

        let data = response.data.unwrap();
        assert_eq!(data["id"], 123);
        assert_eq!(data["name"], "Alice");
    }

    #[test]
    fn test_success_response_with_null() {
        let response = ApiResponse::success(serde_json::Value::Null, "操作成功");

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "操作成功");
        assert!(response.data.is_some());
        assert_eq!(response.data.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn test_fail_response() {
        let response = ApiResponse::fail(400001, "参数错误");

        assert_eq!(response.code, 400001);
        assert_eq!(response.message, "参数错误");
        assert!(response.data.is_none());
    }

    #[test]
    fn test_from_error() {
        let error = BaseError::FieldRequired("username".to_string());
        let response = ApiResponse::from_error(error);

        assert_ne!(response.code, 0);
        assert!(response.message.contains("username"));
        assert!(response.data.is_none());
    }

    #[test]
    fn test_default_response() {
        let response = ApiResponse::default();

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "OK");
        assert!(response.data.is_none());
    }

    #[test]
    fn test_serialize_response() {
        let response = ApiResponse::success(json!({ "count": 10 }), "查询成功");

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":0"));
        assert!(json.contains("\"message\":\"查询成功\""));
        assert!(json.contains("\"data\""));
    }

    #[test]
    fn test_serialize_fail_response() {
        let response = ApiResponse::fail(500001, "服务器错误");

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":500001"));
        assert!(json.contains("\"message\":\"服务器错误\""));
        // data 字段应该被跳过
        assert!(!json.contains("\"data\""));
    }
}
