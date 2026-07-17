//! 启动期预编译的 Table View 运行时产物。

use super::{ActionHandle, ModuleName, ViewName};
use std::sync::Arc;

/// 已解析字段引用和按钮 Action slot 的只读 View。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledTableView {
    module: ModuleName,
    name: ViewName,
    table: yang_db::TableRef,
    fields: Arc<[yang_db::FieldRef]>,
    actions: Arc<[ActionHandle]>,
}

impl CompiledTableView {
    pub(crate) fn new(
        module: ModuleName,
        name: ViewName,
        table: yang_db::TableRef,
        fields: Vec<yang_db::FieldRef>,
        actions: Vec<ActionHandle>,
    ) -> Self {
        Self {
            module,
            name,
            table,
            fields: fields.into(),
            actions: actions.into(),
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
}
