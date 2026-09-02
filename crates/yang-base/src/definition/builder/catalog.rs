//! 构建期产生的确定性只读定义快照。

use crate::definition::AddonSpec;

/// 构建期产生的确定性只读定义快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefinitionCatalog {
    pub(super) addons: Vec<AddonSpec>,
}

impl DefinitionCatalog {
    /// 按名称稳定排序的 Addon 定义。
    pub fn addons(&self) -> &[AddonSpec] {
        &self.addons
    }

    /// 返回按稳定注册顺序排列的原生表定义。
    pub fn tables(&self) -> impl Iterator<Item = &crate::definition::TableSpec> {
        self.addons
            .iter()
            .flat_map(|addon| &addon.modules)
            .filter_map(|module| module.table.as_ref())
    }
}
