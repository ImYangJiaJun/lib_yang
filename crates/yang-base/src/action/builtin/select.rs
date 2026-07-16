//! SelectAction - 分页查询带 where 条件 + 排序
#![cfg(feature = "mysql")]

use crate::action::sql_bridge::count_with_tree;
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::{Record, SortOrder, WhereCondition};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use yang_base_derive::Action;

fn default_page() -> u32 {
    1
}
fn default_page_size() -> u32 {
    10
}
fn default_sort_order() -> SortOrder {
    SortOrder::Asc
}

/// 排序条目（JSON 形态：`{"field": "id", "direction": "desc"}`）。
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OrderByItem {
    /// 字段名
    pub field: String,
    /// 方向，缺省为 Asc
    #[serde(default = "default_sort_order")]
    pub direction: SortOrder,
}

/// SelectAction 的输入。
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectQuery {
    /// 页码（1 起步），缺省 1
    #[serde(default = "default_page")]
    pub page: u32,
    /// 每页条数，缺省 10，必须 1..=100
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    /// where 布尔过滤树（叶子 + And/Or 嵌套），JSON key 为 `"where"`，缺省无条件
    #[serde(rename = "where", default)]
    pub where_clause: Option<WhereCondition>,
    /// 排序规则列表
    #[serde(default)]
    pub order_by: Vec<OrderByItem>,
    /// 是否额外执行 COUNT 查询
    #[serde(default)]
    pub count_total: bool,
}

/// 查询结果。
#[derive(Serialize, schemars::JsonSchema)]
pub struct SelectResult {
    /// 数据列表
    pub items: Vec<Record>,
    /// 当前页码
    pub page: u32,
    /// 每页条数
    pub page_size: u32,
    /// 总数（仅 count_total=true 时返回）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

/// 分页 + 多条件 AND 查询。
#[derive(Action)]
#[action(
    name = "select",
    display_name = "查询列表",
    description = "分页 + 多条件 AND 查询"
)]
pub struct SelectAction;

impl SelectAction {
    /// 创建 SelectAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for SelectAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for SelectAction {
    type Input = SelectQuery;
    type Output = SelectResult;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: SelectQuery,
    ) -> Result<SelectResult, BaseError> {
        let SelectQuery {
            page,
            page_size,
            where_clause,
            order_by,
            count_total,
        } = input;
        if page == 0 || page_size == 0 || page_size > 100 {
            return Err(BaseError::ParamInvalid(
                "page/page_size".into(),
                "page>=1, 1<=page_size<=100".into(),
            ));
        }

        // 默认投影当前角色可读且非 secret 的字段。
        // 【LOGIC-3】认证必须在 COUNT 之前，避免未登录用户绕过权限获取总数。
        if ctx.user.is_none() {
            return Err(BaseError::Unauthorized("需要登录".to_string()));
        }
        let mut q = ctx.table_query()?;
        q.ensure_readable_projection()?;

        let total = if count_total {
            match &where_clause {
                Some(tree) => Some(count_with_tree(&ctx, tree.clone()).await?),
                None => Some(ctx.table_query()?.count().await?),
            }
        } else {
            None
        };
        // 整棵 where 树一次性递归校验 + 并入（含字段存在性/筛选权限/嵌套深度）
        if let Some(tree) = where_clause {
            q = q.where_tree(tree)?;
        }
        for OrderByItem { field, direction } in order_by {
            q = q.order_by(&field, direction)?;
        }
        // 设置分页参数
        q = q.page(page as usize, page_size as usize)?;
        let items = q.all().await?;
        Ok(SelectResult {
            items,
            page,
            page_size,
            total,
        })
    }
}
