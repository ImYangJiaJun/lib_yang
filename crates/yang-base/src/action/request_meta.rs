//! 与具体 Web 框架无关的请求传输元数据。

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;

const REDACTED: &str = "[REDACTED]";

/// Action 请求的传输层元数据。
///
/// 所有字段均可缺失，便于非 HTTP 调用、测试和内部任务继续通过现有
/// `Request::new(body)` / `ActionContext::new(request, tools)` 构造。耗时不属于本结构，
/// 应由 dispatch span 或 metrics 在执行完成后计算。
#[derive(Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestMeta {
    /// 原始请求方法，例如 `GET`、`POST`。
    pub method: Option<String>,
    /// 传输适配器观察到的原始 URI；可能包含敏感 query，因此 Debug 始终脱敏。
    pub original_uri: Option<String>,
    /// URI scheme，例如 `http`、`https`。
    pub scheme: Option<String>,
    /// 对端网络地址。
    pub peer_addr: Option<SocketAddr>,
    /// 本地监听地址。
    pub local_addr: Option<SocketAddr>,
    /// 传输适配器附加的可选字符串元数据。
    ///
    /// 使用有序映射使测试、诊断和后续 catalog 投影保持确定性；Debug 只展示 key。
    pub extensions: BTreeMap<String, String>,
}

impl RequestMeta {
    /// 创建所有传输字段均缺失的元数据。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置请求方法。
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    /// 设置原始 URI。
    pub fn with_original_uri(mut self, uri: impl Into<String>) -> Self {
        self.original_uri = Some(uri.into());
        self
    }

    /// 设置 URI scheme。
    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = Some(scheme.into());
        self
    }

    /// 设置对端地址。
    pub fn with_peer_addr(mut self, peer_addr: SocketAddr) -> Self {
        self.peer_addr = Some(peer_addr);
        self
    }

    /// 设置本地监听地址。
    pub fn with_local_addr(mut self, local_addr: SocketAddr) -> Self {
        self.local_addr = Some(local_addr);
        self
    }

    /// 添加一项传输扩展；空白 key 被忽略。
    pub fn extension(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let key = key.trim();
        if !key.is_empty() {
            self.extensions.insert(key.to_string(), value.into());
        }
        self
    }
}

impl fmt::Debug for RequestMeta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let original_uri = self.original_uri.as_ref().map(|_| REDACTED);
        let peer_addr = self.peer_addr.as_ref().map(|_| REDACTED);
        let local_addr = self.local_addr.as_ref().map(|_| REDACTED);
        let extension_keys: Vec<&str> = self.extensions.keys().map(String::as_str).collect();

        formatter
            .debug_struct("RequestMeta")
            .field("method", &self.method)
            .field("original_uri", &original_uri)
            .field("scheme", &self.scheme)
            .field("peer_addr", &peer_addr)
            .field("local_addr", &local_addr)
            .field("extension_keys", &extension_keys)
            .finish()
    }
}
