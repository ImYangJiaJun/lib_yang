//! 审批相关客户端与表单契约。

pub mod client;
pub mod form;
pub mod idempotency;
pub mod value;
pub mod widget_rules;

pub use client::{
    ApprovalClient, ApprovalDefinition, CreateOutcome, GetOutcome, InstanceDetail,
    SubscribeOutcome, TaskNode, TimelineRecord,
};
pub use form::{
    form_snapshot_text, parse_form, parse_snapshot, FormError, FormWidget, OptionItem,
    WidgetOptions,
};
pub use idempotency::{derive_idempotency_key, derive_uuid};
pub use value::{
    amount_currency, build_form, build_group_form, build_signature, labels_equivalent,
    labels_from_cell, normalize_decimal_label, unwrap_typed_cell, ConvertError, Converter,
    FixedOffset, FormInput, GroupFormInput, GroupFormRow, WidgetMap,
};
pub use widget_rules::*;

use crate::transport::{Method, Request};

/// 该请求是否是「创建审批实例」。
///
/// 替代宿主散落各处的 URL 字符串嗅探：URL 构造在本 crate，路径调整时调用方
/// 不会静默失效。
pub fn is_create_request(request: &Request) -> bool {
    request.method == Method::Post
        && request
            .url
            .split('?')
            .next()
            .unwrap_or_default()
            .ends_with("/open-apis/approval/v4/instances")
}
