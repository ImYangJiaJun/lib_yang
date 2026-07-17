//! BR 心智连续的 Tables 深模块。

use super::{
    PaginatedResult, QueryParams, Record, RelationBatchExecutor, RelationData, RelationLoader,
    SortOrder, TableQuery, WhereCondition, DEFAULT_QUERY_PAGE_SIZE,
};
use crate::definition::CompiledTableView;
use crate::error::BaseError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 带批量关系数据的稳定列表响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableListResult {
    /// 分页主数据。
    pub page: PaginatedResult<Record>,
    /// 按源字段分组的批量关系记录。
    pub relations: RelationData,
}

/// `table_tree` 的稳定树节点响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableTreeNode {
    /// 当前节点原始列。
    #[serde(flatten)]
    pub record: Record,
    /// 子节点，保持数据库结果中的相对顺序。
    pub children: Vec<TableTreeNode>,
}

/// 统一封装列表参数、View 字段、分页与关系批量加载。
#[derive(Debug, Clone)]
pub struct Tables {
    query: TableQuery,
    relation_loader: RelationLoader,
}

impl Tables {
    /// 从已经绑定权限和租户范围的 TableQuery 创建。
    pub fn new(query: TableQuery) -> Self {
        Self {
            query,
            relation_loader: RelationLoader::default(),
        }
    }

    /// 应用启动期预编译的 View 字段。
    pub fn view(mut self, view: &CompiledTableView) -> Result<Self, BaseError> {
        let fields = view
            .fields()
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .rsplit_once('.')
                    .map_or(field.as_str(), |(_, name)| name)
            })
            .collect::<Vec<_>>();
        self.query = self.query.select_fields(&fields)?;
        Ok(self)
    }

    /// 配置启动期预编译的关系批量加载器。
    pub fn relations(mut self, loader: RelationLoader) -> Self {
        self.relation_loader = loader;
        self
    }

    /// 应用通用筛选条件。
    pub fn where_from(mut self, conditions: &[WhereCondition]) -> Result<Self, BaseError> {
        self.query = self.query.where_and(conditions.to_vec())?;
        Ok(self)
    }

    /// 在定义允许搜索的文本字段上应用关键词。
    pub fn search(mut self, keyword: Option<&str>) -> Result<Self, BaseError> {
        self.query = self.query.search(keyword)?;
        Ok(self)
    }

    /// 应用受权限保护的排序。
    pub fn order(mut self, field: &str, order: SortOrder) -> Result<Self, BaseError> {
        self.query = self.query.order_by(field, order)?;
        Ok(self)
    }

    /// 应用分页。
    pub fn page(mut self, page: usize, limit: usize) -> Result<Self, BaseError> {
        self.query = self.query.page(page, limit)?;
        Ok(self)
    }

    /// 一次性应用标准 QueryParams。
    pub fn params(mut self, params: QueryParams) -> Result<Self, BaseError> {
        self.query = self.query.apply_params(params)?;
        Ok(self)
    }

    /// 返回标准列表参数默认值。
    pub fn params_table() -> QueryParams {
        QueryParams::new().with_pagination(1, DEFAULT_QUERY_PAGE_SIZE)
    }

    /// 执行标准分页列表。
    pub async fn table_list(self) -> Result<PaginatedResult<Record>, BaseError> {
        self.query.paginate_records().await
    }

    /// 执行选择器数据查询；默认受 TableQuery 最大分页保护。
    pub async fn table_select(self) -> Result<Vec<Record>, BaseError> {
        self.query.all().await
    }

    /// 查询全量选择结果并按默认 `id` / `parent_id` 组装树。
    pub async fn table_tree(self) -> Result<Vec<TableTreeNode>, BaseError> {
        let rows = self.query.all().await?;
        Self::build_tree(rows, "id", "parent_id")
    }

    /// 将已查询记录组装为树；关系键只用于内存索引，不进入 SQL。
    pub fn build_tree(
        rows: Vec<Record>,
        id_field: &str,
        parent_field: &str,
    ) -> Result<Vec<TableTreeNode>, BaseError> {
        let mut ids = HashMap::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let id = row.get(id_field).ok_or_else(|| {
                BaseError::ParamInvalid(id_field.to_string(), "树节点缺少主键".to_string())
            })?;
            ids.insert(value_key(id), index);
        }

        let mut children = vec![Vec::new(); rows.len()];
        let mut roots = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let parent = row.get(parent_field).unwrap_or(&Value::Null);
            if parent.is_null() {
                roots.push(index);
            } else if let Some(parent_index) = ids.get(&value_key(parent)).copied() {
                children[parent_index].push(index);
            } else {
                roots.push(index);
            }
        }

        let mut rows = rows.into_iter().map(Some).collect::<Vec<_>>();
        let mut visiting = HashSet::new();
        let nodes = roots
            .into_iter()
            .map(|index| build_node(index, &mut rows, &children, &mut visiting))
            .collect::<Result<Vec<_>, _>>()?;
        if rows.iter().any(Option::is_some) {
            return Err(BaseError::ConfigError("树关系包含循环".to_string()));
        }
        Ok(nodes)
    }

    /// 执行分页列表并按关系种类批量加载展示数据。
    pub async fn table_list_with_relations<E>(
        self,
        executor: &E,
    ) -> Result<TableListResult, BaseError>
    where
        E: RelationBatchExecutor,
    {
        let page = self.query.paginate_records().await?;
        let relations = self.relation_loader.load(&page.data, executor).await?;
        Ok(TableListResult { page, relations })
    }
}

fn value_key(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn build_node(
    index: usize,
    rows: &mut [Option<Record>],
    children: &[Vec<usize>],
    visiting: &mut HashSet<usize>,
) -> Result<TableTreeNode, BaseError> {
    if !visiting.insert(index) {
        return Err(BaseError::ConfigError("树关系包含循环".to_string()));
    }
    let record = rows[index]
        .take()
        .ok_or_else(|| BaseError::ConfigError("树节点被重复引用".to_string()))?;
    let nodes = children[index]
        .iter()
        .copied()
        .map(|child| build_node(child, rows, children, visiting))
        .collect::<Result<Vec<_>, _>>()?;
    visiting.remove(&index);
    Ok(TableTreeNode {
        record,
        children: nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_builder_keeps_order_and_nests_children() {
        let rows = vec![
            Record::new().set("id", 1).set("parent_id", Value::Null),
            Record::new().set("id", 2).set("parent_id", 1),
        ];
        let tree = Tables::build_tree(rows, "id", "parent_id").expect("树关系应有效");
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(
            tree[0].children[0].record.get("id"),
            Some(&serde_json::json!(2))
        );
    }
}
