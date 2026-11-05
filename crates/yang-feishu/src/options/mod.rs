//! 飞书审批「关联外部选项」协议、i18n 与游标。

pub mod i18n;
pub mod pagination;
pub mod protocol;

pub use i18n::{
    build_result_body, i18n_key, normalize_linkage_value, OptionRow, DEFAULT_LOCALE, I18N_PREFIX,
};
pub use pagination::{decode_cursor, encode_cursor};
pub use protocol::{
    FeishuData, FeishuEnvelope, FeishuI18nResource, FeishuOption, FeishuOptionsRequest,
    FeishuResult, FeishuResultBody,
};

/// 外部选项模块错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// `page_token` 非法（fail-closed）。
    #[error("分页标记非法")]
    InvalidCursor,
}
