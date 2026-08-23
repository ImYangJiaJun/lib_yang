//! 浏览器刷新会话的 Cookie 与同源边界。
//!
//! 浏览器会话把 Refresh Token 放在 Host-only、HttpOnly、SameSite=Strict 的 Cookie 里，
//! Access Token 只出现在响应体。本模块提供：
//!
//! - Cookie 的签发/清除（名称与 Path 作用域由构造函数注入）；
//! - 变更类端点的同源校验（`Sec-Fetch-Site` + `Origin`/`Referer` 与 `Host` 比对），
//!   通过校验时返回请求是否为 HTTPS，供调用方决定 Cookie 是否带 `Secure`。

use crate::action::{ApiResponse, Request};
use schemars::JsonSchema;
use serde::Serialize;
use yang_base::BaseError;

/// 仅含 Access Token 的浏览器会话响应。
#[derive(Debug, Serialize, JsonSchema)]
pub struct BrowserAccessToken {
    /// 新签发的 Access Token。
    pub access_token: String,
}

/// 要求重新登录的通用响应。
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReloginRequired {
    /// 是否必须重新登录（恒为 `true`）。
    pub relogin_required: bool,
}

/// 浏览器刷新会话 Cookie 能力：Cookie 名称与 Path 作用域参数化。
#[derive(Debug, Clone)]
pub struct BrowserSession {
    cookie_name: String,
    cookie_path: String,
}

impl BrowserSession {
    /// 用 Cookie 名称与 Path 作用域创建会话能力。
    pub fn new(cookie_name: impl Into<String>, cookie_path: impl Into<String>) -> Self {
        Self {
            cookie_name: cookie_name.into(),
            cookie_path: cookie_path.into(),
        }
    }

    /// 从请求 Cookie 头提取 Refresh Token；缺失或为空返回 `Unauthorized`。
    pub fn refresh_token(&self, request: &Request) -> Result<String, BaseError> {
        request
            .cookie()
            .and_then(|header| {
                header.split(';').find_map(|part| {
                    let (name, value) = part.trim().split_once('=')?;
                    (name == self.cookie_name && !value.is_empty()).then(|| value.to_string())
                })
            })
            .ok_or_else(|| BaseError::Unauthorized("刷新会话 Cookie 缺失".to_string()))
    }

    /// 校验变更类请求来自同源页面；返回请求是否经 HTTPS（用于 Cookie `Secure` 属性）。
    ///
    /// 非浏览器客户端通常没有 `Origin`/`Referer`；Cookie 不会被浏览器自动附带，
    /// 因而不具备跨站请求伪造条件，此时返回 `Ok(false)`。
    pub fn validate_same_origin(request: &Request) -> Result<bool, BaseError> {
        if request
            .get_header("sec-fetch-site")
            .is_some_and(|value| !matches!(value, "same-origin" | "none"))
        {
            return Err(BaseError::PermissionDenied(
                "浏览器会话请求必须来自同源页面".to_string(),
            ));
        }

        let source = request
            .get_header("origin")
            .or_else(|| request.get_header("referer"));
        let Some(source) = source else {
            return Ok(false);
        };
        let host = request
            .get_header("host")
            .ok_or_else(|| BaseError::PermissionDenied("同源校验缺少 Host".to_string()))?;
        let uri = source
            .parse::<http_uri::Uri>()
            .map_err(|_| BaseError::PermissionDenied("Origin/Referer 非法".to_string()))?;
        let source_host = uri
            .authority()
            .map(|authority| authority.as_str())
            .ok_or_else(|| BaseError::PermissionDenied("Origin/Referer 缺少主机".to_string()))?;
        if !source_host.eq_ignore_ascii_case(host.trim()) {
            return Err(BaseError::PermissionDenied(
                "浏览器会话请求必须来自同源页面".to_string(),
            ));
        }
        Ok(uri.scheme_str() == Some("https"))
    }

    /// 构造会话建立响应：Access Token 进响应体，Refresh Token 进 HttpOnly Cookie。
    pub fn token_response(
        &self,
        access_token: String,
        refresh_token: String,
        secure: bool,
    ) -> Result<ApiResponse, BaseError> {
        no_store(
            ApiResponse::success(BrowserAccessToken { access_token }, "会话已建立")?
                .with_header("set-cookie", self.refresh_cookie(&refresh_token, secure))?,
        )
    }

    /// 为已有响应追加清除会话 Cookie 的头。
    pub fn clear_response(
        &self,
        response: ApiResponse,
        secure: bool,
    ) -> Result<ApiResponse, BaseError> {
        no_store(response.with_header("set-cookie", self.clear_refresh_cookie(secure))?)
    }

    /// 构造「请重新登录」响应并清除会话 Cookie。
    pub fn relogin_response(&self, message: &str, secure: bool) -> Result<ApiResponse, BaseError> {
        self.clear_response(
            ApiResponse::success(
                ReloginRequired {
                    relogin_required: true,
                },
                message,
            )?,
            secure,
        )
    }

    fn refresh_cookie(&self, token: &str, secure: bool) -> String {
        format!(
            "{}={token}; Path={}; HttpOnly; SameSite=Strict{}",
            self.cookie_name,
            self.cookie_path,
            if secure { "; Secure" } else { "" }
        )
    }

    fn clear_refresh_cookie(&self, secure: bool) -> String {
        format!(
            "{}=; Path={}; HttpOnly; SameSite=Strict; Max-Age=0{}",
            self.cookie_name,
            self.cookie_path,
            if secure { "; Secure" } else { "" }
        )
    }
}

fn no_store(response: ApiResponse) -> Result<ApiResponse, BaseError> {
    response
        .with_header("cache-control", "no-store")?
        .with_header("pragma", "no-cache")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn session() -> BrowserSession {
        BrowserSession::new("yang_refresh", "/api/v1/users")
    }

    #[test]
    fn refresh_cookie_is_http_only_strict_and_host_only() {
        let cookie = session().refresh_cookie("secret", true);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Path=/api/v1/users"));
        assert!(!cookie.contains("Domain="));
    }

    #[test]
    fn browser_posts_require_exact_origin() {
        let mut headers = HashMap::new();
        headers.insert("host".to_string(), "app.example.com".to_string());
        headers.insert("origin".to_string(), "https://app.example.com".to_string());
        headers.insert("sec-fetch-site".to_string(), "same-origin".to_string());
        assert!(BrowserSession::validate_same_origin(
            &Request::new(serde_json::json!({})).headers(headers.clone())
        )
        .unwrap_or(false));
        headers.insert("origin".to_string(), "https://evil.example.com".to_string());
        assert!(BrowserSession::validate_same_origin(
            &Request::new(serde_json::json!({})).headers(headers)
        )
        .is_err());
    }
}
