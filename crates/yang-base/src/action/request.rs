//! Action 请求结构
//!
//! 提供 Action 系统的请求封装，包含请求体、请求头、查询参数和路径参数。

use std::collections::HashMap;

fn parse_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;

    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }

    Some(token)
}

/// Action 请求
///
/// 封装 HTTP 请求信息，用于 Action 执行
///
/// # 字段
///
/// - `body`: 请求体（JSON 格式）
/// - `headers`: 请求头
/// - `query`: 查询参数
/// - `path_params`: 路径参数
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::Request;
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// // 创建请求
/// let mut request = Request::new(json!({
///     "username": "alice",
///     "email": "alice@example.com"
/// }));
///
/// // 添加请求头
/// request = request.header("Content-Type", "application/json");
/// request = request.header("Authorization", "Bearer token123");
///
/// // 添加查询参数
/// request = request.query("page", "1");
/// request = request.query("limit", "10");
///
/// // 添加路径参数
/// request = request.path_param("id", "123");
///
/// // 提取 Token
/// if let Some(token) = request.token() {
///     println!("Token: {}", token);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Request {
    /// 请求体（JSON）
    pub body: serde_json::Value,

    /// 请求头
    pub headers: HashMap<String, String>,

    /// 查询参数
    pub query: HashMap<String, String>,

    /// 路径参数
    pub path_params: HashMap<String, String>,
}

impl Request {
    /// 创建新请求
    ///
    /// # 参数
    ///
    /// - `body`: 请求体（JSON 格式）
    ///
    /// # 返回
    ///
    /// - 新的 Request 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({
    ///     "name": "Alice",
    ///     "age": 30
    /// }));
    /// ```
    pub fn new(body: serde_json::Value) -> Self {
        Self {
            body,
            headers: HashMap::new(),
            query: HashMap::new(),
            path_params: HashMap::new(),
        }
    }

    /// 添加请求头
    ///
    /// # 参数
    ///
    /// - `name`: 请求头名称
    /// - `value`: 请求头值
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .header("Content-Type", "application/json")
    ///     .header("Authorization", "Bearer token123");
    /// ```
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        if name.trim().is_empty() {
            return self;
        }

        self.headers.insert(name.to_ascii_lowercase(), value.into());
        self
    }

    /// 批量添加请求头
    ///
    /// # 参数
    ///
    /// - `headers`: 请求头映射
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let mut headers = HashMap::new();
    /// headers.insert("Content-Type".to_string(), "application/json".to_string());
    /// headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    ///
    /// let request = Request::new(json!({}))
    ///     .headers(headers);
    /// ```
    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        for (name, value) in headers {
            self = self.header(name, value);
        }
        self
    }

    /// 添加查询参数
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    /// - `value`: 参数值
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .query("page", "1")
    ///     .query("limit", "10");
    /// ```
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        if key.trim().is_empty() {
            return self;
        }

        self.query.insert(key, value.into());
        self
    }

    /// 批量添加查询参数
    ///
    /// # 参数
    ///
    /// - `query`: 查询参数映射
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let mut query = HashMap::new();
    /// query.insert("page".to_string(), "1".to_string());
    /// query.insert("limit".to_string(), "10".to_string());
    ///
    /// let request = Request::new(json!({}))
    ///     .queries(query);
    /// ```
    pub fn queries(mut self, query: HashMap<String, String>) -> Self {
        for (key, value) in query {
            self = self.query(key, value);
        }
        self
    }

    /// 添加路径参数
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    /// - `value`: 参数值
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .path_param("id", "123")
    ///     .path_param("action", "update");
    /// ```
    pub fn path_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        if key.trim().is_empty() {
            return self;
        }

        self.path_params.insert(key, value.into());
        self
    }

    /// 批量添加路径参数
    ///
    /// # 参数
    ///
    /// - `path_params`: 路径参数映射
    ///
    /// # 返回
    ///
    /// - 修改后的 Request 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    /// use std::collections::HashMap;
    ///
    /// let mut path_params = HashMap::new();
    /// path_params.insert("id".to_string(), "123".to_string());
    /// path_params.insert("action".to_string(), "update".to_string());
    ///
    /// let request = Request::new(json!({}))
    ///     .path_params(path_params);
    /// ```
    pub fn path_params(mut self, path_params: HashMap<String, String>) -> Self {
        for (key, value) in path_params {
            self = self.path_param(key, value);
        }
        self
    }

    /// 从 Authorization 头提取 Token
    ///
    /// 支持 Bearer Token 格式：`Authorization: Bearer <token>`
    ///
    /// # 返回
    ///
    /// - `Some(&str)`: Token 字符串
    /// - `None`: 未找到 Token 或格式不正确
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .header("Authorization", "Bearer my_secret_token");
    ///
    /// assert_eq!(request.token(), Some("my_secret_token"));
    /// ```
    pub fn token(&self) -> Option<&str> {
        self.get_header("authorization")
            .and_then(parse_bearer_token)
    }

    /// 获取请求头值
    ///
    /// # 参数
    ///
    /// - `name`: 请求头名称
    ///
    /// # 返回
    ///
    /// - `Some(&str)`: 请求头值
    /// - `None`: 请求头不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .header("Content-Type", "application/json");
    ///
    /// assert_eq!(request.get_header("Content-Type"), Some("application/json"));
    /// ```
    pub fn get_header(&self, name: &str) -> Option<&str> {
        if name.trim().is_empty() {
            return None;
        }

        self.headers
            .get(name)
            .or_else(|| self.headers.iter().find_map(|(key, value)| {
                key.eq_ignore_ascii_case(name).then_some(value)
            }))
            .map(|s| s.as_str())
    }

    /// 获取查询参数值
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    ///
    /// # 返回
    ///
    /// - `Some(&str)`: 参数值
    /// - `None`: 参数不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .query("page", "1");
    ///
    /// assert_eq!(request.get_query("page"), Some("1"));
    /// ```
    pub fn get_query(&self, key: &str) -> Option<&str> {
        if key.trim().is_empty() {
            return None;
        }

        self.query.get(key).map(|s| s.as_str())
    }

    /// 获取路径参数值
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    ///
    /// # 返回
    ///
    /// - `Some(&str)`: 参数值
    /// - `None`: 参数不存在
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Request;
    /// use serde_json::json;
    ///
    /// let request = Request::new(json!({}))
    ///     .path_param("id", "123");
    ///
    /// assert_eq!(request.get_path_param("id"), Some("123"));
    /// ```
    pub fn get_path_param(&self, key: &str) -> Option<&str> {
        self.path_params.get(key).map(|s| s.as_str())
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::new(serde_json::Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_header_lookup_is_case_insensitive() {
        let request = Request::new(json!({}))
            .header("Content-Type", "application/json")
            .header("aUtHoRiZaTiOn", "Bearer token123");

        assert_eq!(request.get_header("content-type"), Some("application/json"));
        assert_eq!(request.get_header("AUTHORIZATION"), Some("Bearer token123"));
        assert_eq!(request.token(), Some("token123"));
    }

    #[test]
    fn test_header_insert_normalizes_name_and_overwrites_case_variants() {
        let request = Request::new(json!({}))
            .header("Authorization", "Bearer old")
            .header("authorization", "Bearer new");

        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.headers.get("authorization").map(String::as_str), Some("Bearer new"));
        assert_eq!(request.get_header("Authorization"), Some("Bearer new"));
        assert_eq!(request.token(), Some("new"));
    }

    #[test]
    fn test_header_rejects_blank_names() {
        let request = Request::new(json!({}))
            .header("", "empty")
            .header("   ", "blank")
            .header("X-Request-Id", "abc");

        assert_eq!(request.get_header("x-request-id"), Some("abc"));
        assert_eq!(request.get_header(""), None);
        assert_eq!(request.get_header("   "), None);
    }

    #[test]
    fn test_get_header_rejects_blank_names() {
        let mut request = Request::new(json!({}));
        request.headers.insert("".to_string(), "empty".to_string());
        request.headers.insert("   ".to_string(), "blank".to_string());
        request
            .headers
            .insert("x-request-id".to_string(), "abc".to_string());

        assert_eq!(request.get_header("x-request-id"), Some("abc"));
        assert_eq!(request.get_header(""), None);
        assert_eq!(request.get_header("   "), None);
    }

    #[test]
    fn test_token_accepts_case_insensitive_bearer_scheme() {
        let lower = Request::new(json!({})).header("authorization", "bearer lower-token");
        let upper = Request::new(json!({})).header("authorization", "BEARER upper-token");

        assert_eq!(lower.token(), Some("lower-token"));
        assert_eq!(upper.token(), Some("upper-token"));
    }

    #[test]
    fn test_token_trims_scheme_spacing_and_rejects_empty_or_split_token() {
        let padded = Request::new(json!({})).header("authorization", "Bearer    padded-token");
        let empty = Request::new(json!({})).header("authorization", "Bearer ");
        let split = Request::new(json!({})).header("authorization", "Bearer token extra");

        assert_eq!(padded.token(), Some("padded-token"));
        assert_eq!(empty.token(), None);
        assert_eq!(split.token(), None);
    }

    #[test]
    fn test_query_rejects_blank_keys() {
        let request = Request::new(json!({}))
            .query("", "empty")
            .query("   ", "blank")
            .query("page", "1");

        let mut query = std::collections::HashMap::new();
        query.insert("".to_string(), "empty-bulk".to_string());
        query.insert("   ".to_string(), "blank-bulk".to_string());
        query.insert("limit".to_string(), "10".to_string());

        let request = request.queries(query);

        assert_eq!(request.get_query("page"), Some("1"));
        assert_eq!(request.get_query("limit"), Some("10"));
        assert_eq!(request.get_query(""), None);
        assert_eq!(request.get_query("   "), None);
    }

    #[test]
    fn test_get_query_rejects_blank_keys() {
        let mut request = Request::new(json!({}));
        request.query.insert("".to_string(), "empty".to_string());
        request.query.insert("   ".to_string(), "blank".to_string());
        request.query.insert("page".to_string(), "1".to_string());

        assert_eq!(request.get_query("page"), Some("1"));
        assert_eq!(request.get_query(""), None);
        assert_eq!(request.get_query("   "), None);
    }

    #[test]
    fn test_path_param_rejects_blank_keys() {
        let request = Request::new(json!({}))
            .path_param("", "empty")
            .path_param("   ", "blank")
            .path_param("id", "42");

        let mut path_params = std::collections::HashMap::new();
        path_params.insert("".to_string(), "empty-bulk".to_string());
        path_params.insert("   ".to_string(), "blank-bulk".to_string());
        path_params.insert("slug".to_string(), "demo".to_string());

        let request = request.path_params(path_params);

        assert_eq!(request.get_path_param("id"), Some("42"));
        assert_eq!(request.get_path_param("slug"), Some("demo"));
        assert_eq!(request.get_path_param(""), None);
        assert_eq!(request.get_path_param("   "), None);
    }
}
