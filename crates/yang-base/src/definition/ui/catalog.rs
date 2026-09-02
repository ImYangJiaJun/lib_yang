//! 请求级 UI 目录契约：聚合 Action 演示、TableView 与 Module 页面并维护确定性修订标识。

use super::demo::ActionDemoSchema;
use super::module::ModulePresentationSchema;
use super::table::TableViewSchema;
use super::UI_SCHEMA_VERSION;
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// 一次请求返回给前端的 UI 目录契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UiCatalog {
    /// UI schema 版本；前端必须按版本选择解析器。
    pub schema_version: &'static str,
    /// 当前过滤后完整目录的确定性 SHA-256 修订标识。
    ///
    /// 消费端可用它判断缓存内容是否变化；身份或租户切换仍必须重新请求目录，不能
    /// 把 revision 当作授权凭据。
    pub revision: String,
    /// 当前请求有权访问的 Action 演示契约。
    pub actions: Vec<ActionDemoSchema>,
    /// 当前请求有权访问的通用表格 Views。
    pub table_views: Vec<TableViewSchema>,
    /// 当前请求有权访问的 Module 页面展示契约。
    pub modules: Vec<ModulePresentationSchema>,
}

impl UiCatalog {
    /// 从已经完成请求级过滤的 Action 集合构造目录，并按 operation id 稳定排序。
    pub fn new<I>(actions: I) -> Result<Self, crate::error::BaseError>
    where
        I: IntoIterator<Item = ActionDemoSchema>,
    {
        let mut actions = actions.into_iter().collect::<Vec<_>>();
        actions.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        let mut catalog = Self {
            schema_version: UI_SCHEMA_VERSION,
            revision: String::new(),
            actions,
            table_views: Vec::new(),
            modules: Vec::new(),
        };
        catalog.refresh_revision()?;
        Ok(catalog)
    }

    pub(crate) fn with_table_views<I>(mut self, views: I) -> Result<Self, crate::error::BaseError>
    where
        I: IntoIterator<Item = TableViewSchema>,
    {
        self.table_views = views.into_iter().collect();
        self.table_views
            .sort_by(|left, right| left.view_id.cmp(&right.view_id));
        self.refresh_revision()?;
        Ok(self)
    }

    pub(crate) fn with_modules<I>(mut self, modules: I) -> Result<Self, crate::error::BaseError>
    where
        I: IntoIterator<Item = ModulePresentationSchema>,
    {
        self.modules = modules.into_iter().collect();
        self.modules
            .sort_by(|left, right| left.module_id.cmp(&right.module_id));
        self.refresh_revision()?;
        Ok(self)
    }

    fn refresh_revision(&mut self) -> Result<(), crate::error::BaseError> {
        let payload = serde_json::to_vec(&(
            self.schema_version,
            self.actions.as_slice(),
            self.table_views.as_slice(),
            self.modules.as_slice(),
        ))
        .map_err(|error| crate::error::BaseError::JsonSerializeFailed(error.to_string()))?;
        let digest = Sha256::digest(payload);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut revision = String::with_capacity(digest.len() * 2);
        for byte in digest {
            revision.push(HEX[usize::from(byte >> 4)] as char);
            revision.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        self.revision = revision;
        Ok(())
    }
}
