//! 飞书平台错误码常量。
//!
//! 这些常量集中在一处，避免错误码字面量散落到各模块后语义漂移。

/// 通用频控（HTTP 429；部分旧版 OpenAPI 表现为 HTTP 400）。
pub const CODE_RATE_LIMITED: i32 = 99991400;
/// 多维表格自身的频控：`1254290 TooManyRequest`，**HTTP 200**。
pub const CODE_TOO_MANY_REQUEST: i32 = 1254290;
/// IM 频控可返回 HTTP 400，仍需按业务码退避。
pub const CODE_MESSAGE_RATE_LIMITED: i32 = 230020;
/// 响应体过大（HTTP 200）。确定性失败，重试无用。
pub const CODE_TOO_LARGE_RESPONSE: i32 = 1254030;

/// 重新换取 token **能**修好的：凭证过期/失效。
pub const CODES_TOKEN_EXPIRED: &[i32] = &[99991663, 20013];

/// 重新换取 token **修不了**的：请求里没带 token / 格式错 / 类型错。
pub const CODES_TOKEN_MALFORMED: &[i32] = &[99991661, 99991664, 99991665, 99991668, 99991671];

/// 官方明确写了「稍后重试」的业务码。
///
/// - `1254291` 多维表格写入冲突；
/// - `1255040` 多维表格 HTTP 504 超时；
/// - `1061045` / `1066002` / `1069601` 云盘频控/并发/瞬时失败；
/// - `230049` IM 消息正在发送中。
pub const CODES_RETRYABLE: &[i32] = &[
    1254036, 1254291, 1254607, 1255001, 1255002, 1255040, 1061045, 1066002, 1069601, 230049,
];

/// 权限类失败，**HTTP 200**。
pub const CODES_PERMISSION_DENIED: &[i32] = &[1254302, 1254303];

/// 审批域瞬态错误：官方建议「降低请求频率，并重试」。
pub const CODES_RETRYABLE_APPROVAL: &[i32] = &[1395001];

/// uuid 冲突：该 uuid 已经创建过审批实例（响应丢失，实例其实已建成功）。
pub const CODE_UUID_CONFLICT: i32 = 60012;

/// 审批实例不存在（回捞时区分并发窗口与真实未创建）。
pub const CODE_INSTANCE_NOT_FOUND: i32 = 1390003;

/// 审批定义事件订阅的幂等结果：已订阅或已取消订阅。
pub const CODE_SUBSCRIPTION_EXISTED: i32 = 1390007;
