//! SQL 方言抽象（crate 内部，不导出公开 API）。
//!
//! MySQL 与 PostgreSQL 后端在标识符引号、占位符渲染、条件树→SQL 上的逻辑高度同构，
//! 原本各自复制一份。本模块把公共逻辑参数化收敛到一处：
//!
//! - [`Dialect`]：方言描述，仅含标识符引号字符与占位符风格（`?` / `$N`）。
//! - [`quote_identifier`] / [`quote_qualified`] / [`is_valid_identifier`]：DB-1 标识符
//!   校验与转义的方言参数化版本，各后端 `identifier.rs` 的公开函数薄封装于此。
//! - [`CondNode`] / [`SubqueryNode`] / [`render_condition`]：后端无关的条件树内部
//!   表示与共享渲染器；各后端的公开 `Condition` 枚举在渲染入口一次性转换为此树。
//!
//! 注意边界：upsert 语法、`RETURNING`、聚合 CAST 等能力差异由 `BackendCapabilities`
//! 显式声明并仍属各后端 `query_builder` 的职责，本模块不做也不应做抹平。

use std::fmt::Write as _;

use crate::error::DbError;
use crate::sql_types::{Identifier, QualifiedIdentifier, RenderedCondition};

/// SQL 方言描述：标识符引号字符 + 占位符风格。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Dialect {
    /// 标识符引号字符（MySQL 反引号 / PostgreSQL 双引号）。
    quote: char,
    /// `true` → `$N` 编号占位符（PostgreSQL，1 基，编号 = 参数偏移 + 已压参数数）；
    /// `false` → `?` 位置占位符（MySQL）。
    numbered_placeholders: bool,
}

/// MySQL 方言：反引号引用标识符，`?` 位置占位符。
#[cfg(feature = "mysql")]
pub(crate) const MYSQL: Dialect = Dialect {
    quote: '`',
    numbered_placeholders: false,
};

/// PostgreSQL 方言：双引号引用标识符，`$N` 编号占位符。
#[cfg(feature = "postgres")]
pub(crate) const POSTGRES: Dialect = Dialect {
    quote: '"',
    numbered_placeholders: true,
};

impl Dialect {
    /// 压入一个绑定参数并向 `out` 追加对应方言的占位符。
    ///
    /// 编号占位符的编号为 `parameter_offset + params.len()`（压入后），与 PG 版
    /// 原 `push_placeholder` 的编号规则严格一致；`write!` 直接写入 `out`，
    /// 避免 `format!` 产生短命 String（PERF-9）。写 `String` 不会失败。
    fn push_param<V>(
        self,
        params: &mut Vec<V>,
        value: V,
        parameter_offset: usize,
        out: &mut String,
    ) {
        params.push(value);
        if self.numbered_placeholders {
            let _ = write!(out, "${}", parameter_offset + params.len());
        } else {
            out.push('?');
        }
    }
}

/// 校验是否为合法的 SQL 标识符（方言无关）：`[A-Za-z_][A-Za-z0-9_]*`。
pub(crate) fn is_valid_identifier(s: &str) -> bool {
    Identifier::parse(s).is_ok()
}

/// 校验并用方言引号转义一个标识符（DB-1）。
pub(crate) fn quote_identifier(dialect: Dialect, ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::Unqualified(Identifier::parse(ident)?).render(dialect.quote))
}

/// 校验并转义可能带限定前缀的标识符：`列` / `表.列`，逐段校验并各自加引号（DB-1）。
pub(crate) fn quote_qualified(dialect: Dialect, ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse(ident)?.render(dialect.quote))
}

/// 后端无关的条件树（内部渲染表示）。
///
/// `V` 为各后端的 `SqlValue`；各后端公开的 `Condition` 枚举在渲染入口一次性
/// 转换为本树（值原样移动，不复制），随后由 [`render_condition`] 按方言输出。
/// 列比较操作符已在转换时映射为其 SQL 文本（`&'static str`）。
pub(crate) enum CondNode<V> {
    /// 相等
    Eq(String, V),
    /// 不等
    Ne(String, V),
    /// 大于
    Gt(String, V),
    /// 小于
    Lt(String, V),
    /// 大于等于
    Gte(String, V),
    /// 小于等于
    Lte(String, V),
    /// IN 条件
    In(String, Vec<V>),
    /// NOT IN 条件
    NotIn(String, Vec<V>),
    /// BETWEEN 条件
    Between(String, V, V),
    /// LIKE 条件（模式串已在转换时包装为后端值类型）。
    Like(String, V),
    /// IS NULL 条件
    IsNull(String),
    /// IS NOT NULL 条件
    IsNotNull(String),
    /// 两个标识符之间的受控比较（操作符已映射为 SQL 文本）。
    ColumnComparison(String, &'static str, String),
    /// 字段与受控服务端表达式的比较（MySQL 专属变体）。
    ///
    /// 表达式已在转换时预渲染为固定片段文本 + 可选绑定参数（如偏移秒数）；
    /// PG 的 `Condition` 无此变体，故只在 `mysql` feature 下存在。
    #[cfg(feature = "mysql")]
    ColumnExpr(String, &'static str, &'static str, Option<V>),
    /// EXISTS 子查询。
    Exists(Box<SubqueryNode<V>>),
    /// NOT EXISTS 子查询。
    NotExists(Box<SubqueryNode<V>>),
    /// IN 子查询。
    InSubquery(String, Box<SubqueryNode<V>>),
    /// AND 组合
    And(Vec<CondNode<V>>),
    /// OR 组合
    Or(Vec<CondNode<V>>),
}

/// 后端无关的受控子查询（内部渲染表示）。
pub(crate) struct SubqueryNode<V> {
    /// 已校验的表名。
    pub(crate) table: String,
    /// 已校验的投影字段（可为限定名）。
    pub(crate) field: String,
    /// AND 连接的条件列表。
    pub(crate) conditions: Vec<CondNode<V>>,
}

/// 按方言把条件树渲染为 SQL 片段与参数列表。
///
/// `parameter_offset` 是调用方参数列表的既有长度：编号占位符（`$N`）从
/// `parameter_offset + 1` 起编号；`?` 占位符方言忽略该参数（传 0）。
/// 校验失败时返回 [`DbError::InvalidArgument`]，且不会污染调用方参数列表
/// （参数先收集进局部缓冲，由调用方在成功后一次性 extend）。
pub(crate) fn render_condition<V>(
    dialect: Dialect,
    condition: CondNode<V>,
    parameter_offset: usize,
) -> Result<RenderedCondition<V>, DbError> {
    let mut rendered = RenderedCondition {
        sql: String::new(),
        params: Vec::new(),
    };
    write_condition(
        dialect,
        condition,
        &mut rendered.sql,
        &mut rendered.params,
        parameter_offset,
    )?;
    Ok(rendered)
}

/// 递归写入条件树；标识符一律经方言引号校验+转义（DB-1），值只进参数列表。
fn write_condition<V>(
    dialect: Dialect,
    condition: CondNode<V>,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    match condition {
        CondNode::Eq(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" = ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Ne(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" != ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Gt(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" > ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Lt(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" < ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Gte(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" >= ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Lte(field, value) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" <= ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::In(field, values) => {
            if values.is_empty() {
                return Err(DbError::InvalidArgument(format!(
                    "IN 条件 `{field}` 的值列表不能为空"
                )));
            }
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" IN (");
            for (index, value) in values.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                dialect.push_param(params, value, parameter_offset, out);
            }
            out.push(')');
            Ok(())
        }
        CondNode::NotIn(field, values) => {
            if values.is_empty() {
                return Err(DbError::InvalidArgument(format!(
                    "NOT IN 条件 `{field}` 的值列表不能为空"
                )));
            }
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" NOT IN (");
            for (index, value) in values.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                dialect.push_param(params, value, parameter_offset, out);
            }
            out.push(')');
            Ok(())
        }
        CondNode::Between(field, start, end) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" BETWEEN ");
            dialect.push_param(params, start, parameter_offset, out);
            out.push_str(" AND ");
            dialect.push_param(params, end, parameter_offset, out);
            Ok(())
        }
        CondNode::Like(field, pattern) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" LIKE ");
            dialect.push_param(params, pattern, parameter_offset, out);
            Ok(())
        }
        CondNode::IsNull(field) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" IS NULL");
            Ok(())
        }
        CondNode::IsNotNull(field) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" IS NOT NULL");
            Ok(())
        }
        CondNode::ColumnComparison(left, op, right) => {
            out.push_str(&quote_qualified(dialect, &left)?);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&quote_qualified(dialect, &right)?);
            Ok(())
        }
        #[cfg(feature = "mysql")]
        CondNode::ColumnExpr(field, op, fragment, param) => {
            // 表达式片段是 SqlExpr 白名单内的固定文本，动态部分（如偏移秒数）
            // 只以绑定参数进入参数列表，绝不内联进 SQL 文本。
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(fragment);
            if let Some(value) = param {
                // 片段文本自带占位符（如 `UNIX_TIMESTAMP() + ?`），参数直接压入即可。
                params.push(value);
            }
            Ok(())
        }
        CondNode::Exists(subquery) => {
            out.push_str("EXISTS (");
            write_subquery(dialect, *subquery, out, params, parameter_offset)?;
            out.push(')');
            Ok(())
        }
        CondNode::NotExists(subquery) => {
            out.push_str("NOT EXISTS (");
            write_subquery(dialect, *subquery, out, params, parameter_offset)?;
            out.push(')');
            Ok(())
        }
        CondNode::InSubquery(field, subquery) => {
            out.push_str(&quote_qualified(dialect, &field)?);
            out.push_str(" IN (");
            write_subquery(dialect, *subquery, out, params, parameter_offset)?;
            out.push(')');
            Ok(())
        }
        CondNode::And(mut conditions) => {
            if conditions.is_empty() {
                return Err(DbError::InvalidArgument("AND 条件组不能为空".to_string()));
            }
            if conditions.len() == 1 {
                return write_condition(
                    dialect,
                    conditions.remove(0),
                    out,
                    params,
                    parameter_offset,
                );
            }
            out.push('(');
            for (index, condition) in conditions.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(" AND ");
                }
                write_condition(dialect, condition, out, params, parameter_offset)?;
            }
            out.push(')');
            Ok(())
        }
        CondNode::Or(mut conditions) => {
            if conditions.is_empty() {
                return Err(DbError::InvalidArgument("OR 条件组不能为空".to_string()));
            }
            if conditions.len() == 1 {
                return write_condition(
                    dialect,
                    conditions.remove(0),
                    out,
                    params,
                    parameter_offset,
                );
            }
            out.push('(');
            for (index, condition) in conditions.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(" OR ");
                }
                write_condition(dialect, condition, out, params, parameter_offset)?;
            }
            out.push(')');
            Ok(())
        }
    }
}

/// 渲染受控子查询：`SELECT <field> FROM <table> [WHERE <cond> AND ...]`。
fn write_subquery<V>(
    dialect: Dialect,
    subquery: SubqueryNode<V>,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    out.push_str("SELECT ");
    out.push_str(&quote_qualified(dialect, &subquery.field)?);
    out.push_str(" FROM ");
    out.push_str(&quote_identifier(dialect, &subquery.table)?);
    if !subquery.conditions.is_empty() {
        out.push_str(" WHERE ");
        for (index, condition) in subquery.conditions.into_iter().enumerate() {
            if index > 0 {
                out.push_str(" AND ");
            }
            write_condition(dialect, condition, out, params, parameter_offset)?;
        }
    }
    Ok(())
}
