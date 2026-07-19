//! BR 心智连续的 Tables 深模块。

#[cfg(feature = "mysql")]
use super::RelationBatchExecutor;
use super::{
    PaginatedResult, QueryParams, Record, RelationData, RelationLoader, SortOrder, TableQuery,
    WhereCondition, DEFAULT_QUERY_PAGE_SIZE,
};
use crate::definition::CompiledTableView;
use crate::error::BaseError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 树查询的默认节点上限（I-7）。
///
/// 树查询需要把全部匹配行读入内存组装，无分页保护；该常量限制单次树查询
/// 可处理的节点数，超出即报错。`TreeViewSpec::max_nodes` 可按 View 覆盖。
pub const DEFAULT_TREE_MAX_NODES: usize = 10_000;

/// 校验树查询节点数未超过上限；超限返回 `ParamInvalid`，提示调用方先收窄筛选。
#[cfg(any(feature = "mysql", test))]
fn ensure_tree_node_cap(count: usize, max_nodes: usize) -> Result<(), BaseError> {
    if count > max_nodes {
        return Err(BaseError::ParamInvalid(
            "tree".to_string(),
            format!("树节点数 {count} 超过上限 {max_nodes}，请先收窄查询范围"),
        ));
    }
    Ok(())
}

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
    #[cfg(feature = "mysql")]
    pub async fn table_list(self) -> Result<PaginatedResult<Record>, BaseError> {
        self.query.paginate_records().await
    }

    /// 执行选择器数据查询。
    ///
    /// 底层 `all()` 一次性读取全部匹配行，**不受** TableQuery 最大分页限制保护；
    /// 调用方必须先通过 where/search 条件把结果规模约束在内存可承载范围内。
    #[cfg(feature = "mysql")]
    pub async fn table_select(self) -> Result<Vec<Record>, BaseError> {
        self.query.all().await
    }

    /// 查询全量选择结果并按默认 `id` / `parent_id` 组装树。
    ///
    /// 节点数受 [`DEFAULT_TREE_MAX_NODES`] 限制，超出即报错；需要自定义上限的
    /// 树 View 应使用 [`Tables::table_tree_view`] 并在 `TreeViewSpec` 中配置。
    #[cfg(feature = "mysql")]
    pub async fn table_tree(self) -> Result<Vec<TableTreeNode>, BaseError> {
        let read_limit = DEFAULT_TREE_MAX_NODES
            .checked_add(1)
            .ok_or_else(|| BaseError::ConfigError("默认树节点上限无法计算截断探针".to_string()))?;
        let rows = self.query.prefetch_limit(read_limit)?.all().await?;
        ensure_tree_node_cap(rows.len(), DEFAULT_TREE_MAX_NODES)?;
        Self::build_tree(rows, "id", "parent_id")
    }

    /// 按启动期已校验的 View 树拓扑查询并组装节点。
    ///
    /// 节点数上限取 `TreeViewSpec::max_nodes`（缺省 [`DEFAULT_TREE_MAX_NODES`]），
    /// 超出即报错，避免全表无界读入内存。
    #[cfg(feature = "mysql")]
    pub async fn table_tree_view(
        self,
        view: &CompiledTableView,
    ) -> Result<Vec<TableTreeNode>, BaseError> {
        let tree = view
            .tree()
            .ok_or_else(|| BaseError::ConfigError(format!("View {} 未声明树拓扑", view.name())))?;
        let max_nodes = tree.max_nodes();
        let read_limit = max_nodes.checked_add(1).ok_or_else(|| {
            BaseError::ConfigError(format!("View {} 的树节点上限过大", view.name()))
        })?;
        let rows = self
            .view(view)?
            .query
            .prefetch_limit(read_limit)?
            .all()
            .await?;
        ensure_tree_node_cap(rows.len(), max_nodes)?;
        Self::build_tree(rows, tree.id_field_name(), tree.parent_field_name())
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
            if id.is_null() {
                return Err(BaseError::ParamInvalid(
                    id_field.to_string(),
                    "树节点主键不能为 null".to_string(),
                ));
            }
            if ids.insert(value_key(id), index).is_some() {
                return Err(BaseError::ConfigError(format!(
                    "树节点主键重复: {id_field}={id}"
                )));
            }
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
    #[cfg(feature = "mysql")]
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
    fn tree_node_cap_rejects_overflow() {
        // 边界：恰好等于上限放行。
        ensure_tree_node_cap(DEFAULT_TREE_MAX_NODES, DEFAULT_TREE_MAX_NODES)
            .expect("节点数等于上限应放行");
        ensure_tree_node_cap(3, 10).expect("节点数低于上限应放行");

        let error = ensure_tree_node_cap(DEFAULT_TREE_MAX_NODES + 1, DEFAULT_TREE_MAX_NODES)
            .expect_err("节点数超过上限必须报错");
        assert!(
            matches!(error, BaseError::ParamInvalid(_, _)),
            "树节点超限应返回 ParamInvalid: {error:?}"
        );

        // 显式配置的小上限同样生效。
        let error = ensure_tree_node_cap(11, 10).expect_err("自定义上限超限必须报错");
        assert!(matches!(error, BaseError::ParamInvalid(_, _)));
    }

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

    #[test]
    fn tree_builder_rejects_null_and_duplicate_ids() {
        let null_id = Tables::build_tree(
            vec![Record::new()
                .set("id", Value::Null)
                .set("parent_id", Value::Null)],
            "id",
            "parent_id",
        )
        .expect_err("null 主键必须失败");
        assert!(matches!(null_id, BaseError::ParamInvalid(_, _)));

        let duplicate = Tables::build_tree(
            vec![
                Record::new().set("id", 1).set("parent_id", Value::Null),
                Record::new().set("id", 1).set("parent_id", Value::Null),
            ],
            "id",
            "parent_id",
        )
        .expect_err("重复主键必须失败");
        assert!(matches!(duplicate, BaseError::ConfigError(_)));
    }

    #[test]
    fn tree_builder_rejects_cycles_without_roots() {
        let cycle = Tables::build_tree(
            vec![
                Record::new().set("id", 1).set("parent_id", 2),
                Record::new().set("id", 2).set("parent_id", 1),
            ],
            "id",
            "parent_id",
        )
        .expect_err("无根循环必须失败");
        assert!(matches!(cycle, BaseError::ConfigError(_)));
    }
}
