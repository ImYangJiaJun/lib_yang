//! 出站调用的统一错误类型。
//!
//! 成败判据是飞书响应体里的**业务码**，不是 HTTP 状态码；因此错误分类比普通 HTTP
//! 客户端多两档审批语义：[`FailureKind::UuidConflict`]（60012，实例已创建但响应丢失）
//! 与 [`FailureKind::InstanceNotFound`]（1390003，回捞时的并发窗口）。
//!
//! 显示文案固定为 `飞书出站调用失败: {message}`，**不拼接中文运维提示**：提示表是宿主
//! 的产品资产，由宿主在自己的展示层补充。

/// 一次出站失败的处置类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// 可退避重试；服务端给了建议秒数则优先采信。
    Retry {
        /// 服务端通过 `x-ogw-ratelimit-reset`（或 `Retry-After`）给出的等待秒数。
        retry_after_seconds: Option<u64>,
    },
    /// 凭证失效：补救是「清缓存 + 强制刷新一次 + 重试一次」，**不是**退避重试。
    TokenExpired,
    /// uuid 冲突（60012）：实例此前已创建成功，只是响应丢了。
    ///
    /// 交上层按 uuid 反查实例详情；**不重试、不写终态字段**。
    UuidConflict,
    /// 实例不存在（1390003）：回捞时用来区分并发窗口与真实未创建。
    ///
    /// 交上层决定是否回到 create；**不是**终态。
    InstanceNotFound,
    /// 绝不重试，且**不动** token 缓存。
    Fatal {
        /// 飞书业务码；拿不到业务码时为 0。
        code: i32,
    },
}

/// 出站调用失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// 失败分类。
    pub kind: FailureKind,
    /// 已脱敏、可读的失败描述。
    pub message: String,
}

impl Error {
    /// 构造一条失败。
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 构造一条不可重试的失败（`Fatal { code }`）。
    pub fn fatal(message: impl Into<String>) -> Self {
        Self::new(FailureKind::Fatal { code: 0 }, message)
    }

    /// 构造一条可退避重试的失败。
    pub fn retry(message: impl Into<String>) -> Self {
        Self::new(
            FailureKind::Retry {
                retry_after_seconds: None,
            },
            message,
        )
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "飞书出站调用失败: {}", self.message)
    }
}

impl std::error::Error for Error {}

/// 出站调用的结果别名。
pub type Result<T> = std::result::Result<T, Error>;
