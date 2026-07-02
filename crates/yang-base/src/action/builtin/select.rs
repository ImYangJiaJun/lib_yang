//! SelectAction - 分页查询带 where 条件 + 排序
#![cfg(feature = "mysql")]

use crate::action::sql_bridge::count_with_tree;
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::{AsColumnName, Filter, SortOrder, TableEntity, WhereCondition};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
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
#[derive(schemars::JsonSchema)]
pub struct OrderByItem<T: TableEntity> {
    /// 字段名枚举
    pub field: T::Field,
    /// 方向，缺省为 Asc
    pub direction: SortOrder,
}

impl<'de, T: TableEntity> Deserialize<'de> for OrderByItem<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw<F> {
            field: F,
            #[serde(default = "default_sort_order")]
            direction: SortOrder,
        }
        let raw = Raw::<T::Field>::deserialize(d)?;
        Ok(OrderByItem {
            field: raw.field,
            direction: raw.direction,
        })
    }
}

/// SelectAction 的输入。
#[derive(schemars::JsonSchema)]
pub struct SelectQuery<T: TableEntity> {
    /// 页码（1 起步），缺省 1
    pub page: u32,
    /// 每页条数，缺省 10，必须 1..=100
    pub page_size: u32,
    /// where 布尔过滤树（叶子 + And/Or 嵌套），JSON key 为 `"where"`，缺省无条件
    pub where_clause: Option<Filter<T::WhereCond>>,
    /// 排序规则列表
    pub order_by: Vec<OrderByItem<T>>,
    /// 是否额外执行 COUNT 查询
    pub count_total: bool,
}

impl<'de, T: TableEntity> Deserialize<'de> for SelectQuery<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw<W, OI> {
            #[serde(default = "default_page")]
            page: u32,
            #[serde(default = "default_page_size")]
            page_size: u32,
            #[serde(rename = "where", default = "Option::default")]
            where_clause: Option<W>,
            #[serde(default = "Vec::new")]
            order_by: Vec<OI>,
            #[serde(default)]
            count_total: bool,
        }
        let raw = Raw::<Filter<T::WhereCond>, OrderByItem<T>>::deserialize(d)?;
        Ok(SelectQuery {
            page: raw.page,
            page_size: raw.page_size,
            where_clause: raw.where_clause,
            order_by: raw.order_by,
            count_total: raw.count_total,
        })
    }
}

/// 查询结果。
#[derive(Serialize, schemars::JsonSchema)]
pub struct SelectResult<T> {
    /// 数据列表
    pub items: Vec<T>,
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
pub struct SelectAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> SelectAction<T> {
    /// 创建 SelectAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for SelectAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for SelectAction<T> {
    type Input = SelectQuery<T>;
    type Output = SelectResult<T>;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: SelectQuery<T>,
    ) -> Result<SelectResult<T>, BaseError> {
        if input.page == 0 || input.page_size == 0 || input.page_size > 100 {
            return Err(BaseError::ParamInvalid(
                "page/page_size".into(),
                "page>=1, 1<=page_size<=100".into(),
            ));
        }

        // 把类型化布尔树降解为受保护层的 WhereCondition（None 表示无条件）
        let where_tree: Option<WhereCondition> =
            input.where_clause.map(Filter::into_where_condition);

        let total = if input.count_total {
            match &where_tree {
                Some(tree) => Some(count_with_tree(&ctx, tree.clone()).await?),
                None => Some(ctx.table_query()?.count().await?),
            }
        } else {
            None
        };

        // 字段读权限强制：始终走整实体 select，先确认当前用户对全部字段可读，
        // 否则返回 FieldPermissionDenied（匿名访问以空角色用户判定）。
        let user = ctx
            .user
            .as_ref()
            .ok_or_else(|| BaseError::Unauthorized("需要登录".to_string()))?;
        let mut q = ctx.table_query()?;
        q.ensure_fields_readable(user)?;
        // 整棵 where 树一次性递归校验 + 并入（含字段存在性/筛选权限/嵌套深度）
        if let Some(tree) = where_tree {
            q = q.where_tree(tree)?;
        }
        for OrderByItem { field, direction } in input.order_by {
            q = q.order_by(field.column_name(), direction)?;
        }
        // 设置分页参数
        q = q.page(input.page as usize, input.page_size as usize)?;
        let items: Vec<T> = q.select::<T>().await?;
        Ok(SelectResult {
            items,
            page: input.page,
            page_size: input.page_size,
            total,
        })
    }
}
