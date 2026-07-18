//! 启动期预编译的 Table View 运行时产物。

use super::{ActionHandle, ModuleName, ViewName};
use std::sync::Arc;

/// 启动期已解析的树 View 拓扑。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledTreeView {
    id_field: yang_db::FieldRef,
    parent_field: yang_db::FieldRef,
    label_field: yang_db::FieldRef,
    max_nodes: usize,
}

impl CompiledTreeView {
    pub(crate) fn new(
        id_field: yang_db::FieldRef,
        parent_field: yang_db::FieldRef,
        label_field: yang_db::FieldRef,
        max_nodes: usize,
    ) -> Self {
        Self {
            id_field,
            parent_field,
            label_field,
            max_nodes,
        }
    }

    /// 返回节点唯一标识字段。
    pub fn id_field(&self) -> &yang_db::FieldRef {
        &self.id_field
    }

    /// 返回节点唯一标识的本地字段名。
    pub fn id_field_name(&self) -> &str {
        local_field_name(&self.id_field)
    }

    /// 返回父节点标识字段。
    pub fn parent_field(&self) -> &yang_db::FieldRef {
        &self.parent_field
    }

    /// 返回父节点标识的本地字段名。
    pub fn parent_field_name(&self) -> &str {
        local_field_name(&self.parent_field)
    }

    /// 返回节点用户可见标签字段。
    pub fn label_field(&self) -> &yang_db::FieldRef {
        &self.label_field
    }

    /// 返回节点标签的本地字段名。
    pub fn label_field_name(&self) -> &str {
        local_field_name(&self.label_field)
    }

    /// 返回启动期解析后的单次树查询节点上限。
    pub fn max_nodes(&self) -> usize {
        self.max_nodes
    }
}

fn local_field_name(field: &yang_db::FieldRef) -> &str {
    field
        .as_str()
        .rsplit_once('.')
        .map_or(field.as_str(), |(_, name)| name)
}

/// 已解析字段引用和按钮 Action slot 的只读 View。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledTableView {
    module: ModuleName,
    name: ViewName,
    table: yang_db::TableRef,
    fields: Arc<[yang_db::FieldRef]>,
    actions: Arc<[ActionHandle]>,
    tree: Option<CompiledTreeView>,
}

impl CompiledTableView {
    pub(crate) fn new(
        module: ModuleName,
        name: ViewName,
        table: yang_db::TableRef,
        fields: Vec<yang_db::FieldRef>,
        actions: Vec<ActionHandle>,
        tree: Option<CompiledTreeView>,
    ) -> Self {
        Self {
            module,
            name,
            table,
            fields: fields.into(),
            actions: actions.into(),
            tree,
        }
    }

    /// 返回所属 Module。
    pub fn module(&self) -> &ModuleName {
        &self.module
    }

    /// 返回 View 局部名称。
    pub fn name(&self) -> &ViewName {
        &self.name
    }

    /// 返回预校验表引用。
    pub fn table(&self) -> &yang_db::TableRef {
        &self.table
    }

    /// 返回预校验字段引用。
    pub fn fields(&self) -> &[yang_db::FieldRef] {
        &self.fields
    }

    /// 返回预解析按钮 Action slot。
    pub fn actions(&self) -> &[ActionHandle] {
        &self.actions
    }

    /// 返回可选的显式树拓扑。
    pub fn tree(&self) -> Option<&CompiledTreeView> {
        self.tree.as_ref()
    }
}
