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
    fn push_param<V: CondValue>(
        self,
        params: &mut Vec<V>,
        value: V,
        parameter_offset: usize,
        out: &mut String,
    ) {
        let cast_suffix = value.cast_suffix();
        params.push(value);
        if self.numbered_placeholders {
            let _ = write!(out, "${}", parameter_offset + params.len());
        } else {
            out.push('?');
        }
        if let Some(suffix) = cast_suffix {
            out.push_str(suffix);
        }
    }

    /// 按方言渲染受控服务端表达式，返回（固定 SQL 片段，按占位符顺序的绑定参数）。
    ///
    /// 编号占位符方言由调用方传入下一个占位符序号；`?` 方言忽略该序号。
    /// MySQL 不支持的表达式（如 `to_char`）返回 [`DbError::UnsupportedOperator`]。
    #[cfg(feature = "postgres")]
    fn render_expr(
        self,
        expression: &crate::SqlExpr,
        placeholder: usize,
    ) -> Result<(String, Vec<crate::reference::SqlExprParam>), DbError> {
        if self.numbered_placeholders {
            #[cfg(feature = "postgres")]
            {
                let rendered = expression.postgres_render(placeholder);
                Ok((rendered.sql, rendered.params))
            }
            #[cfg(not(feature = "postgres"))]
            {
                let _ = (expression, placeholder);
                unreachable!("编号占位符方言只有 PostgreSQL")
            }
        } else {
            #[cfg(feature = "mysql")]
            {
                let _ = placeholder;
                let rendered = expression.mysql_render()?;
                Ok((rendered.sql, rendered.params))
            }
            #[cfg(not(feature = "mysql"))]
            {
                let _ = (expression, placeholder);
                unreachable!("? 占位符方言只有 MySQL")
            }
        }
    }
}

/// 校验是否为合法的 SQL 标识符（方言无关）：`[A-Za-z_][A-Za-z0-9_]*`。
pub(crate) fn is_valid_identifier(s: &str) -> bool {
    Identifier::parse(s).is_ok()
}

/// 校验并用方言引号转义一个标识符（DB-1）。
pub(crate) fn quote_identifier(dialect: Dialect, ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse_with_max_parts(ident, 1)?.render(dialect.quote))
}

/// 校验并转义可能带限定前缀的标识符：`列` / `表.列` / `schema.表.列`，逐段校验并各自加引号（DB-1）。
pub(crate) fn quote_qualified(dialect: Dialect, ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse(ident)?.render(dialect.quote))
}

/// 校验并转义表名：只接受 `table` 或 `schema.table`（最多两段），逐段加引号。
#[cfg(feature = "postgres")]
pub(crate) fn quote_table(dialect: Dialect, ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse_with_max_parts(ident, 2)?.render(dialect.quote))
}

/// 条件值的空值判定。
///
/// 条件渲染器需要把 SQL `NULL` 从绑定参数改成 `IS NULL` / `IS NOT NULL`：
/// PostgreSQL 对未类型化 NULL 的绑定会按参数 OID 严格校验，`text_col = $1`
/// 绑定 int4 NULL 会直接报 `operator does not exist`；而 `= NULL` 在两种方言
/// 下都恒为 UNKNOWN。由各后端实现本 trait，渲染层据此归一。
pub(crate) trait CondValue {
    /// 是否为 SQL NULL。
    fn is_null(&self) -> bool;

    /// 从受控表达式的绑定参数构造后端值。
    #[cfg(feature = "postgres")]
    fn from_expr_param(param: crate::reference::SqlExprParam) -> Self;

    /// 是否为原生 JSON 值（JSON/JSONB 谓词只接受原生 JSON 绑定）。
    ///
    /// 仅 PostgreSQL 有 JSON 谓词变体；默认 `false` 供 MySQL 实现沿用。
    #[cfg(feature = "postgres")]
    fn is_json(&self) -> bool {
        false
    }

    /// 是否为数组值（`ANY` / `ALL` 的右值必须是数组绑定）。
    ///
    /// 仅 PostgreSQL 有数组谓词变体；默认 `false` 供 MySQL 实现沿用。
    #[cfg(feature = "postgres")]
    fn is_array(&self) -> bool {
        false
    }

    /// 是否为范围/多范围文本值（范围谓词的右值约束）。
    ///
    /// 仅 PostgreSQL 有范围谓词变体；默认 `false` 供 MySQL 实现沿用。
    #[cfg(feature = "postgres")]
    fn is_range_value(&self) -> bool {
        false
    }

    /// 绑定参数需要的 PostgreSQL 类型后缀（如 `::inet`）；MySQL 恒为 `None`。
    fn cast_suffix(&self) -> Option<&'static str> {
        None
    }

    /// 数组值是否为空（`ANY`/`ALL` 的空数组有确定的真值语义）。
    #[cfg(feature = "postgres")]
    fn array_is_empty(&self) -> bool {
        false
    }
}

#[cfg(feature = "mysql")]
impl CondValue for crate::mysql::SqlValue {
    fn is_null(&self) -> bool {
        matches!(self, crate::mysql::SqlValue::Null)
    }

    /// 仅在同时启用 postgres 时 trait 才要求该方法（表达式渲染是 PG 路径）。
    #[cfg(feature = "postgres")]
    fn from_expr_param(param: crate::reference::SqlExprParam) -> Self {
        param.into()
    }
}

#[cfg(feature = "postgres")]
impl CondValue for crate::postgres::SqlValue {
    fn is_null(&self) -> bool {
        matches!(self, crate::postgres::SqlValue::Null)
    }

    fn from_expr_param(param: crate::reference::SqlExprParam) -> Self {
        param.into()
    }

    fn is_json(&self) -> bool {
        matches!(self, crate::postgres::SqlValue::Json(_))
    }

    fn is_array(&self) -> bool {
        matches!(
            self,
            crate::postgres::SqlValue::Array(_) | crate::postgres::SqlValue::TypedArray { .. }
        )
    }

    fn is_range_value(&self) -> bool {
        matches!(
            self,
            crate::postgres::SqlValue::Range(_) | crate::postgres::SqlValue::Multirange(_)
        )
    }

    fn array_is_empty(&self) -> bool {
        matches!(
            self,
            crate::postgres::SqlValue::Array(values) if values.is_empty()
        ) || matches!(
            self,
            crate::postgres::SqlValue::TypedArray { values, .. } if values.is_empty()
        )
    }

    fn cast_suffix(&self) -> Option<&'static str> {
        match self {
            crate::postgres::SqlValue::Inet(_) => Some("::inet"),
            crate::postgres::SqlValue::Cidr(_) => Some("::cidr"),
            crate::postgres::SqlValue::MacAddr(_) => Some("::macaddr"),
            crate::postgres::SqlValue::MacAddr8(_) => Some("::macaddr8"),
            crate::postgres::SqlValue::Range(_) => Some("::int4range"),
            crate::postgres::SqlValue::Multirange(_) => Some("::int4multirange"),
            // 带元素类型的数组：字符串族与网络族绑定的是 TEXT[]，需要追加显式
            // 转换；其余元素类型绑定的 Rust 数组 OID 已与列类型一致。
            crate::postgres::SqlValue::TypedArray { element_type, .. } => match *element_type {
                "varchar" => Some("::varchar[]"),
                "inet" => Some("::inet[]"),
                "cidr" => Some("::cidr[]"),
                "macaddr" => Some("::macaddr[]"),
                "macaddr8" => Some("::macaddr8[]"),
                _ => None,
            },
            _ => None,
        }
    }
}

/// 校验并用方言引号转义一个条件里的字段引用。
///
/// PostgreSQL 的 `ON CONFLICT DO UPDATE` 会暴露 `EXCLUDED` 伪表：`excluded.col`
/// 必须保留小写、未加引号的 `excluded` 前缀，写成 `"EXCLUDED"."col"` 会被当成
/// 真实表而报 42P01。其余情况下与 [`quote_qualified`] 完全一致。
fn quote_condition_field(dialect: Dialect, field: &str) -> Result<String, DbError> {
    if dialect.numbered_placeholders {
        if let Some((first, rest)) = field.split_once('.') {
            if first.eq_ignore_ascii_case("excluded") {
                return Ok(format!("excluded.{}", quote_identifier(dialect, rest)?));
            }
        }
    }
    quote_qualified(dialect, field)
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
    /// 模式/正则匹配：操作符文本来自受控白名单（`ILIKE` / `NOT ILIKE` / `~` /
    /// `~*` / `!~` / `!~*` / `REGEXP` / `NOT REGEXP`），模式串走绑定参数。
    PatternMatch(String, &'static str, V),
    /// NULL-safe 比较：`field IS [NOT] DISTINCT FROM value`。
    ///
    /// `bool` 为 `true` 时渲染 `IS DISTINCT FROM`，否则 `IS NOT DISTINCT FROM`。
    /// 右值为 SQL NULL 时内联 `NULL`（该形式对任意列类型都合法）。
    #[cfg(feature = "postgres")]
    DistinctFrom(String, bool, V),
    /// IS NULL 条件
    IsNull(String),
    /// IS NOT NULL 条件
    IsNotNull(String),
    /// 两个标识符之间的受控比较（操作符已映射为 SQL 文本）。
    ColumnComparison(String, &'static str, String),
    /// 字段与受控服务端表达式的比较（MySQL 专属变体）。
    ///
    /// 表达式已在转换时预渲染为固定片段文本 + 按占位符顺序的绑定参数；
    /// PG 的 `Condition` 无此变体，故只在 `mysql` feature 下存在。
    #[cfg(feature = "mysql")]
    ColumnExpr(String, &'static str, String, Vec<V>),
    /// 字段与受控服务端表达式的比较。
    ///
    /// 表达式在渲染期按方言生成固定片段；动态偏移量只以绑定参数进入参数列表，
    /// 占位符编号由渲染器基于调用方参数偏移连续推导。
    #[cfg(feature = "postgres")]
    ExprComparison(String, &'static str, crate::SqlExpr),
    /// EXISTS 子查询。
    Exists(Box<SubqueryNode<V>>),
    /// NOT EXISTS 子查询。
    NotExists(Box<SubqueryNode<V>>),
    /// IN 子查询。
    InSubquery(String, Box<SubqueryNode<V>>),
    /// JSON/JSONB 包含谓词：`field @> $n`（PostgreSQL 专属）。
    ///
    /// 右值必须是原生 JSON 绑定（`SqlValue::Json`）；NULL 渲染为 SQL `NULL`
    ///（谓词恒为 UNKNOWN，fail-closed）。
    #[cfg(feature = "postgres")]
    JsonContains(String, V),
    /// JSON/JSONB 被包含谓词：`field <@ $n`（PostgreSQL 专属）。
    ///
    /// 右值约束与 [`Self::JsonContains`] 相同。
    #[cfg(feature = "postgres")]
    JsonContainedBy(String, V),
    /// JSON/JSONB 键存在谓词：`field ? $n`（PostgreSQL 专属）。
    ///
    /// 键名以绑定参数进入参数列表，绝不内联进 SQL 文本（`?` 运算符右侧
    /// 若拼接原始文本会形成注入面）。
    #[cfg(feature = "postgres")]
    JsonHasKey(String, V),
    /// 数组任一元素相等：`field = ANY($n)`（PostgreSQL 专属）。
    ///
    /// 右值必须是 `SqlValue::Array`，按单个数组绑定参数渲染。
    #[cfg(feature = "postgres")]
    Any(String, V),
    /// 数组全部元素相等：`field = ALL($n)`（PostgreSQL 专属）。
    ///
    /// 右值约束与 [`Self::Any`] 相同。
    #[cfg(feature = "postgres")]
    All(String, V),
    /// 范围包含：`field @> $n`（PostgreSQL 专属）。
    ///
    /// 右值必须是范围或多范围文本值，渲染时由值自身追加显式类型转换。
    #[cfg(feature = "postgres")]
    RangeContains(String, V),
    /// 范围重叠：`field && $n`（PostgreSQL 专属）。
    ///
    /// 右值约束与 [`Self::RangeContains`] 相同。
    #[cfg(feature = "postgres")]
    RangeOverlaps(String, V),
    /// JSONB 路径存在：`jsonb_path_exists((field)::jsonb, $n::jsonpath)`。
    ///
    /// 路径以绑定参数进入，绝不内联进 SQL 文本。
    #[cfg(feature = "postgres")]
    JsonPathExists(String, V),
    /// JSONB 路径谓词：`jsonb_path_match((field)::jsonb, $n::jsonpath, $m::jsonb)`。
    ///
    /// 路径与变量对象都以绑定参数进入。
    #[cfg(feature = "postgres")]
    JsonPathMatch(String, V, V),
    /// 全文检索：`to_tsvector('simple', field) @@ plainto_tsquery('simple', $n)`。
    ///
    /// 查询串以绑定参数进入；配置名固定为 `simple`（不依赖 locale 分词器）。
    #[cfg(feature = "postgres")]
    FullText(String, V),
    /// 全文检索（`to_tsquery` 变体）：查询串支持布尔操作符。
    #[cfg(feature = "postgres")]
    FullTextQuery(String, V),
    /// NOT 包裹（PostgreSQL 构造器已接入；MySQL 侧尚无公开入口）。
    #[cfg(feature = "postgres")]
    Not(Box<CondNode<V>>),
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
pub(crate) fn render_condition<V: CondValue>(
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
fn write_condition<V: CondValue>(
    dialect: Dialect,
    condition: CondNode<V>,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    match condition {
        CondNode::Eq(field, value) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            if value.is_null() {
                out.push_str(" IS NULL");
            } else {
                out.push_str(" = ");
                dialect.push_param(params, value, parameter_offset, out);
            }
            Ok(())
        }
        CondNode::Ne(field, value) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            if value.is_null() {
                out.push_str(" IS NOT NULL");
            } else {
                out.push_str(" != ");
                dialect.push_param(params, value, parameter_offset, out);
            }
            Ok(())
        }
        CondNode::Gt(field, value) => {
            if value.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" > ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Lt(field, value) => {
            if value.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" < ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Gte(field, value) => {
            if value.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" >= ");
            dialect.push_param(params, value, parameter_offset, out);
            Ok(())
        }
        CondNode::Lte(field, value) => {
            if value.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
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
            // SQL 三值逻辑：`x IN (1, NULL)` 对非匹配值返回 UNKNOWN，
            // 因此 NULL 不参与匹配；全部为 NULL 时恒不为 TRUE。
            let values: Vec<V> = values
                .into_iter()
                .filter(|value| !value.is_null())
                .collect();
            if values.is_empty() {
                out.push_str("NULL");
                return Ok(());
            }
            let quoted = quote_condition_field(dialect, &field)?;
            out.push_str(&quoted);
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
            // SQL 三值逻辑：`x NOT IN (1, NULL)` 只可能为 FALSE 或 UNKNOWN，
            // 永远不为 TRUE；列表含 NULL 时整条谓词恒不匹配。
            if values.iter().any(CondValue::is_null) {
                out.push_str("NULL");
                return Ok(());
            }
            let quoted = quote_condition_field(dialect, &field)?;
            out.push_str(&quoted);
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
            if start.is_null() || end.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" BETWEEN ");
            dialect.push_param(params, start, parameter_offset, out);
            out.push_str(" AND ");
            dialect.push_param(params, end, parameter_offset, out);
            Ok(())
        }
        CondNode::Like(field, pattern) => {
            if pattern.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" LIKE ");
            dialect.push_param(params, pattern, parameter_offset, out);
            Ok(())
        }
        CondNode::PatternMatch(field, operator, pattern) => {
            if pattern.is_null() {
                out.push_str("NULL");
                return Ok(());
            }
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push(' ');
            out.push_str(operator);
            out.push(' ');
            dialect.push_param(params, pattern, parameter_offset, out);
            Ok(())
        }
        #[cfg(feature = "postgres")]
        CondNode::DistinctFrom(field, negated, value) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(if negated {
                " IS DISTINCT FROM "
            } else {
                " IS NOT DISTINCT FROM "
            });
            if value.is_null() {
                out.push_str("NULL");
            } else {
                dialect.push_param(params, value, parameter_offset, out);
            }
            Ok(())
        }
        CondNode::IsNull(field) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" IS NULL");
            Ok(())
        }
        CondNode::IsNotNull(field) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" IS NOT NULL");
            Ok(())
        }
        CondNode::ColumnComparison(left, op, right) => {
            out.push_str(&quote_condition_field(dialect, &left)?);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&quote_condition_field(dialect, &right)?);
            Ok(())
        }
        #[cfg(feature = "mysql")]
        CondNode::ColumnExpr(field, op, fragment, expr_params) => {
            // 表达式片段是 SqlExpr 白名单内的固定文本，动态部分（偏移秒数、
            // 格式串、填充串等）只以绑定参数进入参数列表，绝不内联进 SQL 文本。
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&fragment);
            // 片段文本自带的占位符与参数顺序一一对应，直接按序压入即可。
            params.extend(expr_params);
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
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(" IN (");
            write_subquery(dialect, *subquery, out, params, parameter_offset)?;
            out.push(')');
            Ok(())
        }
        #[cfg(feature = "postgres")]
        CondNode::JsonContains(field, value) => write_json_predicate(
            dialect,
            " @> ",
            &field,
            value,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::JsonContainedBy(field, value) => write_json_predicate(
            dialect,
            " <@ ",
            &field,
            value,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::JsonHasKey(field, key) => {
            out.push('(');
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push_str(")::jsonb");
            // `?` 运算符右侧必须是绑定参数，键名不进入 SQL 文本。
            out.push_str(" ? ");
            dialect.push_param(params, key, parameter_offset, out);
            Ok(())
        }
        #[cfg(feature = "postgres")]
        CondNode::Any(field, value) => write_array_predicate(
            dialect,
            " = ANY(",
            &field,
            value,
            out,
            params,
            parameter_offset,
            true,
        ),
        #[cfg(feature = "postgres")]
        CondNode::All(field, value) => write_array_predicate(
            dialect,
            " = ALL(",
            &field,
            value,
            out,
            params,
            parameter_offset,
            false,
        ),
        #[cfg(feature = "postgres")]
        CondNode::RangeContains(field, value) => write_range_predicate(
            dialect,
            " @> ",
            &field,
            value,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::RangeOverlaps(field, value) => write_range_predicate(
            dialect,
            " && ",
            &field,
            value,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::JsonPathExists(field, path) => write_json_path_predicate(
            dialect,
            "jsonb_path_exists",
            &field,
            path,
            None,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::JsonPathMatch(field, path, vars) => write_json_path_predicate(
            dialect,
            "jsonb_path_match",
            &field,
            path,
            Some(vars),
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::FullText(field, query) => write_full_text_predicate(
            dialect,
            "plainto_tsquery",
            &field,
            query,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::FullTextQuery(field, query) => write_full_text_predicate(
            dialect,
            "to_tsquery",
            &field,
            query,
            out,
            params,
            parameter_offset,
        ),
        #[cfg(feature = "postgres")]
        CondNode::ExprComparison(field, op, expression) => {
            out.push_str(&quote_condition_field(dialect, &field)?);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            // 占位符序号必须是「调用方偏移 + 已收集参数 + 1」，与 AND/OR 内其它
            // 条件共享同一编号空间。
            let placeholder = parameter_offset + params.len() + 1;
            let (fragment, expr_params) = dialect.render_expr(&expression, placeholder)?;
            out.push_str(&fragment);
            for param in expr_params {
                params.push(V::from_expr_param(param));
            }
            Ok(())
        }
        #[cfg(feature = "postgres")]
        CondNode::Not(inner) => {
            out.push_str("NOT (");
            write_condition(dialect, *inner, out, params, parameter_offset)?;
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

/// 渲染 JSON/JSONB 谓词：`<field> <operator> $n`（PostgreSQL）。
///
/// 值必须是原生 JSON 绑定；SQL NULL 渲染为字面量 `NULL`（谓词恒为 UNKNOWN，
/// 与既有比较运算的 NULL 处理一致），其余非 JSON 值显式报错。
#[cfg(feature = "postgres")]
fn write_json_predicate<V: CondValue>(
    dialect: Dialect,
    operator: &str,
    field: &str,
    value: V,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    if value.is_null() {
        out.push_str("NULL");
        return Ok(());
    }
    if !value.is_json() {
        return Err(DbError::InvalidArgument(format!(
            "JSON 谓词 `{field}` 的值必须是 SqlValue::Json"
        )));
    }
    // `@>`/`<@`/`?` 只定义在 jsonb 上；json 列必须先显式转换。
    out.push('(');
    out.push_str(&quote_condition_field(dialect, field)?);
    out.push_str(")::jsonb");
    out.push_str(operator);
    dialect.push_param(params, value, parameter_offset, out);
    Ok(())
}

/// 渲染数组谓词：`<field> = ANY($n)` / `<field> = ALL($n)`（PostgreSQL）。
///
/// `operator` 自带开括号与 `ANY`/`ALL` 关键字，本函数补齐右括号。
/// 右值必须是 `SqlValue::Array`：数组作为单个绑定参数渲染，不展开元素。
#[cfg(feature = "postgres")]
#[allow(clippy::too_many_arguments)]
fn write_array_predicate<V: CondValue>(
    dialect: Dialect,
    operator: &str,
    field: &str,
    value: V,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
    is_any: bool,
) -> Result<(), DbError> {
    if !value.is_array() {
        return Err(DbError::InvalidArgument(format!(
            "ANY/ALL 条件 `{field}` 的值必须是 SqlValue::Array"
        )));
    }
    // SQL 语义：x = ANY('{}') 为 FALSE，x = ALL('{}') 为 TRUE。
    if value.array_is_empty() {
        out.push_str(if is_any { "FALSE" } else { "TRUE" });
        return Ok(());
    }
    out.push_str(&quote_condition_field(dialect, field)?);
    out.push_str(operator);
    dialect.push_param(params, value, parameter_offset, out);
    out.push(')');
    Ok(())
}

/// 渲染范围谓词：`<field> @> $n` / `<field> && $n`（PostgreSQL）。
///
/// 右值必须是 [`crate::postgres::SqlValue::Range`] 或
/// [`crate::postgres::SqlValue::Multirange`]：既有类型名映射（`::int4range` /
/// `::int4multirange`）保证 `text` 绑定可被服务端解释为范围类型。
/// SQL NULL 渲染为字面量 `NULL`（谓词恒为 UNKNOWN，fail-closed）。
#[cfg(feature = "postgres")]
fn write_range_predicate<V: CondValue>(
    dialect: Dialect,
    operator: &str,
    field: &str,
    value: V,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    if value.is_null() {
        out.push_str("NULL");
        return Ok(());
    }
    if !value.is_range_value() {
        return Err(DbError::InvalidArgument(format!(
            "范围谓词 `{field}` 的值必须是 SqlValue::Range 或 SqlValue::Multirange"
        )));
    }
    out.push_str(&quote_condition_field(dialect, field)?);
    out.push_str(operator);
    dialect.push_param(params, value, parameter_offset, out);
    Ok(())
}

/// 渲染 JSONB 路径谓词（PostgreSQL）。
///
/// `vars` 为 `Some` 时追加第三个 `jsonb` 参数（[`CondNode::JsonPathMatch`]）。
/// 路径参数追加 `::jsonpath`、变量参数追加 `::jsonb`，因为 `text` 到这两个类型
/// 没有隐式转换，函数解析会失败。路径为 SQL NULL 时渲染 `NULL`。
#[cfg(feature = "postgres")]
#[allow(clippy::too_many_arguments)]
fn write_json_path_predicate<V: CondValue>(
    dialect: Dialect,
    function: &str,
    field: &str,
    path: V,
    vars: Option<V>,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    if path.is_null() {
        out.push_str("NULL");
        return Ok(());
    }
    out.push_str(function);
    out.push_str("((");
    out.push_str(&quote_condition_field(dialect, field)?);
    out.push_str(")::jsonb, ");
    dialect.push_param(params, path, parameter_offset, out);
    out.push_str("::jsonpath");
    if let Some(vars) = vars {
        out.push_str(", ");
        dialect.push_param(params, vars, parameter_offset, out);
        out.push_str("::jsonb");
    }
    out.push(')');
    Ok(())
}

/// 渲染全文检索谓词（PostgreSQL）。
///
/// 配置名固定为 `simple`（字面量，不可注入）；查询串以绑定参数进入，
/// `plainto_tsquery` / `to_tsquery` 由 `query_function` 选择。
#[cfg(feature = "postgres")]
fn write_full_text_predicate<V: CondValue>(
    dialect: Dialect,
    query_function: &str,
    field: &str,
    query: V,
    out: &mut String,
    params: &mut Vec<V>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    if query.is_null() {
        out.push_str("NULL");
        return Ok(());
    }
    out.push_str("to_tsvector('simple', ");
    out.push_str(&quote_condition_field(dialect, field)?);
    out.push_str(") @@ ");
    out.push_str(query_function);
    out.push_str("('simple', ");
    dialect.push_param(params, query, parameter_offset, out);
    out.push(')');
    Ok(())
}

/// 渲染受控子查询：`SELECT <field> FROM <table> [WHERE <cond> AND ...]`。
fn write_subquery<V: CondValue>(
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
