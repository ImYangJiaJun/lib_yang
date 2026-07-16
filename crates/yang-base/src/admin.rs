//! 可选后台展示元数据。
//!
//! 本模块只保存对 Action/Table/ApiCatalog 稳定 ID 的引用，不持有或修改 dispatch 对象。
//! 审核流、业务状态机和权限判定仍属于业务插件，不进入这些展示类型。

use crate::BaseError;
use std::collections::BTreeMap;

/// 后台组件的展示形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdminDisplayKind {
    /// 导航菜单。
    Menu,
    /// 操作按钮。
    Button,
    /// 列表视图。
    List,
    /// 树形视图。
    Tree,
    /// 表单视图。
    Form,
}

/// 展示元数据引用的稳定核心目标。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdminTarget {
    /// ModuleRouter 中的 Action 稳定名称。
    Action {
        /// 模块名。
        module: String,
        /// Action 名。
        action: String,
    },
    /// 模块内的 [`crate::table::TableDefinition`] 稳定名称。
    Table {
        /// 模块名。
        module: String,
        /// 表名。
        table: String,
    },
    /// ApiCatalog 的 operation id。
    ApiOperation {
        /// operation id。
        operation_id: String,
    },
}

impl AdminTarget {
    /// 构造 Action 引用。
    pub fn action(module: &str, action: &str) -> Result<Self, BaseError> {
        validate_stable_id(module)?;
        validate_stable_id(action)?;
        Ok(Self::Action {
            module: module.to_string(),
            action: action.to_string(),
        })
    }

    /// 构造 Table 引用。
    pub fn table(module: &str, table: &str) -> Result<Self, BaseError> {
        validate_stable_id(module)?;
        validate_stable_id(table)?;
        Ok(Self::Table {
            module: module.to_string(),
            table: table.to_string(),
        })
    }

    /// 构造 ApiCatalog operation id 引用。
    pub fn api_operation(operation_id: &str) -> Result<Self, BaseError> {
        validate_stable_id(operation_id)?;
        Ok(Self::ApiOperation {
            operation_id: operation_id.to_string(),
        })
    }
}

/// 一项后台展示描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminMetadata {
    /// 展示项自身的稳定 ID。
    pub id: String,
    /// 用户可见标题。
    pub label: String,
    /// 展示形态。
    pub kind: AdminDisplayKind,
    /// 被引用的核心对象稳定 ID。
    pub target: AdminTarget,
    /// 可选图标名称。
    pub icon: Option<String>,
    /// 可选展示分组。
    pub group: Option<String>,
    /// 确定性排序权重。
    pub order: i32,
}

impl AdminMetadata {
    /// 创建展示项并校验稳定 ID 与非空标题。
    pub fn new(
        id: &str,
        label: &str,
        kind: AdminDisplayKind,
        target: AdminTarget,
    ) -> Result<Self, BaseError> {
        validate_stable_id(id)?;
        if label.trim().is_empty() {
            return Err(BaseError::ConfigError("后台元数据标题不能为空".to_string()));
        }
        Ok(Self {
            id: id.to_string(),
            label: label.to_string(),
            kind,
            target,
            icon: None,
            group: None,
            order: 0,
        })
    }

    /// 设置图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置展示分组。
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// 设置排序权重。
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }
}

/// 确定性、只读的后台元数据注册表。
#[derive(Debug, Clone)]
pub struct AdminMetadataRegistry {
    entries: Vec<AdminMetadata>,
    positions: BTreeMap<String, usize>,
}

impl AdminMetadataRegistry {
    /// 构建注册表；重复 ID 直接失败。
    pub fn new(mut entries: Vec<AdminMetadata>) -> Result<Self, BaseError> {
        entries.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut positions = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            if positions.insert(entry.id.clone(), index).is_some() {
                return Err(BaseError::ConfigError(format!(
                    "后台元数据 ID 重复: {}",
                    entry.id
                )));
            }
        }
        Ok(Self { entries, positions })
    }

    /// 返回确定性排序的全部展示项。
    pub fn entries(&self) -> &[AdminMetadata] {
        &self.entries
    }

    /// 按稳定 ID 查询展示项。
    pub fn get(&self, id: &str) -> Option<&AdminMetadata> {
        self.positions
            .get(id)
            .and_then(|index| self.entries.get(*index))
    }
}

fn validate_stable_id(id: &str) -> Result<(), BaseError> {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return Err(BaseError::ConfigError("稳定 ID 不能为空".to_string()));
    };
    if !first.is_ascii_alphabetic()
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':'))
    {
        return Err(BaseError::ConfigError(format!("非法稳定 ID: {id:?}")));
    }
    Ok(())
}
