//! MySQL 链式 SQL 查询构建器。
//!
//! 按职责拆分为子模块（同一 [`QueryBuilder`] 类型的 impl 块分散在多个文件中）：
//!
//! - `generator`：SQL 文本与参数生成（`SqlGenerator`）
//! - `builder`：链式字段/条件/JOIN/分组/分页方法
//! - `render`：SQL 渲染入口（`to_sql` / `try_to_sql` / 事务行锁渲染）
//! - `read` / `aggregate` / `write`：查询执行
//! - `predicate` / `bind`：谓词转换与参数绑定助手

mod aggregate;
mod bind;
mod builder;
mod generator;
mod predicate;
mod read;
mod render;
mod write;

use std::collections::HashMap;

use sqlx::mysql::MySqlPool;

use crate::mysql::condition::Condition;
use crate::mysql::field::{FieldType, JoinClause, OrderClause};

// 保持既有 crate 内部路径 `crate::mysql::query_builder::*` 可用（transaction.rs 与测试使用）
pub(crate) use bind::bind_param;
pub(crate) use generator::SqlGenerator;
#[cfg(test)]
pub(crate) use predicate::predicate_value;

/// 将 `(字段, 操作符, 值)` 映射为 6 个比较类 `Condition` 变体的共享助手。
///
/// 仅处理比较操作符（`=`、`!=`、`>`、`<`、`>=`、`<=`）。无法匹配时把 `value`
/// 原样通过 `Err` 交还调用方，便于上层继续处理 like 等其它操作符而不丢失所有权。
/// 抽出此助手以消除 `where_and` / `where_or` / `having_cond` 三处重复的映射 match。
#[derive(Debug, Clone, Copy)]
enum UnionOperator {
    Distinct,
    All,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ArithmeticOperator {
    Add,
    Subtract,
}

impl ArithmeticOperator {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
        }
    }
}

/// 查询构建器
enum QueryExecutor<'a> {
    Pool(&'a MySqlPool),
    Transaction(&'a mut crate::mysql::Transaction),
}

pub struct QueryBuilder<'a> {
    executor: QueryExecutor<'a>,
    table: String,
    pub(crate) fields: Vec<String>,
    #[allow(dead_code)]
    pub(crate) conditions: Vec<Condition>,
    #[allow(dead_code)]
    pub(crate) joins: Vec<JoinClause>,
    #[allow(dead_code)]
    pub(crate) order_by: Vec<OrderClause>,
    #[allow(dead_code)]
    pub(crate) group_by: Vec<String>,
    #[allow(dead_code)]
    pub(crate) having_clause: Vec<Condition>,
    pub(crate) limit: Option<u64>,
    pub(crate) offset: Option<u64>,
    pub(crate) distinct: bool,
    unions: Vec<(UnionOperator, Box<QueryBuilder<'a>>)>,
    pub(crate) field_types: HashMap<String, FieldType>,
    /// 受控服务端表达式写入值（UPDATE 的 SET 与 INSERT 的 VALUES 共用）。
    expr_assignments: Vec<(String, crate::SqlExpr)>,
    /// SELECT 投影中的受控服务端标量表达式及其输出别名。
    select_exprs: Vec<(crate::SqlExpr, String)>,
    #[allow(dead_code)]
    enable_logging: bool,
}

impl<'a> QueryBuilder<'a> {
    /// 从共享连接池创建查询构建器（pool 侧正式入口，与 [`crate::Database::table`]
    /// 等价，但不携带连接池的日志开关）。
    ///
    /// 主要用途：构造将要交给 [`crate::mysql::Transaction::select`] /
    /// [`crate::mysql::Transaction::select_for_update`] /
    /// [`crate::mysql::Transaction::select_locked`] 执行的 SELECT——这些方法只读取
    /// 构建器状态并在事务连接上执行，因此构建器必须从不借用事务的 pool 创建。
    pub fn from_pool(pool: &'a MySqlPool, table: &crate::TableRef) -> Self {
        Self::new(pool, table.as_str(), false)
    }

    /// 创建新的查询构建器
    pub(crate) fn new(pool: &'a MySqlPool, table_name: &str, enable_logging: bool) -> Self {
        Self {
            executor: QueryExecutor::Pool(pool),
            table: table_name.to_string(),
            fields: Vec::new(),
            conditions: Vec::new(),
            joins: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            having_clause: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
            unions: Vec::new(),
            field_types: HashMap::new(),
            expr_assignments: Vec::new(),
            select_exprs: Vec::new(),
            enable_logging,
        }
    }

    pub(crate) fn new_transaction(
        transaction: &'a mut crate::mysql::Transaction,
        table_name: &str,
        enable_logging: bool,
    ) -> Self {
        Self {
            executor: QueryExecutor::Transaction(transaction),
            table: table_name.to_string(),
            fields: Vec::new(),
            conditions: Vec::new(),
            joins: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            having_clause: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
            unions: Vec::new(),
            field_types: HashMap::new(),
            expr_assignments: Vec::new(),
            select_exprs: Vec::new(),
            enable_logging,
        }
    }
}
