//! Action 响应结构
//!
//! 提供统一的 API 响应格式，包含状态码、消息和数据。

use crate::error::BaseError;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

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
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
/// 请使用 [`ApiResponse::success`] / [`ApiResponse::fail`] / [`ApiResponse::from_error`] 等构造。
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
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[non_exhaustive]
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

    /// 非 JSON 附件（文件下载/预览/重定向），由传输层消费。
    ///
    /// 仅当 Action 以 [`ResponseBody`] 为输出时由派发边界填充；
    /// `serde(skip)` 保证普通 Action 的 JSON 线格式不受影响。
    #[serde(skip)]
    #[schemars(skip)]
    pub attachment: Option<ResponseAttachment>,

    /// 由 Action 显式声明、传输层消费的 HTTP 状态码覆盖。
    #[serde(skip)]
    #[schemars(skip)]
    http_status: Option<u16>,

    /// 由 Action 显式声明、传输层消费的响应头。
    #[serde(skip)]
    #[schemars(skip)]
    headers: Vec<(String, String)>,
}

impl ApiResponse {
    /// 创建成功响应
    ///
    /// 状态码为 0，包含业务数据。序列化失败时返回 `BaseError::JsonSerializeFailed`。
    ///
    /// # 参数
    ///
    /// - `data`: 响应数据（任何可序列化的类型）
    /// - `message`: 成功消息
    ///
    /// # 返回
    ///
    /// - `Ok(ApiResponse)`: 序列化成功时返回响应实例
    /// - `Err(BaseError::JsonSerializeFailed)`: 序列化失败时返回错误
    ///
    /// # Errors
    ///
    /// - `BaseError::JsonSerializeFailed`: 当 `data` 无法被序列化为 JSON 时
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
    /// )?;
    ///
    /// // 返回影响行数
    /// let response = ApiResponse::success(
    ///     json!({ "affected": 1 }),
    ///     "更新成功"
    /// )?;
    /// ```
    pub fn success<T: Serialize>(data: T, message: impl Into<String>) -> Result<Self, BaseError> {
        // 序列化数据，失败时返回结构化错误而非静默吞错
        let json_value = serde_json::to_value(data)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        Ok(Self {
            code: 0,
            message: message.into(),
            data: Some(json_value),
            attachment: None,
            http_status: None,
            headers: Vec::new(),
        })
    }

    /// 创建成功响应（接受已序列化的 JSON 值）
    ///
    /// 当数据已经是 `serde_json::Value` 类型时，使用此方法可避免额外的序列化开销，
    /// 且不会失败，因此直接返回 `Self` 而非 `Result`。
    ///
    /// # 参数
    ///
    /// - `data`: 已序列化的 JSON 值
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
    /// // 直接使用 serde_json::Value
    /// let data = json!({ "id": 1, "name": "Alice" });
    /// let response = ApiResponse::success_value(data, "获取成功");
    /// assert_eq!(response.code, 0);
    /// ```
    pub fn success_value(data: serde_json::Value, message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data: Some(data),
            attachment: None,
            http_status: None,
            headers: Vec::new(),
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
            attachment: None,
            http_status: None,
            headers: Vec::new(),
        }
    }

    /// 创建携带附件的成功响应
    ///
    /// 由派发边界在识别到 Action 返回 [`ResponseBody`] 时调用。`data` 为 None，
    /// 附件信息仅对传输层可见（`serde(skip)`），JSON 线格式保持不变。
    ///
    /// # 参数
    ///
    /// - `attachment`: 传输层消费的附件描述
    /// - `message`: 成功消息
    ///
    /// # 返回
    ///
    /// - ApiResponse 实例
    pub fn attachment(attachment: ResponseAttachment, message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data: None,
            attachment: Some(attachment),
            http_status: None,
            headers: Vec::new(),
        }
    }

    /// 覆盖 Action 静态声明的成功 HTTP 状态码。
    pub fn with_http_status(mut self, status: u16) -> Result<Self, BaseError> {
        if !(100..=599).contains(&status) {
            return Err(BaseError::ConfigError(format!(
                "响应 HTTP 状态码无效: {status}"
            )));
        }
        self.http_status = Some(status);
        Ok(self)
    }

    /// 追加一个经过最小 RFC 语法校验的响应头。
    ///
    /// 使用列表而不是 Map，以保留多个 `Set-Cookie` 等同名响应头。
    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, BaseError> {
        let name = name.into();
        let value = value.into();
        let valid_name = !name.is_empty()
            && name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#'
                            | b'$'
                            | b'%'
                            | b'&'
                            | b'\''
                            | b'*'
                            | b'+'
                            | b'-'
                            | b'.'
                            | b'^'
                            | b'_'
                            | b'`'
                            | b'|'
                            | b'~'
                    )
            });
        let valid_value = value
            .bytes()
            .all(|byte| byte == b'\t' || (byte >= 0x20 && byte != 0x7f));
        if !valid_name || !valid_value {
            return Err(BaseError::ConfigError(
                "响应头名称或值包含非法字符".to_string(),
            ));
        }
        self.headers.push((name, value));
        Ok(self)
    }

    #[cfg(any(feature = "transport-axum", test))]
    pub(crate) const fn http_status(&self) -> Option<u16> {
        self.http_status
    }

    #[cfg(any(feature = "transport-axum", test))]
    pub(crate) fn response_headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// 从 BaseError 创建失败响应
    ///
    /// 自动提取错误码和错误消息。接受引用以避免消费 `BaseError` 所有权，
    /// 调用方可在构建响应的同时保留 error 用于日志记录等后续用途。
    ///
    /// # 参数
    ///
    /// - `error`: BaseError 引用
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
    /// let response = ApiResponse::from_error(&error);
    ///
    /// assert_ne!(response.code, 0);
    /// assert!(response.message.contains("username"));
    /// assert!(response.data.is_none());
    /// ```
    pub fn from_error(error: &BaseError) -> Self {
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
            attachment: None,
            http_status: None,
            headers: Vec::new(),
        }
    }
}

/// Action 的特殊业务响应体：文件下载、文件预览、重定向。
///
/// Action 将本类型作为 `type Output` 返回时，派发边界
/// （[`DynAction::dispatch`](crate::action::DynAction)）会识别并把它转为
/// [`ApiResponse::attachment`]，普通输出的 JSON 线格式不受影响。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::ResponseBody;
///
/// async fn index(&self, ctx: ActionContext, input: Self::Input) -> Result<Self::Output, BaseError> {
///     // 文件下载
///     Ok(ResponseBody::download("/data/report.pdf", "report.pdf"))
///     // 文件预览
///     // Ok(ResponseBody::preview("/data/a.png"))
///     // 重定向
///     // Ok(ResponseBody::redirect("https://example.com/next"))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseBody {
    /// 文件下载：传输层以 `Content-Disposition: attachment` 返回文件字节。
    Download {
        /// 服务器本地文件路径。
        path: PathBuf,
        /// 下载文件名（写入 Content-Disposition 的 filename 参数）。
        filename: String,
    },
    /// 文件预览：传输层以 `Content-Disposition: inline` 返回文件字节。
    Preview {
        /// 服务器本地文件路径。
        path: PathBuf,
    },
    /// 重定向：传输层返回 302 状态码与 `Location` 头。
    Redirect {
        /// 目标地址。
        url: String,
    },
}

impl ResponseBody {
    /// 构造文件下载响应。
    pub fn download(path: impl Into<PathBuf>, filename: impl Into<String>) -> Self {
        Self::Download {
            path: path.into(),
            filename: filename.into(),
        }
    }

    /// 构造文件预览响应。
    pub fn preview(path: impl Into<PathBuf>) -> Self {
        Self::Preview { path: path.into() }
    }

    /// 构造重定向响应。
    pub fn redirect(url: impl Into<String>) -> Self {
        Self::Redirect { url: url.into() }
    }
}

/// 一次响应携带的非 JSON 附件描述，由传输层消费，不参与 JSON 序列化。
///
/// 与 [`ResponseBody`] 结构一一对应；拆开是为了让 Action 输出类型满足
/// `Serialize + JsonSchema` 契约，而响应上的附件字段不受该契约约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseAttachment {
    /// 文件下载（`Content-Disposition: attachment`）。
    Download {
        /// 服务器本地文件路径。
        path: PathBuf,
        /// 下载文件名。
        filename: String,
    },
    /// 文件预览（`Content-Disposition: inline`）。
    Preview {
        /// 服务器本地文件路径。
        path: PathBuf,
    },
    /// 重定向（302 + `Location`）。
    Redirect {
        /// 目标地址。
        url: String,
    },
}

impl From<ResponseBody> for ResponseAttachment {
    fn from(body: ResponseBody) -> Self {
        match body {
            ResponseBody::Download { path, filename } => Self::Download { path, filename },
            ResponseBody::Preview { path } => Self::Preview { path },
            ResponseBody::Redirect { url } => Self::Redirect { url },
        }
    }
}

/// 派发边界的统一输出去向：[`ResponseBody`] 转为附件响应，其余照旧序列化进 `data`。
///
/// 通过 `&dyn Any` downcast 识别 `ResponseBody`，不引入新的 trait bound，
/// 因此现有 Action 的输出契约与 `Result<ApiResponse, BaseError>` 派发签名保持不变。
pub(crate) fn wrap_dispatch_output<T>(output: T, message: &str) -> Result<ApiResponse, BaseError>
where
    T: Serialize + 'static,
{
    let erased = &output as &dyn std::any::Any;
    if let Some(response) = erased.downcast_ref::<ApiResponse>() {
        return Ok(response.clone());
    }
    if let Some(body) = erased.downcast_ref::<ResponseBody>() {
        return Ok(ApiResponse::attachment(body.clone().into(), message));
    }
    ApiResponse::success(output, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_success_response() {
        // success 现在返回 Result，需要用 ? 或 unwrap
        let response =
            ApiResponse::success(json!({ "id": 123, "name": "Alice" }), "操作成功").unwrap();

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "操作成功");
        assert!(response.data.is_some());

        let data = response.data.unwrap();
        assert_eq!(data["id"], 123);
        assert_eq!(data["name"], "Alice");
    }

    #[test]
    fn test_success_response_with_null() {
        // 使用 success_value 传入已有的 JSON 值（不会失败）
        let response = ApiResponse::success_value(serde_json::Value::Null, "操作成功");

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "操作成功");
        assert!(response.data.is_some());
        assert_eq!(response.data.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn test_success_value_response() {
        // 测试 success_value 便捷构造器
        let data = json!({ "count": 42 });
        let response = ApiResponse::success_value(data, "查询成功");

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "查询成功");
        assert!(response.data.is_some());
        assert_eq!(response.data.unwrap()["count"], 42);
    }

    #[test]
    fn test_success_serialize_error_propagation() {
        // 构造一个序列化时主动返回错误的类型
        struct AlwaysFailSerialize;

        impl serde::Serialize for AlwaysFailSerialize {
            fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::Error;
                Err(S::Error::custom("测试序列化失败"))
            }
        }

        let result = ApiResponse::success(AlwaysFailSerialize, "测试");
        // 序列化失败应该返回 JsonSerializeFailed 错误
        assert!(result.is_err(), "序列化失败应该返回错误");
        if let Err(BaseError::JsonSerializeFailed(_)) = result {
            // 正确：返回了 JsonSerializeFailed
        } else {
            panic!("期望 JsonSerializeFailed 错误");
        }
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
        let response = ApiResponse::from_error(&error);

        assert_ne!(response.code, 0);
        assert!(response.message.contains("username"));
        assert!(response.data.is_none());
        // error 仍可用（未被消费）
        assert_eq!(error.code(), 600006);
    }

    #[test]
    fn test_default_response() {
        let response = ApiResponse::default();

        assert_eq!(response.code, 0);
        assert_eq!(response.message, "OK");
        assert!(response.data.is_none());
    }

    #[test]
    fn transport_metadata_is_validated_and_never_serialized() {
        let response = ApiResponse::default()
            .with_http_status(304)
            .expect("304 应是合法 HTTP 状态码")
            .with_header("set-cookie", "session=one; HttpOnly")
            .expect("Set-Cookie 应是合法响应头")
            .with_header("set-cookie", "session=two; HttpOnly")
            .expect("重复 Set-Cookie 必须保留");
        assert_eq!(response.http_status(), Some(304));
        assert_eq!(response.response_headers().len(), 2);
        let wire = serde_json::to_value(&response).expect("响应应可序列化");
        assert!(wire.get("http_status").is_none());
        assert!(wire.get("headers").is_none());
        assert!(ApiResponse::default().with_http_status(99).is_err());
        assert!(ApiResponse::default()
            .with_header("bad header", "value")
            .is_err());
        assert!(ApiResponse::default()
            .with_header("x-test", "line\r\nbreak")
            .is_err());
    }

    #[test]
    fn test_serialize_response() {
        // success 现在返回 Result
        let response = ApiResponse::success(json!({ "count": 10 }), "查询成功").unwrap();

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

    #[test]
    fn test_response_body_constructors() {
        // download 携带路径与下载文件名
        let download = ResponseBody::download("/tmp/report.pdf", "report.pdf");
        assert_eq!(
            download,
            ResponseBody::Download {
                path: std::path::PathBuf::from("/tmp/report.pdf"),
                filename: "report.pdf".to_string(),
            }
        );
        // preview 仅携带路径
        assert_eq!(
            ResponseBody::preview("/tmp/a.png"),
            ResponseBody::Preview {
                path: std::path::PathBuf::from("/tmp/a.png"),
            }
        );
        // redirect 仅携带目标地址
        assert_eq!(
            ResponseBody::redirect("https://example.com/next"),
            ResponseBody::Redirect {
                url: "https://example.com/next".to_string(),
            }
        );
    }

    #[test]
    fn test_response_body_into_attachment() {
        // ResponseBody 到传输层附件描述的一一映射
        let attachment: ResponseAttachment = ResponseBody::download("/tmp/a.bin", "a.bin").into();
        assert_eq!(
            attachment,
            ResponseAttachment::Download {
                path: std::path::PathBuf::from("/tmp/a.bin"),
                filename: "a.bin".to_string(),
            }
        );
        let attachment: ResponseAttachment = ResponseBody::preview("/tmp/b.png").into();
        assert_eq!(
            attachment,
            ResponseAttachment::Preview {
                path: std::path::PathBuf::from("/tmp/b.png"),
            }
        );
        let attachment: ResponseAttachment = ResponseBody::redirect("/home").into();
        assert_eq!(
            attachment,
            ResponseAttachment::Redirect {
                url: "/home".to_string(),
            }
        );
    }

    #[test]
    fn test_response_body_implements_action_output_contract() {
        // 作为 Action::Output 必须满足 Serialize + JsonSchema + 'static
        fn assert_output_contract<T: Serialize + schemars::JsonSchema + 'static>() {}
        assert_output_contract::<ResponseBody>();

        let body = ResponseBody::redirect("/next");
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("/next"), "ResponseBody 应可序列化: {json}");
        // schema 生成不应 panic（output_schema 由构建期消费）
        let _schema = schemars::schema_for!(ResponseBody);
    }

    #[test]
    fn test_attachment_response_json_wire_format_unchanged() {
        // 附件响应序列化后不得出现 attachment 字段，保持 JSON 线格式不变
        let response = ApiResponse::attachment(
            ResponseAttachment::Redirect {
                url: "/next".to_string(),
            },
            "成功",
        );
        assert_eq!(response.code, 0);
        assert!(response.data.is_none());
        let json = serde_json::to_string(&response).unwrap();
        assert!(
            !json.contains("attachment"),
            "线格式不得包含附件信息: {json}"
        );
        assert!(json.contains("\"code\":0"));
    }

    #[test]
    fn test_plain_response_has_no_attachment() {
        // 普通成功/失败响应不带附件
        let ok = ApiResponse::success(json!({"a": 1}), "ok").unwrap();
        assert!(ok.attachment.is_none());
        let fail = ApiResponse::fail(1, "bad");
        assert!(fail.attachment.is_none());
        let default = ApiResponse::default();
        assert!(default.attachment.is_none());
    }

    #[test]
    fn test_wrap_dispatch_output_recognizes_response_body() {
        // ResponseBody 输出转为附件响应，不进 data
        let response = wrap_dispatch_output(ResponseBody::redirect("/next"), "成功").unwrap();
        assert_eq!(response.code, 0);
        assert!(response.data.is_none());
        assert_eq!(
            response.attachment,
            Some(ResponseAttachment::Redirect {
                url: "/next".to_string(),
            })
        );
    }

    #[test]
    fn test_wrap_dispatch_output_passes_through_plain_output() {
        // 普通输出照旧序列化进 data，attachment 为 None
        #[derive(Serialize)]
        struct Plain {
            value: i32,
        }
        let response = wrap_dispatch_output(Plain { value: 7 }, "成功").unwrap();
        assert_eq!(response.code, 0);
        assert_eq!(response.data.unwrap()["value"], 7);
        assert!(response.attachment.is_none());
    }
}
