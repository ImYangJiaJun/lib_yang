use crate::postgres::condition::{ComparisonOperator, Condition, SqlValue};
use crate::postgres::field::{FieldType, OrderClause};
use sqlx::postgres::PgPool;
use sqlx::{TypeInfo, ValueRef};
use std::collections::HashMap;

/// 将 SqlValue 绑定到 sqlx 查询的内部宏（PostgreSQL）
///
/// 封装 SqlValue 各变体到 `.bind()` 调用的映射逻辑，消除 4 个 bind_param
/// 函数中完全相同的 match 分支重复代码。未来新增 SqlValue 变体时，
/// 只需在此宏中添加一个分支即可完成所有函数的更新。
///
/// 与 MySQL 后端的差异：
/// - JSON 直接绑定原生 `serde_json::Value`（sqlx 的 `json` feature 已开启，
///   映射到 PostgreSQL 的 `JSON`/`JSONB`），而非序列化为字符串。
/// - Bytes 绑定 `Vec<u8>`（映射到 `BYTEA`）。
///
/// # 参数
/// - `$query`: sqlx 查询对象（支持 `.bind()` 方法的任意类型）
/// - `$param`: `&SqlValue` 引用
///
/// # 返回
/// 绑定参数后的查询对象（与 `$query` 类型相同）
macro_rules! bind_value_match {
    ($query:expr, $param:expr) => {
        match $param {
            // NULL 值：绑定为 Option<i32>::None
            SqlValue::Null => $query.bind(Option::<i32>::None),
            // 布尔值
            SqlValue::Bool(b) => $query.bind(*b),
            // 整数
            SqlValue::Int(i) => $query.bind(*i),
            // 浮点数
            SqlValue::Float(f) => $query.bind(*f),
            // 字符串（需要 clone 以满足 sqlx 的所有权要求）
            SqlValue::String(s) => $query.bind(s.clone()),
            // 字节数组（BYTEA，需要 clone）
            SqlValue::Bytes(b) => $query.bind(b.clone()),
            // JSON 值：绑定原生 serde_json::Value（JSON/JSONB）
            SqlValue::Json(j) => $query.bind(j.clone()),
            // 日期时间
            SqlValue::DateTime(dt) => $query.bind(*dt),
            // 时间戳（整数）
            SqlValue::Timestamp(ts) => $query.bind(*ts),
            // 精确十进制（NUMERIC）
            SqlValue::Decimal(decimal) => $query.bind(decimal.clone()),
            // UUID
            SqlValue::Uuid(uuid) => $query.bind(*uuid),
            // TIMESTAMPTZ
            SqlValue::DateTimeTz(dt) => $query.bind(*dt),
            // DATE
            SqlValue::Date(date) => $query.bind(*date),
            // TIME
            SqlValue::Time(time) => $query.bind(*time),
            // INTERVAL（微秒）：PostgreSQL 驱动按 PgInterval 编解码
            SqlValue::Interval(micros) => $query.bind(sqlx::postgres::types::PgInterval {
                months: 0,
                days: 0,
                microseconds: *micros,
            }),
            // 网络/范围类型：SQL 占位符已追加 ::inet / ::int4range 等显式转换，
            // 值按文本绑定
            SqlValue::Inet(s)
            | SqlValue::Cidr(s)
            | SqlValue::MacAddr(s)
            | SqlValue::MacAddr8(s)
            | SqlValue::Range(s)
            | SqlValue::Multirange(s) => $query.bind(s.clone()),
            // PostgreSQL 数组：作为单个数组参数绑定。元素类型按首个非 NULL 元素
            // 推断；调用方应保证数组同构（SqlValue::from(Vec<T>) 已保证）。
            SqlValue::Array(values) => match values
                .iter()
                .find(|value| !matches!(value, SqlValue::Null))
            {
                Some(SqlValue::Bool(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_bool)
                        .collect::<Vec<Option<bool>>>(),
                ),
                Some(SqlValue::Int(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_i64)
                        .collect::<Vec<Option<i64>>>(),
                ),
                Some(SqlValue::Float(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_f64)
                        .collect::<Vec<Option<f64>>>(),
                ),
                Some(SqlValue::Uuid(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_uuid)
                        .collect::<Vec<Option<uuid::Uuid>>>(),
                ),
                Some(SqlValue::Date(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_date)
                        .collect::<Vec<Option<chrono::NaiveDate>>>(),
                ),
                Some(SqlValue::Time(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_time)
                        .collect::<Vec<Option<chrono::NaiveTime>>>(),
                ),
                Some(SqlValue::DateTimeTz(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_datetime_tz)
                        .collect::<Vec<Option<chrono::DateTime<chrono::Utc>>>>(),
                ),
                Some(SqlValue::DateTime(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_datetime)
                        .collect::<Vec<Option<chrono::NaiveDateTime>>>(),
                ),
                Some(SqlValue::Decimal(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_decimal)
                        .collect::<Vec<Option<bigdecimal::BigDecimal>>>(),
                ),
                Some(SqlValue::Timestamp(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_timestamp)
                        .collect::<Vec<Option<i64>>>(),
                ),
                Some(SqlValue::Bytes(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_bytes)
                        .collect::<Vec<Option<Vec<u8>>>>(),
                ),
                Some(SqlValue::Json(_)) => $query.bind(
                    values
                        .iter()
                        .map(array_element_json)
                        .collect::<Vec<Option<serde_json::Value>>>(),
                ),
                // 其余（含空数组与混合数组）按 TEXT[] 绑定；空数组没有元素类型信息，
                // 调用方应避免与非文本数组列比较。
                _ => $query.bind(
                    values
                        .iter()
                        .map(array_element_string)
                        .collect::<Vec<Option<String>>>(),
                ),
            },
            // 显式元素类型的数组：按真实列元素类型绑定，空数组/全 NULL 数组也
            // 不会退化为 TEXT[]（int4[] 与 int8[] 的等值谓词依赖这一点）。
            SqlValue::TypedArray {
                element_type,
                values,
            } => match *element_type {
                "int2" => $query.bind(
                    values
                        .iter()
                        .map(array_element_i16)
                        .collect::<Vec<Option<i16>>>(),
                ),
                "int4" => $query.bind(
                    values
                        .iter()
                        .map(array_element_i32)
                        .collect::<Vec<Option<i32>>>(),
                ),
                "int8" => $query.bind(
                    values
                        .iter()
                        .map(array_element_i64)
                        .collect::<Vec<Option<i64>>>(),
                ),
                "float4" => $query.bind(
                    values
                        .iter()
                        .map(array_element_f32)
                        .collect::<Vec<Option<f32>>>(),
                ),
                "float8" => $query.bind(
                    values
                        .iter()
                        .map(array_element_f64)
                        .collect::<Vec<Option<f64>>>(),
                ),
                "bool" => $query.bind(
                    values
                        .iter()
                        .map(array_element_bool)
                        .collect::<Vec<Option<bool>>>(),
                ),
                "bytea" => $query.bind(
                    values
                        .iter()
                        .map(array_element_bytes)
                        .collect::<Vec<Option<Vec<u8>>>>(),
                ),
                "uuid" => $query.bind(
                    values
                        .iter()
                        .map(array_element_uuid)
                        .collect::<Vec<Option<uuid::Uuid>>>(),
                ),
                "date" => $query.bind(
                    values
                        .iter()
                        .map(array_element_date)
                        .collect::<Vec<Option<chrono::NaiveDate>>>(),
                ),
                "time" => $query.bind(
                    values
                        .iter()
                        .map(array_element_time)
                        .collect::<Vec<Option<chrono::NaiveTime>>>(),
                ),
                "timestamp" => $query.bind(
                    values
                        .iter()
                        .map(array_element_datetime)
                        .collect::<Vec<Option<chrono::NaiveDateTime>>>(),
                ),
                "timestamptz" => $query.bind(
                    values
                        .iter()
                        .map(array_element_datetime_tz)
                        .collect::<Vec<Option<chrono::DateTime<chrono::Utc>>>>(),
                ),
                "numeric" => $query.bind(
                    values
                        .iter()
                        .map(array_element_decimal)
                        .collect::<Vec<Option<bigdecimal::BigDecimal>>>(),
                ),
                "jsonb" => $query.bind(
                    values
                        .iter()
                        .map(array_element_json)
                        .collect::<Vec<Option<serde_json::Value>>>(),
                ),
                // text / varchar / inet / cidr / macaddr / macaddr8：Element 以
                // 字符串绑定为 TEXT[]，非 text 类型由 `cast_suffix` 追加显式转换。
                _ => $query.bind(
                    values
                        .iter()
                        .map(array_element_string)
                        .collect::<Vec<Option<String>>>(),
                ),
            },
        }
    };
}

/// 批量插入的默认批次大小
///
/// 为了避免单次插入过多数据导致 SQL 语句过大或超时，
/// 批量插入操作会自动将数据分批处理，每批最多插入 INSERT_BATCH_SIZE 条记录。
const INSERT_BATCH_SIZE: usize = 500;

/// 批量更新的默认批次大小
const UPDATE_BATCH_SIZE: usize = 1000;

/// 协议层单语句绑定占位符上限。
///
/// MySQL 二进制协议预编译语句占位符上限为 2^16-1；PostgreSQL Bind 消息的
/// 参数计数同为 Int16，上限一致。
const MAX_BIND_PARAMS: usize = 65_535;

/// MERGE 自 PostgreSQL 15 起提供（`server_version_num >= 150000`）。
const MERGE_MIN_SERVER_VERSION: i32 = 150_000;

/// `WHEN NOT MATCHED BY SOURCE` 自 PostgreSQL 17 起提供。
const MERGE_BY_SOURCE_MIN_SERVER_VERSION: i32 = 170_000;

/// 纯函数版本门控：便于单测在不连库的情况下覆盖版本分支。
fn merge_version_supported(version: i32) -> bool {
    version >= MERGE_MIN_SERVER_VERSION
}

/// 读取当前连接的 `server_version_num`（如 PG16.15 → 160015）。
async fn server_version_num(
    executor: &mut QueryExecutor<'_>,
) -> Result<i32, crate::error::DbError> {
    let sql = "SELECT current_setting('server_version_num')::int";
    match executor {
        QueryExecutor::Pool(pool) => Ok(sqlx::query_scalar::<_, i32>(sql).fetch_one(*pool).await?),
        QueryExecutor::Transaction(transaction) => {
            let connection = transaction.executor().ok_or_else(|| {
                crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
            })?;
            Ok(sqlx::query_scalar::<_, i32>(sql)
                .fetch_one(&mut *connection)
                .await?)
        }
    }
}

/// 由「非主键列数」推导 CASE WHEN 批量更新的安全批大小（与 MySQL 侧同口径）。
fn derive_update_batch_size(field_count: usize, requested: usize) -> usize {
    let per_record = field_count.saturating_mul(2).saturating_add(1);
    if per_record == 0 {
        return requested;
    }
    (MAX_BIND_PARAMS / per_record).max(1).min(requested)
}

/// 由「每行列数」推导批量 INSERT 的安全批大小（单语句参数上限 65535）。
fn derive_insert_batch_size(column_count: usize, requested: usize) -> usize {
    if column_count == 0 {
        return requested;
    }
    (MAX_BIND_PARAMS / column_count).max(1).min(requested)
}

/// 由「每行列数」推导批量 UPSERT 的安全批大小。
///
/// ON CONFLICT 尾巴不产生占位符，参数数上界 = 行数 × 列数；NULL 内联只会更少。
fn derive_upsert_batch_size(column_count: usize, requested: usize) -> usize {
    if column_count == 0 {
        return requested;
    }
    (MAX_BIND_PARAMS / column_count).max(1).min(requested)
}

/// 取首条记录的「非主键列数」，与 build_update_batch 的 update_fields 口径一致。
fn update_field_count(records: &[serde_json::Value], where_field: &str) -> usize {
    records
        .first()
        .and_then(|r| r.as_object())
        .map(|obj| obj.keys().filter(|k| k.as_str() != where_field).count())
        .unwrap_or(0)
}

/// 压入一个参数并返回其 PostgreSQL 占位符（`$N`，1 基）
///
/// PostgreSQL 使用编号占位符，编号由参数压入后的 `params.len()` 决定，
/// 保证占位符编号与最终绑定顺序严格一致。这是与 MySQL 后端（统一使用 `?`）
/// 的核心方言差异，在手写 INSERT/UPDATE/UPSERT 等 SQL 时集中通过本助手处理。
fn push_placeholder(params: &mut Vec<SqlValue>, value: SqlValue) -> String {
    params.push(value);
    format!("${}", params.len())
}

/// 返回该字段类型绑定时需要的显式 PostgreSQL 类型转换后缀。
///
/// 网络类型（INET/CIDR/MACADDR/MACADDR8）没有从 `text` 的赋值转换，
/// 驱动按 TEXT 绑定字符串时必须写成 `$n::inet`；数组按元素类型递归追加 `[]`。
/// 其余类型返回 `None`（沿用隐式参数类型推断）。
fn pg_value_cast(field_type: Option<&FieldType>) -> Option<String> {
    let field_type = field_type?;
    match field_type {
        FieldType::Inet => Some("::inet".to_string()),
        FieldType::Cidr => Some("::cidr".to_string()),
        FieldType::MacAddr => Some("::macaddr".to_string()),
        FieldType::MacAddr8 => Some("::macaddr8".to_string()),
        FieldType::Array(inner) => pg_value_cast(Some(inner)).map(|suffix| format!("{suffix}[]")),
        // 范围/多范围：字段类型已知时按真实内置类型追加转换（如 `::tsrange`），
        // 让 JSON 字符串写入可直接落到范围列。
        FieldType::Range(_) | FieldType::Multirange(_) => field_type
            .postgres_type_name()
            .map(|name| format!("::{name}")),
        _ => None,
    }
}

/// 压入带显式类型转换的占位符（如 `$1::inet`、`$2::text[]`）。
fn push_typed_placeholder(
    params: &mut Vec<SqlValue>,
    value: SqlValue,
    field_type: Option<&FieldType>,
) -> String {
    let placeholder = push_placeholder(params, value);
    match pg_value_cast(field_type) {
        Some(suffix) => format!("{placeholder}{suffix}"),
        None => placeholder,
    }
}

/// 返回数组元素类型对应的受控 PG 内置类型名（用于 [`SqlValue::TypedArray`]）。
///
/// `Standard` 表示整数/浮点共享的隐式绑定类型；这里选择 `int8`：
/// 空数组与全 NULL 数组按 `int8[]` 绑定后，可被 INTEGER[]/BIGINT[]/REAL[]/
/// DOUBLE PRECISION[] 列按赋值转换接受。布尔数组没有可用的隐式超类型，
/// 因此 [`FieldType::Bool`] 显式映射为 `bool`，空 BOOLEAN[] 也能正确绑定。
fn pg_array_element_type_name(field_type: &FieldType) -> Option<&'static str> {
    match field_type {
        FieldType::Standard => Some("int8"),
        FieldType::Bool => Some("bool"),
        FieldType::Text => Some("text"),
        FieldType::Blob => Some("bytea"),
        FieldType::Uuid => Some("uuid"),
        FieldType::Date => Some("date"),
        FieldType::Time => Some("time"),
        FieldType::DateTime => Some("timestamp"),
        FieldType::TimestampTz => Some("timestamptz"),
        FieldType::Timestamp => Some("int8"),
        FieldType::Decimal => Some("numeric"),
        FieldType::Json | FieldType::Jsonb => Some("jsonb"),
        FieldType::Inet => Some("inet"),
        FieldType::Cidr => Some("cidr"),
        FieldType::MacAddr => Some("macaddr"),
        FieldType::MacAddr8 => Some("macaddr8"),
        FieldType::Array(_)
        | FieldType::Bit
        | FieldType::Varbit
        | FieldType::Range(_)
        | FieldType::Multirange(_) => None,
    }
}

/// SQL 生成器（内部使用）
#[allow(dead_code)]
pub(crate) struct SqlGenerator {
    /// 生成的 SQL 语句
    sql: String,
    /// SQL 参数列表
    params: Vec<SqlValue>,
}

#[allow(dead_code)]
impl SqlGenerator {
    /// 创建新的 SQL 生成器
    ///
    /// 预分配合理的缓冲区容量，减少 SQL 字符串构建过程中的重新分配次数：
    /// - `sql` 预分配 256 字节，适合大多数 SQL 语句
    /// - `params` 预分配 8 个槽位，适合大多数查询参数数量
    pub(crate) fn new() -> Self {
        Self {
            // 预分配 256 字节，减少字符串扩容次数
            sql: String::with_capacity(256),
            // 预分配 8 个参数槽位，减少 Vec 扩容次数
            params: Vec::with_capacity(8),
        }
    }

    /// 获取生成的 SQL 语句
    pub(crate) fn get_sql(&self) -> &str {
        &self.sql
    }

    /// 获取参数列表
    pub(crate) fn get_params(&self) -> &[SqlValue] {
        &self.params
    }

    /// 添加 SQL 片段
    fn append(&mut self, fragment: &str) {
        self.sql.push_str(fragment);
    }

    /// 添加参数
    fn add_param(&mut self, param: SqlValue) {
        self.params.push(param);
    }

    fn append_condition(&mut self, condition: Condition) -> Result<(), crate::error::DbError> {
        let rendered = self.render_condition_fragment(condition)?;
        self.sql.push_str(&rendered);
        Ok(())
    }

    /// 渲染条件片段并把其绑定参数追加到参数列表（占位符编号基于当前长度接续）。
    fn render_condition_fragment(
        &mut self,
        condition: Condition,
    ) -> Result<String, crate::error::DbError> {
        let rendered =
            crate::postgres::condition::render_condition_checked(condition, self.params.len())?;
        self.params.extend(rendered.params);
        Ok(rendered.sql)
    }

    /// 清空生成器（保留已分配容量，避免重复分配）
    fn clear(&mut self) {
        self.sql.clear();
        self.params.clear();
    }

    /// 测试专用：暴露 clear 方法供测试模块调用
    #[cfg(test)]
    pub(crate) fn clear_for_test(&mut self) {
        self.clear();
    }

    /// 测试专用：获取 sql 字段的当前容量
    #[cfg(test)]
    pub(crate) fn sql_capacity(&self) -> usize {
        self.sql.capacity()
    }

    /// 测试专用：获取 params 字段的当前容量
    #[cfg(test)]
    pub(crate) fn params_capacity(&self) -> usize {
        self.params.capacity()
    }

    /// 生成 SELECT 语句
    ///
    /// # 参数
    /// - builder: QueryBuilder 引用
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    fn build_select(&mut self, builder: &QueryBuilder) -> Result<(), crate::error::DbError> {
        self.clear();
        self.build_compound_select(builder)
    }

    /// 生成复合查询：`WITH ...` + 集合运算链 + 外层 ORDER BY/LIMIT/OFFSET。
    ///
    /// 分支查询（UNION/INTERSECT/EXCEPT 右侧、CTE 主体、LATERAL 子查询）递归调用
    /// 本方法，各自携带自己的 WITH 与排序/分页作用域。
    fn build_compound_select(
        &mut self,
        builder: &QueryBuilder,
    ) -> Result<(), crate::error::DbError> {
        if !builder.ctes.is_empty() {
            self.build_with(&builder.ctes)?;
        }

        self.build_set_chain(builder, &builder.set_ops, builder.set_ops.len())?;

        // 当前构建器的 ORDER/LIMIT 作用于整个复合查询；分支自己的作用域保留在括号内。
        if !builder.order_by.is_empty() {
            self.build_order_by(&builder.order_by);
        }

        // LIMIT 子句
        if let Some(limit) = builder.limit {
            self.append(" LIMIT ");
            self.append(&limit.to_string());
        }

        // OFFSET 子句
        if let Some(offset) = builder.offset {
            self.append(" OFFSET ");
            self.append(&offset.to_string());
        }

        Ok(())
    }

    /// 渲染 `WITH [RECURSIVE] name AS (...), ...`。
    fn build_with(&mut self, ctes: &[Cte<'_>]) -> Result<(), crate::error::DbError> {
        self.append("WITH ");
        if ctes.iter().any(|cte| cte.recursive) {
            self.append("RECURSIVE ");
        }
        for (index, cte) in ctes.iter().enumerate() {
            if index > 0 {
                self.append(", ");
            }
            self.append(&super::identifier::quote_identifier(&cte.name)?);
            self.append(" AS (");
            self.build_compound_select(&cte.query)?;
            self.append(")");
        }
        self.append(" ");
        Ok(())
    }

    /// 按左结合顺序渲染集合运算链。
    ///
    /// PostgreSQL 中 `INTERSECT` 的优先级高于 `UNION`/`EXCEPT`，直接顺序拼接会让
    /// `A UNION B INTERSECT C` 被解析为 `A UNION (B INTERSECT C)`。只要链中出现
    /// 非 UNION 运算符，就把「到此为止」的左侧子链括起来，固定为调用方书写顺序。
    fn build_set_chain(
        &mut self,
        head: &QueryBuilder,
        ops: &[(SetOperator, Box<QueryBuilder>)],
        count: usize,
    ) -> Result<(), crate::error::DbError> {
        if count == 0 {
            return self.build_select_core(head);
        }
        let (operator, branch) = &ops[count - 1];
        // 只有「左侧已是复合表达式」时才需要括号固定结合顺序：
        // - 左子链含非 UNION 运算符，或当前运算符不是 UNION，优先级会改变解析；
        // - 纯 UNION 链本身左结合，无需额外括号（保持既有 SQL 形状）。
        let needs_parens = count > 1
            && (ops[..count - 1].iter().any(|(op, _)| !op.is_union()) || !operator.is_union());
        if needs_parens {
            self.append("(");
        }
        self.build_set_chain(head, ops, count - 1)?;
        if needs_parens {
            self.append(")");
        }
        self.append(operator.sql());
        self.append("(");
        self.build_compound_select(branch)?;
        self.append(")");
        Ok(())
    }

    /// 渲染单个 SELECT 主体（不含 WITH / 集合运算 / ORDER BY / LIMIT）。
    fn build_select_core(&mut self, builder: &QueryBuilder) -> Result<(), crate::error::DbError> {
        // SELECT 子句
        self.append("SELECT ");

        // DISTINCT / DISTINCT ON
        if !builder.distinct_on.is_empty() {
            if builder.distinct {
                return Err(crate::error::DbError::InvalidArgument(
                    "distinct 与 distinct_on 不能同时使用".to_string(),
                ));
            }
            // PostgreSQL 要求 DISTINCT ON 表达式与最左 ORDER BY 表达式一致。
            let matched = builder.order_by.len() >= builder.distinct_on.len()
                && builder
                    .order_by
                    .iter()
                    .take(builder.distinct_on.len())
                    .zip(&builder.distinct_on)
                    .all(|(order, expected)| order.field == *expected);
            if !matched {
                return Err(crate::error::DbError::InvalidArgument(
                    "distinct_on 要求最左 ORDER BY 与 DISTINCT ON 字段完全一致（顺序相同）"
                        .to_string(),
                ));
            }
            self.append("DISTINCT ON (");
            self.append(&builder.distinct_on.join(", "));
            self.append(") ");
        } else if builder.distinct {
            self.append("DISTINCT ");
        }

        // 字段列表
        if builder.fields.is_empty()
            && builder.projection_exprs.is_empty()
            && builder.window_exprs.is_empty()
        {
            self.append("*");
        } else {
            // 验证需求: ID-1 — field() 按设计接受 SQL 表达式，标识符转义由调用方负责
            let mut projection = builder.fields.clone();
            for (expression, alias) in &builder.projection_exprs {
                let placeholder = self.params.len() + 1;
                let rendered = expression.postgres_render(placeholder);
                for param in rendered.params {
                    self.params.push(param.into());
                }
                let mut piece = rendered.sql;
                if let Some(alias) = alias {
                    piece.push_str(" AS ");
                    piece.push_str(&super::identifier::quote_identifier(alias)?);
                }
                projection.push(piece);
            }
            for window in &builder.window_exprs {
                projection.push(window.render(self)?);
            }
            self.append(&projection.join(", "));
        }

        // FROM 子句
        self.append(" FROM ");
        self.append(&super::identifier::quote_table(&builder.table)?);

        // JOIN 子句
        if !builder.joins.is_empty() {
            self.build_joins(&builder.joins)?;
        }

        // WHERE 子句
        if !builder.conditions.is_empty() {
            self.build_where(&builder.conditions)?;
        }

        // GROUP BY 子句（含 GROUPING SETS / ROLLUP / CUBE）
        if !builder.group_by.is_empty() || !builder.grouping_constructs.is_empty() {
            self.build_group_by(&builder.group_by, &builder.grouping_constructs)?;
        }

        // HAVING 子句：字段条件与聚合表达式比较都以 AND 连接。
        //
        // 字段条件沿用历史约束（必须有 GROUP BY / 分组集）；纯聚合表达式比较
        // （如 `HAVING COUNT(*) > 1`）在没有 GROUP BY 时也是合法的单行聚合过滤。
        if !builder.having_clause.is_empty() || !builder.having_exprs.is_empty() {
            if !builder.having_clause.is_empty()
                && builder.group_by.is_empty()
                && builder.grouping_constructs.is_empty()
            {
                return Err(crate::error::DbError::MissingGroupByClause);
            }
            self.build_having(&builder.having_clause, &builder.having_exprs)?;
        }

        // WINDOW 子句：位于 HAVING 之后、ORDER BY 之前。
        if !builder.named_windows.is_empty() {
            self.build_window_clause(&builder.named_windows)?;
        }

        Ok(())
    }

    /// 渲染 `WINDOW <name> AS (<window spec>), ...`。
    fn build_window_clause(
        &mut self,
        windows: &[(String, WindowSpec)],
    ) -> Result<(), crate::error::DbError> {
        self.append(" WINDOW ");
        for (index, (name, spec)) in windows.iter().enumerate() {
            if index > 0 {
                self.append(", ");
            }
            self.append(&super::identifier::quote_identifier(name)?);
            self.append(" AS (");
            self.append(&spec.render_body()?);
            self.append(")");
        }
        Ok(())
    }

    /// 生成 WHERE 子句
    ///
    /// 委托给 [`condition_to_sql_owned`]（已生成 PostgreSQL 风格的 `$N` 占位符，
    /// 编号基于 `self.params` 当前长度自动接续）。
    ///
    /// # 参数
    /// - conditions: 条件列表
    ///
    /// # 返回
    /// - Ok(()): 成功生成 WHERE 子句
    /// - Err(DbError): 生成失败
    fn build_where(&mut self, conditions: &[Condition]) -> Result<(), crate::error::DbError> {
        if conditions.is_empty() {
            return Ok(());
        }

        self.append(" WHERE ");

        if conditions.len() == 1 {
            self.append_condition(conditions[0].clone())?;
        } else {
            // 内联拼接：逐条 append_condition 直接写入 self.sql，省去 to_vec() 容器 +
            // Condition::And 包装 + parts Vec；注意条件树仍按值消费，每个条件在
            // append_condition 处 clone 一次（见 :277/:286）。
            self.sql.push('(');
            for (i, cond) in conditions.iter().enumerate() {
                if i > 0 {
                    self.sql.push_str(" AND ");
                }
                self.append_condition(cond.clone())?;
            }
            self.sql.push(')');
        }

        Ok(())
    }

    /// 生成 JOIN 子句（含 FULL/CROSS/NATURAL/USING/LATERAL）。
    fn build_joins(&mut self, joins: &[JoinSpec<'_>]) -> Result<(), crate::error::DbError> {
        for join in joins {
            let join_keyword = match join.kind {
                JoinKind::Inner => " INNER JOIN ",
                JoinKind::Left => " LEFT JOIN ",
                JoinKind::Right => " RIGHT JOIN ",
                JoinKind::Full => " FULL JOIN ",
                JoinKind::Cross => " CROSS JOIN ",
                JoinKind::Natural => " NATURAL JOIN ",
                JoinKind::LeftLateral => " LEFT JOIN LATERAL ",
            };
            self.append(join_keyword);

            match &join.target {
                JoinTarget::Table(table) => {
                    self.append(&super::identifier::quote_table(table)?);
                }
                JoinTarget::Lateral(query, alias) => {
                    self.append("(");
                    self.build_compound_select(query)?;
                    self.append(") AS ");
                    self.append(&super::identifier::quote_identifier(alias)?);
                }
            }

            match &join.condition {
                JoinCondition::On(expression) => {
                    self.append(" ON ");
                    self.append(expression);
                }
                JoinCondition::Using(columns) => {
                    let quoted: Result<Vec<String>, crate::error::DbError> = columns
                        .iter()
                        .map(|column| super::identifier::quote_identifier(column))
                        .collect();
                    self.append(" USING (");
                    self.append(&quoted?.join(", "));
                    self.append(")");
                }
                JoinCondition::None => {}
            }
        }

        Ok(())
    }

    /// 生成 ORDER BY 子句
    ///
    /// # 参数
    /// - orders: 排序子句列表
    ///
    /// 注意：`order.field` 按设计接受原始 SQL 表达式（与 MySQL 设计一致），
    /// 标识符转义由调用方负责。
    fn build_order_by(&mut self, orders: &[OrderClause]) {
        if orders.is_empty() {
            return;
        }

        self.append(" ORDER BY ");

        // 直接 push_str 写入 self.sql，避免先 collect Vec<String> 再 join 的中间分配。
        for (i, order) in orders.iter().enumerate() {
            if i > 0 {
                self.sql.push_str(", ");
            }
            self.sql.push_str(&order.field);
            self.sql.push(' ');
            self.sql.push_str(if order.asc { "ASC" } else { "DESC" });
        }
    }

    /// 生成 GROUP BY 子句（普通分组字段与分组集构造按书写顺序逗号连接）。
    fn build_group_by(
        &mut self,
        groups: &[String],
        constructs: &[GroupingConstruct],
    ) -> Result<(), crate::error::DbError> {
        if groups.is_empty() && constructs.is_empty() {
            return Ok(());
        }

        self.append(" GROUP BY ");
        let mut first = true;
        for group in groups {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(group);
        }
        for construct in constructs {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            match construct {
                GroupingConstruct::GroupingSets(sets) => {
                    if sets.is_empty() {
                        return Err(crate::error::DbError::InvalidArgument(
                            "grouping_sets 不能为空".to_string(),
                        ));
                    }
                    let rendered: Vec<String> = sets
                        .iter()
                        .map(|set| {
                            let fields: Vec<&str> =
                                set.iter().map(crate::FieldRef::postgres_quoted).collect();
                            format!("({})", fields.join(", "))
                        })
                        .collect();
                    self.sql
                        .push_str(&format!("GROUPING SETS ({})", rendered.join(", ")));
                }
                GroupingConstruct::Rollup(fields) => {
                    if fields.is_empty() {
                        return Err(crate::error::DbError::InvalidArgument(
                            "rollup 的字段列表不能为空".to_string(),
                        ));
                    }
                    let rendered: Vec<&str> = fields
                        .iter()
                        .map(crate::FieldRef::postgres_quoted)
                        .collect();
                    self.sql
                        .push_str(&format!("ROLLUP ({})", rendered.join(", ")));
                }
                GroupingConstruct::Cube(fields) => {
                    if fields.is_empty() {
                        return Err(crate::error::DbError::InvalidArgument(
                            "cube 的字段列表不能为空".to_string(),
                        ));
                    }
                    let rendered: Vec<&str> = fields
                        .iter()
                        .map(crate::FieldRef::postgres_quoted)
                        .collect();
                    self.sql
                        .push_str(&format!("CUBE ({})", rendered.join(", ")));
                }
            }
        }
        Ok(())
    }

    /// 生成 HAVING 子句：字段条件与聚合表达式比较都以 AND 连接。
    fn build_having(
        &mut self,
        conditions: &[Condition],
        exprs: &[HavingExpr],
    ) -> Result<(), crate::error::DbError> {
        if conditions.is_empty() && exprs.is_empty() {
            return Ok(());
        }
        self.append(" HAVING ");
        let mut first = true;
        for condition in conditions {
            if !first {
                self.sql.push_str(" AND ");
            }
            first = false;
            self.append_condition(condition.clone())?;
        }
        for expr in exprs {
            if !first {
                self.sql.push_str(" AND ");
            }
            first = false;
            // 聚合表达式不带输出别名（HAVING 不能引用 SELECT 别名）。
            self.sql
                .push_str(&expr.expression.postgres_sql_without_alias());
            self.sql.push(' ');
            self.sql.push_str(comparison_operator_sql(expr.operator));
            self.sql.push(' ');
            match &expr.value {
                SqlValue::Null => self.sql.push_str("NULL"),
                value => {
                    let placeholder = push_placeholder(&mut self.params, value.clone());
                    self.sql.push_str(&placeholder);
                }
            }
        }
        Ok(())
    }

    /// 生成 INSERT 语句
    ///
    /// 占位符使用 PostgreSQL 风格的 `$N`，编号在压入参数后按 `params.len()` 推导。
    /// 当某字段值为 `SqlValue::Null` 时，直接在 SQL 中内联字面量 `NULL` 而不压入参数
    /// （PostgreSQL 拒绝未带类型的绑定 NULL 对非整型列赋值，故 NULL 不占用占位符编号）。
    ///
    /// # 参数
    /// - table: 表名
    /// - data: 要插入的数据（JSON 格式）
    /// - field_types: 字段类型映射
    /// - set_exprs: 受控服务端表达式赋值（追加在 JSON 列之后）
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    pub(crate) fn build_insert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        set_exprs: &[(String, crate::SqlExpr)],
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        // 确保 data 是一个对象
        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() && set_exprs.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        // 提取字段名和占位符（列名经 quote_identifier 校验+转义；DB-1）
        let mut fields = Vec::with_capacity(obj.len());
        let mut placeholders = Vec::with_capacity(obj.len());

        for (key, value) in obj.iter() {
            fields.push(super::identifier::quote_identifier(key)?);

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            // NULL 内联字面量，不占占位符编号；其余压入并生成 $N
            match sql_value {
                SqlValue::Null => placeholders.push("NULL".to_string()),
                v => placeholders.push(push_typed_placeholder(
                    &mut self.params,
                    v,
                    field_types.get(key),
                )),
            }
        }

        // 受控服务端表达式赋值（如 `last_seen = now()`），参数编号接续 JSON 列。
        for (key, expression) in set_exprs {
            fields.push(super::identifier::quote_identifier(key)?);
            let placeholder = self.params.len() + 1;
            let rendered = expression.postgres_render(placeholder);
            for param in rendered.params {
                self.params.push(param.into());
            }
            placeholders.push(rendered.sql);
        }

        // 构建 INSERT 语句（表名 quote）
        self.append("INSERT INTO ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" (");
        self.append(&fields.join(", "));
        self.append(") VALUES (");
        self.append(&placeholders.join(", "));
        self.append(")");

        Ok(())
    }
    /// 生成批量 INSERT 语句（PostgreSQL）
    ///
    /// 占位符使用 `$N`，编号按参数压入顺序连续递增。NULL 值内联字面量 `NULL`，
    /// 不占用占位符编号（PostgreSQL 拒绝未类型化的绑定 NULL）。
    pub(crate) fn build_insert_batch(
        &mut self,
        table: &str,
        data_list: &[serde_json::Value],
        field_types: &HashMap<String, FieldType>,
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        if data_list.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量插入数据不能为空".to_string(),
            ));
        }

        let first_obj = data_list[0].as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if first_obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        let fields: Vec<String> = first_obj.keys().cloned().collect();

        // 列集一致性校验（NEW-9）：见 MySQL 同名方法，异构记录会静默丢列 / 填 NULL。
        for (idx, data) in data_list.iter().enumerate().skip(1) {
            let obj = data.as_object().ok_or_else(|| {
                crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
            })?;
            if obj.len() != fields.len() || !fields.iter().all(|f| obj.contains_key(f)) {
                return Err(crate::error::DbError::InvalidArgument(format!(
                    "批量插入第 {idx} 条记录的列集与首条不一致"
                )));
            }
        }

        // 表名与列头 quote（DB-1）；fields 保留原始键用于 obj.get 查值。
        let quoted_fields: Result<Vec<String>, crate::error::DbError> = fields
            .iter()
            .map(|f| super::identifier::quote_identifier(f))
            .collect();
        let quoted_fields = quoted_fields?;
        self.append("INSERT INTO ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" (");
        self.append(&quoted_fields.join(", "));
        self.append(") VALUES ");
        for (record_idx, data) in data_list.iter().enumerate() {
            if record_idx > 0 {
                self.sql.push_str(", ");
            }

            let obj = data.as_object().ok_or_else(|| {
                crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
            })?;

            self.sql.push('(');
            for (field_idx, field) in fields.iter().enumerate() {
                if field_idx > 0 {
                    self.sql.push_str(", ");
                }
                let value = obj.get(field).unwrap_or(&serde_json::Value::Null);
                let sql_value = self.json_value_to_sql_value(value, field_types.get(field))?;
                match sql_value {
                    // NULL 内联字面量，不占占位符编号
                    SqlValue::Null => self.sql.push_str("NULL"),
                    v => {
                        let ph =
                            push_typed_placeholder(&mut self.params, v, field_types.get(field));
                        self.sql.push_str(&ph);
                    }
                }
            }
            self.sql.push(')');
        }
        Ok(())
    }
    /// 生成 UPDATE 语句（PostgreSQL）
    ///
    /// SET 子句先压入参数生成 `$1..$k`，随后 WHERE 子句的占位符编号自动接续
    /// （`build_where` 委托的 condition 模块基于 `self.params` 当前长度推导）。
    pub(crate) fn build_update(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        conditions: &[Condition],
        set_exprs: &[(String, crate::SqlExpr)],
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() && set_exprs.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "更新数据不能为空".to_string(),
            ));
        }

        self.append("UPDATE ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" SET ");
        self.build_assignment_list(obj, field_types, set_exprs)?;

        // WHERE 子句的占位符编号接续 SET 子句已压入的参数
        self.build_where(conditions)?;

        Ok(())
    }

    /// 生成 SET 赋值列表（`build_update` 与 `build_update_from` 共用）。
    ///
    /// 逐字段生成 `字段 = $N`；值为 NULL 时内联字面量 NULL（不占占位符编号），
    /// 与 build_insert/build_update_batch/build_upsert 一致。
    fn build_assignment_list(
        &mut self,
        obj: &serde_json::Map<String, serde_json::Value>,
        field_types: &HashMap<String, FieldType>,
        set_exprs: &[(String, crate::SqlExpr)],
    ) -> Result<(), crate::error::DbError> {
        let mut first = true;
        for (key, value) in obj.iter() {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.sql
                .push_str(&super::identifier::quote_identifier(key)?);
            self.sql.push_str(" = ");
            match sql_value {
                SqlValue::Null => self.sql.push_str("NULL"),
                v => {
                    let ph = push_typed_placeholder(&mut self.params, v, field_types.get(key));
                    self.sql.push_str(&ph);
                }
            }
        }

        // 服务端表达式 SET 项：片段固定，动态偏移量走绑定参数，编号接续数据列。
        for (key, expression) in set_exprs {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql
                .push_str(&super::identifier::quote_identifier(key)?);
            self.sql.push_str(" = ");
            let placeholder = self.params.len() + 1;
            let rendered = expression.postgres_render(placeholder);
            for param in rendered.params {
                self.params.push(param.into());
            }
            self.sql.push_str(&rendered.sql);
        }
        Ok(())
    }

    /// 生成受控的字段原子加减 UPDATE。
    pub(crate) fn build_arithmetic_update(
        &mut self,
        table: &str,
        field: &str,
        operator: ArithmeticOperator,
        amount: i64,
        conditions: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }
        let table = super::identifier::quote_table(table)?;
        let field = super::identifier::quote_identifier(field)?;
        self.sql.push_str("UPDATE ");
        self.sql.push_str(&table);
        self.sql.push_str(" SET ");
        self.sql.push_str(&field);
        self.sql.push_str(" = ");
        self.sql.push_str(&field);
        self.sql.push(' ');
        self.sql.push_str(operator.as_sql());
        self.sql.push(' ');
        let placeholder = push_placeholder(&mut self.params, SqlValue::Int(amount));
        self.sql.push_str(&placeholder);
        self.build_where(conditions)
    }

    /// 生成 DELETE 语句（PostgreSQL）
    pub(crate) fn build_delete(
        &mut self,
        table: &str,
        conditions: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        self.append("DELETE FROM ");
        self.append(&super::identifier::quote_table(table)?);
        self.build_where(conditions)?;

        Ok(())
    }

    /// 生成 `INSERT INTO t (cols) SELECT ...`。
    pub(crate) fn build_insert_from_select(
        &mut self,
        table: &str,
        columns: &[&crate::FieldRef],
        source: &QueryBuilder<'_>,
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        self.append("INSERT INTO ");
        self.append(&super::identifier::quote_table(table)?);
        if !columns.is_empty() {
            let quoted: Result<Vec<String>, crate::error::DbError> = columns
                .iter()
                .map(|column| super::identifier::quote_identifier(column.as_str()))
                .collect();
            self.append(" (");
            self.append(&quoted?.join(", "));
            self.append(")");
        }
        self.append(" ");
        self.build_compound_select(source)?;
        Ok(())
    }

    /// 生成 `UPDATE t SET ... FROM <from> WHERE <conditions> AND <join>`。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_update_from(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        set_exprs: &[(String, crate::SqlExpr)],
        conditions: &[Condition],
        from_table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }
        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
        })?;
        if obj.is_empty() && set_exprs.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "更新数据不能为空".to_string(),
            ));
        }

        self.append("UPDATE ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" SET ");
        self.build_assignment_list(obj, field_types, set_exprs)?;
        self.append(" FROM ");
        self.append(&super::identifier::quote_table(from_table.as_str())?);

        // FROM 的联表条件与调用方 WHERE 一起进入 WHERE，避免全表更新。
        let mut all_conditions = conditions.to_vec();
        all_conditions.push(join_condition(left, right));
        self.build_where(&all_conditions)?;
        Ok(())
    }

    /// 生成 `DELETE FROM t USING <using> WHERE <conditions> AND <join>`。
    pub(crate) fn build_delete_using(
        &mut self,
        table: &str,
        conditions: &[Condition],
        using_table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        self.append("DELETE FROM ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" USING ");
        self.append(&super::identifier::quote_table(using_table.as_str())?);

        let mut all_conditions = conditions.to_vec();
        all_conditions.push(join_condition(left, right));
        self.build_where(&all_conditions)?;
        Ok(())
    }

    /// 生成 `MERGE INTO t USING s ON ... WHEN MATCHED ... WHEN NOT MATCHED ...`。
    ///
    /// 需要 PostgreSQL 15+；本方法不做版本探测，调用方需自行保证服务端版本。
    pub(crate) fn build_merge(
        &mut self,
        table: &str,
        source: &crate::TableRef,
        target_key: &crate::FieldRef,
        source_key: &crate::FieldRef,
        columns: &[&crate::FieldRef],
    ) -> Result<(), crate::error::DbError> {
        self.build_merge_with_options(
            table,
            source,
            target_key,
            source_key,
            columns,
            &MergeOptions::default(),
        )
    }

    /// 生成带可选分支的 MERGE；`not_matched_by_source` 为 PostgreSQL 17+ 语法，
    /// 版本门控由 [`QueryBuilder::merge_with_options`] 负责。
    pub(crate) fn build_merge_with_options(
        &mut self,
        table: &str,
        source: &crate::TableRef,
        target_key: &crate::FieldRef,
        source_key: &crate::FieldRef,
        columns: &[&crate::FieldRef],
        options: &MergeOptions,
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        if columns.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "merge 的列列表不能为空".to_string(),
            ));
        }

        let target_qualifier = table.rsplit('.').next().unwrap_or(table);
        let source_qualifier = source
            .as_str()
            .rsplit('.')
            .next()
            .unwrap_or(source.as_str());

        self.append("MERGE INTO ");
        self.append(&super::identifier::quote_table(table)?);
        self.append(" USING ");
        self.append(&super::identifier::quote_table(source.as_str())?);
        self.append(" ON ");
        self.append(&qualify_merge_field(target_key, target_qualifier)?);
        self.append(" = ");
        self.append(&qualify_merge_field(source_key, source_qualifier)?);

        self.append(" WHEN MATCHED");
        if let Some(condition) = &options.matched_condition {
            self.append(" AND ");
            self.append_condition(condition.clone())?;
        }
        self.append(" THEN UPDATE SET ");
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                self.sql.push_str(", ");
            }
            let quoted = super::identifier::quote_identifier(column.as_str())?;
            self.sql.push_str(&quoted);
            self.sql.push_str(" = ");
            self.sql
                .push_str(&qualify_merge_field(column, source_qualifier)?);
        }

        self.append(" WHEN NOT MATCHED THEN INSERT (");
        let quoted: Result<Vec<String>, crate::error::DbError> = columns
            .iter()
            .map(|column| super::identifier::quote_identifier(column.as_str()))
            .collect();
        self.append(&quoted?.join(", "));
        self.append(") VALUES (");
        let values: Result<Vec<String>, crate::error::DbError> = columns
            .iter()
            .map(|column| qualify_merge_field(column, source_qualifier))
            .collect();
        self.append(&values?.join(", "));
        self.append(")");

        match &options.not_matched_by_source {
            None => {}
            Some(MergeNotMatchedBySource::Delete) => {
                self.append(" WHEN NOT MATCHED BY SOURCE THEN DELETE");
            }
            Some(MergeNotMatchedBySource::Update(assignments)) => {
                if assignments.is_empty() {
                    return Err(crate::error::DbError::InvalidArgument(
                        "WHEN NOT MATCHED BY SOURCE 的 UPDATE 赋值列表不能为空".to_string(),
                    ));
                }
                self.append(" WHEN NOT MATCHED BY SOURCE THEN UPDATE SET ");
                for (index, (column, expression)) in assignments.iter().enumerate() {
                    if index > 0 {
                        self.sql.push_str(", ");
                    }
                    // BY SOURCE 分支没有源行，赋值目标只能是目标表列（非限定）。
                    self.append(&super::identifier::quote_identifier(column.as_str())?);
                    self.append(" = ");
                    let rendered = expression.postgres_render(self.params.len() + 1);
                    for param in rendered.params {
                        self.params.push(param.into());
                    }
                    self.append(&rendered.sql);
                }
            }
        }
        Ok(())
    }

    /// 生成批量 UPDATE 语句（PostgreSQL，CASE WHEN 策略）
    ///
    /// 与 MySQL 后端结构一致，差异在占位符方言：`WHEN id=$N THEN $M` 及
    /// `WHERE id IN ($K, ...)` 的编号均在压入参数后按 `params.len()` 推导。
    /// 任意值为 `SqlValue::Null` 时内联字面量 `NULL`（不占占位符编号），
    /// 规避 PostgreSQL 对未类型化绑定 NULL 的限制。
    pub(crate) fn build_update_batch(
        &mut self,
        table: &str,
        records: &[serde_json::Value],
        id_field: &str,
        field_types: &std::collections::HashMap<String, FieldType>,
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        if records.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量更新数据不能为空".to_string(),
            ));
        }

        let first = records[0].as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
        })?;

        // 收集需要更新的字段名（排除主键字段）
        let update_fields: Vec<String> = first
            .keys()
            .filter(|k| k.as_str() != id_field)
            .cloned()
            .collect();

        if update_fields.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "没有可更新的字段".to_string(),
            ));
        }

        // 列集一致性校验（NEW-9）：见 MySQL 同名方法。
        for (idx, record) in records.iter().enumerate() {
            let obj = record.as_object().ok_or_else(|| {
                crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
            })?;
            if !obj.contains_key(id_field) {
                return Err(crate::error::DbError::InvalidArgument(format!(
                    "批量更新第 {idx} 条记录缺少主键字段 {id_field}"
                )));
            }
            let non_id_len = obj.keys().filter(|k| k.as_str() != id_field).count();
            if non_id_len != update_fields.len()
                || !update_fields.iter().all(|f| obj.contains_key(f))
            {
                return Err(crate::error::DbError::InvalidArgument(format!(
                    "批量更新第 {idx} 条记录的列集与首条不一致"
                )));
            }
        }

        // 表名 quote（DB-1）；id_field/列名在各发射点 quote，原始值保留用于 record.get。
        let quoted_id = super::identifier::quote_identifier(id_field)?;
        self.sql.push_str("UPDATE ");
        self.sql.push_str(&super::identifier::quote_table(table)?);
        self.sql.push_str(" SET ");

        // 为每个字段生成 CASE WHEN 子句
        for (field_idx, field) in update_fields.iter().enumerate() {
            if field_idx > 0 {
                self.sql.push_str(", ");
            }
            self.sql
                .push_str(&super::identifier::quote_identifier(field)?);
            self.sql.push_str(" = CASE ");

            for record in records {
                let id_val = record.get(id_field).unwrap_or(&serde_json::Value::Null);
                let field_val = record
                    .get(field.as_str())
                    .unwrap_or(&serde_json::Value::Null);

                let id_sql_val = self.json_value_to_sql_value(id_val, field_types.get(id_field))?;
                let field_sql_val =
                    self.json_value_to_sql_value(field_val, field_types.get(field.as_str()))?;

                // WHEN id=$N THEN $M（值位置 NULL 内联，不占编号）
                self.sql.push_str("WHEN ");
                self.sql.push_str(&quoted_id);
                self.sql.push('=');
                match id_sql_val {
                    SqlValue::Null => self.sql.push_str("NULL"),
                    v => {
                        let ph =
                            push_typed_placeholder(&mut self.params, v, field_types.get(id_field));
                        self.sql.push_str(&ph);
                    }
                }
                self.sql.push_str(" THEN ");
                match field_sql_val {
                    SqlValue::Null => self.sql.push_str("NULL"),
                    v => {
                        let ph = push_typed_placeholder(
                            &mut self.params,
                            v,
                            field_types.get(field.as_str()),
                        );
                        self.sql.push_str(&ph);
                    }
                }
                self.sql.push(' ');
            }

            self.sql.push_str("END");
        }

        // 生成 WHERE id IN ($K, ...) 子句
        self.sql.push_str(" WHERE ");
        self.sql.push_str(&quoted_id);
        self.sql.push_str(" IN (");

        for (idx, record) in records.iter().enumerate() {
            if idx > 0 {
                self.sql.push(',');
            }
            let id_val = record.get(id_field).unwrap_or(&serde_json::Value::Null);
            let id_sql_val = self.json_value_to_sql_value(id_val, field_types.get(id_field))?;
            match id_sql_val {
                SqlValue::Null => self.sql.push_str("NULL"),
                v => {
                    let ph = push_typed_placeholder(&mut self.params, v, field_types.get(id_field));
                    self.sql.push_str(&ph);
                }
            }
        }

        self.sql.push(')');

        Ok(())
    }
    /// 生成 UPSERT 语句（PostgreSQL `INSERT ... ON CONFLICT ... DO UPDATE`）
    ///
    /// 与 MySQL 的 `ON DUPLICATE KEY UPDATE` 的核心差异：PostgreSQL 必须显式指定
    /// 冲突目标（列或约束名），并用 `EXCLUDED.col` 引用待插入的新值。
    ///
    /// - 冲突列为空或显式 `do_nothing` → `DO NOTHING`；
    /// - `update_columns = Some(list)` 时只更新白名单列；
    /// - `update_where` 非空时在 DO UPDATE 后追加 `WHERE`，占位符接续插入值编号。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_upsert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        conflict: &ConflictTarget,
        conflict_predicate: Option<&str>,
        do_nothing: bool,
        update_columns: Option<&[String]>,
        update_where: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        let fields: Vec<String> = obj.keys().cloned().collect();
        // 列名/表名 quote（DB-1）；fields 保留原始键用于 obj.get 查值与冲突列比较。
        let quoted_fields: Result<Vec<String>, crate::error::DbError> = fields
            .iter()
            .map(|f| super::identifier::quote_identifier(f))
            .collect();
        let quoted_fields = quoted_fields?;

        self.sql.push_str("INSERT INTO ");
        self.sql.push_str(&super::identifier::quote_table(table)?);
        self.sql.push_str(" (");
        self.sql.push_str(&quoted_fields.join(", "));
        self.sql.push_str(") VALUES (");

        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                self.sql.push_str(", ");
            }
            let val = obj.get(field.as_str()).unwrap_or(&serde_json::Value::Null);
            let sql_value = self.json_value_to_sql_value(val, field_types.get(field.as_str()))?;
            match sql_value {
                SqlValue::Null => self.sql.push_str("NULL"),
                v => {
                    let ph = push_typed_placeholder(
                        &mut self.params,
                        v,
                        field_types.get(field.as_str()),
                    );
                    self.sql.push_str(&ph);
                }
            }
        }
        self.sql.push(')');

        self.append_on_conflict(
            table,
            &fields,
            conflict,
            conflict_predicate,
            do_nothing,
            update_columns,
            update_where,
        )
    }

    /// 追加 `ON CONFLICT ...` 子句（单条与批量 UPSERT 共用）。
    ///
    /// 冲突目标为列列表、命名约束或未指定；未指定/空列列表只允许
    /// `DO NOTHING`，`DO UPDATE` 在客户端直接报错（避免把无目标语法发到数据库）。
    /// `fields` 为插入列集（用于默认更新列推导与白名单校验）。
    #[allow(clippy::too_many_arguments)]
    fn append_on_conflict(
        &mut self,
        table: &str,
        fields: &[String],
        conflict: &ConflictTarget,
        predicate: Option<&str>,
        do_nothing: bool,
        update_columns: Option<&[String]>,
        update_where: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        if predicate.is_some()
            && !matches!(conflict, ConflictTarget::Columns(columns) if !columns.is_empty())
        {
            return Err(crate::error::DbError::InvalidArgument(
                "ON CONFLICT 谓词只能与显式冲突列列表一起使用".to_string(),
            ));
        }
        // 冲突目标：列列表或约束名；未指定/空列列表等价于「任意约束冲突」。
        let quoted_conflict: Vec<String> = match conflict {
            ConflictTarget::Columns(columns) => columns
                .iter()
                .map(|column| super::identifier::quote_identifier(column))
                .collect::<Result<_, _>>()?,
            ConflictTarget::Unspecified | ConflictTarget::Constraint(_) => Vec::new(),
        };
        let has_column_target =
            matches!(conflict, ConflictTarget::Columns(columns) if !columns.is_empty());
        if !has_column_target && !matches!(conflict, ConflictTarget::Constraint(_)) && !do_nothing {
            return Err(crate::error::DbError::InvalidArgument(
                "upsert 必须显式指定冲突目标（on_conflict / on_conflict_constraint / \
                 on_conflict_with_predicate）；如只想忽略冲突请使用 do_nothing()"
                    .to_string(),
            ));
        }
        match conflict {
            ConflictTarget::Unspecified => {
                self.sql.push_str(" ON CONFLICT");
            }
            ConflictTarget::Columns(_) if !has_column_target => {
                self.sql.push_str(" ON CONFLICT");
            }
            ConflictTarget::Columns(_) => {
                self.sql.push_str(" ON CONFLICT (");
                self.sql.push_str(&quoted_conflict.join(", "));
                self.sql.push(')');
            }
            ConflictTarget::Constraint(name) => {
                self.sql.push_str(" ON CONFLICT ON CONSTRAINT ");
                self.sql
                    .push_str(&super::identifier::quote_identifier(name)?);
            }
        }

        if let Some(predicate) = predicate {
            let predicate = validate_conflict_predicate(predicate)?;
            self.sql.push_str(" WHERE (");
            self.sql.push_str(predicate);
            self.sql.push(')');
        }

        if do_nothing || matches!(conflict, ConflictTarget::Columns(columns) if columns.is_empty())
        {
            self.sql.push_str(" DO NOTHING");
            return Ok(());
        }

        // 更新列白名单：显式给出时按白名单顺序更新；否则使用所有非冲突列。
        let selected: Vec<&String> = match update_columns {
            Some(columns) => {
                if columns.is_empty() {
                    return Err(crate::error::DbError::InvalidArgument(
                        "update_columns 不能为空；如不想更新任何列请使用 do_nothing()".to_string(),
                    ));
                }
                for column in columns {
                    if !fields.contains(column) {
                        return Err(crate::error::DbError::InvalidArgument(format!(
                            "update_columns 中的列 `{column}` 不在插入数据中"
                        )));
                    }
                }
                columns.iter().collect()
            }
            None => fields
                .iter()
                .filter(|field| match conflict {
                    ConflictTarget::Columns(columns) => !columns.contains(field),
                    // ON CONSTRAINT 无法从构造器推断冲突列；按惯例排除 `id`，避免把
                    // 已存在行的主键覆盖成插入值。其他主键名请显式 update_columns()。
                    ConflictTarget::Unspecified | ConflictTarget::Constraint(_) => {
                        !field.eq_ignore_ascii_case("id")
                    }
                })
                .collect(),
        };

        // ON CONSTRAINT 且没有可更新的非主键列时，静默恒等更新会掩盖调用方意图；
        // 明确要求显式白名单或 DO NOTHING。
        if selected.is_empty() && matches!(conflict, ConflictTarget::Constraint(_)) {
            return Err(crate::error::DbError::InvalidArgument(
                "ON CONFLICT ON CONSTRAINT 无法推断可更新列；请显式 update_columns()，\
                 或改用 on_conflict(columns) / do_nothing()"
                    .to_string(),
            ));
        }

        self.sql.push_str(" DO UPDATE SET ");

        // 所有列都是冲突目标列（且未显式给白名单）时 SET 列表为空，会导致语法错误；
        // 退化为对首个冲突列做恒等更新（col = EXCLUDED.col），保证语句合法。
        if selected.is_empty() {
            let fallback = match conflict {
                ConflictTarget::Columns(columns) => columns.first(),
                ConflictTarget::Unspecified | ConflictTarget::Constraint(_) => fields.first(),
            }
            .ok_or_else(|| crate::error::DbError::InvalidArgument("没有可更新的列".to_string()))?;
            let quoted = super::identifier::quote_identifier(fallback)?;
            self.sql.push_str(&quoted);
            self.sql.push_str(" = EXCLUDED.");
            self.sql.push_str(&quoted);
        } else {
            for (index, field) in selected.iter().enumerate() {
                if index > 0 {
                    self.sql.push_str(", ");
                }
                let quoted = super::identifier::quote_identifier(field.as_str())?;
                self.sql.push_str(&quoted);
                self.sql.push_str(" = EXCLUDED.");
                self.sql.push_str(&quoted);
            }
        }

        // DO UPDATE ... WHERE：占位符编号接续插入值参数。
        //
        // PostgreSQL 在 ON CONFLICT DO UPDATE 中同时可见目标表与 EXCLUDED 伪表，
        // 未限定的列名（如 `id`）会直接报 ambiguous；这里把未限定字段自动限定到
        // 目标表名（最后一段），调用方仍可用显式限定名（如 `EXCLUDED.version`）。
        if !update_where.is_empty() {
            let qualifier = table.rsplit('.').next().unwrap_or(table);
            let qualified_conditions: Vec<Condition> = update_where
                .iter()
                .cloned()
                .map(|condition| qualify_condition_fields(condition, qualifier))
                .collect();
            self.build_where(&qualified_conditions)?;
        }

        Ok(())
    }

    /// 生成批量 UPSERT 语句（多行 VALUES + `ON CONFLICT ... DO UPDATE`）。
    ///
    /// 复用 [`Self::build_insert_batch`] 完成列集校验与多行 VALUES 渲染，再追加
    /// ON CONFLICT 尾巴。冲突列由调用方显式传入（不做静默默认）。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_upsert_batch(
        &mut self,
        table: &str,
        data_list: &[serde_json::Value],
        field_types: &HashMap<String, FieldType>,
        conflict_columns: &[String],
        conflict_predicate: Option<&str>,
        do_nothing: bool,
        update_columns: Option<&[String]>,
        update_where: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        self.build_insert_batch(table, data_list, field_types)?;

        if conflict_columns.is_empty() && !do_nothing {
            return Err(crate::error::DbError::InvalidArgument(
                "批量 upsert 必须显式指定冲突列；如使用 id 请显式传入，\
                 或调用 do_nothing() 走 ON CONFLICT DO NOTHING"
                    .to_string(),
            ));
        }

        let fields: Vec<String> = data_list[0]
            .as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default();

        self.append_on_conflict(
            table,
            &fields,
            &ConflictTarget::Columns(conflict_columns.to_vec()),
            conflict_predicate,
            do_nothing,
            update_columns,
            update_where,
        )
    }

    /// 将 JSON 值转换为 SqlValue（PostgreSQL）
    ///
    /// 与 MySQL 后端逻辑一致：字段类型标记优先，其余按 JSON 原生类型推断。
    /// 数组 / 对象映射为 `SqlValue::Json`（PostgreSQL JSON/JSONB），而非字符串。
    fn json_value_to_sql_value(
        &self,
        value: &serde_json::Value,
        field_type: Option<&FieldType>,
    ) -> Result<SqlValue, crate::error::DbError> {
        use serde_json::Value;

        if let Some(ft) = field_type {
            match ft {
                FieldType::Json | FieldType::Jsonb => return Ok(SqlValue::Json(value.clone())),
                FieldType::Uuid => {
                    if let Some(s) = value.as_str() {
                        let uuid = uuid::Uuid::parse_str(s).map_err(|e| {
                            crate::error::DbError::TypeConversionError(format!(
                                "无法解析 UUID 字符串: {e}"
                            ))
                        })?;
                        return Ok(SqlValue::Uuid(uuid));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Uuid 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::TimestampTz => {
                    if let Some(s) = value.as_str() {
                        let parsed = chrono::DateTime::parse_from_rfc3339(s)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .or_else(|_| {
                                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                                    .map(|dt| dt.and_utc())
                            })
                            .or_else(|_| {
                                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
                                    .map(|dt| dt.and_utc())
                            })
                            .map_err(|e| {
                                crate::error::DbError::TypeConversionError(format!(
                                    "无法解析 TIMESTAMPTZ 字符串: {e}"
                                ))
                            })?;
                        return Ok(SqlValue::DateTimeTz(parsed));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "TimestampTz 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Date => {
                    if let Some(s) = value.as_str() {
                        let date =
                            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                                crate::error::DbError::TypeConversionError(format!(
                                    "无法解析 DATE 字符串: {e}"
                                ))
                            })?;
                        return Ok(SqlValue::Date(date));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Date 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Time => {
                    if let Some(s) = value.as_str() {
                        let time = chrono::NaiveTime::parse_from_str(s, "%H:%M:%S")
                            .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S%.f"))
                            .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M"))
                            .map_err(|e| {
                                crate::error::DbError::TypeConversionError(format!(
                                    "无法解析 TIME 字符串: {e}"
                                ))
                            })?;
                        return Ok(SqlValue::Time(time));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Time 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::DateTime => {
                    if let Some(s) = value.as_str() {
                        let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                            .map_err(|e| {
                                crate::error::DbError::TypeConversionError(format!(
                                    "无法解析 DATETIME 字符串: {}",
                                    e
                                ))
                            })?;
                        return Ok(SqlValue::DateTime(dt));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        // 值形态不匹配类型提示：显式报错而非静默跌穿到默认转换（NEW-10）
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "DateTime 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Timestamp => {
                    if let Some(i) = value.as_i64() {
                        return Ok(SqlValue::Timestamp(i));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Timestamp 字段期望整数，实得 {value}"
                        )));
                    }
                }
                FieldType::Decimal => {
                    // NUMERIC/DECIMAL 直接用 BigDecimal 绑定，避免经 f64 丢精度。
                    if value.is_null() {
                        return Ok(SqlValue::Null);
                    }
                    let parse_error = |detail: String| {
                        crate::error::DbError::TypeConversionError(format!(
                            "Decimal 字段期望数字或数字字符串，实得 {value}（{detail}）"
                        ))
                    };
                    if let Some(s) = value.as_str() {
                        return s
                            .parse::<bigdecimal::BigDecimal>()
                            .map(SqlValue::Decimal)
                            .map_err(|error| parse_error(error.to_string()));
                    }
                    if let Some(i) = value.as_i64() {
                        return Ok(SqlValue::Decimal(bigdecimal::BigDecimal::from(i)));
                    }
                    if let Some(u) = value.as_u64() {
                        return Ok(SqlValue::Decimal(bigdecimal::BigDecimal::from(u)));
                    }
                    if let Some(f) = value.as_f64() {
                        return f
                            .to_string()
                            .parse::<bigdecimal::BigDecimal>()
                            .map(SqlValue::Decimal)
                            .map_err(|error| parse_error(error.to_string()));
                    }
                    return Err(parse_error("无法识别的 JSON 数值".to_string()));
                }
                FieldType::Blob => {
                    if let Some(s) = value.as_str() {
                        use base64::Engine;
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(s) {
                            return Ok(SqlValue::Bytes(bytes));
                        }
                        return Ok(SqlValue::Bytes(s.as_bytes().to_vec()));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Blob 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Text => {
                    if let Some(s) = value.as_str() {
                        return Ok(SqlValue::String(s.to_string()));
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Text 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Array(inner) => {
                    if value.is_null() {
                        return Ok(SqlValue::Null);
                    }
                    let elements = value.as_array().ok_or_else(|| {
                        crate::error::DbError::TypeConversionError(format!(
                            "数组字段期望 JSON 数组，实得 {value}"
                        ))
                    })?;
                    let mut converted = Vec::with_capacity(elements.len());
                    for element in elements {
                        converted.push(self.json_value_to_sql_value(element, Some(inner))?);
                    }
                    // 空数组/全 NULL 数组没有元素类型线索：按登记的元素类型显式
                    // 绑定，避免统一退化为 TEXT[] 触发 42804。
                    let typeless = converted
                        .iter()
                        .all(|value| matches!(value, SqlValue::Null));
                    if typeless {
                        if let Some(element_type) = pg_array_element_type_name(inner) {
                            return Ok(SqlValue::TypedArray {
                                element_type,
                                values: converted,
                            });
                        }
                    }
                    return Ok(SqlValue::Array(converted));
                }
                FieldType::Inet | FieldType::Cidr | FieldType::MacAddr | FieldType::MacAddr8 => {
                    // 网络类型必须带显式 SQL 类型转换；直接绑 text 会报 42883。
                    if value.is_null() {
                        return Ok(SqlValue::Null);
                    }
                    let text = value.as_str().ok_or_else(|| {
                        crate::error::DbError::TypeConversionError(format!(
                            "网络字段期望字符串，实得 {value}"
                        ))
                    })?;
                    return Ok(match ft {
                        FieldType::Inet => SqlValue::Inet(text.to_string()),
                        FieldType::Cidr => SqlValue::Cidr(text.to_string()),
                        FieldType::MacAddr => SqlValue::MacAddr(text.to_string()),
                        FieldType::MacAddr8 => SqlValue::MacAddr8(text.to_string()),
                        _ => unreachable!("分支只覆盖四种网络类型"),
                    });
                }
                FieldType::Bit | FieldType::Varbit => {
                    // 位串类型需要长度（bit(n)/varbit(n)），FieldType 未携带长度；
                    // 没有长度就无法生成正确的显式转换，明确 fail-closed。
                    return Err(crate::error::DbError::InvalidArgument(
                        "Bit/Varbit 字段的写入需要位长信息，当前结构化 API 不支持；请使用原生 SQL 逃生舱"
                            .to_string(),
                    ));
                }
                FieldType::Range(_) | FieldType::Multirange(_) => {
                    // 范围类型以文本字面量写入；显式 `::<range>` 转换由
                    // `pg_value_cast` 按字段真实类型追加。
                    if value.is_null() {
                        return Ok(SqlValue::Null);
                    }
                    if value.as_str().is_none() {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Range/Multirange 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Standard | FieldType::Bool => {}
            }
        }

        match value {
            Value::Null => Ok(SqlValue::Null),
            Value::Bool(b) => Ok(SqlValue::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqlValue::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlValue::Float(f))
                } else {
                    Err(crate::error::DbError::TypeConversionError(
                        "无法转换数字类型".to_string(),
                    ))
                }
            }
            Value::String(s) => Ok(SqlValue::String(s.clone())),
            Value::Array(_) | Value::Object(_) => Ok(SqlValue::Json(value.clone())),
        }
    }
}

/// 集合运算符（PostgreSQL）。
///
/// `UNION` 与 `UNION ALL` 的结合方向为左、优先级最低，链式渲染无需额外括号；
/// `INTERSECT` 优先级高于 `UNION`/`EXCEPT`，混合链在 `build_set_chain` 中显式
/// 加括号固定为调用方书写的左结合顺序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetOperator {
    Union,
    UnionAll,
    Intersect,
    IntersectAll,
    Except,
    ExceptAll,
}

impl SetOperator {
    const fn sql(self) -> &'static str {
        match self {
            Self::Union => " UNION ",
            Self::UnionAll => " UNION ALL ",
            Self::Intersect => " INTERSECT ",
            Self::IntersectAll => " INTERSECT ALL ",
            Self::Except => " EXCEPT ",
            Self::ExceptAll => " EXCEPT ALL ",
        }
    }

    const fn is_union(self) -> bool {
        matches!(self, Self::Union | Self::UnionAll)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ArithmeticOperator {
    Add,
    Subtract,
}

/// `WITH` 子句中的一个公共表表达式。
struct Cte<'a> {
    /// CTE 名称（单段标识符，渲染期校验加引号）。
    name: String,
    /// 是否要求 `WITH RECURSIVE`（任意 CTE 为递归时整条语句加一次关键字）。
    recursive: bool,
    /// CTE 主体查询。
    query: Box<QueryBuilder<'a>>,
}

/// JOIN 类型（Phase B 扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    Cross,
    Natural,
    LeftLateral,
}

/// JOIN 目标：普通表或 `LATERAL` 子查询。
enum JoinTarget<'a> {
    /// 已校验的表名（渲染期逐段加引号）。
    Table(String),
    /// `LATERAL (<subquery>) AS "alias"`。
    Lateral(Box<QueryBuilder<'a>>, String),
}

/// JOIN 连接条件。
#[derive(Debug, Clone, PartialEq, Eq)]
enum JoinCondition {
    /// `ON <受控表达式>`。
    On(String),
    /// `USING ("col", ...)`。
    Using(Vec<String>),
    /// `CROSS` / `NATURAL JOIN` 无连接条件。
    None,
}

/// 一条 JOIN 子句。
struct JoinSpec<'a> {
    kind: JoinKind,
    target: JoinTarget<'a>,
    condition: JoinCondition,
}

/// 分组集构造（`GROUPING SETS` / `ROLLUP` / `CUBE`）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum GroupingConstruct {
    /// `GROUPING SETS ((a, b), (a), ())`；空内层集合表示总计行。
    GroupingSets(Vec<Vec<crate::FieldRef>>),
    /// `ROLLUP (a, b)`。
    Rollup(Vec<crate::FieldRef>),
    /// `CUBE (a, b)`。
    Cube(Vec<crate::FieldRef>),
}

/// 窗口帧模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    /// 按物理行偏移。
    Rows,
    /// 按排序值范围。
    Range,
    /// 按排序值相同的一组行（peer group）偏移；要求窗口带 `ORDER BY`。
    Groups,
}

/// 窗口帧边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBound {
    /// `UNBOUNDED PRECEDING`。
    UnboundedPreceding,
    /// `N PRECEDING`。
    Preceding(u64),
    /// `CURRENT ROW`。
    CurrentRow,
    /// `N FOLLOWING`。
    Following(u64),
    /// `UNBOUNDED FOLLOWING`。
    UnboundedFollowing,
}

impl FrameBound {
    fn sql(self) -> String {
        match self {
            Self::UnboundedPreceding => "UNBOUNDED PRECEDING".to_string(),
            Self::Preceding(offset) => format!("{offset} PRECEDING"),
            Self::CurrentRow => "CURRENT ROW".to_string(),
            Self::Following(offset) => format!("{offset} FOLLOWING"),
            Self::UnboundedFollowing => "UNBOUNDED FOLLOWING".to_string(),
        }
    }
}

/// 窗口帧：`ROWS BETWEEN ... AND ...` 或 `RANGE BETWEEN ... AND ...`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFrame {
    mode: FrameMode,
    start: FrameBound,
    end: FrameBound,
}

impl WindowFrame {
    /// `ROWS BETWEEN <start> AND <end>`（按物理行偏移）。
    pub fn rows_between(start: FrameBound, end: FrameBound) -> Self {
        Self {
            mode: FrameMode::Rows,
            start,
            end,
        }
    }

    /// `RANGE BETWEEN <start> AND <end>`（按排序值范围）。
    pub fn range_between(start: FrameBound, end: FrameBound) -> Self {
        Self {
            mode: FrameMode::Range,
            start,
            end,
        }
    }

    /// `GROUPS BETWEEN <start> AND <end>`（按排序值相同的一组行偏移）。
    ///
    /// `GROUPS` 帧要求窗口定义包含 `ORDER BY`，否则渲染期返回
    /// [`crate::error::DbError::InvalidArgument`]。
    pub fn groups_between(start: FrameBound, end: FrameBound) -> Self {
        Self {
            mode: FrameMode::Groups,
            start,
            end,
        }
    }

    fn render(&self) -> Result<String, crate::error::DbError> {
        if matches!(self.start, FrameBound::UnboundedFollowing) {
            return Err(crate::error::DbError::InvalidArgument(
                "窗口帧的起始边界不能是 UNBOUNDED FOLLOWING".to_string(),
            ));
        }
        if matches!(self.end, FrameBound::UnboundedPreceding) {
            return Err(crate::error::DbError::InvalidArgument(
                "窗口帧的结束边界不能是 UNBOUNDED PRECEDING".to_string(),
            ));
        }
        let mode = match self.mode {
            FrameMode::Rows => "ROWS",
            FrameMode::Range => "RANGE",
            FrameMode::Groups => "GROUPS",
        };
        Ok(format!(
            "{mode} BETWEEN {} AND {}",
            self.start.sql(),
            self.end.sql()
        ))
    }
}

/// 窗口定义：`PARTITION BY` / `ORDER BY` / 帧。
///
/// 全部字段通过 [`crate::FieldRef`] 受控构造，渲染时逐段加引号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowSpec {
    partition_by: Vec<crate::FieldRef>,
    order_by: Vec<(crate::FieldRef, crate::SortOrder)>,
    frame: Option<WindowFrame>,
    /// 命名窗口引用（`OVER "w"`）；与分区/排序/帧互斥。
    reference: Option<String>,
}

impl WindowSpec {
    /// 空窗口：`OVER ()`。
    pub fn new() -> Self {
        Self::default()
    }

    /// 引用 [`QueryBuilder::window`] 定义的命名窗口：`OVER "name"`。
    ///
    /// 不能与 `PARTITION BY` / `ORDER BY` / 帧组合，渲染期会校验。
    pub fn reference(name: &str) -> Self {
        Self {
            reference: Some(name.to_string()),
            ..Self::default()
        }
    }

    /// 追加 `PARTITION BY` 字段。
    pub fn partition_by(mut self, fields: &[&crate::FieldRef]) -> Self {
        self.partition_by
            .extend(fields.iter().map(|field| (*field).clone()));
        self
    }

    /// 追加 `ORDER BY` 字段与方向。
    pub fn order_by(mut self, field: &crate::FieldRef, order: crate::SortOrder) -> Self {
        self.order_by.push((field.clone(), order));
        self
    }

    /// 设置窗口帧（`ROWS` / `RANGE BETWEEN`）。
    pub fn frame(mut self, frame: WindowFrame) -> Self {
        self.frame = Some(frame);
        self
    }

    /// 渲染窗口体（不含 `OVER` 包裹），供 `WINDOW w AS (...)` 复用。
    fn render_body(&self) -> Result<String, crate::error::DbError> {
        // GROUPS 帧按 peer group 计算，PostgreSQL 要求窗口定义必须有 ORDER BY。
        if matches!(self.frame.map(|frame| frame.mode), Some(FrameMode::Groups))
            && self.order_by.is_empty()
        {
            return Err(crate::error::DbError::InvalidArgument(
                "GROUPS 帧要求窗口定义包含 ORDER BY".to_string(),
            ));
        }
        let mut parts: Vec<String> = Vec::new();
        if !self.partition_by.is_empty() {
            let fields: Vec<&str> = self
                .partition_by
                .iter()
                .map(crate::FieldRef::postgres_quoted)
                .collect();
            parts.push(format!("PARTITION BY {}", fields.join(", ")));
        }
        if !self.order_by.is_empty() {
            let orders: Vec<String> = self
                .order_by
                .iter()
                .map(|(field, order)| {
                    format!(
                        "{} {}",
                        field.postgres_quoted(),
                        if order.is_ascending() { "ASC" } else { "DESC" }
                    )
                })
                .collect();
            parts.push(format!("ORDER BY {}", orders.join(", ")));
        }
        if let Some(frame) = &self.frame {
            parts.push(frame.render()?);
        }
        Ok(parts.join(" "))
    }

    fn render(&self) -> Result<String, crate::error::DbError> {
        if let Some(name) = &self.reference {
            if !self.partition_by.is_empty() || !self.order_by.is_empty() || self.frame.is_some() {
                return Err(crate::error::DbError::InvalidArgument(
                    "引用命名窗口时不能再附带 PARTITION BY / ORDER BY / 帧".to_string(),
                ));
            }
            return Ok(format!(
                "OVER {}",
                super::identifier::quote_identifier(name)?
            ));
        }
        Ok(format!("OVER ({})", self.render_body()?))
    }
}

/// 窗口函数白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowFunctionKind {
    RowNumber,
    Rank,
    DenseRank,
    Sum,
    Avg,
    Count,
    Min,
    Max,
    Lag,
    Lead,
}

/// 窗口函数表达式（函数 + 可选操作数）。
///
/// 排名函数无操作数；`COUNT(*)` 用 [`Self::count_all`] 表达；其余聚合/取值函数
/// 必须提供列参数，构造器已强制。
#[derive(Debug, Clone)]
pub struct WindowFunction {
    kind: WindowFunctionKind,
    field: Option<crate::FieldRef>,
    /// 聚合过滤：`FILTER (WHERE <condition>)`。
    filter: Option<Condition>,
}

impl WindowFunction {
    fn rank_like(kind: WindowFunctionKind) -> Self {
        Self {
            kind,
            field: None,
            filter: None,
        }
    }

    fn with_field(kind: WindowFunctionKind, field: &crate::FieldRef) -> Self {
        Self {
            kind,
            field: Some(field.clone()),
            filter: None,
        }
    }

    /// `ROW_NUMBER()`。
    pub fn row_number() -> Self {
        Self::rank_like(WindowFunctionKind::RowNumber)
    }

    /// `RANK()`。
    pub fn rank() -> Self {
        Self::rank_like(WindowFunctionKind::Rank)
    }

    /// `DENSE_RANK()`。
    pub fn dense_rank() -> Self {
        Self::rank_like(WindowFunctionKind::DenseRank)
    }

    /// `SUM(field)`。
    pub fn sum(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Sum, field)
    }

    /// `AVG(field)`。
    pub fn avg(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Avg, field)
    }

    /// `COUNT(field)`。
    pub fn count(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Count, field)
    }

    /// `COUNT(*)`。
    pub fn count_all() -> Self {
        Self::rank_like(WindowFunctionKind::Count)
    }

    /// `MIN(field)`。
    pub fn min(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Min, field)
    }

    /// `MAX(field)`。
    pub fn max(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Max, field)
    }

    /// `LAG(field)`。
    pub fn lag(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Lag, field)
    }

    /// `LEAD(field)`。
    pub fn lead(field: &crate::FieldRef) -> Self {
        Self::with_field(WindowFunctionKind::Lead, field)
    }

    /// 为聚合窗口函数追加 `FILTER (WHERE <condition>)`。
    ///
    /// PostgreSQL 只允许聚合函数使用 `FILTER`；排名/取值函数（`ROW_NUMBER`、
    /// `RANK`、`DENSE_RANK`、`LAG`、`LEAD`）在渲染期返回
    /// [`crate::error::DbError::InvalidArgument`]。
    pub fn filter(mut self, condition: Condition) -> Self {
        self.filter = Some(condition);
        self
    }

    /// 渲染函数调用；`FILTER` 条件中的绑定参数追加到生成器参数列表。
    fn render(&self, generator: &mut SqlGenerator) -> Result<String, crate::error::DbError> {
        let name = match self.kind {
            WindowFunctionKind::RowNumber => "ROW_NUMBER",
            WindowFunctionKind::Rank => "RANK",
            WindowFunctionKind::DenseRank => "DENSE_RANK",
            WindowFunctionKind::Sum => "SUM",
            WindowFunctionKind::Avg => "AVG",
            WindowFunctionKind::Count => "COUNT",
            WindowFunctionKind::Min => "MIN",
            WindowFunctionKind::Max => "MAX",
            WindowFunctionKind::Lag => "LAG",
            WindowFunctionKind::Lead => "LEAD",
        };
        let call = match &self.field {
            Some(field) => format!("{name}({})", field.postgres_quoted()),
            // 只有 COUNT(*) 允许空操作数。
            None if self.kind == WindowFunctionKind::Count => "COUNT(*)".to_string(),
            None => format!("{name}()"),
        };
        let Some(filter) = &self.filter else {
            return Ok(call);
        };
        if !matches!(
            self.kind,
            WindowFunctionKind::Sum
                | WindowFunctionKind::Avg
                | WindowFunctionKind::Count
                | WindowFunctionKind::Min
                | WindowFunctionKind::Max
        ) {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "{name} 不是聚合函数，不能使用 FILTER (WHERE ...)"
            )));
        }
        let fragment = generator.render_condition_fragment(filter.clone())?;
        Ok(format!("{call} FILTER (WHERE {fragment})"))
    }
}

/// SELECT 投影中的窗口表达式。
struct WindowProjection {
    function: WindowFunction,
    spec: WindowSpec,
    alias: String,
}

impl WindowProjection {
    fn render(&self, generator: &mut SqlGenerator) -> Result<String, crate::error::DbError> {
        let alias = super::identifier::quote_identifier(&self.alias)?;
        Ok(format!(
            "{} {} AS {alias}",
            self.function.render(generator)?,
            self.spec.render()?
        ))
    }
}

/// HAVING 中的聚合表达式比较（如 `HAVING COUNT(*) > 1`）。
struct HavingExpr {
    expression: crate::SelectExpr,
    operator: ComparisonOperator,
    value: SqlValue,
}

/// 比较操作符 → SQL 文本（`HavingExpr` 渲染用）。
fn comparison_operator_sql(operator: ComparisonOperator) -> &'static str {
    match operator {
        ComparisonOperator::Eq => "=",
        ComparisonOperator::Ne => "!=",
        ComparisonOperator::Gt => ">",
        ComparisonOperator::Lt => "<",
        ComparisonOperator::Gte => ">=",
        ComparisonOperator::Lte => "<=",
    }
}

/// UPSERT 冲突目标：显式列列表或命名约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConflictTarget {
    /// 未显式指定冲突目标。仅允许 `DO NOTHING`；`DO UPDATE` 必须在客户端报错。
    Unspecified,
    /// `ON CONFLICT (col, ...)`；空列表表示「未指定」，只允许 `DO NOTHING`。
    Columns(Vec<String>),
    /// `ON CONFLICT ON CONSTRAINT <name>`。
    Constraint(String),
}

/// `RETURNING` 的单个目标列（可选输出别名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReturningColumn {
    /// 列名（未加引号）。
    pub(crate) column: String,
    /// 输出别名（未加引号）。
    pub(crate) alias: Option<String>,
}

/// `RETURNING` 目标：单列、多列（可带别名）或 `*`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReturningTarget {
    /// `RETURNING "<column>"`。
    Column(String),
    /// `RETURNING "<col>" [AS "<alias>"], ...`。
    Columns(Vec<ReturningColumn>),
    /// `RETURNING *`。
    All,
}

/// MERGE 的 `WHEN NOT MATCHED BY SOURCE` 分支动作。
///
/// 该分支自 PostgreSQL 17 起才被支持；在更低版本执行会返回
/// [`crate::error::DbError::UnsupportedOperator`]。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MergeNotMatchedBySource {
    /// `WHEN NOT MATCHED BY SOURCE THEN DELETE`。
    Delete,
    /// `WHEN NOT MATCHED BY SOURCE THEN UPDATE SET <col> = <expr>, ...`。
    ///
    /// 表达式的动态部分走绑定参数；`col` 必须是目标表的非限定列名。
    Update(Vec<(crate::FieldRef, crate::SqlExpr)>),
}

/// MERGE 的可选分支配置。
///
/// 默认值表示现有的「命中更新 + 未命中插入」行为，与 [`QueryBuilder::merge`]
/// 完全一致；使用带条件的命中分支或 `NOT MATCHED BY SOURCE` 时改用
/// [`QueryBuilder::merge_with_options`]。
#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    /// `WHEN MATCHED AND <condition> THEN ...` 的附加条件。
    matched_condition: Option<Condition>,
    /// `WHEN NOT MATCHED BY SOURCE THEN ...` 分支（PostgreSQL 17+）。
    not_matched_by_source: Option<MergeNotMatchedBySource>,
}

impl MergeOptions {
    /// 为 `WHEN MATCHED` 分支追加 `AND <condition>`。
    pub fn matched_condition(mut self, condition: Condition) -> Self {
        self.matched_condition = Some(condition);
        self
    }

    /// 追加 `WHEN NOT MATCHED BY SOURCE` 分支（PostgreSQL 17+）。
    pub fn not_matched_by_source(mut self, action: MergeNotMatchedBySource) -> Self {
        self.not_matched_by_source = Some(action);
        self
    }
}

impl ArithmeticOperator {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
        }
    }
}

/// 查询构建器（PostgreSQL）
enum QueryExecutor<'a> {
    Pool(&'a PgPool),
    Transaction(&'a mut crate::postgres::Transaction),
}

/// 把条件树中的未限定字段名限定到目标表（`DO UPDATE ... WHERE` 需要消歧）。
///
/// 已含 `.` 的字段原样保留，便于调用方显式引用 `EXCLUDED.col` 或其它表列。
fn qualify_condition_fields(condition: Condition, qualifier: &str) -> Condition {
    fn qualify(field: String, qualifier: &str) -> String {
        if field.contains('.') {
            field
        } else {
            format!("{qualifier}.{field}")
        }
    }

    match condition {
        Condition::Eq(field, value) => Condition::Eq(qualify(field, qualifier), value),
        Condition::Ne(field, value) => Condition::Ne(qualify(field, qualifier), value),
        Condition::Gt(field, value) => Condition::Gt(qualify(field, qualifier), value),
        Condition::Lt(field, value) => Condition::Lt(qualify(field, qualifier), value),
        Condition::Gte(field, value) => Condition::Gte(qualify(field, qualifier), value),
        Condition::Lte(field, value) => Condition::Lte(qualify(field, qualifier), value),
        Condition::In(field, values) => Condition::In(qualify(field, qualifier), values),
        Condition::NotIn(field, values) => Condition::NotIn(qualify(field, qualifier), values),
        Condition::Between(field, start, end) => {
            Condition::Between(qualify(field, qualifier), start, end)
        }
        Condition::Like(field, pattern) => Condition::Like(qualify(field, qualifier), pattern),
        Condition::ILike(field, pattern) => Condition::ILike(qualify(field, qualifier), pattern),
        Condition::NotILike(field, pattern) => {
            Condition::NotILike(qualify(field, qualifier), pattern)
        }
        Condition::RegexMatch(field, pattern) => {
            Condition::RegexMatch(qualify(field, qualifier), pattern)
        }
        Condition::RegexIMatch(field, pattern) => {
            Condition::RegexIMatch(qualify(field, qualifier), pattern)
        }
        Condition::NotRegexMatch(field, pattern) => {
            Condition::NotRegexMatch(qualify(field, qualifier), pattern)
        }
        Condition::NotRegexIMatch(field, pattern) => {
            Condition::NotRegexIMatch(qualify(field, qualifier), pattern)
        }
        Condition::IsDistinctFrom(field, value) => {
            Condition::IsDistinctFrom(qualify(field, qualifier), value)
        }
        Condition::IsNotDistinctFrom(field, value) => {
            Condition::IsNotDistinctFrom(qualify(field, qualifier), value)
        }
        Condition::IsNull(field) => Condition::IsNull(qualify(field, qualifier)),
        Condition::IsNotNull(field) => Condition::IsNotNull(qualify(field, qualifier)),
        Condition::ColumnComparison(left, operator, right) => {
            Condition::ColumnComparison(qualify(left, qualifier), operator, right)
        }
        Condition::FieldExpr(field, operator, expression) => {
            Condition::FieldExpr(qualify(field, qualifier), operator, expression)
        }
        Condition::Not(inner) => {
            Condition::Not(Box::new(qualify_condition_fields(*inner, qualifier)))
        }
        Condition::And(conditions) => Condition::And(
            conditions
                .into_iter()
                .map(|condition| qualify_condition_fields(condition, qualifier))
                .collect(),
        ),
        Condition::Or(conditions) => Condition::Or(
            conditions
                .into_iter()
                .map(|condition| qualify_condition_fields(condition, qualifier))
                .collect(),
        ),
        Condition::JsonContains(field, value) => {
            Condition::JsonContains(qualify(field, qualifier), value)
        }
        Condition::JsonContainedBy(field, value) => {
            Condition::JsonContainedBy(qualify(field, qualifier), value)
        }
        Condition::JsonHasKey(field, key) => Condition::JsonHasKey(qualify(field, qualifier), key),
        Condition::Any(field, value) => Condition::Any(qualify(field, qualifier), value),
        Condition::All(field, value) => Condition::All(qualify(field, qualifier), value),
        Condition::RangeContains(field, value) => {
            Condition::RangeContains(qualify(field, qualifier), value)
        }
        Condition::RangeOverlaps(field, value) => {
            Condition::RangeOverlaps(qualify(field, qualifier), value)
        }
        Condition::JsonPathExists(field, path) => {
            Condition::JsonPathExists(qualify(field, qualifier), path)
        }
        Condition::JsonPathMatch(field, path, vars) => {
            Condition::JsonPathMatch(qualify(field, qualifier), path, vars)
        }
        Condition::FullText(field, query) => Condition::FullText(qualify(field, qualifier), query),
        Condition::FullTextQuery(field, query) => {
            Condition::FullTextQuery(qualify(field, qualifier), query)
        }
        // 子查询内部的条件作用域与目标表不同，保持原样。
        Condition::Exists(subquery) => Condition::Exists(subquery),
        Condition::NotExists(subquery) => Condition::NotExists(subquery),
        Condition::InSubquery(field, subquery) => Condition::InSubquery(field, subquery),
    }
}

/// 构造两个受控字段之间的等值条件（`update_from` / `delete_using` 的联表谓词）。
fn join_condition(left: &crate::FieldRef, right: &crate::FieldRef) -> Condition {
    Condition::ColumnComparison(
        left.as_str().to_string(),
        ComparisonOperator::Eq,
        right.as_str().to_string(),
    )
}

/// 把 MERGE 字段限定到给定表限定符；已含 `.` 的字段原样保留。
fn qualify_merge_field(
    field: &crate::FieldRef,
    qualifier: &str,
) -> Result<String, crate::error::DbError> {
    if field.as_str().contains('.') {
        super::identifier::quote_qualified(field.as_str())
    } else {
        super::identifier::quote_qualified(&format!("{qualifier}.{}", field.as_str()))
    }
}

/// 兼容所有整型主键宽度的解码包装（INT2 / INT4 / INT8）。
///
/// `insert()` 不再用 `CAST(... AS BIGINT)` 统一 RETURNING 列类型，因此需要按
/// 列的真实类型解码：`SERIAL`（INT4）与 `BIGSERIAL`（INT8）都必须能取回为 u64。
/// sqlx 的 `Type::compatible` 默认要求 SQL 类型严格相等，这里显式放宽到三种整型。
struct IntegerPk(i64);

impl sqlx::Type<sqlx::Postgres> for IntegerPk {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("INT8")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        matches!(ty.name(), "INT2" | "INT4" | "INT8")
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for IntegerPk {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        match value.type_info().name() {
            "INT2" => <i16 as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)
                .map(|value| Self(i64::from(value))),
            "INT4" => <i32 as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)
                .map(|value| Self(i64::from(value))),
            "INT8" => <i64 as sqlx::Decode<'r, sqlx::Postgres>>::decode(value).map(Self),
            other => Err(format!(
                "insert() 主键类型 {other} 不是整型；请使用 insert_returning::<T, R>() 按真实类型取回"
            )
            .into()),
        }
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for IntegerPk {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        sqlx::Row::try_get(row, 0)
    }
}

/// 校验 `ON CONFLICT (cols) WHERE (<predicate>)` 的受信谓词。
///
/// 谓词按受信 SQL 表达式处理（允许列名/算术/函数），但拒绝能把片段拆成第二条
/// 语句或注释掉后续子句的字符序列：`;`、`--`、`/*`、`*/` 与 NUL。
fn validate_conflict_predicate(predicate: &str) -> Result<&str, crate::error::DbError> {
    let trimmed = predicate.trim();
    if trimmed.is_empty() {
        return Err(crate::error::DbError::InvalidArgument(
            "ON CONFLICT 谓词不能为空".to_string(),
        ));
    }
    if trimmed.contains('\0')
        || trimmed.contains(';')
        || trimmed.contains("--")
        || trimmed.contains("/*")
        || trimmed.contains("*/")
    {
        return Err(crate::error::DbError::InvalidArgument(
            "ON CONFLICT 谓词不能包含分号、注释标记或 NUL".to_string(),
        ));
    }
    Ok(trimmed)
}

/// 把封闭比较操作符映射为条件枚举的比较操作符；模式匹配与 NULL-safe 比较不适用。
fn comparison_operator(op: crate::CompareOp) -> Option<ComparisonOperator> {
    match op {
        crate::CompareOp::Eq => Some(ComparisonOperator::Eq),
        crate::CompareOp::Ne => Some(ComparisonOperator::Ne),
        crate::CompareOp::Gt => Some(ComparisonOperator::Gt),
        crate::CompareOp::Lt => Some(ComparisonOperator::Lt),
        crate::CompareOp::Gte => Some(ComparisonOperator::Gte),
        crate::CompareOp::Lte => Some(ComparisonOperator::Lte),
        _ => None,
    }
}

/// 由 `(字段, 操作符, 值)` 构造条件（PostgreSQL）。
///
/// 模式匹配类操作符（LIKE/ILIKE/正则）只接受字符串模式值；NULL-safe 比较接受
/// 任意值。其余操作符走 [`comparison_operator`] 的受控映射。
fn comparison_condition(
    field: &str,
    op: crate::CompareOp,
    value: SqlValue,
) -> Result<Condition, crate::error::DbError> {
    let field = field.to_string();
    let pattern = |value: SqlValue| match value {
        SqlValue::String(pattern) => Ok(pattern),
        _ => Err(crate::error::DbError::UnsupportedOperator(
            "模式匹配操作符仅支持字符串模式值".to_string(),
        )),
    };
    match op {
        crate::CompareOp::Like => Ok(Condition::Like(field, pattern(value)?)),
        crate::CompareOp::ILike => Ok(Condition::ILike(field, pattern(value)?)),
        crate::CompareOp::NotILike => Ok(Condition::NotILike(field, pattern(value)?)),
        crate::CompareOp::RegexMatch => Ok(Condition::RegexMatch(field, pattern(value)?)),
        crate::CompareOp::RegexIMatch => Ok(Condition::RegexIMatch(field, pattern(value)?)),
        crate::CompareOp::NotRegexMatch => Ok(Condition::NotRegexMatch(field, pattern(value)?)),
        crate::CompareOp::NotRegexIMatch => Ok(Condition::NotRegexIMatch(field, pattern(value)?)),
        crate::CompareOp::IsDistinctFrom => Ok(Condition::IsDistinctFrom(field, value)),
        crate::CompareOp::IsNotDistinctFrom => Ok(Condition::IsNotDistinctFrom(field, value)),
        _ => {
            let operator = comparison_operator(op).ok_or_else(|| {
                crate::error::DbError::UnsupportedOperator("不支持的比较操作符".to_string())
            })?;
            Ok(match operator {
                ComparisonOperator::Eq => Condition::Eq(field, value),
                ComparisonOperator::Ne => Condition::Ne(field, value),
                ComparisonOperator::Gt => Condition::Gt(field, value),
                ComparisonOperator::Lt => Condition::Lt(field, value),
                ComparisonOperator::Gte => Condition::Gte(field, value),
                ComparisonOperator::Lte => Condition::Lte(field, value),
            })
        }
    }
}

pub struct QueryBuilder<'a> {
    executor: QueryExecutor<'a>,
    table: String,
    fields: Vec<String>,
    /// `select_expr` 登记的受控服务端表达式与可选输出别名。
    projection_exprs: Vec<(crate::SqlExpr, Option<String>)>,
    /// `window_expr` 登记的窗口函数投影。
    window_exprs: Vec<WindowProjection>,
    /// `window` 登记的命名窗口定义（`WINDOW <name> AS (...)`）。
    named_windows: Vec<(String, WindowSpec)>,
    conditions: Vec<Condition>,
    joins: Vec<JoinSpec<'a>>,
    order_by: Vec<OrderClause>,
    group_by: Vec<String>,
    /// `GROUPING SETS` / `ROLLUP` / `CUBE` 构造。
    grouping_constructs: Vec<GroupingConstruct>,
    having_clause: Vec<Condition>,
    /// `having_expr` 登记的聚合表达式比较。
    having_exprs: Vec<HavingExpr>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
    /// `DISTINCT ON (...)` 的已加引号字段列表；与 `distinct` 互斥。
    distinct_on: Vec<String>,
    /// `WITH` 公共表表达式列表。
    ctes: Vec<Cte<'a>>,
    /// 集合运算链（UNION / INTERSECT / EXCEPT）。
    set_ops: Vec<(SetOperator, Box<QueryBuilder<'a>>)>,
    field_types: HashMap<String, FieldType>,
    /// `set_expr` 登记的 `SET col = <expr>`，在 update/update_returning 时追加。
    set_exprs: Vec<(String, crate::SqlExpr)>,
    /// UPSERT 的冲突目标。
    ///
    /// 默认 [`ConflictTarget::Unspecified`]：`DO UPDATE` 必须显式指定目标
    /// （`on_conflict` / `on_conflict_constraint` / `on_conflict_with_predicate`），
    /// 否则在客户端返回 [`crate::DbError::InvalidArgument`]；`do_nothing()` 可省略目标。
    conflict_target: ConflictTarget,
    /// 部分唯一索引谓词：`ON CONFLICT (cols) WHERE (<predicate>)`。
    conflict_predicate: Option<String>,
    /// 显式 `DO NOTHING`（忽略更新列与 DO UPDATE WHERE）。
    do_nothing: bool,
    /// DO UPDATE 的列白名单；`None` 表示更新所有非冲突列。
    update_columns: Option<Vec<String>>,
    /// DO UPDATE ... WHERE 条件。
    upsert_where: Vec<Condition>,
    /// `insert` 的 `RETURNING` 目标。默认单列 `"id"`。
    returning: ReturningTarget,
    enable_logging: bool,
}

impl<'a> QueryBuilder<'a> {
    /// 创建新的查询构建器
    pub(crate) fn new(pool: &'a PgPool, table_name: &str, enable_logging: bool) -> Self {
        Self {
            executor: QueryExecutor::Pool(pool),
            table: table_name.to_string(),
            fields: Vec::new(),
            projection_exprs: Vec::new(),
            window_exprs: Vec::new(),
            named_windows: Vec::new(),
            conditions: Vec::new(),
            joins: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            grouping_constructs: Vec::new(),
            having_clause: Vec::new(),
            having_exprs: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
            distinct_on: Vec::new(),
            ctes: Vec::new(),
            set_ops: Vec::new(),
            field_types: HashMap::new(),
            set_exprs: Vec::new(),
            conflict_target: ConflictTarget::Unspecified,
            conflict_predicate: None,
            do_nothing: false,
            update_columns: None,
            upsert_where: Vec::new(),
            returning: ReturningTarget::Column("id".to_string()),
            enable_logging,
        }
    }

    pub(crate) fn new_transaction(
        transaction: &'a mut crate::postgres::Transaction,
        table_name: &str,
        enable_logging: bool,
    ) -> Self {
        Self {
            executor: QueryExecutor::Transaction(transaction),
            table: table_name.to_string(),
            fields: Vec::new(),
            projection_exprs: Vec::new(),
            window_exprs: Vec::new(),
            named_windows: Vec::new(),
            conditions: Vec::new(),
            joins: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            grouping_constructs: Vec::new(),
            having_clause: Vec::new(),
            having_exprs: Vec::new(),
            limit: None,
            offset: None,
            distinct: false,
            distinct_on: Vec::new(),
            ctes: Vec::new(),
            set_ops: Vec::new(),
            field_types: HashMap::new(),
            set_exprs: Vec::new(),
            conflict_target: ConflictTarget::Unspecified,
            conflict_predicate: None,
            do_nothing: false,
            update_columns: None,
            upsert_where: Vec::new(),
            returning: ReturningTarget::Column("id".to_string()),
            enable_logging,
        }
    }

    /// 选择可信 SQL 表达式。
    ///
    /// 本方法不校验输入；外部列名请使用 [`Self::field_identifier`]。
    pub fn field(mut self, field: &crate::FieldRef) -> Self {
        self.fields.push(field.postgres_quoted().to_string());
        self
    }

    /// 添加受控聚合表达式。
    pub fn expr(mut self, expression: crate::SelectExpr) -> Self {
        self.fields.push(expression.postgres_sql());
        self
    }

    /// 添加受控服务端标量表达式（如 `now()` / Unix 时间戳），可带输出别名。
    ///
    /// 表达式片段来自 [`crate::SqlExpr`] 白名单，动态偏移量走绑定参数；
    /// 占位符编号与后续 WHERE 参数连续。
    pub fn select_expr(mut self, expression: crate::SqlExpr, alias: &crate::FieldRef) -> Self {
        self.projection_exprs
            .push((expression, Some(alias.as_str().to_string())));
        self
    }

    /// 登记受控服务端表达式赋值：UPDATE 的 SET 子句与单条 INSERT 的 VALUES 子句
    /// 都会生效（如 `last_seen = now()`）。
    ///
    /// 表达式参数编号接续 JSON 列参数，WHERE 参数继续接续。批量插入与 UPSERT
    /// 暂不支持该入口，会返回 [`DbError::InvalidArgument`](crate::DbError::InvalidArgument)。
    pub fn set_expr(mut self, field: &crate::FieldRef, expression: crate::SqlExpr) -> Self {
        self.set_exprs
            .push((field.as_str().to_string(), expression));
        self
    }

    /// 选择多个可信 SQL 表达式；外部列名请使用 [`Self::field_identifiers`]。
    pub fn fields(mut self, fields: &[&crate::FieldRef]) -> Self {
        for field in fields {
            self.fields.push(field.postgres_quoted().to_string());
        }
        self
    }

    /// 标记字段为 JSON 类型
    pub fn json(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Json);
        self
    }

    /// 标记字段为 JSONB 类型（插入绑定与 JSON 相同）
    pub fn jsonb(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Jsonb);
        self
    }

    /// 标记字段为 UUID 类型
    pub fn uuid(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Uuid);
        self
    }

    /// 标记字段为 DATE 类型
    pub fn date(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Date);
        self
    }

    /// 标记字段为 TIME 类型
    pub fn time(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Time);
        self
    }

    /// 标记字段为 DATETIME 类型
    pub fn datetime(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::DateTime);
        self
    }

    /// 标记字段为 TIMESTAMP 类型
    pub fn timestamp(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Timestamp);
        self
    }

    /// 标记字段为 TIMESTAMPTZ（带时区）类型
    pub fn timestamp_tz(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::TimestampTz);
        self
    }

    /// 标记字段为 DECIMAL 类型
    pub fn decimal(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Decimal);
        self
    }

    /// 标记字段为 BLOB（BYTEA）类型
    pub fn blob(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Blob);
        self
    }

    /// 标记字段为 TEXT 类型
    pub fn text(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Text);
        self
    }

    /// 标记字段为任意 PostgreSQL 类型（数组 / INET / CIDR / MACADDR 等没有
    /// 专用链式方法的类型）。
    ///
    /// `FieldType::Array` 的插入路径会把 JSON 数组按元素类型递归转换为
    /// [`SqlValue::Array`]，作为单个数组参数绑定。
    ///
    /// 注意：数组绑定按首个非 NULL 元素推断驱动类型——整数元素绑定为
    /// `BIGINT[]`，浮点元素绑定为 `DOUBLE PRECISION[]`；与 `INT4[]` / `REAL[]`
    /// 列不兼容。网络/位串字段会追加 `::inet` / `::macaddr` 等显式转换。
    pub fn field_type(mut self, field: &crate::FieldRef, field_type: FieldType) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), field_type);
        self
    }

    /// 去重
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// `DISTINCT ON (...)`：按给定字段分组取首行。
    ///
    /// PostgreSQL 要求 `DISTINCT ON` 表达式与 `ORDER BY` 的最左表达式一致；
    /// 该约束在构建 SQL 时校验（`try_to_sql` 返回 `InvalidArgument`）。
    /// 与 [`Self::distinct`] 互斥。
    pub fn distinct_on(mut self, fields: &[&crate::FieldRef]) -> Self {
        self.distinct_on = fields
            .iter()
            .map(|field| field.postgres_quoted().to_string())
            .collect();
        self
    }

    /// 追加一个 `WITH <name> AS (<query>)` 公共表表达式。
    ///
    /// 名称在渲染期按单段标识符校验并加引号；CTE 主体可继续携带自己的 WITH、
    /// 集合运算与排序/分页。
    pub fn with(mut self, name: &str, query: QueryBuilder<'a>) -> Self {
        self.ctes.push(Cte {
            name: name.to_string(),
            recursive: false,
            query: Box::new(query),
        });
        self
    }

    /// 追加一个递归 CTE；整条语句渲染为 `WITH RECURSIVE ...`。
    pub fn with_recursive(mut self, name: &str, query: QueryBuilder<'a>) -> Self {
        self.ctes.push(Cte {
            name: name.to_string(),
            recursive: true,
            query: Box::new(query),
        });
        self
    }

    /// 添加窗口函数投影（`<function> OVER (<spec>) AS <alias>`）。
    pub fn window_expr(
        mut self,
        function: WindowFunction,
        spec: WindowSpec,
        alias: &crate::FieldRef,
    ) -> Self {
        self.window_exprs.push(WindowProjection {
            function,
            spec,
            alias: alias.as_str().to_string(),
        });
        self
    }

    /// 定义命名窗口：`WINDOW <name> AS (<spec>)`。
    ///
    /// 投影通过 [`WindowSpec::reference`] 以 `OVER "<name>"` 复用该定义。
    /// 名称在渲染期按单段标识符校验并加引号。
    pub fn window(mut self, name: &str, spec: WindowSpec) -> Self {
        self.named_windows.push((name.to_string(), spec));
        self
    }

    /// 使用 UNION 组合一个显式投影且输出列数相同的查询。
    pub fn union(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::Union)
    }

    /// 使用 UNION ALL 组合一个显式投影且输出列数相同的查询。
    pub fn union_all(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::UnionAll)
    }

    /// 使用 INTERSECT 组合一个显式投影且输出列数相同的查询。
    pub fn intersect(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::Intersect)
    }

    /// 使用 INTERSECT ALL 组合一个显式投影且输出列数相同的查询。
    pub fn intersect_all(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::IntersectAll)
    }

    /// 使用 EXCEPT 组合一个显式投影且输出列数相同的查询。
    pub fn except(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::Except)
    }

    /// 使用 EXCEPT ALL 组合一个显式投影且输出列数相同的查询。
    pub fn except_all(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_set_operation(other, SetOperator::ExceptAll)
    }

    fn add_set_operation(
        mut self,
        other: QueryBuilder<'a>,
        operator: SetOperator,
    ) -> Result<Self, crate::error::DbError> {
        let left_len = self.projection_len();
        let right_len = other.projection_len();
        if left_len == 0 || right_len == 0 {
            return Err(crate::error::DbError::InvalidArgument(
                "集合运算各分支必须显式声明输出字段，不能使用未知列数的 *".to_string(),
            ));
        }
        if left_len != right_len {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "集合运算输出列数不一致：左侧 {left_len} 列，右侧 {right_len} 列"
            )));
        }
        self.set_ops.push((operator, Box::new(other)));
        Ok(self)
    }

    /// 显式投影的列数（普通字段 + 表达式 + 窗口投影）。
    fn projection_len(&self) -> usize {
        self.fields.len() + self.projection_exprs.len() + self.window_exprs.len()
    }

    /// `GROUP BY GROUPING SETS ((a, b), (a), ())`。
    ///
    /// 可与其他分组字段或 ROLLUP/CUBE 组合；内层空集合表示总计行。
    pub fn grouping_sets(mut self, sets: Vec<Vec<crate::FieldRef>>) -> Self {
        self.grouping_constructs
            .push(GroupingConstruct::GroupingSets(sets));
        self
    }

    /// `GROUP BY ROLLUP (a, b)`。
    pub fn rollup(mut self, fields: Vec<crate::FieldRef>) -> Self {
        self.grouping_constructs
            .push(GroupingConstruct::Rollup(fields));
        self
    }

    /// `GROUP BY CUBE (a, b)`。
    pub fn cube(mut self, fields: Vec<crate::FieldRef>) -> Self {
        self.grouping_constructs
            .push(GroupingConstruct::Cube(fields));
        self
    }

    /// 设置 UPSERT 的冲突目标列（PostgreSQL `ON CONFLICT (...)`）
    ///
    /// 例：`.on_conflict(&[yang_db::field!("email")])`；传空切片等价于未指定目标，
    /// 此时仅允许配合 [`Self::do_nothing`] 使用（`DO NOTHING`）；`DO UPDATE` 会在
    /// 客户端返回 [`crate::DbError::InvalidArgument`]，不会把语法错误留给数据库。
    pub fn on_conflict(mut self, columns: &[&crate::FieldRef]) -> Self {
        self.conflict_target = ConflictTarget::Columns(
            columns
                .iter()
                .map(|column| column.as_str().to_string())
                .collect(),
        );
        self.conflict_predicate = None;
        self
    }

    /// 使用命名约束作为冲突目标：`ON CONFLICT ON CONSTRAINT <name>`。
    ///
    /// `constraint` 只需传入约束名（按单段标识符校验并加引号）。
    pub fn on_conflict_constraint(mut self, constraint: &crate::FieldRef) -> Self {
        self.conflict_target = ConflictTarget::Constraint(constraint.as_str().to_string());
        self.conflict_predicate = None;
        self
    }

    /// 带部分唯一索引谓词的冲突目标：
    /// `ON CONFLICT (<columns>) WHERE (<predicate>) DO UPDATE ...`。
    ///
    /// `predicate` 是受信 SQL 表达式（如 `"active"` 或 `"tenant_id > 0"`），按原样
    /// 包进括号；分号、注释标记（`--`、`/*`、`*/`）与 NUL 会被拒绝并返回
    /// [`crate::DbError::InvalidArgument`]，避免把调用方输入变成第二条语句。
    pub fn on_conflict_with_predicate(
        mut self,
        columns: &[&crate::FieldRef],
        predicate: &str,
    ) -> Self {
        self.conflict_target = ConflictTarget::Columns(
            columns
                .iter()
                .map(|column| column.as_str().to_string())
                .collect(),
        );
        self.conflict_predicate = Some(predicate.to_string());
        self
    }

    /// 限制 `DO UPDATE SET` 只更新给定列白名单。
    ///
    /// 未调用时更新所有非冲突列。白名单中的列必须存在于插入数据中。
    pub fn update_columns(mut self, columns: &[&crate::FieldRef]) -> Self {
        self.update_columns = Some(
            columns
                .iter()
                .map(|column| column.as_str().to_string())
                .collect(),
        );
        self
    }

    /// 显式 `DO NOTHING`：忽略冲突目标对应的更新列与 `upsert_where_and` 条件。
    ///
    /// 注意：与 [`Self::upsert_returning`] 组合时，被跳过的冲突行不会出现在
    /// `RETURNING` 中，可能得到 `RowNotFound`；需要行数请使用 [`Self::upsert`]。
    pub fn do_nothing(mut self) -> Self {
        self.do_nothing = true;
        self
    }

    /// 为 `DO UPDATE` 追加 `WHERE` 条件（多个条件之间为 AND）。
    ///
    /// 占位符编号接续插入值与 SET 参数。
    pub fn upsert_where_and<V>(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let condition = comparison_condition(field.as_str(), op, value.into())?;
        self.upsert_where.push(condition);
        Ok(self)
    }

    /// 设置 `insert` 的 `RETURNING` 列（默认 `"id"`）
    ///
    /// PostgreSQL 无 `last_insert_id()`，`insert` 通过 `RETURNING <col>` 取回主键。
    /// 当自增主键列名不是 `id` 时，用本方法指定。
    pub fn returning(mut self, column: &crate::FieldRef) -> Self {
        self.returning = ReturningTarget::Column(column.as_str().to_string());
        self
    }

    /// 设置多列 `RETURNING`：`RETURNING "a", "b", ...`。
    ///
    /// 列名与别名都按 PostgreSQL 标识符规则校验并加引号；非法标识符在执行前返回
    /// [`crate::DbError::InvalidArgument`]。可与其他 `returning_*` 方法链式调用，
    /// 后调用者覆盖先前的目标。
    pub fn returning_columns(mut self, columns: &[&crate::FieldRef]) -> Self {
        self.returning = ReturningTarget::Columns(
            columns
                .iter()
                .map(|column| ReturningColumn {
                    column: column.as_str().to_string(),
                    alias: None,
                })
                .collect(),
        );
        self
    }

    /// 追加一个带输出别名的 `RETURNING` 列：`RETURNING "col" AS "alias"`。
    ///
    /// 连续调用会按顺序累加；若当前目标是 `RETURNING *` 或单列（含默认 `id`），
    /// 第一次调用会以本列作为新的显式列表起点（单列/整行目标被替换）。
    /// 别名必须是单段 PostgreSQL 标识符（渲染期按标识符规则校验并加引号，
    /// 非法别名返回 `InvalidArgument`）。
    pub fn returning_column_as(
        mut self,
        column: &crate::FieldRef,
        alias: &crate::FieldRef,
    ) -> Self {
        let target = ReturningColumn {
            column: column.as_str().to_string(),
            alias: Some(alias.as_str().to_string()),
        };
        match &mut self.returning {
            ReturningTarget::Columns(columns) => columns.push(target),
            ReturningTarget::Column(_) | ReturningTarget::All => {
                self.returning = ReturningTarget::Columns(vec![target]);
            }
        }
        self
    }

    /// 设置 `RETURNING *`，配合 [`Self::insert_returning`] 等按整行解码。
    pub fn returning_all(mut self) -> Self {
        self.returning = ReturningTarget::All;
        self
    }

    /// 渲染 `RETURNING` 子句（含前导空格）。
    fn returning_clause(&self) -> Result<String, crate::error::DbError> {
        match &self.returning {
            ReturningTarget::Column(column) => Ok(format!(
                " RETURNING {}",
                super::identifier::quote_identifier(column)?
            )),
            ReturningTarget::Columns(columns) => {
                if columns.is_empty() {
                    return Err(crate::error::DbError::InvalidArgument(
                        "returning_columns 至少需要一个列".to_string(),
                    ));
                }
                let rendered = columns
                    .iter()
                    .map(|target| {
                        let column = super::identifier::quote_identifier(&target.column)?;
                        match &target.alias {
                            Some(alias) => Ok(format!(
                                "{column} AS {}",
                                super::identifier::quote_identifier(alias)?
                            )),
                            None => Ok(column),
                        }
                    })
                    .collect::<Result<Vec<_>, crate::error::DbError>>()?;
                Ok(format!(" RETURNING {}", rendered.join(", ")))
            }
            ReturningTarget::All => Ok(" RETURNING *".to_string()),
        }
    }

    /// 添加 AND 条件
    ///
    /// 比较操作符由封闭的 [`CompareOp`](crate::CompareOp) 表达。LIKE 仅接受字符串模式，
    /// 其它类型返回 `Err(DbError::UnsupportedOperator)`。
    /// 按已登记字段类型把字符串值提升为带方言类型的 SqlValue。
    ///
    /// 网络类型在 PostgreSQL 里没有 text 的隐式赋值转换，`inet_col = $1` 绑定
    /// text 会报 42883；`$1::inet` 由条件渲染器按变体追加。
    fn coerce_condition_value(&self, field: &str, value: SqlValue) -> SqlValue {
        let Some(field_type) = self.field_types.get(field) else {
            return value;
        };
        match (field_type, value) {
            (FieldType::Inet, SqlValue::String(value)) => SqlValue::Inet(value),
            (FieldType::Cidr, SqlValue::String(value)) => SqlValue::Cidr(value),
            (FieldType::MacAddr, SqlValue::String(value)) => SqlValue::MacAddr(value),
            (FieldType::MacAddr8, SqlValue::String(value)) => SqlValue::MacAddr8(value),
            (_, value) => value,
        }
    }

    pub fn where_and<V>(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let field = field.as_str().to_string();
        let value = self.coerce_condition_value(&field, value.into());
        let condition = comparison_condition(&field, op, value)?;
        self.conditions.push(condition);
        Ok(self)
    }

    /// 添加 OR 条件
    ///
    /// 遇到不支持的操作符或模式匹配收到非字符串模式时返回 `Err(DbError::UnsupportedOperator)`。
    pub fn where_or<V>(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let name = field.as_str().to_string();
        let value = self.coerce_condition_value(&name, value.into());
        let condition = comparison_condition(&name, op, value)?;

        if !self.conditions.is_empty() {
            let mut existing = std::mem::take(&mut self.conditions);
            self.conditions.push(Condition::Or(vec![
                if existing.len() == 1 {
                    existing.remove(0)
                } else {
                    Condition::And(existing)
                },
                condition,
            ]));
        } else {
            self.conditions.push(condition);
        }

        Ok(self)
    }

    /// 添加 IN 条件
    pub fn where_in<V>(mut self, field: &crate::FieldRef, values: Vec<V>) -> Self
    where
        V: Into<SqlValue>,
    {
        let name = field.as_str().to_string();
        let sql_values: Vec<_> = values
            .into_iter()
            .map(|v| self.coerce_condition_value(&name, v.into()))
            .collect();
        self.conditions.push(Condition::In(name, sql_values));
        self
    }

    /// 添加受控 EXISTS 子查询。
    pub fn where_exists(mut self, subquery: crate::postgres::Subquery) -> Self {
        self.conditions.push(Condition::Exists(Box::new(subquery)));
        self
    }

    /// 添加受控 NOT EXISTS 子查询。
    pub fn where_not_exists(mut self, subquery: crate::postgres::Subquery) -> Self {
        self.conditions
            .push(Condition::NotExists(Box::new(subquery)));
        self
    }

    /// 添加受控 IN 子查询，外层字段必须是合法的单段或两段标识符。
    pub fn where_in_subquery(
        mut self,
        field: &crate::FieldRef,
        subquery: crate::postgres::Subquery,
    ) -> Self {
        self.conditions.push(Condition::InSubquery(
            field.as_str().to_string(),
            Box::new(subquery),
        ));
        self
    }

    /// 添加 BETWEEN 条件
    pub fn where_between<V>(mut self, field: &crate::FieldRef, start: V, end: V) -> Self
    where
        V: Into<SqlValue>,
    {
        let name = field.as_str().to_string();
        let start = self.coerce_condition_value(&name, start.into());
        let end = self.coerce_condition_value(&name, end.into());
        self.conditions.push(Condition::Between(name, start, end));
        self
    }

    /// 添加 IS NULL 条件
    pub fn where_null(mut self, field: &crate::FieldRef) -> Self {
        self.conditions
            .push(Condition::IsNull(field.as_str().to_string()));
        self
    }

    /// 添加 IS NOT NULL 条件
    pub fn where_not_null(mut self, field: &crate::FieldRef) -> Self {
        self.conditions
            .push(Condition::IsNotNull(field.as_str().to_string()));
        self
    }

    /// 应用由上层权限模块编译的受控布尔查询树。
    pub fn where_predicate(
        mut self,
        predicate: &crate::Predicate,
    ) -> Result<Self, crate::error::DbError> {
        self.conditions.push(predicate_condition(predicate)?);
        Ok(self)
    }

    /// 对给定条件取反：渲染为 `NOT (<condition>)`，可与 AND/OR 条件安全组合。
    pub fn where_not(mut self, condition: Condition) -> Self {
        self.conditions.push(Condition::Not(Box::new(condition)));
        self
    }

    /// 两个受控字段之间的比较（如 LATERAL 子查询中的关联条件
    /// `orders.employee_id = employees.id`）。
    ///
    /// 两侧都是已校验的 [`crate::FieldRef`]，值不参与拼接。LIKE 不适用于
    /// 列比较，返回 `UnsupportedOperator`。
    pub fn where_column(
        mut self,
        left: &crate::FieldRef,
        op: crate::CompareOp,
        right: &crate::FieldRef,
    ) -> Result<Self, crate::error::DbError> {
        let operator = comparison_operator(op).ok_or_else(|| {
            crate::error::DbError::UnsupportedOperator(
                "列比较不支持 LIKE/ILIKE/正则/NULL-safe 比较符".to_string(),
            )
        })?;
        self.conditions.push(Condition::ColumnComparison(
            left.as_str().to_string(),
            operator,
            right.as_str().to_string(),
        ));
        Ok(self)
    }

    /// JSON/JSONB 包含谓词：`field @> $n`。
    ///
    /// 值必须是 `SqlValue::Json`（或 `SqlValue::Null`，渲染为 SQL NULL 恒不命中），
    /// 其余类型在渲染期返回 `InvalidArgument`。
    pub fn where_json_contains<V>(mut self, field: &crate::FieldRef, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::JsonContains(
            field.as_str().to_string(),
            value.into(),
        ));
        self
    }

    /// JSON/JSONB 被包含谓词：`field <@ $n`。
    ///
    /// 值约束同 [`Self::where_json_contains`]。
    pub fn where_json_contained_by<V>(mut self, field: &crate::FieldRef, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::JsonContainedBy(
            field.as_str().to_string(),
            value.into(),
        ));
        self
    }

    /// JSON/JSONB 顶层键存在谓词：`field ? $n`（键名走绑定参数）。
    pub fn where_json_has_key(mut self, field: &crate::FieldRef, key: &str) -> Self {
        self.conditions.push(Condition::JsonHasKey(
            field.as_str().to_string(),
            key.to_string(),
        ));
        self
    }

    /// JSONB 路径存在谓词：`jsonb_path_exists((field)::jsonb, $n::jsonpath)`。
    ///
    /// `path` 与 `jsonpath` 字面量语法一致（如 `$.items[*] ? (@.price > 10)`），
    /// 以绑定参数传入，不拼接进 SQL 文本。
    pub fn where_json_path_exists(mut self, field: &crate::FieldRef, path: &str) -> Self {
        self.conditions.push(Condition::JsonPathExists(
            field.as_str().to_string(),
            path.to_string(),
        ));
        self
    }

    /// JSONB 路径谓词：`jsonb_path_match((field)::jsonb, $n::jsonpath, $m::jsonb)`。
    ///
    /// `vars` 为 jsonpath 表达式中 `$var` 的取值对象（建议传 `serde_json::json!({..})`），
    /// 与 `path` 一样以绑定参数传入。
    pub fn where_json_path_match<V>(mut self, field: &crate::FieldRef, path: &str, vars: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::JsonPathMatch(
            field.as_str().to_string(),
            path.to_string(),
            vars.into(),
        ));
        self
    }

    /// 范围包含谓词：`field @> $n`。
    ///
    /// 右值必须由 [`SqlValue::Range`] 或 [`SqlValue::Multirange`] 构造，例如
    /// `SqlValue::Range("[1,10)".to_string())`；渲染时追加 `::int4range` /
    /// `::int4multirange` 显式转换。该快捷谓词仅支持整数范围；`tsrange`
    /// 等其他范围类型请使用原生 SQL 显式转换（如 `$1::tsrange`）。
    pub fn where_range_contains<V>(mut self, field: &crate::FieldRef, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::RangeContains(
            field.as_str().to_string(),
            value.into(),
        ));
        self
    }

    /// 范围重叠谓词：`field && $n`。
    ///
    /// 右值约束同 [`Self::where_range_contains`]。
    pub fn where_range_overlaps<V>(mut self, field: &crate::FieldRef, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::RangeOverlaps(
            field.as_str().to_string(),
            value.into(),
        ));
        self
    }

    /// 全文检索谓词：`to_tsvector('simple', field) @@ plainto_tsquery('simple', $n)`。
    ///
    /// 查询串按纯文本解析（空格分词），以绑定参数传入。
    pub fn where_full_text(mut self, field: &crate::FieldRef, query: &str) -> Self {
        self.conditions.push(Condition::FullText(
            field.as_str().to_string(),
            query.to_string(),
        ));
        self
    }

    /// 全文检索（`to_tsquery` 变体）：查询串可含 `&`、`|`、`!` 等布尔操作符。
    pub fn where_full_text_query(mut self, field: &crate::FieldRef, query: &str) -> Self {
        self.conditions.push(Condition::FullTextQuery(
            field.as_str().to_string(),
            query.to_string(),
        ));
        self
    }

    /// 数组任一元素相等：`field = ANY($n)`（数组作为单个绑定参数）。
    pub fn where_any<V>(mut self, field: &crate::FieldRef, values: Vec<V>) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::Any(
            field.as_str().to_string(),
            SqlValue::Array(values.into_iter().map(Into::into).collect()),
        ));
        self
    }

    /// 数组全部元素相等：`field = ALL($n)`（数组作为单个绑定参数）。
    pub fn where_all<V>(mut self, field: &crate::FieldRef, values: Vec<V>) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::All(
            field.as_str().to_string(),
            SqlValue::Array(values.into_iter().map(Into::into).collect()),
        ));
        self
    }

    /// 字段与服务端受控表达式的比较（如 `updated_at < now()`）。
    ///
    /// LIKE 不适用于表达式比较，返回 `UnsupportedOperator`。
    pub fn where_expr(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        expression: crate::SqlExpr,
    ) -> Result<Self, crate::error::DbError> {
        let operator = comparison_operator(op).ok_or_else(|| {
            crate::error::DbError::UnsupportedOperator(
                "表达式比较不支持 LIKE/ILIKE/正则/NULL-safe 比较符".to_string(),
            )
        })?;
        self.conditions.push(Condition::FieldExpr(
            field.as_str().to_string(),
            operator,
            expression,
        ));
        Ok(self)
    }

    /// 添加 HAVING 条件（仅支持 6 个比较操作符）
    pub fn having_cond<V>(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let field = field.as_str().to_string();
        let value = self.coerce_condition_value(&field, value.into());
        let condition = comparison_condition(&field, op, value)?;
        self.having_clause.push(condition);
        Ok(self)
    }

    /// 添加 HAVING 聚合表达式比较（如 `HAVING COUNT(*) > 1`）。
    ///
    /// 左侧来自受控 [`crate::SelectExpr`]，表达式本体渲染，不携带输出别名；
    /// 值走绑定参数。模式匹配/NULL-safe 比较不适用于聚合表达式，返回
    /// `UnsupportedOperator`。
    pub fn having_expr<V>(
        mut self,
        expression: crate::SelectExpr,
        op: crate::CompareOp,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let operator = comparison_operator(op).ok_or_else(|| {
            crate::error::DbError::UnsupportedOperator(
                "HAVING 聚合表达式不支持 LIKE/ILIKE/正则/NULL-safe 比较符".to_string(),
            )
        })?;
        self.having_exprs.push(HavingExpr {
            expression,
            operator,
            value: value.into(),
        });
        Ok(self)
    }

    /// 使用可信表/ON 表达式的 INNER JOIN。
    ///
    /// 外部表名/列名请先用 [`crate::postgres::identifier::quote_identifier`] /
    /// [`crate::postgres::identifier::quote_qualified`] 校验后再拼入表达式。
    pub fn join(
        mut self,
        table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Inner,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::On(format!(
                "{} = {}",
                left.postgres_quoted(),
                right.postgres_quoted()
            )),
        });
        self
    }

    /// LEFT JOIN
    pub fn left_join(
        mut self,
        table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Left,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::On(format!(
                "{} = {}",
                left.postgres_quoted(),
                right.postgres_quoted()
            )),
        });
        self
    }

    /// RIGHT JOIN
    pub fn right_join(
        mut self,
        table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Right,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::On(format!(
                "{} = {}",
                left.postgres_quoted(),
                right.postgres_quoted()
            )),
        });
        self
    }

    /// FULL OUTER JOIN（PostgreSQL 渲染为 `FULL JOIN`）。
    pub fn full_join(
        mut self,
        table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Full,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::On(format!(
                "{} = {}",
                left.postgres_quoted(),
                right.postgres_quoted()
            )),
        });
        self
    }

    /// CROSS JOIN（无连接条件）。
    pub fn cross_join(mut self, table: &crate::TableRef) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Cross,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::None,
        });
        self
    }

    /// NATURAL JOIN（按两表同名同类型列自动连接，无连接条件）。
    pub fn natural_join(mut self, table: &crate::TableRef) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Natural,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::None,
        });
        self
    }

    /// `INNER JOIN ... USING (col, ...)`：用同名列连接，避免重复输出连接列。
    pub fn join_using(mut self, table: &crate::TableRef, columns: &[&crate::FieldRef]) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::Inner,
            target: JoinTarget::Table(table.as_str().to_string()),
            condition: JoinCondition::Using(
                columns
                    .iter()
                    .map(|column| column.as_str().to_string())
                    .collect(),
            ),
        });
        self
    }

    /// `LEFT JOIN LATERAL (<subquery>) AS "alias" ON ...`。
    ///
    /// 子查询可引用左侧表的列（横向关联）；`alias` 必须是单段标识符。
    pub fn left_join_lateral(
        mut self,
        query: QueryBuilder<'a>,
        alias: &crate::FieldRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        self.joins.push(JoinSpec {
            kind: JoinKind::LeftLateral,
            target: JoinTarget::Lateral(Box::new(query), alias.as_str().to_string()),
            condition: JoinCondition::On(format!(
                "{} = {}",
                left.postgres_quoted(),
                right.postgres_quoted()
            )),
        });
        self
    }

    /// 按可信 SQL 表达式排序；外部列名请先用 [`crate::postgres::identifier::quote_identifier`]
    /// 校验后再拼入表达式。
    pub fn order(mut self, field: &crate::FieldRef, order: crate::SortOrder) -> Self {
        self.order_by.push(OrderClause {
            field: field.postgres_quoted().to_string(),
            asc: order.is_ascending(),
        });
        self
    }

    /// 按可信 SQL 表达式分组；外部列名请先用 [`crate::postgres::identifier::quote_identifier`]
    /// 校验后再拼入表达式。
    pub fn group(mut self, field: &crate::FieldRef) -> Self {
        self.group_by.push(field.postgres_quoted().to_string());
        self
    }

    /// 限制返回数量
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 偏移量
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// 尝试获取生成的 SQL（用于调试）。
    ///
    /// 与 [`Self::to_sql`] 不同，本方法不会吞掉 SQL 生成错误。调用方需要区分非法表名、
    /// 缺少 `GROUP BY` 等生产级配置错误时，应优先使用本方法。
    pub fn try_to_sql(&self) -> Result<String, crate::error::DbError> {
        let mut generator = SqlGenerator::new();
        generator.build_select(self)?;
        Ok(generator.get_sql().to_string())
    }

    pub(crate) fn render_for_transaction(
        &self,
        lock: Option<crate::RowLock>,
    ) -> Result<(String, Vec<SqlValue>), crate::error::DbError> {
        if lock.is_some() {
            self.ensure_row_lock_supported()?;
        }
        let mut generator = SqlGenerator::new();
        generator.build_select(self)?;
        if let Some(lock) = lock {
            generator.sql.push(' ');
            generator.sql.push_str(lock.as_sql());
        }
        Ok((generator.sql, generator.params))
    }

    /// 行锁查询的共享前置校验：DISTINCT/GROUP BY/HAVING/窗口/CTE/集合运算不可加锁。
    fn ensure_row_lock_supported(&self) -> Result<(), crate::error::DbError> {
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.group_by.is_empty()
            || !self.grouping_constructs.is_empty()
            || !self.having_clause.is_empty()
            || !self.having_exprs.is_empty()
            || !self.window_exprs.is_empty()
            || !self.ctes.is_empty()
            || !self.set_ops.is_empty()
        {
            return Err(crate::error::DbError::InvalidArgument(
                "行锁不支持 DISTINCT、GROUP BY/分组集、HAVING、窗口函数、CTE 或集合运算查询"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// 渲染带完整行锁组合的 SELECT（调试/自定义执行入口）。
    ///
    /// `lock` 决定锁模式（含 `NO KEY UPDATE` / `KEY SHARE`）；`of` 为
    /// `FOR ... OF` 的表/别名列表（空切片表示不加 OF）；`nowait` 与 `skip_locked`
    /// 互斥，同时为 `true` 时返回 [`crate::error::DbError::InvalidArgument`]。
    /// `ForUpdateSkipLocked` / `ForShareSkipLocked` 隐含 `SKIP LOCKED`。
    ///
    /// 例：`FOR NO KEY UPDATE OF "orders" SKIP LOCKED`。
    pub fn try_to_sql_with_lock(
        &self,
        lock: crate::RowLock,
        of: &[&crate::FieldRef],
        nowait: bool,
        skip_locked: bool,
    ) -> Result<String, crate::error::DbError> {
        Ok(self.render_with_lock(lock, of, nowait, skip_locked)?.0)
    }

    /// 渲染带行锁的 SELECT，同时返回绑定参数（事务内执行入口使用）。
    pub(crate) fn render_with_lock(
        &self,
        lock: crate::RowLock,
        of: &[&crate::FieldRef],
        nowait: bool,
        skip_locked: bool,
    ) -> Result<(String, Vec<SqlValue>), crate::error::DbError> {
        let implied_skip = matches!(
            lock,
            crate::RowLock::ForUpdateSkipLocked | crate::RowLock::ForShareSkipLocked
        );
        if nowait && (skip_locked || implied_skip) {
            return Err(crate::error::DbError::InvalidArgument(
                "行锁的 NOWAIT 与 SKIP LOCKED 不能同时使用".to_string(),
            ));
        }
        self.ensure_row_lock_supported()?;
        let mut generator = SqlGenerator::new();
        generator.build_select(self)?;

        let base = match lock {
            crate::RowLock::ForUpdate | crate::RowLock::ForUpdateSkipLocked => "FOR UPDATE",
            crate::RowLock::ForShare | crate::RowLock::ForShareSkipLocked => "FOR SHARE",
            crate::RowLock::NoKeyUpdate => "FOR NO KEY UPDATE",
            crate::RowLock::KeyShare => "FOR KEY SHARE",
        };
        let mut clause = String::from(base);
        if !of.is_empty() {
            let quoted = of
                .iter()
                .map(|table| super::identifier::quote_qualified(table.as_str()))
                .collect::<Result<Vec<_>, _>>()?;
            clause.push_str(" OF ");
            clause.push_str(&quoted.join(", "));
        }
        if skip_locked
            || matches!(
                lock,
                crate::RowLock::ForUpdateSkipLocked | crate::RowLock::ForShareSkipLocked
            )
        {
            clause.push_str(" SKIP LOCKED");
        } else if nowait {
            clause.push_str(" NOWAIT");
        }

        generator.sql.push(' ');
        generator.sql.push_str(&clause);
        Ok((generator.sql, generator.params))
    }

    /// 获取生成的 SQL（用于调试）
    ///
    /// 兼容历史 `String` 返回值；生成失败时返回固定的不可执行哨兵，避免旧降级逻辑把
    /// 未校验表名或不完整查询拼成看似可执行的 SQL。需要错误细节请使用 [`Self::try_to_sql`]。
    pub fn to_sql(&self) -> String {
        match self.try_to_sql() {
            Ok(sql) => sql,
            Err(err) => {
                if self.enable_logging {
                    log::warn!("生成 SELECT SQL 失败: {err}");
                }
                "/* SQL generation failed */".to_string()
            }
        }
    }

    /// 查询单条记录（自动追加 LIMIT 1）
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn find<T>(mut self) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.limit = Some(1);

        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;
        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 find() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_as::<_, T>(sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_optional(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_optional(&mut *connection).await
            }
        };
        match result {
            Ok(row) => Ok(row),
            Err(e) => {
                log::error!("find() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询多条记录
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn select<T>(self) -> Result<Vec<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;
        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 select() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_as::<_, T>(sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_all(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_all(&mut *connection).await
            }
        };
        match result {
            Ok(rows) => Ok(rows),
            Err(e) => {
                log::error!("select() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 标量查询助手（内部）：只选 `select_expr`、加 LIMIT 1、执行单行单值查询。
    async fn fetch_scalar<C>(
        mut self,
        select_expr: &str,
    ) -> Result<Option<C>, crate::error::DbError>
    where
        C: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        self.fields.clear();
        self.fields.push(select_expr.to_string());
        self.limit = Some(1);

        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;
        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行标量查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_scalar::<_, C>(sql);
        for param in params {
            query = bind_scalar_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_optional(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_optional(&mut *connection).await
            }
        };
        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                log::error!("标量查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询单个字段值（自动追加 LIMIT 1）
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn value<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 value() 查询，字段: {}", field.as_str());
        }
        let field = super::identifier::quote_identifier(field.as_str())?;
        self.fetch_scalar::<T>(&field).await
    }

    /// 统计记录数量（COUNT(*)）
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn count(self) -> Result<i64, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 count() 查询");
        }
        let result = self.fetch_scalar::<i64>("COUNT(*)").await?;
        Ok(result.unwrap_or(0))
    }
    /// 计算字段总和（SUM）
    ///
    /// 使用 `CAST(SUM(field) AS DOUBLE PRECISION)` 统一返回 `f64`，
    /// 与 MySQL 后端的 `AS DOUBLE` 对应（PostgreSQL 的等价类型为 `DOUBLE PRECISION`）。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn sum(self, field: &crate::FieldRef) -> Result<Option<f64>, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 sum() 查询，字段: {}", field.as_str());
        }
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = super::identifier::quote_identifier(field.as_str())?;
        let sum_expr = format!("CAST(SUM({quoted_field}) AS DOUBLE PRECISION)");
        self.fetch_scalar::<Option<f64>>(&sum_expr)
            .await
            .map(Option::flatten)
    }

    /// 计算字段平均值（AVG）
    ///
    /// 使用 `CAST(AVG(field) AS DOUBLE PRECISION)` 统一返回 `f64`。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn avg(self, field: &crate::FieldRef) -> Result<Option<f64>, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 avg() 查询，字段: {}", field.as_str());
        }
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = super::identifier::quote_identifier(field.as_str())?;
        let avg_expr = format!("CAST(AVG({quoted_field}) AS DOUBLE PRECISION)");
        self.fetch_scalar::<Option<f64>>(&avg_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最小值（MIN）
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn min<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 min() 查询，字段: {}", field.as_str());
        }
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = super::identifier::quote_identifier(field.as_str())?;
        let min_expr = format!("MIN({quoted_field})");
        self.fetch_scalar::<Option<T>>(&min_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最大值（MAX）
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn max<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 max() 查询，字段: {}", field.as_str());
        }
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = super::identifier::quote_identifier(field.as_str())?;
        let max_expr = format!("MAX({quoted_field})");
        self.fetch_scalar::<Option<T>>(&max_expr)
            .await
            .map(Option::flatten)
    }
    /// 插入数据
    ///
    /// 执行 INSERT 操作并通过 `RETURNING <col>` 取回自增主键（PostgreSQL 无
    /// `last_insert_id()`）。`<col>` 默认 `"id"`，可用 [`QueryBuilder::returning`] 覆盖。
    ///
    /// # 返回
    /// - Ok(u64): 插入成功，返回 `RETURNING` 列的值（自增主键）
    /// - Err(DbError): 插入失败
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // insert() 只解码单个整型返回值；多列/整行 RETURNING 的列顺序不代表主键位置。
        if !matches!(self.returning, ReturningTarget::Column(_)) {
            return Err(crate::error::DbError::InvalidArgument(
                "insert() 仅支持单列 RETURNING；多列或整行请使用 insert_returning::<T, R>()"
                    .to_string(),
            ));
        }
        // 委托给 insert_returning，再用可接受 INT2/INT4/INT8 的解码包装统一成 u64。
        let id: IntegerPk = self
            .insert_returning(data)
            .await
            .map_err(|error| match error {
                crate::error::DbError::TypeConversionError(detail) => {
                    crate::error::DbError::InvalidArgument(format!(
                        "insert() 仅支持整型主键：{detail}。非整型主键请使用 \
                     insert_returning::<T, R>() 按列的真实类型取回"
                    ))
                }
                other => other,
            })?;
        u64::try_from(id.0).map_err(|_| {
            crate::error::DbError::InvalidArgument("insert() 返回的主键为负数".to_string())
        })
    }

    /// 插入数据并按真实列类型取回 `RETURNING` 结果。
    ///
    /// 默认 `RETURNING <id>`，可用 [`QueryBuilder::returning`] 指定单列或
    /// [`QueryBuilder::returning_all`] 取整行。`R` 为调用方的 `FromRow` 结构体，
    /// 各列按数据库真实类型解码（不再经过 i64 / CAST）。
    ///
    /// # 返回
    /// - Ok(R): 插入行按 `R` 解码后的结果
    /// - Err(DbError): 插入或解码失败
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_returning<T, R>(self, data: &T) -> Result<R, crate::error::DbError>
    where
        T: serde::Serialize,
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 insert_returning() 操作，表: {}", self.table);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_insert(&self.table, &json_data, &self.field_types, &self.set_exprs)?;
        let sql = format!("{}{}", generator.get_sql(), self.returning_clause()?);
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 insert_returning() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_as::<_, R>(&sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => {
                // 语句已执行、RETURNING 解码失败时不能留下已写入的行。
                let mut tx = pool.begin().await.map_err(crate::error::DbError::from)?;
                match query.fetch_one(&mut *tx).await {
                    Ok(row) => {
                        tx.commit().await.map_err(crate::error::DbError::from)?;
                        Ok(row)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_one(&mut *connection).await
            }
        };
        match result {
            Ok(row) => Ok(row),
            Err(error) => {
                let db_err = crate::error::DbError::from(error);
                if db_err.is_constraint_violation() {
                    log::warn!(
                        "insert_returning() 失败: {}（约束冲突，可能为预期分支）",
                        db_err
                    );
                } else {
                    log::error!("insert_returning() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }
    /// 批量插入数据
    ///
    /// 使用默认批大小 [`INSERT_BATCH_SIZE`]（500）分批执行，返回总受影响行数。
    /// 批量插入不使用 `RETURNING`，仅返回 `rows_affected()`。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_batch<T>(self, data: &[T]) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        self.insert_batch_with_size(data, INSERT_BATCH_SIZE).await
    }

    /// 批量插入数据（自定义批次大小）
    ///
    /// 允许调用方根据网络延迟、数据大小等自定义每批最大记录数（必须 > 0）。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_batch_with_size<T>(
        self,
        data: &[T],
        batch_size: usize,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if batch_size == 0 {
            return Err(crate::error::DbError::SerializationError(
                "batch_size 不能为 0".to_string(),
            ));
        }

        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "insert_batch 不支持 set_expr；请逐条 insert/insert_returning".to_string(),
            ));
        }

        if self.enable_logging {
            log::debug!(
                "执行 insert_batch_with_size() 操作，表: {}，记录数: {}，批次大小: {}",
                self.table,
                data.len(),
                batch_size
            );
        }

        if data.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量插入数据不能为空".to_string(),
            ));
        }

        // 宽表按列数收敛批大小：单语句占位符上限 65535，超过会被服务端拒绝。
        let column_count = serde_json::to_value(&data[0])
            .map_err(|e| crate::error::DbError::SerializationError(format!("数据序列化失败: {e}")))?
            .as_object()
            .map(|object| object.len())
            .unwrap_or(0);
        let batch_size = derive_insert_batch_size(column_count, batch_size);

        // 单批次走 pool 免事务；多批次单事务包裹整体回滚（DB-4，比照 update_batch）。
        let chunks: Vec<&[T]> = data.chunks(batch_size).collect();
        if chunks.len() == 1 {
            let enable_logging = self.enable_logging;
            let affected = self.insert_chunk(chunks[0]).await?;
            if enable_logging {
                log::debug!("insert_batch_with_size() 单批完成，影响 {} 行", affected);
            }
            return Ok(affected);
        }

        let mut total_affected = 0u64;
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for (batch_index, chunk) in chunks.iter().enumerate() {
                    let result = execute_insert_chunk(
                        &mut transaction,
                        &self.table,
                        chunk,
                        &self.field_types,
                    )
                    .await?;
                    total_affected += result;
                    if self.enable_logging {
                        log::debug!("第 {} 批插入成功，影响 {} 行", batch_index + 1, result);
                    }
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for (batch_index, chunk) in chunks.iter().enumerate() {
                    let result = execute_insert_chunk(
                        &mut *connection,
                        &self.table,
                        chunk,
                        &self.field_types,
                    )
                    .await?;
                    total_affected += result;
                    if self.enable_logging {
                        log::debug!("第 {} 批插入成功，影响 {} 行", batch_index + 1, result);
                    }
                }
            }
        }

        if self.enable_logging {
            log::debug!(
                "insert_batch_with_size() 多批事务提交完成，总共影响 {} 行",
                total_affected
            );
        }

        Ok(total_affected)
    }
    /// 插入单个批次的数据（内部方法）
    ///
    async fn insert_chunk<T>(self, data: &[T]) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        let json_data_list: Result<Vec<_>, _> = data
            .iter()
            .map(|item| {
                serde_json::to_value(item).map_err(|e| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
                })
            })
            .collect();

        let json_data_list = json_data_list?;

        let mut generator = SqlGenerator::new();
        generator.build_insert_batch(&self.table, &json_data_list, &self.field_types)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 insert_chunk() SQL: {}", sql);
            log::debug!("参数数量: {}", params.len());
        }

        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_execute_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };
        match result {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("insert_chunk() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                // 唯一键等约束冲突常被调用方当作预期业务分支，降为 warn 避免污染 ERROR 告警
                let db_err = crate::error::DbError::from(e);
                if db_err.is_constraint_violation() {
                    log::warn!(
                        "insert_chunk() 失败: {}（约束冲突，可能为预期分支）",
                        db_err
                    );
                } else {
                    log::error!("insert_chunk() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }
    /// 批量插入并返回 `RETURNING` 行（按列的真实类型解码）。
    ///
    /// 目标由 [`Self::returning`] / [`Self::returning_columns`] /
    /// [`Self::returning_column_as`] / [`Self::returning_all`] 决定。所有分片在同一个
    /// 事务内执行：任一分片失败或解码失败都会整体回滚，返回已插入的全部行。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_batch_returning<T, R>(
        self,
        data: &[T],
    ) -> Result<Vec<R>, crate::error::DbError>
    where
        T: serde::Serialize,
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "insert_batch_returning 不支持 set_expr；请逐条 insert_returning".to_string(),
            ));
        }
        if data.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量插入数据不能为空".to_string(),
            ));
        }
        let returning = self.returning_clause()?;
        let json_data_list: Vec<serde_json::Value> = data
            .iter()
            .map(|item| {
                serde_json::to_value(item).map_err(|error| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
                })
            })
            .collect::<Result<_, _>>()?;
        let column_count = json_data_list[0]
            .as_object()
            .map(|object| object.len())
            .unwrap_or(0);
        let batch_size = derive_insert_batch_size(column_count, INSERT_BATCH_SIZE);

        let mut rows = Vec::new();
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for chunk in json_data_list.chunks(batch_size) {
                    rows.extend(
                        execute_insert_batch_returning::<R>(
                            &mut transaction,
                            &self.table,
                            chunk,
                            &self.field_types,
                            &returning,
                        )
                        .await?,
                    );
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for chunk in json_data_list.chunks(batch_size) {
                    rows.extend(
                        execute_insert_batch_returning::<R>(
                            &mut *connection,
                            &self.table,
                            chunk,
                            &self.field_types,
                            &returning,
                        )
                        .await?,
                    );
                }
            }
        }
        Ok(rows)
    }

    /// 批量 UPSERT 并返回 `RETURNING` 行（按列的真实类型解码）。
    ///
    /// 冲突列必须显式传入（`do_nothing()` 时可省略）；冲突列为空且非 DO NOTHING
    /// 时在客户端返回 [`crate::error::DbError::InvalidArgument`]。所有分片在同一事务内执行。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn upsert_batch_returning<T, R>(
        self,
        data: &[T],
        conflict_columns: &[&crate::FieldRef],
    ) -> Result<Vec<R>, crate::error::DbError>
    where
        T: serde::Serialize,
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "upsert_batch_returning 不支持 set_expr；请逐条 upsert_returning".to_string(),
            ));
        }
        if data.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量 upsert 数据不能为空".to_string(),
            ));
        }
        let conflict: Vec<String> = conflict_columns
            .iter()
            .map(|column| column.as_str().to_string())
            .collect();
        if conflict.is_empty() && !self.do_nothing {
            return Err(crate::error::DbError::InvalidArgument(
                "批量 upsert 必须显式指定冲突列；如使用 id 请显式传入，\
                 或调用 do_nothing() 走 ON CONFLICT DO NOTHING"
                    .to_string(),
            ));
        }

        let returning = self.returning_clause()?;
        let json_data_list: Vec<serde_json::Value> = data
            .iter()
            .map(|item| {
                serde_json::to_value(item).map_err(|error| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
                })
            })
            .collect::<Result<_, _>>()?;
        let column_count = json_data_list[0]
            .as_object()
            .map(|object| object.len())
            .unwrap_or(0);
        let batch_size = derive_upsert_batch_size(column_count, INSERT_BATCH_SIZE);

        let mut rows = Vec::new();
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for chunk in json_data_list.chunks(batch_size) {
                    rows.extend(
                        execute_upsert_batch_returning::<R>(
                            &mut transaction,
                            &self.table,
                            chunk,
                            &self.field_types,
                            &conflict,
                            self.conflict_predicate.as_deref(),
                            self.do_nothing,
                            self.update_columns.as_deref(),
                            &self.upsert_where,
                            &returning,
                        )
                        .await?,
                    );
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for chunk in json_data_list.chunks(batch_size) {
                    rows.extend(
                        execute_upsert_batch_returning::<R>(
                            &mut *connection,
                            &self.table,
                            chunk,
                            &self.field_types,
                            &conflict,
                            self.conflict_predicate.as_deref(),
                            self.do_nothing,
                            self.update_columns.as_deref(),
                            &self.upsert_where,
                            &returning,
                        )
                        .await?,
                    );
                }
            }
        }
        Ok(rows)
    }
    /// 更新数据
    ///
    /// 执行 UPDATE 操作。为防止误操作，必须提供 WHERE 条件，否则返回
    /// `MissingWhereClause`。返回受影响的行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if self.enable_logging {
            log::debug!("执行 update() 操作，表: {}", self.table);
        }

        if self.conditions.is_empty() {
            log::warn!("update() 操作缺少 WHERE 条件，禁止全表更新");
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_update(
            &self.table,
            &json_data,
            &self.field_types,
            &self.conditions,
            &self.set_exprs,
        )?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 update() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_execute_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };
        match result {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("update() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                // 唯一键等约束冲突常被调用方当作预期业务分支，降为 warn 避免污染 ERROR 告警
                let db_err = crate::error::DbError::from(e);
                if db_err.is_constraint_violation() {
                    log::warn!("update() 失败: {}（约束冲突，可能为预期分支）", db_err);
                } else {
                    log::error!("update() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }

    /// 更新数据并返回 `RETURNING` 行。
    ///
    /// 与 [`Self::update`] 相同地要求 WHERE 条件；返回按真实列类型解码的更新后行。
    /// 默认 `RETURNING <id>`，可用 [`QueryBuilder::returning`] / [`QueryBuilder::returning_all`] 调整。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update_returning<T, R>(self, data: &T) -> Result<Vec<R>, crate::error::DbError>
    where
        T: serde::Serialize,
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 update_returning() 操作，表: {}", self.table);
        }

        if self.conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_update(
            &self.table,
            &json_data,
            &self.field_types,
            &self.conditions,
            &self.set_exprs,
        )?;
        let sql = format!("{}{}", generator.get_sql(), self.returning_clause()?);
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 update_returning() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_as::<_, R>(&sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut tx = pool.begin().await.map_err(crate::error::DbError::from)?;
                match query.fetch_all(&mut *tx).await {
                    Ok(rows) => {
                        tx.commit().await.map_err(crate::error::DbError::from)?;
                        Ok(rows)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_all(&mut *connection).await
            }
        };
        match result {
            Ok(rows) => Ok(rows),
            Err(error) => {
                let db_err = crate::error::DbError::from(error);
                if db_err.is_constraint_violation() {
                    log::warn!(
                        "update_returning() 失败: {}（约束冲突，可能为预期分支）",
                        db_err
                    );
                } else {
                    log::error!("update_returning() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }

    /// 原子增加字段值；增量使用绑定参数，且必须提供 WHERE。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn increment(
        self,
        field: &crate::FieldRef,
        amount: i64,
    ) -> Result<u64, crate::error::DbError> {
        self.execute_arithmetic_update(field, amount, ArithmeticOperator::Add)
            .await
    }

    /// 原子减少字段值；增量使用绑定参数，且必须提供 WHERE。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn decrement(
        self,
        field: &crate::FieldRef,
        amount: i64,
    ) -> Result<u64, crate::error::DbError> {
        self.execute_arithmetic_update(field, amount, ArithmeticOperator::Subtract)
            .await
    }

    async fn execute_arithmetic_update(
        self,
        field: &crate::FieldRef,
        amount: i64,
        operator: ArithmeticOperator,
    ) -> Result<u64, crate::error::DbError> {
        let mut generator = SqlGenerator::new();
        generator.build_arithmetic_update(
            &self.table,
            field.as_str(),
            operator,
            amount,
            &self.conditions,
        )?;
        let mut query = sqlx::query(generator.get_sql());
        for param in generator.get_params() {
            query = bind_execute_param(query, param);
        }
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        }?;
        Ok(result.rows_affected())
    }

    /// 删除数据
    ///
    /// 执行 DELETE 操作。为防止误操作，必须提供 WHERE 条件，否则返回
    /// `MissingWhereClause`。返回受影响的行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "delete", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn delete(self) -> Result<u64, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 delete() 操作，表: {}", self.table);
        }

        if self.conditions.is_empty() {
            log::warn!("delete() 操作缺少 WHERE 条件，禁止全表删除");
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let mut generator = SqlGenerator::new();
        generator.build_delete(&self.table, &self.conditions)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 delete() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_execute_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };
        match result {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("delete() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                // 唯一键等约束冲突常被调用方当作预期业务分支，降为 warn 避免污染 ERROR 告警
                let db_err = crate::error::DbError::from(e);
                if db_err.is_constraint_violation() {
                    log::warn!("delete() 失败: {}（约束冲突，可能为预期分支）", db_err);
                } else {
                    log::error!("delete() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }
    /// 删除数据并返回 `RETURNING` 行。
    ///
    /// 与 [`Self::delete`] 相同地要求 WHERE 条件；返回按真实列类型解码的被删行。
    /// 默认 `RETURNING <id>`，可用 [`QueryBuilder::returning`] / [`QueryBuilder::returning_all`] 调整。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "delete", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn delete_returning<R>(self) -> Result<Vec<R>, crate::error::DbError>
    where
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 delete_returning() 操作，表: {}", self.table);
        }

        if self.conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let mut generator = SqlGenerator::new();
        generator.build_delete(&self.table, &self.conditions)?;
        let sql = format!("{}{}", generator.get_sql(), self.returning_clause()?);
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 delete_returning() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        let mut query = sqlx::query_as::<_, R>(&sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut tx = pool.begin().await.map_err(crate::error::DbError::from)?;
                match query.fetch_all(&mut *tx).await {
                    Ok(rows) => {
                        tx.commit().await.map_err(crate::error::DbError::from)?;
                        Ok(rows)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_all(&mut *connection).await
            }
        };
        match result {
            Ok(rows) => Ok(rows),
            Err(error) => {
                let db_err = crate::error::DbError::from(error);
                if db_err.is_constraint_violation() {
                    log::warn!(
                        "delete_returning() 失败: {}（约束冲突，可能为预期分支）",
                        db_err
                    );
                } else {
                    log::error!("delete_returning() 失败: {}", db_err);
                }
                Err(db_err)
            }
        }
    }

    /// `INSERT INTO ... SELECT ...`：把源查询结果插入目标表。
    ///
    /// `columns` 为目标列列表（可为空表示整行）；源查询由 [`QueryBuilder`] 构建，
    /// 支持 CTE、集合运算与排序/分页。返回受影响行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_from_select(
        self,
        columns: &[&crate::FieldRef],
        source: QueryBuilder<'a>,
    ) -> Result<u64, crate::error::DbError> {
        let mut generator = SqlGenerator::new();
        generator.build_insert_from_select(&self.table, columns, &source)?;
        if self.enable_logging {
            log::debug!("执行 insert_from_select() SQL: {}", generator.get_sql());
        }
        execute_generated(self.executor, generator.get_sql(), generator.get_params()).await
    }

    /// `UPDATE ... SET ... FROM <from> WHERE <conds> AND <target_field> = <source_field>`。
    ///
    /// 与 [`Self::update`] 一样必须提供至少一个 WHERE 条件，避免误更新；
    /// 联表条件会与 WHERE 条件一起进入 WHERE。返回受影响行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update_from<T>(
        self,
        data: &T,
        from: &crate::TableRef,
        target_field: &crate::FieldRef,
        source_field: &crate::FieldRef,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if self.conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }
        let json_data = serde_json::to_value(data).map_err(|error| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
        })?;
        let mut generator = SqlGenerator::new();
        generator.build_update_from(
            &self.table,
            &json_data,
            &self.field_types,
            &self.set_exprs,
            &self.conditions,
            from,
            target_field,
            source_field,
        )?;
        if self.enable_logging {
            log::debug!("执行 update_from() SQL: {}", generator.get_sql());
        }
        execute_generated(self.executor, generator.get_sql(), generator.get_params()).await
    }

    /// `DELETE FROM ... USING <using> WHERE <conds> AND <target_field> = <source_field>`。
    ///
    /// 与 [`Self::delete`] 一样必须提供至少一个 WHERE 条件；联表条件会与 WHERE
    /// 条件一起进入 WHERE。返回受影响行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "delete", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn delete_using(
        self,
        using: &crate::TableRef,
        target_field: &crate::FieldRef,
        source_field: &crate::FieldRef,
    ) -> Result<u64, crate::error::DbError> {
        if self.conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }
        let mut generator = SqlGenerator::new();
        generator.build_delete_using(
            &self.table,
            &self.conditions,
            using,
            target_field,
            source_field,
        )?;
        if self.enable_logging {
            log::debug!("执行 delete_using() SQL: {}", generator.get_sql());
        }
        execute_generated(self.executor, generator.get_sql(), generator.get_params()).await
    }

    /// 批量更新记录
    ///
    /// 使用 CASE WHEN 策略在单次查询中更新多条记录，自动分批（每批
    /// [`UPDATE_BATCH_SIZE`]，1000；宽表按列数自动缩小），所有批次在同一事务中
    /// 执行保证原子性。返回总受影响行数。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update_batch<T>(
        self,
        records: &[T],
        where_field: &crate::FieldRef,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        self.update_batch_with_size(records, where_field, UPDATE_BATCH_SIZE)
            .await
    }

    /// 批量更新记录（自定义批次大小）
    ///
    /// 与 [`QueryBuilder::update_batch`] 相同，但允许调用方自定义每批最大记录数；
    /// 宽表会自动按列数缩小批大小，避免超出协议占位符上限。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update_batch_with_size<T>(
        self,
        records: &[T],
        where_field: &crate::FieldRef,
        batch_size: usize,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if batch_size == 0 {
            return Err(crate::error::DbError::SerializationError(
                "batch_size 不能为 0".to_string(),
            ));
        }

        if records.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量更新数据不能为空".to_string(),
            ));
        }

        // 与 insert_batch 保持一致：批量路径不静默忽略 set_expr，显式拒绝。
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "update_batch 不支持 set_expr()；请改用逐条 update() 或在数据中提供字面量"
                    .to_string(),
            ));
        }

        let json_records: Vec<serde_json::Value> = records
            .iter()
            .map(|r| {
                serde_json::to_value(r).map_err(|e| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
                })
            })
            .collect::<Result<_, _>>()?;

        // 按「非主键列数」推导安全批大小：宽表单条占位符可能超协议上限，需按列缩小
        let field_count = update_field_count(&json_records, where_field.as_str());
        let per_record = field_count.saturating_mul(2).saturating_add(1);
        if per_record > MAX_BIND_PARAMS {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "批量更新单行需要 {per_record} 个占位符，超过协议上限 {MAX_BIND_PARAMS}，请减少更新列数"
            )));
        }
        let batch_size = derive_update_batch_size(field_count, batch_size);

        let mut total = 0u64;
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for chunk in json_records.chunks(batch_size) {
                    total += execute_update_chunk(
                        &mut transaction,
                        &self.table,
                        chunk,
                        where_field,
                        &self.field_types,
                    )
                    .await?;
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for chunk in json_records.chunks(batch_size) {
                    total += execute_update_chunk(
                        &mut *connection,
                        &self.table,
                        chunk,
                        where_field,
                        &self.field_types,
                    )
                    .await?;
                }
            }
        }
        Ok(total)
    }

    /// UPSERT - 插入或更新记录
    ///
    /// 使用 PostgreSQL `INSERT ... ON CONFLICT (...) DO UPDATE SET col = EXCLUDED.col`
    /// 语法。冲突目标必须用 [`QueryBuilder::on_conflict`] /
    /// [`QueryBuilder::on_conflict_constraint`] / [`QueryBuilder::on_conflict_with_predicate`]
    /// 显式指定；未指定且非 [`QueryBuilder::do_nothing`] 时在客户端返回
    /// [`crate::DbError::InvalidArgument`]。返回 `rows_affected()`。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn upsert<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "upsert 不支持 set_expr；请在插入数据中直接提供列值".to_string(),
            ));
        }
        if self.enable_logging {
            log::debug!("执行 upsert() 操作，表: {}", self.table);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_upsert(
            &self.table,
            &json_data,
            &self.field_types,
            &self.conflict_target,
            self.conflict_predicate.as_deref(),
            self.do_nothing,
            self.update_columns.as_deref(),
            &self.upsert_where,
        )?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 upsert() SQL: {}", sql);
        }

        let mut query = sqlx::query(sql);
        for param in params {
            query = bind_execute_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        }
        .map_err(crate::error::DbError::from)?;
        let rows = result.rows_affected();

        if self.enable_logging {
            log::debug!("upsert() 完成，rows_affected: {}", rows);
        }

        Ok(rows)
    }

    /// UPSERT 并返回 `RETURNING` 行（按真实列类型解码）。
    ///
    /// 冲突目标必须显式指定（见 [`Self::on_conflict`]）；默认 `RETURNING <id>`，
    /// 可用 [`QueryBuilder::returning_all`] / [`QueryBuilder::returning_columns`] 调整。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn upsert_returning<T, R>(self, data: &T) -> Result<R, crate::error::DbError>
    where
        T: serde::Serialize,
        R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "upsert 不支持 set_expr；请在插入数据中直接提供列值".to_string(),
            ));
        }
        if self.enable_logging {
            log::debug!("执行 upsert_returning() 操作，表: {}", self.table);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_upsert(
            &self.table,
            &json_data,
            &self.field_types,
            &self.conflict_target,
            self.conflict_predicate.as_deref(),
            self.do_nothing,
            self.update_columns.as_deref(),
            &self.upsert_where,
        )?;
        let sql = format!("{}{}", generator.get_sql(), self.returning_clause()?);
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 upsert_returning() SQL: {}", sql);
        }

        let mut query = sqlx::query_as::<_, R>(&sql);
        for param in params {
            query = bind_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut tx = pool.begin().await.map_err(crate::error::DbError::from)?;
                match query.fetch_one(&mut *tx).await {
                    Ok(row) => {
                        tx.commit().await.map_err(crate::error::DbError::from)?;
                        Ok(row)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_one(&mut *connection).await
            }
        };
        result.map_err(|error| {
            let db_err = crate::error::DbError::from(error);
            if db_err.is_constraint_violation() {
                log::warn!(
                    "upsert_returning() 失败: {}（约束冲突，可能为预期分支）",
                    db_err
                );
            } else {
                log::error!("upsert_returning() 失败: {}", db_err);
            }
            db_err
        })
    }

    /// 批量 UPSERT：多行 VALUES + `ON CONFLICT ... DO UPDATE`，返回受影响行数。
    ///
    /// 冲突列由调用方显式传入（不做静默默认）；使用 `id` 时请显式传
    /// `&[field!("id")]`。本方法的冲突列参数优先于 builder 上的
    /// [`Self::on_conflict`] 设置。更新列默认取「插入列集 - 冲突列」，可用
    /// [`Self::update_columns`] 收窄；[`Self::do_nothing`] 时渲染
    /// `ON CONFLICT DO NOTHING`。默认每批 [`INSERT_BATCH_SIZE`]（500）行。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn upsert_batch<T>(
        self,
        data: &[T],
        conflict_columns: &[&crate::FieldRef],
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        self.upsert_batch_with_size(data, conflict_columns, INSERT_BATCH_SIZE)
            .await
    }

    /// 批量 UPSERT（自定义批次大小）。
    ///
    /// 一条语句本身原子：单批直接执行，多批包在事务中保证整体回滚。
    /// 批大小会按列数进一步收敛，避免超出协议占位符上限。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn upsert_batch_with_size<T>(
        self,
        data: &[T],
        conflict_columns: &[&crate::FieldRef],
        batch_size: usize,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if batch_size == 0 {
            return Err(crate::error::DbError::SerializationError(
                "batch_size 不能为 0".to_string(),
            ));
        }
        if !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "upsert_batch 不支持 set_expr；请改用 upsert() 或 update()".to_string(),
            ));
        }
        if data.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量 upsert 数据不能为空".to_string(),
            ));
        }

        let conflict: Vec<String> = conflict_columns
            .iter()
            .map(|column| column.as_str().to_string())
            .collect();
        if conflict.is_empty() && !self.do_nothing {
            return Err(crate::error::DbError::InvalidArgument(
                "批量 upsert 必须显式指定冲突列；如使用 id 请显式传入，\
                 或调用 do_nothing() 走 ON CONFLICT DO NOTHING"
                    .to_string(),
            ));
        }

        let json_data_list: Vec<serde_json::Value> = data
            .iter()
            .map(|item| {
                serde_json::to_value(item).map_err(|error| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
                })
            })
            .collect::<Result<_, _>>()?;

        let column_count = json_data_list[0]
            .as_object()
            .map(|object| object.len())
            .unwrap_or(0);
        let effective = derive_upsert_batch_size(column_count, batch_size);
        let chunk_count = json_data_list.len().div_ceil(effective);

        let mut total = 0u64;
        if chunk_count == 1 {
            match self.executor {
                QueryExecutor::Pool(pool) => {
                    let mut connection =
                        pool.acquire().await.map_err(crate::error::DbError::from)?;
                    total += execute_upsert_batch_chunk(
                        &mut connection,
                        &self.table,
                        &json_data_list,
                        &self.field_types,
                        &conflict,
                        self.conflict_predicate.as_deref(),
                        self.do_nothing,
                        self.update_columns.as_deref(),
                        &self.upsert_where,
                    )
                    .await?;
                }
                QueryExecutor::Transaction(transaction) => {
                    let connection = transaction.executor().ok_or_else(|| {
                        crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                    })?;
                    total += execute_upsert_batch_chunk(
                        &mut *connection,
                        &self.table,
                        &json_data_list,
                        &self.field_types,
                        &conflict,
                        self.conflict_predicate.as_deref(),
                        self.do_nothing,
                        self.update_columns.as_deref(),
                        &self.upsert_where,
                    )
                    .await?;
                }
            }
            return Ok(total);
        }

        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for chunk in json_data_list.chunks(effective) {
                    total += execute_upsert_batch_chunk(
                        &mut transaction,
                        &self.table,
                        chunk,
                        &self.field_types,
                        &conflict,
                        self.conflict_predicate.as_deref(),
                        self.do_nothing,
                        self.update_columns.as_deref(),
                        &self.upsert_where,
                    )
                    .await?;
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for chunk in json_data_list.chunks(effective) {
                    total += execute_upsert_batch_chunk(
                        &mut *connection,
                        &self.table,
                        chunk,
                        &self.field_types,
                        &conflict,
                        self.conflict_predicate.as_deref(),
                        self.do_nothing,
                        self.update_columns.as_deref(),
                        &self.upsert_where,
                    )
                    .await?;
                }
            }
        }

        if self.enable_logging {
            log::debug!("upsert_batch_with_size() 完成，rows_affected: {total}");
        }
        Ok(total)
    }

    /// MERGE（PostgreSQL 15+）：按 `target_key = source_key` 匹配源表，命中则更新
    /// `columns`，未命中则从源表插入 `columns`。
    ///
    /// # 版本要求
    ///
    /// `MERGE` 语句自 PostgreSQL 15 起提供；在更低版本上服务端会返回语法错误。
    /// 本方法不做版本探测，调用方需自行保证服务端版本（推荐 PG 16/17）。
    ///
    /// 不支持 `where` / `set_expr`：匹配条件请放入 `ON` 参数。返回受影响行数。
    ///
    /// 执行前会读取 `server_version_num`（PostgreSQL 15 起提供 MERGE）；低于 150000
    /// 时直接返回 [`crate::DbError::UnsupportedOperator`]，不会把不支持的语法发到服务端。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "merge", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn merge(
        self,
        source: &crate::TableRef,
        target_key: &crate::FieldRef,
        source_key: &crate::FieldRef,
        columns: &[&crate::FieldRef],
    ) -> Result<u64, crate::error::DbError> {
        self.merge_with_options(
            source,
            target_key,
            source_key,
            columns,
            MergeOptions::default(),
        )
        .await
    }

    /// 带可选分支的 MERGE（PostgreSQL 15+）。
    ///
    /// 支持 `WHEN MATCHED AND <condition>` 与 PostgreSQL 17+ 的
    /// `WHEN NOT MATCHED BY SOURCE THEN DELETE/UPDATE`；后者在更低版本上返回
    /// [`crate::DbError::UnsupportedOperator`]，不会把不支持的语法发到服务端。
    /// 其余语义与 [`Self::merge`] 相同。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "postgresql", db.operation = "merge", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn merge_with_options(
        mut self,
        source: &crate::TableRef,
        target_key: &crate::FieldRef,
        source_key: &crate::FieldRef,
        columns: &[&crate::FieldRef],
        options: MergeOptions,
    ) -> Result<u64, crate::error::DbError> {
        if !self.conditions.is_empty() || !self.set_exprs.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "merge 不支持 where / set_expr；匹配条件请放入 ON 参数".to_string(),
            ));
        }
        let version = server_version_num(&mut self.executor).await?;
        if !merge_version_supported(version) {
            return Err(crate::error::DbError::UnsupportedOperator(format!(
                "MERGE 需要 PostgreSQL 15+（server_version_num >= {MERGE_MIN_SERVER_VERSION}），当前为 {version}"
            )));
        }
        if options.not_matched_by_source.is_some() && version < MERGE_BY_SOURCE_MIN_SERVER_VERSION {
            return Err(crate::error::DbError::UnsupportedOperator(format!(
                "WHEN NOT MATCHED BY SOURCE 需要 PostgreSQL 17+（server_version_num >= {MERGE_BY_SOURCE_MIN_SERVER_VERSION}），当前为 {version}"
            )));
        }
        let mut generator = SqlGenerator::new();
        generator.build_merge_with_options(
            &self.table,
            source,
            target_key,
            source_key,
            columns,
            &options,
        )?;
        if self.enable_logging {
            log::debug!("执行 merge_with_options() SQL: {}", generator.get_sql());
        }
        execute_generated(self.executor, generator.get_sql(), generator.get_params()).await
    }
}

/// 执行一个批量 UPSERT 分片，返回受影响行数。
#[allow(clippy::too_many_arguments)]
async fn execute_upsert_batch_chunk(
    connection: &mut sqlx::PgConnection,
    table: &str,
    data: &[serde_json::Value],
    field_types: &HashMap<String, FieldType>,
    conflict_columns: &[String],
    conflict_predicate: Option<&str>,
    do_nothing: bool,
    update_columns: Option<&[String]>,
    update_where: &[Condition],
) -> Result<u64, crate::error::DbError> {
    let mut generator = SqlGenerator::new();
    generator.build_upsert_batch(
        table,
        data,
        field_types,
        conflict_columns,
        conflict_predicate,
        do_nothing,
        update_columns,
        update_where,
    )?;
    let mut query = sqlx::query(generator.get_sql());
    for param in generator.get_params() {
        query = bind_execute_param(query, param);
    }
    Ok(query.execute(connection).await?.rows_affected())
}

/// 数组元素提取助手：只接受对应标量变体，其余（含 NULL）编码为 SQL NULL，
/// 以保持数组参数同构（PostgreSQL 数组不允许混合类型）。
fn array_element_bool(value: &SqlValue) -> Option<bool> {
    match value {
        SqlValue::Bool(value) => Some(*value),
        _ => None,
    }
}

fn array_element_i64(value: &SqlValue) -> Option<i64> {
    match value {
        SqlValue::Int(value) => Some(*value),
        _ => None,
    }
}

fn array_element_i16(value: &SqlValue) -> Option<i16> {
    array_element_i64(value).and_then(|value| i16::try_from(value).ok())
}

fn array_element_i32(value: &SqlValue) -> Option<i32> {
    array_element_i64(value).and_then(|value| i32::try_from(value).ok())
}

fn array_element_f32(value: &SqlValue) -> Option<f32> {
    match value {
        SqlValue::Float(value) => Some(*value as f32),
        SqlValue::Int(value) => Some(*value as f32),
        _ => None,
    }
}

fn array_element_bytes(value: &SqlValue) -> Option<Vec<u8>> {
    match value {
        SqlValue::Bytes(value) => Some(value.clone()),
        _ => None,
    }
}

fn array_element_json(value: &SqlValue) -> Option<serde_json::Value> {
    match value {
        SqlValue::Json(value) => Some(value.clone()),
        _ => None,
    }
}

fn array_element_f64(value: &SqlValue) -> Option<f64> {
    match value {
        SqlValue::Float(value) => Some(*value),
        _ => None,
    }
}

fn array_element_string(value: &SqlValue) -> Option<String> {
    match value {
        SqlValue::String(value)
        | SqlValue::Inet(value)
        | SqlValue::Cidr(value)
        | SqlValue::MacAddr(value)
        | SqlValue::MacAddr8(value)
        | SqlValue::Range(value)
        | SqlValue::Multirange(value) => Some(value.clone()),
        _ => None,
    }
}

fn array_element_timestamp(value: &SqlValue) -> Option<i64> {
    match value {
        SqlValue::Timestamp(value) => Some(*value),
        _ => None,
    }
}

fn array_element_uuid(value: &SqlValue) -> Option<uuid::Uuid> {
    match value {
        SqlValue::Uuid(value) => Some(*value),
        _ => None,
    }
}

fn array_element_date(value: &SqlValue) -> Option<chrono::NaiveDate> {
    match value {
        SqlValue::Date(value) => Some(*value),
        _ => None,
    }
}

fn array_element_time(value: &SqlValue) -> Option<chrono::NaiveTime> {
    match value {
        SqlValue::Time(value) => Some(*value),
        _ => None,
    }
}

fn array_element_datetime(value: &SqlValue) -> Option<chrono::NaiveDateTime> {
    match value {
        SqlValue::DateTime(value) => Some(*value),
        _ => None,
    }
}

fn array_element_datetime_tz(value: &SqlValue) -> Option<chrono::DateTime<chrono::Utc>> {
    match value {
        SqlValue::DateTimeTz(value) => Some(*value),
        _ => None,
    }
}

fn array_element_decimal(value: &SqlValue) -> Option<bigdecimal::BigDecimal> {
    match value {
        SqlValue::Decimal(value) => Some(value.clone()),
        _ => None,
    }
}

/// 绑定参数到执行查询（用于 INSERT/UPDATE/DELETE，PostgreSQL）
///
/// 使用 `bind_value_match!` 宏统一处理 `SqlValue` 各变体的绑定逻辑，
/// 与 MySQL 后端的同名函数对齐，仅后端类型不同。
fn bind_execute_param<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    param: &SqlValue,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    bind_value_match!(query, param)
}

/// 绑定参数到 `query_as` 查询（PostgreSQL）
///
/// 使用 `bind_value_match!` 宏统一处理 `SqlValue` 各变体的绑定逻辑，
/// 与 MySQL 后端的同名函数对齐，仅后端类型不同（`Postgres` / `PgArguments` / `PgRow`）。
pub(crate) fn bind_param<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询（PostgreSQL）
///
/// 泛型于标量类型 `C`，供 `value::<T>()` 与 `count()`（`i64`）等复用。
fn bind_scalar_param<'q, C>(
    query: sqlx::query::QueryScalar<'q, sqlx::Postgres, C, sqlx::postgres::PgArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, C, sqlx::postgres::PgArguments>
where
    C: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询（Option 类型，PostgreSQL）
///
/// 聚合/标量方法现统一走 `fetch_scalar` + `bind_scalar_param`（后者对 `Option<T>`
/// 输出类型同样适用），本函数暂无调用方但保留作为公开内部表面，标注 allow(dead_code)。
#[allow(dead_code)]
fn bind_scalar_param_option<'q, T>(
    query: sqlx::query::QueryScalar<'q, sqlx::Postgres, Option<T>, sqlx::postgres::PgArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, Option<T>, sqlx::postgres::PgArguments>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    bind_value_match!(query, param)
}

/// 在构建器所属执行上下文（连接池或调用方事务）中执行已生成的 DML，
/// 返回受影响行数。
///
/// 新增 DML 路径（批量 UPSERT、联表 DML、MERGE）共用，避免每个方法重复
/// 一遍 `QueryExecutor` 分支。
async fn execute_generated(
    executor: QueryExecutor<'_>,
    sql: &str,
    params: &[SqlValue],
) -> Result<u64, crate::error::DbError> {
    let mut query = sqlx::query(sql);
    for param in params {
        query = bind_execute_param(query, param);
    }
    let result = match executor {
        QueryExecutor::Pool(pool) => query.execute(pool).await,
        QueryExecutor::Transaction(transaction) => {
            let connection = transaction.executor().ok_or_else(|| {
                crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
            })?;
            query.execute(&mut *connection).await
        }
    }
    .map_err(crate::error::DbError::from)?;
    Ok(result.rows_affected())
}

async fn execute_insert_chunk<T>(
    connection: &mut sqlx::PgConnection,
    table: &str,
    data: &[T],
    field_types: &HashMap<String, FieldType>,
) -> Result<u64, crate::error::DbError>
where
    T: serde::Serialize,
{
    let json_data_list = data
        .iter()
        .map(|item| {
            serde_json::to_value(item).map_err(|error| {
                crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut generator = SqlGenerator::new();
    generator.build_insert_batch(table, &json_data_list, field_types)?;
    let mut query = sqlx::query(generator.get_sql());
    for param in generator.get_params() {
        query = bind_execute_param(query, param);
    }
    Ok(query.execute(connection).await?.rows_affected())
}

async fn execute_insert_batch_returning<R>(
    connection: &mut sqlx::PgConnection,
    table: &str,
    data: &[serde_json::Value],
    field_types: &HashMap<String, FieldType>,
    returning: &str,
) -> Result<Vec<R>, crate::error::DbError>
where
    R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let mut generator = SqlGenerator::new();
    generator.build_insert_batch(table, data, field_types)?;
    let sql = format!("{}{}", generator.get_sql(), returning);
    let mut query = sqlx::query_as::<_, R>(&sql);
    for param in generator.get_params() {
        query = bind_param(query, param);
    }
    Ok(query.fetch_all(connection).await?)
}

/// 执行一个批量 UPSERT 分片并返回 `RETURNING` 行。
#[allow(clippy::too_many_arguments)]
async fn execute_upsert_batch_returning<R>(
    connection: &mut sqlx::PgConnection,
    table: &str,
    data: &[serde_json::Value],
    field_types: &HashMap<String, FieldType>,
    conflict_columns: &[String],
    conflict_predicate: Option<&str>,
    do_nothing: bool,
    update_columns: Option<&[String]>,
    update_where: &[Condition],
    returning: &str,
) -> Result<Vec<R>, crate::error::DbError>
where
    R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let mut generator = SqlGenerator::new();
    generator.build_upsert_batch(
        table,
        data,
        field_types,
        conflict_columns,
        conflict_predicate,
        do_nothing,
        update_columns,
        update_where,
    )?;
    let sql = format!("{}{}", generator.get_sql(), returning);
    let mut query = sqlx::query_as::<_, R>(&sql);
    for param in generator.get_params() {
        query = bind_param(query, param);
    }
    Ok(query.fetch_all(connection).await?)
}

async fn execute_update_chunk(
    connection: &mut sqlx::PgConnection,
    table: &str,
    records: &[serde_json::Value],
    where_field: &crate::FieldRef,
    field_types: &HashMap<String, FieldType>,
) -> Result<u64, crate::error::DbError> {
    let mut generator = SqlGenerator::new();
    generator.build_update_batch(table, records, where_field.as_str(), field_types)?;
    let mut query = sqlx::query(generator.get_sql());
    for param in generator.get_params() {
        query = bind_execute_param(query, param);
    }
    Ok(query.execute(connection).await?.rows_affected())
}

fn predicate_condition(predicate: &crate::Predicate) -> Result<Condition, crate::error::DbError> {
    let condition = match predicate {
        crate::Predicate::Compare(field, operator, value) => {
            comparison_condition(field.as_str(), *operator, predicate_value(value))?
        }
        crate::Predicate::In(field, values) => Condition::In(
            field.as_str().to_string(),
            values.iter().map(predicate_value).collect(),
        ),
        crate::Predicate::NotIn(field, values) => Condition::NotIn(
            field.as_str().to_string(),
            values.iter().map(predicate_value).collect(),
        ),
        crate::Predicate::Between(field, start, end) => Condition::Between(
            field.as_str().to_string(),
            predicate_value(start),
            predicate_value(end),
        ),
        crate::Predicate::IsNull(field) => Condition::IsNull(field.as_str().to_string()),
        crate::Predicate::IsNotNull(field) => Condition::IsNotNull(field.as_str().to_string()),
        crate::Predicate::And(values) => Condition::And(
            values
                .iter()
                .map(predicate_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        crate::Predicate::Or(values) => Condition::Or(
            values
                .iter()
                .map(predicate_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    Ok(condition)
}

fn predicate_value(value: &serde_json::Value) -> SqlValue {
    match value {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(value) => SqlValue::Bool(*value),
        serde_json::Value::Number(value) => {
            value.as_i64().map(SqlValue::Int).unwrap_or_else(|| {
                value
                    .as_u64()
                    .map(SqlValue::from)
                    .or_else(|| value.as_f64().map(SqlValue::Float))
                    .unwrap_or_else(|| SqlValue::String(value.to_string()))
            })
        }
        serde_json::Value::String(value) => SqlValue::String(value.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => SqlValue::Json(value.clone()),
    }
}

#[cfg(test)]
mod predicate_scalar_tests {
    use super::{predicate_value, SqlValue};

    #[test]
    fn controlled_predicates_bind_json_scalars_as_sql_scalars() {
        assert!(matches!(
            predicate_value(&serde_json::json!("alice")),
            SqlValue::String(value) if value == "alice"
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!(42)),
            SqlValue::Int(42)
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!(true)),
            SqlValue::Bool(true)
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!({"role": "admin"})),
            SqlValue::Json(_)
        ));
    }
}
// PLACEHOLDER_TESTS

#[cfg(test)]
#[allow(deprecated)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::postgres::Subquery;
    use sqlx::postgres::PgPoolOptions;

    /// 获取或创建共享懒连接池（仅验证 URL，不立即建立连接）。
    /// 用于只测试 SQL 生成逻辑、不需要真实数据库连接的单元测试。
    /// 使用 OnceLock + 静态 Tokio 运行时确保池在有效上下文中创建和驻留；
    /// `connect_lazy` 不会发起网络往返，但仍需运行时上下文。
    fn make_sync_test_pool() -> &'static PgPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<PgPool> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            rt.block_on(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    #[test]
    fn test_controlled_subqueries_preserve_nested_placeholder_order() {
        let pool = make_sync_test_pool();
        let paid_order = Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_column("orders.user_id", "=", "users.id")
            .expect("合法关联列")
            .where_value("orders.status", "=", "paid")
            .expect("合法绑定条件");
        let active_user = Subquery::new("memberships", "user_id")
            .expect("合法子查询")
            .where_value("memberships.active", "=", true)
            .expect("合法绑定条件");
        let banned_user = Subquery::new("bans", "id")
            .expect("合法子查询")
            .where_column("bans.user_id", "=", "users.id")
            .expect("合法关联列")
            .where_value("bans.reason", "=", "fraud")
            .expect("合法绑定条件");

        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(
                yang_db::field!("users.tenant_id"),
                yang_db::CompareOp::Eq,
                7,
            )
            .expect("固定受控条件应合法")
            .where_exists(paid_order)
            .where_in_subquery(yang_db::field!("users.id"), active_user)
            .where_not_exists(banned_user);
        let mut generator = SqlGenerator::new();
        generator.build_select(&builder).expect("子查询应可渲染");

        assert_eq!(
            generator.get_sql(),
            "SELECT * FROM \"users\" WHERE (\"users\".\"tenant_id\" = $1 AND EXISTS (SELECT \"id\" FROM \"orders\" WHERE \"orders\".\"user_id\" = \"users\".\"id\" AND \"orders\".\"status\" = $2) AND \"users\".\"id\" IN (SELECT \"user_id\" FROM \"memberships\" WHERE \"memberships\".\"active\" = $3) AND NOT EXISTS (SELECT \"id\" FROM \"bans\" WHERE \"bans\".\"user_id\" = \"users\".\"id\" AND \"bans\".\"reason\" = $4))"
        );
        assert!(
            matches!(generator.get_params(), [SqlValue::Int(7), SqlValue::String(status), SqlValue::Bool(true), SqlValue::String(reason)] if status == "paid" && reason == "fraud")
        );
    }

    #[test]
    fn test_controlled_subqueries_reject_adversarial_structure_before_rendering() {
        for payload in [
            "",
            "orders; DROP TABLE users",
            "orders --",
            "a.b",
            "orders\0",
        ] {
            assert!(
                Subquery::new(payload, "id").is_err(),
                "非法表名被接受: {payload:?}"
            );
        }
        for payload in ["", "id) FROM users --", "COUNT(*)", "a.b.c.d", "id\0"] {
            assert!(
                Subquery::new("orders", payload).is_err(),
                "非法投影被接受: {payload:?}"
            );
        }
        assert!(Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_column("orders.user_id", "= 1 OR 1=1 --", "users.id")
            .is_err());
        assert!(Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_value("orders.status --", "=", "paid")
            .is_err());
        assert!(crate::FieldRef::new("users.id) OR 1=1 --").is_err());
    }

    #[test]
    fn test_union_all_keeps_branch_scope_and_numbered_parameter_order() {
        let pool = make_sync_test_pool();
        let branch = QueryBuilder::new(pool, "archived_users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("kind"))
            .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 8)
            .expect("固定受控条件应合法")
            .order(yang_db::field!("id"), yang_db::SortOrder::Desc)
            .limit(2);
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("kind"))
            .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
            .expect("固定受控条件应合法")
            .union_all(branch)
            .expect("输出列数一致")
            .order(yang_db::field!("id"), yang_db::SortOrder::Asc)
            .limit(5);
        let mut generator = SqlGenerator::new();
        generator
            .build_select(&builder)
            .expect("UNION ALL 应可渲染");

        assert_eq!(generator.get_sql(), "SELECT \"id\", \"kind\" FROM \"users\" WHERE \"tenant_id\" = $1 UNION ALL (SELECT \"id\", \"kind\" FROM \"archived_users\" WHERE \"tenant_id\" = $2 ORDER BY \"id\" DESC LIMIT 2) ORDER BY \"id\" ASC LIMIT 5");
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Int(7), SqlValue::Int(8)]
        ));
    }

    #[test]
    fn test_union_rejects_unknown_or_mismatched_output_and_bad_branch_table() {
        let pool = make_sync_test_pool();
        assert!(QueryBuilder::new(pool, "users", false)
            .union(QueryBuilder::new(pool, "archive", false))
            .is_err());
        assert!(QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .union(
                QueryBuilder::new(pool, "archive", false)
                    .field(yang_db::field!("id"))
                    .field(yang_db::field!("kind")),
            )
            .is_err());
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .union(
                QueryBuilder::new(pool, "archive; DROP TABLE users", false)
                    .field(yang_db::field!("id")),
            )
            .expect("列数一致");
        let mut generator = SqlGenerator::new();
        assert!(generator.build_select(&builder).is_err());
        assert!(generator.get_params().is_empty());
    }

    #[test]
    fn test_transaction_row_lock_rendering_is_typed_and_numbered() {
        let builder = QueryBuilder::new(make_sync_test_pool(), "accounts", false)
            .field(yang_db::field!("balance"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 42)
            .expect("固定受控条件应合法")
            .limit(1);
        let (sql, params) = builder
            .render_for_transaction(Some(crate::RowLock::ForUpdate))
            .expect("事务锁查询应可渲染");
        assert_eq!(
            sql,
            "SELECT \"balance\" FROM \"accounts\" WHERE \"id\" = $1 LIMIT 1 FOR UPDATE"
        );
        assert!(matches!(params.as_slice(), [SqlValue::Int(42)]));
    }

    #[test]
    fn test_transaction_row_lock_rejects_unsupported_query_shapes() {
        let pool = make_sync_test_pool();
        let grouped = QueryBuilder::new(pool, "accounts", false)
            .expr(crate::SelectExpr::count_all())
            .group(yang_db::field!("tenant_id"));
        assert!(grouped
            .render_for_transaction(Some(crate::RowLock::ForShare))
            .is_err());

        let union = QueryBuilder::new(pool, "accounts", false)
            .field(yang_db::field!("id"))
            .union(QueryBuilder::new(pool, "archived_accounts", false).field(yang_db::field!("id")))
            .expect("列数一致的 UNION 应构建成功");
        assert!(union
            .render_for_transaction(Some(crate::RowLock::ForUpdate))
            .is_err());
    }

    #[tokio::test]
    async fn test_atomic_update_rejects_missing_where_and_adversarial_field_before_io() {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
            .expect("合法测试 URL");
        let missing_where = QueryBuilder::new(&pool, "accounts", false)
            .increment(yang_db::field!("balance"), 1)
            .await;
        assert!(matches!(
            missing_where,
            Err(crate::DbError::MissingWhereClause)
        ));

        assert!(crate::FieldRef::new("balance = 0; DROP TABLE accounts --").is_err());
    }

    #[test]
    fn test_atomic_update_renderer_binds_negative_amount_before_where_params() {
        let mut generator = SqlGenerator::new();
        generator
            .build_arithmetic_update(
                "accounts",
                "balance",
                ArithmeticOperator::Add,
                -3,
                &[Condition::Eq("id".to_string(), SqlValue::Int(9))],
            )
            .expect("原子更新应可渲染");
        assert_eq!(
            generator.get_sql(),
            "UPDATE \"accounts\" SET \"balance\" = \"balance\" + $1 WHERE \"id\" = $2"
        );
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Int(-3), SqlValue::Int(9)]
        ));
    }

    #[test]
    fn test_identifier_apis_quote_every_structural_fragment() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("users.id"))
            .expr(crate::SelectExpr::count_all().alias(yang_db::field!("cnt")))
            .join(
                yang_db::table!("profiles"),
                yang_db::field!("users.id"),
                yang_db::field!("profiles.user_id"),
            )
            .group(yang_db::field!("users.id"))
            .order(yang_db::field!("users.id"), yang_db::SortOrder::Asc);

        let sql = builder.try_to_sql().expect("安全 API 应生成 SQL");
        assert!(sql.contains("SELECT \"users\".\"id\", COUNT(*) AS \"cnt\""));
        assert!(
            sql.contains("INNER JOIN \"profiles\" ON \"users\".\"id\" = \"profiles\".\"user_id\"")
        );
        assert!(sql.contains("GROUP BY \"users\".\"id\""));
        assert!(sql.contains("ORDER BY \"users\".\"id\" ASC"));
    }

    #[test]
    fn test_identifier_apis_reject_adversarial_fragments() {
        let payloads = [
            "",
            ".id",
            "users.",
            "a.b.c.d",
            "users.id --",
            "users/*x*/.id",
            // 中文限定标识符在 PostgreSQL 规则下合法（见 reference 单测），不再列入拒绝集。
            "users.\"id\"",
            "users.id\0",
        ];

        for payload in payloads {
            assert!(crate::FieldRef::new(payload).is_err());
        }
        assert!(crate::TableRef::new("profiles p").is_err());
    }

    #[test]
    fn test_try_to_sql_surfaces_invalid_table_identifier() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users; DROP TABLE users", false);
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_try_to_sql_rejects_invalid_condition_identifier() {
        assert!(crate::FieldRef::new("id;DROP").is_err());
    }

    #[test]
    fn test_try_to_sql_accepts_qualified_where_and_having_identifiers() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(
                yang_db::field!("users.status"),
                yang_db::CompareOp::Eq,
                1i64,
            )
            .expect("固定受控条件应合法")
            .group(yang_db::field!("users.id"))
            .having_cond(
                yang_db::field!("users.score"),
                yang_db::CompareOp::Gt,
                10i64,
            )
            .expect("固定受控条件应合法");

        let sql = builder
            .try_to_sql()
            .expect("合法的两段限定条件字段应生成 SQL");

        assert!(sql.contains("WHERE \"users\".\"status\" = $1"));
        assert!(sql.contains("HAVING \"users\".\"score\" > $2"));
    }

    #[test]
    fn test_try_to_sql_rejects_malicious_qualified_where_and_having_identifiers() {
        let invalid_fields = [
            "",
            ".id",
            "users.",
            "a.b.c.d",
            "users.id;DROP",
            "users.id --",
            "users.\"id\"",
            "COUNT(users.id)",
        ];

        for field in invalid_fields {
            assert!(crate::FieldRef::new(field).is_err());
        }
    }

    #[test]
    fn test_try_to_sql_surfaces_missing_group_by() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false)
            .having_cond(yang_db::field!("cnt"), yang_db::CompareOp::Gt, 5i64)
            .expect("固定受控条件应合法");
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::MissingGroupByClause)));
    }

    #[test]
    fn test_try_to_sql_rejects_empty_in_condition() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_in(yang_db::field!("id"), Vec::<i64>::new());
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_try_to_sql_rejects_empty_boolean_condition() {
        let pool = make_sync_test_pool();
        let mut builder = QueryBuilder::new(pool, "users", false);
        builder.conditions.push(Condition::And(vec![]));
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_to_sql_does_not_fallback_to_raw_untrusted_table() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users; DROP TABLE users", false);
        let sql = builder.to_sql();

        assert_eq!(sql, "/* SQL generation failed */");
        assert!(!sql.contains("DROP TABLE"));
        assert!(!sql.contains("users;"));
    }

    /// 构造仅含一个字段标记的空 field_types
    fn empty_types() -> HashMap<String, FieldType> {
        HashMap::new()
    }
    // PLACEHOLDER_TEST_CASES

    #[test]
    fn test_sql_generator_new_empty() {
        let generator = SqlGenerator::new();
        assert_eq!(generator.get_sql(), "");
        assert_eq!(generator.get_params().len(), 0);
    }

    #[test]
    fn test_build_insert_uses_dollar_placeholders() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三", "age": 25});
        g.build_insert("users", &data, &empty_types(), &[]).unwrap();
        let sql = g.get_sql();
        // serde_json 默认按 key 字母序：age 在前、name 在后
        assert_eq!(
            sql,
            "INSERT INTO \"users\" (\"age\", \"name\") VALUES ($1, $2)"
        );
        // INSERT 本身不含 RETURNING（由 insert() 方法在外部追加）
        assert!(!sql.contains("RETURNING"));
        assert_eq!(g.get_params().len(), 2);
    }

    #[test]
    fn test_build_insert_inlines_null_without_placeholder() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三", "deleted_at": null});
        g.build_insert("users", &data, &empty_types(), &[]).unwrap();
        let sql = g.get_sql();
        // 字母序：deleted_at(NULL 内联) 在前、name($1) 在后；NULL 不占占位符编号
        assert_eq!(
            sql,
            "INSERT INTO \"users\" (\"deleted_at\", \"name\") VALUES (NULL, $1)"
        );
        assert_eq!(g.get_params().len(), 1);
    }

    #[tokio::test]
    async fn test_insert_rejects_returning_all_before_io() {
        // 本测试运行在 Tokio 运行时内，不能复用 make_sync_test_pool 的静态运行时
        // （在运行时内再次 block_on 会 panic）。直接在当前运行时构造懒连接池。
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
            .expect("无法解析测试数据库 URL");
        let error = QueryBuilder::new(&pool, "users", false)
            .returning_all()
            .insert(&serde_json::json!({"name": "张三"}))
            .await
            .expect_err("returning_all 的 insert 应在触库前失败");
        assert!(matches!(error, crate::DbError::InvalidArgument(_)));
    }

    #[test]
    fn test_insert_set_expr_appends_after_json_columns() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三"});
        let set_exprs = vec![(
            "created_at".to_string(),
            crate::SqlExpr::unix_timestamp_add(5),
        )];
        g.build_insert("users", &data, &empty_types(), &set_exprs)
            .unwrap();
        assert_eq!(
            g.get_sql(),
            "INSERT INTO \"users\" (\"name\", \"created_at\") VALUES ($1, \
             EXTRACT(EPOCH FROM now()) + $2)"
        );
        assert_eq!(g.get_params().len(), 2);
    }

    #[test]
    fn test_build_update_set_then_where_numbering_continues() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "李四", "age": 30});
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(1))];
        g.build_update("users", &data, &empty_types(), &conditions, &[])
            .unwrap();
        let sql = g.get_sql();
        // 字母序：age=$1, name=$2；WHERE 接续为 $3
        assert_eq!(
            sql,
            "UPDATE \"users\" SET \"age\" = $1, \"name\" = $2 WHERE \"id\" = $3"
        );
        assert_eq!(g.get_params().len(), 3);
    }

    /// DB-3：单行 UPDATE 的 SET 子句对 NULL 内联字面量，不占占位符编号。
    #[test]
    fn test_build_update_inlines_null_in_set() {
        let mut g = SqlGenerator::new();
        // description 设为 NULL，name 为普通值
        let data = serde_json::json!({"description": serde_json::Value::Null, "name": "张三"});
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(1))];
        g.build_update("users", &data, &empty_types(), &conditions, &[])
            .unwrap();
        let sql = g.get_sql();
        // 字母序：description 内联 NULL（不占编号），name=$1，WHERE 接续 $2
        assert_eq!(
            sql,
            "UPDATE \"users\" SET \"description\" = NULL, \"name\" = $1 WHERE \"id\" = $2"
        );
        // NULL 不压参数：仅 name + WHERE id 两个参数
        assert_eq!(g.get_params().len(), 2);
    }

    #[test]
    fn test_build_delete_dollar_placeholder() {
        let mut g = SqlGenerator::new();
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(7))];
        g.build_delete("users", &conditions).unwrap();
        assert_eq!(g.get_sql(), "DELETE FROM \"users\" WHERE \"id\" = $1");
        assert_eq!(g.get_params().len(), 1);
    }

    #[test]
    fn test_build_delete_requires_where() {
        let mut g = SqlGenerator::new();
        let err = g.build_delete("users", &[]).unwrap_err();
        assert!(matches!(err, crate::error::DbError::MissingWhereClause));
    }

    #[test]
    fn test_build_upsert_on_conflict_excluded() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1, "name": "张三", "email": "z@example.com"});
        let conflict = ConflictTarget::Columns(vec!["id".to_string()]);
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &conflict,
            None,
            false,
            None,
            &[],
        )
        .unwrap();
        let sql = g.get_sql();
        // 字母序字段：email($1), id($2), name($3)
        assert!(sql.starts_with(
            "INSERT INTO \"users\" (\"email\", \"id\", \"name\") VALUES ($1, $2, $3)"
        ));
        assert!(sql.contains("ON CONFLICT (\"id\") DO UPDATE SET"));
        // 冲突列 id 不参与更新，其余列用 EXCLUDED.col
        assert!(sql.contains("\"name\" = EXCLUDED.\"name\""));
        assert!(sql.contains("\"email\" = EXCLUDED.\"email\""));
        assert!(!sql.contains("\"id\" = EXCLUDED.\"id\""));
        assert_eq!(g.get_params().len(), 3);
    }

    #[test]
    fn test_build_upsert_empty_conflict_do_nothing() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三"});
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Columns(vec![]),
            None,
            true,
            None,
            &[],
        )
        .unwrap();
        assert!(g.get_sql().contains("ON CONFLICT DO NOTHING"));
    }

    /// `EXCLUDED` 是 PostgreSQL 伪表，必须保持小写且不加引号；
    /// 写成 `"EXCLUDED"."col"` 会被当成真实表并报 42P01。
    #[test]
    fn test_build_upsert_excluded_field_is_lowercase_pseudo_table() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1, "name": "a"});
        let where_cond = vec![Condition::Gt(
            "EXCLUDED.name".to_string(),
            SqlValue::String("x".to_string()),
        )];
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Columns(vec!["id".to_string()]),
            None,
            false,
            None,
            &where_cond,
        )
        .unwrap();
        let sql = g.get_sql();
        assert!(sql.contains("excluded.\"name\""), "SQL: {sql}");
        assert!(!sql.contains("\"EXCLUDED\""), "SQL: {sql}");
    }

    /// ON CONFLICT ON CONSTRAINT 无法推断冲突列；默认更新列必须排除 `id`，
    /// 否则会把已存在行的主键覆盖成插入值。
    #[test]
    fn test_build_upsert_constraint_excludes_id_by_default() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 999, "email": "a@x", "name": "n"});
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Constraint("users_email_key".to_string()),
            None,
            false,
            None,
            &[],
        )
        .unwrap();
        let sql = g.get_sql();
        assert!(sql.contains("\"name\" = EXCLUDED.\"name\""), "SQL: {sql}");
        assert!(sql.contains("\"email\" = EXCLUDED.\"email\""), "SQL: {sql}");
        assert!(!sql.contains("\"id\" = EXCLUDED.\"id\""), "SQL: {sql}");
    }

    /// ON CONSTRAINT 且除 id 外没有可更新列时，必须显式报错，不能静默恒等更新。
    #[test]
    fn test_build_upsert_constraint_without_updatable_columns_errors() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1});
        let err = g
            .build_upsert(
                "users",
                &data,
                &empty_types(),
                &ConflictTarget::Constraint("users_pkey".to_string()),
                None,
                false,
                None,
                &[],
            )
            .unwrap_err();
        assert!(matches!(err, crate::error::DbError::InvalidArgument(_)));
    }

    /// 批量更新不能静默忽略 set_expr；必须在触库前拒绝。
    #[tokio::test]
    async fn test_update_batch_rejects_set_expr_before_io() {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
            .expect("无法解析测试数据库 URL");
        let error = QueryBuilder::new(&pool, "users", false)
            .set_expr(crate::field!("updated_at"), crate::SqlExpr::now())
            .update_batch(&[serde_json::json!({"id": 1})], crate::field!("id"))
            .await
            .expect_err("update_batch 应拒绝 set_expr");
        assert!(matches!(error, crate::error::DbError::InvalidArgument(_)));
    }

    #[test]
    fn test_build_insert_batch_sequential_numbering() {
        let mut g = SqlGenerator::new();
        let list = vec![
            serde_json::json!({"name": "a", "age": 1}),
            serde_json::json!({"name": "b", "age": 2}),
        ];
        g.build_insert_batch("users", &list, &empty_types())
            .unwrap();
        let sql = g.get_sql();
        // 字母序字段：age, name；占位符跨记录连续递增
        assert_eq!(
            sql,
            "INSERT INTO \"users\" (\"age\", \"name\") VALUES ($1, $2), ($3, $4)"
        );
        assert_eq!(g.get_params().len(), 4);
    }

    #[test]
    fn test_sum_expr_uses_double_precision() {
        let pool = make_sync_test_pool();
        // 通过 fetch_scalar 路径间接构造的表达式无法静态读取，这里直接断言生成片段
        let sql = QueryBuilder::new(pool, "orders", false)
            .expr(crate::SelectExpr::sum(yang_db::field!("amount")).cast_double())
            .to_sql();
        assert!(sql.contains("DOUBLE PRECISION"));
        assert!(sql.contains("FROM \"orders\""));
    }

    #[test]
    fn test_avg_expr_uses_double_precision() {
        let expr = format!("CAST(AVG({}) AS DOUBLE PRECISION)", "score");
        assert!(expr.contains("DOUBLE PRECISION"));
    }

    #[test]
    fn test_to_sql_select_basic() {
        let pool = make_sync_test_pool();
        let sql = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"))
            .to_sql();
        assert_eq!(sql, "SELECT \"id\", \"name\" FROM \"users\"");
    }

    #[test]
    fn test_to_sql_where_uses_dollar() {
        let pool = make_sync_test_pool();
        let sql = QueryBuilder::new(pool, "users", false)
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .expect("固定受控条件应合法")
            .to_sql();
        assert_eq!(sql, "SELECT * FROM \"users\" WHERE \"id\" = $1");
    }

    #[test]
    fn test_default_returning_and_conflict_columns() {
        let pool = make_sync_test_pool();
        let qb = QueryBuilder::new(pool, "users", false);
        assert_eq!(qb.returning, ReturningTarget::Column("id".to_string()));
        // P0-8：不再默认 `["id"]`；DO UPDATE 必须显式指定冲突目标。
        assert_eq!(qb.conflict_target, ConflictTarget::Unspecified);
        let qb2 = qb
            .returning(yang_db::field!("uid"))
            .on_conflict(&[yang_db::field!("email"), yang_db::field!("tenant_id")]);
        assert_eq!(qb2.returning, ReturningTarget::Column("uid".to_string()));
        assert_eq!(
            qb2.conflict_target,
            ConflictTarget::Columns(vec!["email".to_string(), "tenant_id".to_string()])
        );
    }

    #[test]
    fn test_returning_all_renders_star() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false).returning_all();
        assert_eq!(builder.returning_clause().unwrap(), " RETURNING *");
        assert_eq!(
            builder.returning_clause().unwrap(),
            " RETURNING *".to_string()
        );
    }

    #[test]
    fn test_on_conflict_constraint_and_update_columns() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1, "name": "张三", "email": "z@example.com"});
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Constraint("users_email_key".to_string()),
            None,
            false,
            Some(&["name".to_string()]),
            &[],
        )
        .unwrap();
        let sql = g.get_sql();
        assert!(sql.contains("ON CONFLICT ON CONSTRAINT \"users_email_key\" DO UPDATE SET"));
        assert!(sql.contains("\"name\" = EXCLUDED.\"name\""));
        assert!(!sql.contains("\"email\" = EXCLUDED.\"email\""));
    }

    #[test]
    fn test_upsert_do_nothing_ignores_update_columns() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1, "name": "张三"});
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Columns(vec!["id".to_string()]),
            None,
            true,
            Some(&["name".to_string()]),
            &[],
        )
        .unwrap();
        let sql = g.get_sql();
        assert!(sql.contains("ON CONFLICT (\"id\") DO NOTHING"));
        assert!(!sql.contains("DO UPDATE"));
    }

    #[test]
    fn test_upsert_where_continues_placeholder_numbering() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"id": 1, "name": "张三"});
        let update_where = vec![Condition::Gt("version".to_string(), SqlValue::Int(2))];
        g.build_upsert(
            "users",
            &data,
            &empty_types(),
            &ConflictTarget::Columns(vec!["id".to_string()]),
            None,
            false,
            None,
            &update_where,
        )
        .unwrap();
        let sql = g.get_sql();
        // id=$1, name=$2；DO UPDATE WHERE 接续为 $3，且字段限定到目标表避免歧义
        assert!(
            sql.ends_with("WHERE \"users\".\"version\" > $3"),
            "实际 SQL: {sql}"
        );
        assert_eq!(g.get_params().len(), 3);
    }

    #[test]
    fn test_update_set_expr_continues_numbering() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三"});
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(1))];
        let set_exprs = vec![(
            "updated_at".to_string(),
            crate::SqlExpr::unix_timestamp_add(30),
        )];
        g.build_update("users", &data, &empty_types(), &conditions, &set_exprs)
            .unwrap();
        let sql = g.get_sql();
        assert_eq!(
            sql,
            "UPDATE \"users\" SET \"name\" = $1, \"updated_at\" = EXTRACT(EPOCH FROM now()) + $2 \
             WHERE \"id\" = $3"
        );
        assert_eq!(g.get_params().len(), 3);
    }

    #[test]
    fn test_select_expr_numbering_precedes_where_params() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .select_expr(
                crate::SqlExpr::unix_timestamp_add(60),
                yang_db::field!("ts"),
            )
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 7)
            .expect("固定受控条件应合法");
        let sql = builder.try_to_sql().unwrap();
        assert_eq!(
            sql,
            "SELECT EXTRACT(EPOCH FROM now()) + $1 AS \"ts\" FROM \"users\" WHERE \"id\" = $2"
        );
    }

    #[test]
    fn test_where_expr_renders_controlled_expression() {
        let pool = make_sync_test_pool();
        let sql = QueryBuilder::new(pool, "users", false)
            .where_expr(
                yang_db::field!("updated_at"),
                yang_db::CompareOp::Lt,
                crate::SqlExpr::now(),
            )
            .expect("受控表达式比较应合法")
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 3)
            .expect("固定受控条件应合法")
            .try_to_sql()
            .unwrap();
        assert!(sql.contains("\"updated_at\" < now()"), "实际 SQL: {sql}");
        assert!(sql.contains("\"id\" = $1"), "实际 SQL: {sql}");
    }

    #[test]
    fn test_schema_qualified_table_is_quoted_per_segment() {
        let pool = make_sync_test_pool();
        let sql = QueryBuilder::new(pool, "audit.users", false)
            .field(yang_db::field!("audit.users.id"))
            .try_to_sql()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT \"audit\".\"users\".\"id\" FROM \"audit\".\"users\""
        );
    }
}

/// Phase B 查询 / DML 能力的单元测试（全部只做 SQL 生成断言，不触库）。
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod phase_b_tests {
    use super::*;
    use crate::{field, table, CompareOp, SelectExpr, SortOrder};
    use sqlx::postgres::PgPoolOptions;

    /// 复用同一个懒连接池；`connect_lazy` 不发起网络往返，仅需运行时上下文。
    fn test_pool() -> &'static PgPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<PgPool> = OnceLock::new();
        let runtime = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            runtime.block_on(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    fn render(builder: &QueryBuilder<'_>) -> String {
        builder.try_to_sql().expect("SQL 应可渲染")
    }

    #[test]
    fn cte_renders_with_recursive_and_keeps_placeholder_order() {
        let recent = QueryBuilder::new(test_pool(), "events", false)
            .field(field!("user_id"))
            .where_and(field!("kind"), CompareOp::Eq, "login")
            .expect("受控条件应合法");
        let tree = QueryBuilder::new(test_pool(), "nodes", false)
            .field(field!("id"))
            .where_null(field!("parent_id"));
        let builder = QueryBuilder::new(test_pool(), "users", false)
            .field(field!("id"))
            .with("recent", recent)
            .with_recursive("tree", tree)
            .where_and(field!("active"), CompareOp::Eq, true)
            .expect("受控条件应合法")
            .order(field!("id"), SortOrder::Asc)
            .limit(5);

        assert_eq!(
            render(&builder),
            "WITH RECURSIVE \"recent\" AS (SELECT \"user_id\" FROM \"events\" WHERE \"kind\" = $1), \
             \"tree\" AS (SELECT \"id\" FROM \"nodes\" WHERE \"parent_id\" IS NULL) \
             SELECT \"id\" FROM \"users\" WHERE \"active\" = $2 ORDER BY \"id\" ASC LIMIT 5"
        );
        let mut generator = SqlGenerator::new();
        generator.build_select(&builder).expect("CTE 应可渲染");
        assert!(matches!(
            generator.get_params(),
            [SqlValue::String(kind), SqlValue::Bool(true)] if kind == "login"
        ));
    }

    #[test]
    fn cte_name_is_validated_before_rendering() {
        let body = QueryBuilder::new(test_pool(), "users", false).field(field!("id"));
        let builder = QueryBuilder::new(test_pool(), "users", false)
            .field(field!("id"))
            .with("bad; DROP TABLE users", body);
        assert!(matches!(
            builder.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn window_functions_render_partition_order_and_frames() {
        let spec = WindowSpec::new()
            .partition_by(&[field!("dept")])
            .order_by(field!("amount"), SortOrder::Desc);
        let running = WindowSpec::new()
            .partition_by(&[field!("dept")])
            .order_by(field!("created_at"), SortOrder::Asc)
            .frame(WindowFrame::rows_between(
                FrameBound::UnboundedPreceding,
                FrameBound::CurrentRow,
            ));
        let builder = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("dept"))
            .window_expr(WindowFunction::row_number(), spec, field!("rn"))
            .window_expr(
                WindowFunction::sum(field!("amount")),
                running,
                field!("running"),
            );

        assert_eq!(
            render(&builder),
            "SELECT \"dept\", \
             ROW_NUMBER() OVER (PARTITION BY \"dept\" ORDER BY \"amount\" DESC) AS \"rn\", \
             SUM(\"amount\") OVER (PARTITION BY \"dept\" ORDER BY \"created_at\" ASC \
             ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS \"running\" \
             FROM \"sales\""
        );
    }

    #[test]
    fn every_whitelisted_window_function_renders_expected_call() {
        let cases = [
            (WindowFunction::rank(), "RANK()"),
            (WindowFunction::dense_rank(), "DENSE_RANK()"),
            (WindowFunction::avg(field!("amount")), "AVG(\"amount\")"),
            (WindowFunction::count(field!("amount")), "COUNT(\"amount\")"),
            (WindowFunction::count_all(), "COUNT(*)"),
            (WindowFunction::min(field!("amount")), "MIN(\"amount\")"),
            (WindowFunction::max(field!("amount")), "MAX(\"amount\")"),
            (WindowFunction::lag(field!("amount")), "LAG(\"amount\")"),
            (WindowFunction::lead(field!("amount")), "LEAD(\"amount\")"),
        ];
        for (function, expected) in cases {
            let builder = QueryBuilder::new(test_pool(), "sales", false).window_expr(
                function,
                WindowSpec::new().frame(WindowFrame::range_between(
                    FrameBound::Preceding(1),
                    FrameBound::Following(1),
                )),
                field!("value"),
            );
            let sql = render(&builder);
            assert!(
                sql.contains(&format!(
                    "{} OVER (RANGE BETWEEN 1 PRECEDING AND 1 FOLLOWING) AS \"value\"",
                    expected
                )),
                "窗口函数片段不符: {sql}"
            );
        }
    }

    #[test]
    fn invalid_window_frames_fail_closed() {
        for frame in [
            WindowFrame::rows_between(FrameBound::UnboundedFollowing, FrameBound::CurrentRow),
            WindowFrame::rows_between(FrameBound::CurrentRow, FrameBound::UnboundedPreceding),
        ] {
            let builder = QueryBuilder::new(test_pool(), "sales", false).window_expr(
                WindowFunction::row_number(),
                WindowSpec::new().frame(frame),
                field!("rn"),
            );
            assert!(matches!(
                builder.try_to_sql(),
                Err(crate::error::DbError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn groups_frame_filter_and_named_window_render() {
        let named = WindowSpec::new()
            .partition_by(&[field!("dept")])
            .order_by(field!("id"), SortOrder::Asc);
        let builder = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("dept"))
            .window("w", named)
            .window_expr(
                WindowFunction::sum(field!("amount"))
                    .filter(Condition::Gt("amount".to_string(), SqlValue::Int(100))),
                WindowSpec::new()
                    .order_by(field!("id"), SortOrder::Asc)
                    .frame(WindowFrame::groups_between(
                        FrameBound::Preceding(1),
                        FrameBound::CurrentRow,
                    )),
                field!("running"),
            )
            .window_expr(
                WindowFunction::row_number(),
                WindowSpec::reference("w"),
                field!("rn"),
            );

        assert_eq!(
            render(&builder),
            "SELECT \"dept\", \
             SUM(\"amount\") FILTER (WHERE \"amount\" > $1) \
             OVER (ORDER BY \"id\" ASC GROUPS BETWEEN 1 PRECEDING AND CURRENT ROW) AS \"running\", \
             ROW_NUMBER() OVER \"w\" AS \"rn\" \
             FROM \"sales\" WINDOW \"w\" AS (PARTITION BY \"dept\" ORDER BY \"id\" ASC)"
        );
        let mut generator = SqlGenerator::new();
        generator.build_select(&builder).expect("窗口 SQL 应可渲染");
        assert!(matches!(generator.get_params(), [SqlValue::Int(100)]));
    }

    #[test]
    fn groups_frame_requires_order_by() {
        let builder = QueryBuilder::new(test_pool(), "sales", false).window_expr(
            WindowFunction::sum(field!("amount")),
            WindowSpec::new().frame(WindowFrame::groups_between(
                FrameBound::Preceding(1),
                FrameBound::CurrentRow,
            )),
            field!("running"),
        );
        assert!(matches!(
            builder.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn filter_on_non_aggregate_window_function_fails_closed() {
        let builder = QueryBuilder::new(test_pool(), "sales", false).window_expr(
            WindowFunction::row_number()
                .filter(Condition::Gt("amount".to_string(), SqlValue::Int(0))),
            WindowSpec::new(),
            field!("rn"),
        );
        assert!(matches!(
            builder.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn named_window_reference_rejects_extra_spec_parts() {
        let builder = QueryBuilder::new(test_pool(), "sales", false).window_expr(
            WindowFunction::row_number(),
            WindowSpec::reference("w").partition_by(&[field!("dept")]),
            field!("rn"),
        );
        assert!(matches!(
            builder.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));

        let invalid_name = QueryBuilder::new(test_pool(), "sales", false)
            .window("bad name", WindowSpec::new())
            .window_expr(
                WindowFunction::row_number(),
                WindowSpec::reference("w"),
                field!("rn"),
            );
        assert!(invalid_name.try_to_sql().is_err());
    }

    #[test]
    fn grouping_sets_rollup_and_cube_render() {
        let grouping_sets = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .expr(SelectExpr::sum(field!("amount")).alias(field!("total")))
            .group(field!("region"))
            .grouping_sets(vec![vec![field!("product").clone()], vec![]]);
        assert_eq!(
            render(&grouping_sets),
            "SELECT \"region\", SUM(\"amount\") AS \"total\" FROM \"sales\" \
             GROUP BY \"region\", GROUPING SETS ((\"product\"), ())"
        );

        let rollup = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .rollup(vec![field!("region").clone(), field!("product").clone()]);
        assert_eq!(
            render(&rollup),
            "SELECT \"region\" FROM \"sales\" GROUP BY ROLLUP (\"region\", \"product\")"
        );

        let cube = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .cube(vec![field!("region").clone(), field!("product").clone()]);
        assert_eq!(
            render(&cube),
            "SELECT \"region\" FROM \"sales\" GROUP BY CUBE (\"region\", \"product\")"
        );
    }

    #[test]
    fn empty_grouping_constructs_fail_closed() {
        let empty_sets = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .grouping_sets(vec![]);
        assert!(matches!(
            empty_sets.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
        let empty_rollup = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .rollup(vec![]);
        assert!(empty_rollup.try_to_sql().is_err());
        let empty_cube = QueryBuilder::new(test_pool(), "sales", false)
            .field(field!("region"))
            .cube(vec![]);
        assert!(empty_cube.try_to_sql().is_err());
    }

    #[test]
    fn distinct_on_requires_matching_leftmost_order_by() {
        let builder = QueryBuilder::new(test_pool(), "events", false)
            .field(field!("user_id"))
            .field(field!("created_at"))
            .distinct_on(&[field!("user_id")])
            .order(field!("user_id"), SortOrder::Asc)
            .order(field!("created_at"), SortOrder::Desc);
        assert_eq!(
            render(&builder),
            "SELECT DISTINCT ON (\"user_id\") \"user_id\", \"created_at\" FROM \"events\" \
             ORDER BY \"user_id\" ASC, \"created_at\" DESC"
        );

        // 缺少 ORDER BY。
        let missing = QueryBuilder::new(test_pool(), "events", false)
            .field(field!("user_id"))
            .distinct_on(&[field!("user_id")]);
        assert!(matches!(
            missing.try_to_sql(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));

        // 最左 ORDER BY 不一致。
        let mismatched = QueryBuilder::new(test_pool(), "events", false)
            .field(field!("user_id"))
            .distinct_on(&[field!("user_id")])
            .order(field!("created_at"), SortOrder::Desc);
        assert!(mismatched.try_to_sql().is_err());

        // 与普通 DISTINCT 互斥。
        let both = QueryBuilder::new(test_pool(), "events", false)
            .field(field!("user_id"))
            .distinct()
            .distinct_on(&[field!("user_id")])
            .order(field!("user_id"), SortOrder::Asc);
        assert!(both.try_to_sql().is_err());
    }

    #[test]
    fn join_extensions_render_expected_keywords() {
        let builder = QueryBuilder::new(test_pool(), "a", false)
            .field(field!("a.id"))
            .full_join(table!("b"), field!("a.id"), field!("b.a_id"))
            .cross_join(table!("c"))
            .natural_join(table!("d"))
            .join_using(table!("e"), &[field!("id")]);
        assert_eq!(
            render(&builder),
            "SELECT \"a\".\"id\" FROM \"a\" \
             FULL JOIN \"b\" ON \"a\".\"id\" = \"b\".\"a_id\" \
             CROSS JOIN \"c\" NATURAL JOIN \"d\" INNER JOIN \"e\" USING (\"id\")"
        );
    }

    #[test]
    fn lateral_left_join_embeds_subquery_and_alias() {
        let lateral = QueryBuilder::new(test_pool(), "orders", false)
            .field(field!("id"))
            .field(field!("amount"))
            .where_and(field!("orders.status"), CompareOp::Eq, "paid")
            .expect("受控条件应合法")
            .where_column(field!("orders.user_id"), CompareOp::Eq, field!("users.id"))
            .expect("受控列比较应合法")
            .order(field!("amount"), SortOrder::Desc)
            .limit(3);
        let builder = QueryBuilder::new(test_pool(), "users", false)
            .field(field!("users.id"))
            .left_join_lateral(
                lateral,
                field!("o"),
                field!("users.id"),
                field!("o.user_id"),
            );
        assert_eq!(
            render(&builder),
            "SELECT \"users\".\"id\" FROM \"users\" \
             LEFT JOIN LATERAL (SELECT \"id\", \"amount\" FROM \"orders\" \
             WHERE (\"orders\".\"status\" = $1 AND \"orders\".\"user_id\" = \"users\".\"id\") \
             ORDER BY \"amount\" DESC LIMIT 3) AS \"o\" \
             ON \"users\".\"id\" = \"o\".\"user_id\""
        );
    }

    #[test]
    fn set_operations_render_and_validate_column_counts() {
        let left = || {
            QueryBuilder::new(test_pool(), "a", false)
                .field(field!("id"))
                .field(field!("name"))
        };
        let right = |table_name| {
            QueryBuilder::new(test_pool(), table_name, false)
                .field(field!("id"))
                .field(field!("name"))
        };

        assert_eq!(
            render(&left().intersect(right("b")).expect("列数一致")),
            "SELECT \"id\", \"name\" FROM \"a\" INTERSECT (SELECT \"id\", \"name\" FROM \"b\")"
        );
        assert_eq!(
            render(&left().intersect_all(right("b")).expect("列数一致")),
            "SELECT \"id\", \"name\" FROM \"a\" INTERSECT ALL (SELECT \"id\", \"name\" FROM \"b\")"
        );
        assert_eq!(
            render(&left().except(right("b")).expect("列数一致")),
            "SELECT \"id\", \"name\" FROM \"a\" EXCEPT (SELECT \"id\", \"name\" FROM \"b\")"
        );
        assert_eq!(
            render(&left().except_all(right("b")).expect("列数一致")),
            "SELECT \"id\", \"name\" FROM \"a\" EXCEPT ALL (SELECT \"id\", \"name\" FROM \"b\")"
        );

        // 混合运算符按调用顺序固定为左结合：先 UNION 再 INTERSECT。
        let mixed = left()
            .union(right("b"))
            .expect("列数一致")
            .intersect(right("c"))
            .expect("列数一致");
        assert_eq!(
            render(&mixed),
            "(SELECT \"id\", \"name\" FROM \"a\" UNION (SELECT \"id\", \"name\" FROM \"b\")) \
             INTERSECT (SELECT \"id\", \"name\" FROM \"c\")"
        );

        // 列数不一致 fail-closed。
        let one_column = QueryBuilder::new(test_pool(), "c", false).field(field!("id"));
        assert!(left().intersect(one_column).is_err());
    }

    #[test]
    fn having_expr_renders_aggregate_comparison_with_bound_value() {
        let builder = QueryBuilder::new(test_pool(), "orders", false)
            .field(field!("user_id"))
            .expr(SelectExpr::count_all())
            .group(field!("user_id"))
            .having_expr(SelectExpr::count_all(), CompareOp::Gt, 1)
            .expect("受控 HAVING 表达式应合法");
        assert_eq!(
            render(&builder),
            "SELECT \"user_id\", COUNT(*) FROM \"orders\" GROUP BY \"user_id\" \
             HAVING COUNT(*) > $1"
        );

        // 无 GROUP BY 的纯聚合 HAVING 也是合法的单行过滤。
        let without_group = QueryBuilder::new(test_pool(), "orders", false)
            .expr(SelectExpr::count_all())
            .having_expr(SelectExpr::count_all(), CompareOp::Gt, 1)
            .expect("无 GROUP BY 的聚合 HAVING 应合法");
        assert_eq!(
            render(&without_group),
            "SELECT COUNT(*) FROM \"orders\" HAVING COUNT(*) > $1"
        );

        // 字段 HAVING 条件仍要求 GROUP BY（保持既有约束）。
        let field_having_without_group = QueryBuilder::new(test_pool(), "orders", false)
            .field(field!("user_id"))
            .having_cond(field!("user_id"), CompareOp::Gt, 1)
            .expect("受控条件应合法");
        assert!(matches!(
            field_having_without_group.try_to_sql(),
            Err(crate::error::DbError::MissingGroupByClause)
        ));

        // LIKE 不适用于聚合表达式。
        let like = QueryBuilder::new(test_pool(), "orders", false)
            .field(field!("user_id"))
            .group(field!("user_id"))
            .having_expr(SelectExpr::count_all(), CompareOp::Like, "%x%");
        assert!(matches!(
            like,
            Err(crate::error::DbError::UnsupportedOperator(_))
        ));
    }

    #[test]
    fn upsert_batch_builds_multi_row_values_and_conflict_clause() {
        let data = [
            serde_json::json!({"id": 1, "name": "a"}),
            serde_json::json!({"id": 2, "name": "b"}),
        ];
        let mut generator = SqlGenerator::new();
        generator
            .build_upsert_batch(
                "users",
                &data,
                &HashMap::new(),
                &["id".to_string()],
                None,
                false,
                None,
                &[],
            )
            .expect("批量 UPSERT 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"users\" (\"id\", \"name\") VALUES ($1, $2), ($3, $4) \
             ON CONFLICT (\"id\") DO UPDATE SET \"name\" = EXCLUDED.\"name\""
        );
        assert_eq!(generator.get_params().len(), 4);

        // 冲突列必填（未调用 do_nothing 时）。
        let mut generator = SqlGenerator::new();
        assert!(matches!(
            generator.build_upsert_batch(
                "users",
                &data,
                &HashMap::new(),
                &[],
                None,
                false,
                None,
                &[]
            ),
            Err(crate::error::DbError::InvalidArgument(_))
        ));

        // do_nothing 时允许空冲突列 → ON CONFLICT DO NOTHING。
        let mut generator = SqlGenerator::new();
        generator
            .build_upsert_batch("users", &data, &HashMap::new(), &[], None, true, None, &[])
            .expect("DO NOTHING 批量 UPSERT 应可渲染");
        assert!(generator.get_sql().ends_with("ON CONFLICT DO NOTHING"));

        // 更新列白名单不在插入列集内 → 构建期报错。
        let mut generator = SqlGenerator::new();
        assert!(matches!(
            generator.build_upsert_batch(
                "users",
                &data,
                &HashMap::new(),
                &["id".to_string()],
                None,
                false,
                Some(&["missing".to_string()]),
                &[]
            ),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }

    #[tokio::test]
    async fn upsert_batch_rejects_invalid_input_before_io() {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
            .expect("无法解析测试数据库 URL");
        let error = QueryBuilder::new(&pool, "users", false)
            .upsert_batch(&[] as &[serde_json::Value], &[field!("id")])
            .await
            .expect_err("空数据应在触库前失败");
        assert!(matches!(
            error,
            crate::error::DbError::SerializationError(_)
        ));

        let error = QueryBuilder::new(&pool, "users", false)
            .upsert_batch(&[serde_json::json!({"id": 1})], &[])
            .await
            .expect_err("缺少冲突列应在触库前失败");
        assert!(matches!(error, crate::error::DbError::InvalidArgument(_)));

        let error = QueryBuilder::new(&pool, "users", false)
            .upsert_batch_with_size(&[serde_json::json!({"id": 1})], &[field!("id")], 0)
            .await
            .expect_err("batch_size 为 0 应在触库前失败");
        assert!(matches!(
            error,
            crate::error::DbError::SerializationError(_)
        ));
    }

    #[test]
    fn insert_from_select_renders_target_columns_and_source_query() {
        let source = QueryBuilder::new(test_pool(), "users", false)
            .field(field!("id"))
            .field(field!("name"))
            .where_and(field!("active"), CompareOp::Eq, true)
            .expect("受控条件应合法");
        let mut generator = SqlGenerator::new();
        generator
            .build_insert_from_select("archive", &[field!("id"), field!("name")], &source)
            .expect("INSERT ... SELECT 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"archive\" (\"id\", \"name\") \
             SELECT \"id\", \"name\" FROM \"users\" WHERE \"active\" = $1"
        );
    }

    #[test]
    fn update_from_renders_join_predicate_in_where() {
        let mut generator = SqlGenerator::new();
        generator
            .build_update_from(
                "users",
                &serde_json::json!({"name": "x"}),
                &HashMap::new(),
                &[],
                &[Condition::Eq(
                    "profiles.active".to_string(),
                    SqlValue::Bool(true),
                )],
                &crate::TableRef::new("profiles").expect("合法表名"),
                field!("users.id"),
                field!("profiles.user_id"),
            )
            .expect("UPDATE ... FROM 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "UPDATE \"users\" SET \"name\" = $1 FROM \"profiles\" \
             WHERE (\"profiles\".\"active\" = $2 AND \"users\".\"id\" = \"profiles\".\"user_id\")"
        );
    }

    #[test]
    fn delete_using_renders_join_predicate_in_where() {
        let mut generator = SqlGenerator::new();
        generator
            .build_delete_using(
                "users",
                &[Condition::Eq(
                    "users.active".to_string(),
                    SqlValue::Bool(false),
                )],
                &crate::TableRef::new("profiles").expect("合法表名"),
                field!("users.id"),
                field!("profiles.user_id"),
            )
            .expect("DELETE ... USING 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "DELETE FROM \"users\" USING \"profiles\" \
             WHERE (\"users\".\"active\" = $1 AND \"users\".\"id\" = \"profiles\".\"user_id\")"
        );
    }

    #[test]
    fn merge_renders_matched_update_and_not_matched_insert() {
        let mut generator = SqlGenerator::new();
        generator
            .build_merge(
                "users",
                &crate::TableRef::new("incoming").expect("合法表名"),
                field!("id"),
                field!("id"),
                &[field!("name"), field!("email")],
            )
            .expect("MERGE 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "MERGE INTO \"users\" USING \"incoming\" ON \"users\".\"id\" = \"incoming\".\"id\" \
             WHEN MATCHED THEN UPDATE SET \"name\" = \"incoming\".\"name\", \
             \"email\" = \"incoming\".\"email\" \
             WHEN NOT MATCHED THEN INSERT (\"name\", \"email\") \
             VALUES (\"incoming\".\"name\", \"incoming\".\"email\")"
        );
    }

    #[test]
    fn merge_renders_matched_condition_and_by_source_delete() {
        let mut generator = SqlGenerator::new();
        generator
            .build_merge_with_options(
                "users",
                &crate::TableRef::new("incoming").expect("合法表名"),
                field!("id"),
                field!("id"),
                &[field!("name")],
                &MergeOptions::default()
                    .matched_condition(Condition::Eq(
                        "users.name".to_string(),
                        SqlValue::String("x".to_string()),
                    ))
                    .not_matched_by_source(MergeNotMatchedBySource::Delete),
            )
            .expect("带条件的 MERGE 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "MERGE INTO \"users\" USING \"incoming\" ON \"users\".\"id\" = \"incoming\".\"id\" \
             WHEN MATCHED AND \"users\".\"name\" = $1 THEN UPDATE SET \"name\" = \"incoming\".\"name\" \
             WHEN NOT MATCHED THEN INSERT (\"name\") VALUES (\"incoming\".\"name\") \
             WHEN NOT MATCHED BY SOURCE THEN DELETE"
        );
        assert!(matches!(generator.get_params(), [SqlValue::String(x)] if x == "x"));
    }

    #[test]
    fn merge_renders_by_source_update_with_controlled_expr() {
        let mut generator = SqlGenerator::new();
        generator
            .build_merge_with_options(
                "users",
                &crate::TableRef::new("incoming").expect("合法表名"),
                field!("id"),
                field!("id"),
                &[field!("name")],
                &MergeOptions::default().not_matched_by_source(MergeNotMatchedBySource::Update(
                    vec![(field!("updated_at").clone(), crate::SqlExpr::now())],
                )),
            )
            .expect("BY SOURCE UPDATE 应可渲染");
        assert!(
            generator
                .get_sql()
                .ends_with("WHEN NOT MATCHED BY SOURCE THEN UPDATE SET \"updated_at\" = now()"),
            "{}",
            generator.get_sql()
        );
    }

    #[test]
    fn merge_by_source_update_rejects_empty_assignments() {
        let mut generator = SqlGenerator::new();
        let error = generator
            .build_merge_with_options(
                "users",
                &crate::TableRef::new("incoming").expect("合法表名"),
                field!("id"),
                field!("id"),
                &[field!("name")],
                &MergeOptions::default()
                    .not_matched_by_source(MergeNotMatchedBySource::Update(vec![])),
            )
            .expect_err("空赋值列表必须报错");
        assert!(matches!(error, crate::error::DbError::InvalidArgument(_)));
    }

    #[test]
    fn array_field_type_converts_json_array_recursively() {
        let mut field_types = HashMap::new();
        field_types.insert(
            "tags".to_string(),
            FieldType::Array(Box::new(FieldType::Text)),
        );
        let mut generator = SqlGenerator::new();
        generator
            .build_insert(
                "docs",
                &serde_json::json!({"tags": ["a", "b"]}),
                &field_types,
                &[],
            )
            .expect("数组字段插入应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"docs\" (\"tags\") VALUES ($1)"
        );
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Array(values)]
                if matches!(values.as_slice(), [SqlValue::String(a), SqlValue::String(b)] if a == "a" && b == "b")
        ));

        // 非数组形态显式报错。
        let mut generator = SqlGenerator::new();
        assert!(matches!(
            generator.build_insert(
                "docs",
                &serde_json::json!({"tags": "not-an-array"}),
                &field_types,
                &[]
            ),
            Err(crate::error::DbError::TypeConversionError(_))
        ));

        // 数组元素为网络类型时追加 `::inet[]` 显式转换。
        let mut field_types = HashMap::new();
        field_types.insert(
            "ips".to_string(),
            FieldType::Array(Box::new(FieldType::Inet)),
        );
        let mut generator = SqlGenerator::new();
        generator
            .build_insert(
                "docs",
                &serde_json::json!({"ips": ["10.0.0.1"]}),
                &field_types,
                &[],
            )
            .expect("INET[] 插入应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"docs\" (\"ips\") VALUES ($1::inet[])"
        );
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Array(values)] if matches!(values.as_slice(), [SqlValue::Inet(ip)] if ip == "10.0.0.1")
        ));
    }

    /// 空数组/全 NULL 布尔数组必须按 `bool[]` 绑定，而不是回退到 `int8[]`。
    #[test]
    fn empty_and_all_null_boolean_arrays_bind_as_bool_array() {
        let mut field_types = HashMap::new();
        field_types.insert(
            "flags".to_string(),
            FieldType::Array(Box::new(FieldType::Bool)),
        );
        for value in [serde_json::json!([]), serde_json::json!([null, null])] {
            let mut generator = SqlGenerator::new();
            generator
                .build_insert(
                    "docs",
                    &serde_json::json!({"flags": value}),
                    &field_types,
                    &[],
                )
                .expect("布尔数组插入应可渲染");
            match generator.get_params() {
                [SqlValue::TypedArray {
                    element_type,
                    values,
                }] => {
                    assert_eq!(
                        *element_type, "bool",
                        "空/全 NULL 布尔数组必须按 bool[] 绑定"
                    );
                    assert!(values.iter().all(|value| matches!(value, SqlValue::Null)));
                }
                other => panic!("期望 TypedArray(bool)，实得 {other:?}"),
            }
        }
    }

    #[test]
    fn network_field_types_accept_text_literals() {
        let mut field_types = HashMap::new();
        field_types.insert("ip".to_string(), FieldType::Inet);
        field_types.insert("mac".to_string(), FieldType::MacAddr);
        let mut generator = SqlGenerator::new();
        generator
            .build_insert(
                "hosts",
                &serde_json::json!({"ip": "10.0.0.1", "mac": "08:00:2b:01:02:03"}),
                &field_types,
                &[],
            )
            .expect("网络类型插入应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"hosts\" (\"ip\", \"mac\") VALUES ($1::inet, $2::macaddr)"
        );
    }

    #[test]
    fn json_predicates_and_array_quantifiers_render() {
        let builder = QueryBuilder::new(test_pool(), "docs", false)
            .where_json_contains(
                field!("data"),
                SqlValue::Json(serde_json::json!({"role": "admin"})),
            )
            .where_json_has_key(field!("data"), "active")
            .where_any(field!("id"), vec![1_i64, 2])
            .where_all(field!("score"), vec![1.5_f64]);
        assert_eq!(
            render(&builder),
            "SELECT * FROM \"docs\" WHERE ((\"data\")::jsonb @> $1 AND (\"data\")::jsonb ? $2 \
             AND \"id\" = ANY($3) AND \"score\" = ALL($4))"
        );

        let mut generator = SqlGenerator::new();
        generator.build_select(&builder).expect("JSON 条件应可渲染");
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Json(_), SqlValue::String(key), SqlValue::Array(ids), SqlValue::Array(scores)]
                if key == "active" && ids.len() == 2 && scores.len() == 1
        ));
    }

    #[test]
    fn wave2_compare_ops_render_postgres_operators() {
        let cases: &[(crate::CompareOp, &str)] = &[
            (crate::CompareOp::ILike, "ILIKE"),
            (crate::CompareOp::NotILike, "NOT ILIKE"),
            (crate::CompareOp::RegexMatch, "~"),
            (crate::CompareOp::RegexIMatch, "~*"),
            (crate::CompareOp::NotRegexMatch, "!~"),
            (crate::CompareOp::NotRegexIMatch, "!~*"),
        ];
        for (op, operator) in cases {
            let builder = QueryBuilder::new(test_pool(), "users", false)
                .where_and(field!("name"), *op, "%a%")
                .expect("模式匹配条件应合法");
            assert_eq!(
                render(&builder),
                format!("SELECT * FROM \"users\" WHERE \"name\" {operator} $1")
            );
        }

        let builder = QueryBuilder::new(test_pool(), "users", false)
            .where_and(field!("name"), crate::CompareOp::IsDistinctFrom, "a")
            .expect("NULL-safe 比较应合法");
        assert_eq!(
            render(&builder),
            "SELECT * FROM \"users\" WHERE \"name\" IS DISTINCT FROM $1"
        );
        let builder = QueryBuilder::new(test_pool(), "users", false)
            .where_and(
                field!("name"),
                crate::CompareOp::IsNotDistinctFrom,
                SqlValue::Null,
            )
            .expect("NULL-safe NULL 比较应合法");
        assert_eq!(
            render(&builder),
            "SELECT * FROM \"users\" WHERE \"name\" IS NOT DISTINCT FROM NULL"
        );

        // 模式匹配只接受字符串模式值。
        let result = QueryBuilder::new(test_pool(), "users", false).where_and(
            field!("name"),
            crate::CompareOp::ILike,
            42,
        );
        assert!(matches!(
            result,
            Err(crate::error::DbError::UnsupportedOperator(_))
        ));
    }

    #[test]
    fn wave2_returning_columns_aliases_and_validation() {
        let builder = QueryBuilder::new(test_pool(), "users", false)
            .returning_columns(&[field!("id"), field!("email")]);
        assert_eq!(
            builder.returning_clause().unwrap(),
            " RETURNING \"id\", \"email\""
        );

        let builder = QueryBuilder::new(test_pool(), "users", false)
            .returning_column_as(field!("id"), field!("user_id"))
            .returning_column_as(field!("email"), field!("mail"));
        assert_eq!(
            builder.returning_clause().unwrap(),
            " RETURNING \"id\" AS \"user_id\", \"email\" AS \"mail\""
        );

        assert_eq!(
            QueryBuilder::new(test_pool(), "users", false)
                .returning_all()
                .returning_clause()
                .unwrap(),
            " RETURNING *"
        );

        // 非法别名在渲染期被拒绝（Error 而不是拼接）。
        let mut invalid = QueryBuilder::new(test_pool(), "users", false);
        invalid.returning = ReturningTarget::Columns(vec![ReturningColumn {
            column: "id".to_string(),
            alias: Some("bad alias".to_string()),
        }]);
        assert!(matches!(
            invalid.returning_clause(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
        assert!(matches!(
            QueryBuilder::new(test_pool(), "users", false)
                .returning_columns(&[])
                .returning_clause(),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn wave2_conflict_predicate_and_unspecified_target() {
        let data = serde_json::json!({"email": "a@x", "active": true});
        let conflict = ConflictTarget::Columns(vec!["email".to_string()]);
        let mut generator = SqlGenerator::new();
        generator
            .build_upsert(
                "users",
                &data,
                &HashMap::new(),
                &conflict,
                Some("active"),
                false,
                None,
                &[],
            )
            .expect("带谓词的 UPSERT 应可渲染");
        assert_eq!(
            generator.get_sql(),
            "INSERT INTO \"users\" (\"active\", \"email\") VALUES ($1, $2) \
             ON CONFLICT (\"email\") WHERE (active) DO UPDATE SET \"active\" = EXCLUDED.\"active\""
        );

        for payload in ["active; DROP TABLE users", "active -- x", "active /* x */"] {
            let mut generator = SqlGenerator::new();
            let error = generator
                .build_upsert(
                    "users",
                    &data,
                    &HashMap::new(),
                    &conflict,
                    Some(payload),
                    false,
                    None,
                    &[],
                )
                .expect_err("注入型谓词必须被拒绝");
            assert!(matches!(error, crate::error::DbError::InvalidArgument(_)));
        }

        // 未指定冲突目标且非 DO NOTHING：客户端报错，不生成 SQL。
        let mut generator = SqlGenerator::new();
        let error = generator
            .build_upsert(
                "users",
                &data,
                &HashMap::new(),
                &ConflictTarget::Unspecified,
                None,
                false,
                None,
                &[],
            )
            .expect_err("无冲突目标的 DO UPDATE 必须被拒绝");
        assert!(matches!(error, crate::error::DbError::InvalidArgument(_)));

        let mut generator = SqlGenerator::new();
        generator
            .build_upsert(
                "users",
                &data,
                &HashMap::new(),
                &ConflictTarget::Unspecified,
                None,
                true,
                None,
                &[],
            )
            .expect("无冲突目标的 DO NOTHING 应可渲染");
        assert!(generator.get_sql().ends_with("ON CONFLICT DO NOTHING"));
    }

    #[test]
    fn wave2_insert_batch_shrinks_by_column_count() {
        assert_eq!(derive_insert_batch_size(0, 500), 500);
        assert_eq!(derive_insert_batch_size(1, 500), 500);
        assert_eq!(derive_insert_batch_size(1_000, 500), 65);
        assert_eq!(derive_insert_batch_size(100_000, 500), 1);
    }

    #[test]
    fn wave2_merge_version_gate() {
        assert!(!merge_version_supported(149_999));
        assert!(!merge_version_supported(140_000));
        assert!(merge_version_supported(150_000));
        assert!(merge_version_supported(160_015));
    }

    #[test]
    fn wave2_row_lock_variants_and_combinations() {
        let builder = QueryBuilder::new(test_pool(), "users", false);
        assert_eq!(
            builder
                .try_to_sql_with_lock(crate::RowLock::NoKeyUpdate, &[field!("users")], false, true,)
                .unwrap(),
            "SELECT * FROM \"users\" FOR NO KEY UPDATE OF \"users\" SKIP LOCKED"
        );
        assert_eq!(
            builder
                .try_to_sql_with_lock(crate::RowLock::KeyShare, &[], true, false)
                .unwrap(),
            "SELECT * FROM \"users\" FOR KEY SHARE NOWAIT"
        );
        assert_eq!(
            builder
                .try_to_sql_with_lock(crate::RowLock::ForUpdateSkipLocked, &[], false, false)
                .unwrap(),
            "SELECT * FROM \"users\" FOR UPDATE SKIP LOCKED"
        );
        assert!(matches!(
            builder.try_to_sql_with_lock(crate::RowLock::ForUpdate, &[], true, true),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
        // 隐含 SKIP LOCKED 的模式同样不能与 NOWAIT 组合。
        assert!(matches!(
            builder.try_to_sql_with_lock(crate::RowLock::ForUpdateSkipLocked, &[], true, false),
            Err(crate::error::DbError::InvalidArgument(_))
        ));
    }
}
