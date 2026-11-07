//! 多维表格客户端与 DTO。

pub mod cells;
pub mod client;
pub mod dto;
mod endpoints;
pub mod fields;
pub mod filters;
pub mod link;
pub mod pagination;
mod v3;

pub use cells::{cell_label, encode_cell_value, CellEncodeError, CellValue};
pub use client::BitableClient;
pub use dto::{
    BatchGetRecordsData, BitableTableItem, BitableTablesData, BitableViewItem, BitableViewsData,
    FieldItem, FieldProperty, ListFieldsData, ListRecordsData, PropertyOption, RecordItem,
    RecordPerson, SearchRecordsData,
};
pub use fields::{
    check_coordinate_field, is_option_source_field_type, is_single_value_field_type,
    rekey_cells_by_field_id, resolve_field_name, resolve_field_names, SYSTEM_FIELD_KEYS,
};
pub use filters::pending_backfill_filter;
pub use link::{
    is_document_link, parse_bitable_link, resolve_bitable_link, BitableCoordinates,
    ParsedBitableLink,
};
pub use pagination::{PaginationState, RecordsSnapshot};

/// 列出记录的单页上限（官方 500）。
pub const PAGE_SIZE: u32 = 500;
/// 列出字段/数据表/视图的单页上限（官方 100）。
pub(crate) const PAGE_SIZE_FIELDS: u32 = 100;
/// 单轮最多翻页数。
pub const MAX_PAGES: u32 = 40;
/// 查询记录的单页上限（与 [`PAGE_SIZE`] 同值但出处不同）。
pub const SEARCH_PAGE_SIZE: u32 = 500;
/// 批量回写的子批上限。
pub const BATCH_UPDATE_CHUNK: usize = 100;
