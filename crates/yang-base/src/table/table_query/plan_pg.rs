//! PostgreSQL 查询计划编译（`postgres` feature）。
//!
//! 复用与 MySQL 完全相同的权限校验、软删守卫、租户条件、分页与排序输入；差异只在
//! 底层 `yang_db::postgres::QueryBuilder` 的构造与字段类型登记。所有参与 WHERE /
//! 写入的字段都会在注入 builder 前登记 [`PgFieldType`]，保证 JSON(B)、DECIMAL、
//! BYTEA、DATE / TIMESTAMPTZ 等类型的参数按原生类型绑定。

#![cfg(feature = "postgres")]

use super::TableQuery;
use crate::error::BaseError;
use crate::table::{FieldType, SortOrder, WhereCondition};
use serde_json::Value;
use yang_db::postgres::{
    Condition as PgCondition, FieldType as PgFieldType, QueryBuilder as PgQueryBuilder,
    SqlValue as PgSqlValue,
};

/// 把应用层字段类型映射为 PostgreSQL 绑定类型。
///
/// 整数 / 浮点 / 布尔在 PostgreSQL 中由服务端按列类型推导，使用
/// [`PgFieldType::Standard`]（无需特殊转换）；有专用绑定语义的类型逐一映射：
/// `DateTime` → `TIMESTAMPTZ`、`Timestamp`（Unix 秒）→ `BIGINT`、`Json` → `JSONB`。
pub(super) fn pg_field_type(field_type: &FieldType) -> PgFieldType {
    match field_type {
        FieldType::String { .. } | FieldType::Text | FieldType::Enum { .. } => PgFieldType::Text,
        FieldType::Integer | FieldType::BigInt | FieldType::Float | FieldType::Double => {
            PgFieldType::Standard
        }
        // 布尔单独映射：空/全 NULL 布尔数组没有元素类型线索，必须按 bool[]
        // 绑定，回退到 Standard(int8[]) 会与 boolean[] 列类型不匹配（42804）。
        FieldType::Boolean => PgFieldType::Bool,
        FieldType::Decimal { .. } => PgFieldType::Decimal,
        FieldType::Date => PgFieldType::Date,
        FieldType::DateTime => PgFieldType::TimestampTz,
        FieldType::Timestamp => PgFieldType::Timestamp,
        FieldType::Json => PgFieldType::Jsonb,
        FieldType::Uuid => PgFieldType::Uuid,
        FieldType::Bytes => PgFieldType::Blob,
        FieldType::Array(element) => PgFieldType::Array(Box::new(pg_field_type(element))),
    }
}

/// 把应用层数组元素类型映射为 PostgreSQL 内置元素类型名。
///
/// [`PgSqlValue::TypedArray`] 用它决定绑定的 Rust 数组类型：`int4[]` 必须绑定为
/// `i32` 数组（绑定成 `int8[]` 会因 `integer[] = bigint[]` 无运算符报 42883）。
fn pg_array_element_type(element: &FieldType) -> Result<&'static str, BaseError> {
    Ok(match element {
        FieldType::Integer => "int4",
        FieldType::BigInt => "int8",
        FieldType::Float => "float4",
        FieldType::Double => "float8",
        FieldType::Boolean => "bool",
        FieldType::String { .. } | FieldType::Enum { .. } => "varchar",
        FieldType::Text => "text",
        FieldType::Decimal { .. } => "numeric",
        FieldType::Date => "date",
        FieldType::DateTime => "timestamptz",
        FieldType::Timestamp => "int8",
        FieldType::Json => "jsonb",
        FieldType::Uuid => "uuid",
        FieldType::Bytes => "bytea",
        FieldType::Array(inner) => {
            return Err(BaseError::UnsupportedCapability(format!(
                "数组等值/排序谓词不支持嵌套数组元素类型: {inner:?}"
            )))
        }
    })
}

/// 把 JSON 值转换为 PostgreSQL 原生绑定值（未做字段类型特化的通用路径）。
fn plain_pg_value(value: &Value) -> PgSqlValue {
    match value {
        Value::Null => PgSqlValue::Null,
        Value::Bool(value) => PgSqlValue::Bool(*value),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                PgSqlValue::Int(value)
            } else if let Some(value) = number.as_u64() {
                PgSqlValue::from(value)
            } else if let Some(value) = number.as_f64() {
                PgSqlValue::Float(value)
            } else {
                PgSqlValue::String(number.to_string())
            }
        }
        Value::String(value) => PgSqlValue::String(value.clone()),
        Value::Array(_) | Value::Object(_) => PgSqlValue::Json(value.clone()),
    }
}

/// 按字段类型转换条件值。
///
/// PostgreSQL 的扩展协议会按 Rust 类型声明参数 OID：`&str` 绑定为 `text`，因此
/// `date_col = $1` / `numeric_col = $1` / `jsonb_col = $1` 会因缺少交叉类型运算符
/// 报 42883。这里把 Date / DateTime / Decimal / Json 转换为原生 `SqlValue` 变体，
/// 其余类型沿用 JSON 自然映射（数值 → INT/FLOAT、布尔 → BOOL、字符串 → TEXT）。
fn pg_condition_value(
    field: &str,
    field_type: &FieldType,
    value: &Value,
) -> Result<PgSqlValue, BaseError> {
    if value.is_null() {
        return Ok(PgSqlValue::Null);
    }
    let invalid = |detail: String| BaseError::InvalidFieldType(field.to_string(), detail);
    match field_type {
        FieldType::Date => {
            let text = value
                .as_str()
                .ok_or_else(|| invalid(format!("期望日期字符串，实际: {value}")))?;
            let date = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .map_err(|error| invalid(format!("日期格式无效（期望 YYYY-MM-DD）: {error}")))?;
            Ok(PgSqlValue::Date(date))
        }
        FieldType::DateTime => {
            let text = value
                .as_str()
                .ok_or_else(|| invalid(format!("期望日期时间字符串，实际: {value}")))?;
            let datetime = chrono::DateTime::parse_from_rfc3339(text)
                .map_err(|error| invalid(format!("日期时间格式无效（期望 RFC 3339）: {error}")))?
                .with_timezone(&chrono::Utc);
            Ok(PgSqlValue::DateTimeTz(datetime))
        }
        FieldType::Decimal { .. } => {
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let decimal: sqlx::types::BigDecimal = text
                .parse()
                .map_err(|error| invalid(format!("DECIMAL 值无效: {error}")))?;
            Ok(PgSqlValue::Decimal(decimal))
        }
        FieldType::Json => Ok(PgSqlValue::Json(value.clone())),
        FieldType::Uuid => {
            let text = value
                .as_str()
                .ok_or_else(|| invalid(format!("期望 UUID 字符串，实际: {value}")))?;
            let uuid = sqlx::types::Uuid::parse_str(text)
                .map_err(|error| invalid(format!("UUID 格式无效: {error}")))?;
            Ok(PgSqlValue::Uuid(uuid))
        }
        FieldType::Bytes => {
            use base64::Engine;
            let text = value
                .as_str()
                .ok_or_else(|| invalid(format!("期望 Base64 二进制字符串，实际: {value}")))?;
            // 与 yang-db 的 Blob 写入路径一致：非法 Base64 退化为原始字节。
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text)
                .unwrap_or_else(|_| text.as_bytes().to_vec());
            Ok(PgSqlValue::Bytes(bytes))
        }
        FieldType::Array(element) => {
            let items = value
                .as_array()
                .ok_or_else(|| invalid(format!("期望 JSON 数组，实际: {value}")))?;
            let converted = items
                .iter()
                .map(|item| pg_condition_value(field, element, item))
                .collect::<Result<Vec<_>, _>>()?;
            // INTEGER[] 的列类型是 int4[]：高于 i32 的元素必须显式报错，
            // 否则绑定期会被静默截断为 NULL。
            if matches!(element.as_ref(), FieldType::Integer) {
                for item in &converted {
                    if let PgSqlValue::Int(value) = item {
                        if i32::try_from(*value).is_err() {
                            return Err(invalid(format!("整数数组元素超出 int4 范围: {value}")));
                        }
                    }
                }
            }
            Ok(PgSqlValue::TypedArray {
                element_type: pg_array_element_type(element)?,
                values: converted,
            })
        }
        _ => Ok(plain_pg_value(value)),
    }
}

/// 构造 `"field" <dir> NULLS FIRST|LAST, "field"` 的受控排序表达式。
///
/// `yang-db` 的 `QueryBuilder::order` 只接受 [`yang_db::FieldRef`]，渲染时整体加
/// 引号，无法直接表达 `NULLS FIRST/LAST`。这里用其 `#[doc(hidden)]` 的可信构造器
/// 嵌入已加引号的片段；字段名来自构建期校验过的表定义，不存在注入风险。
///
/// ponytail: 依赖 `order()` 对 FieldRef 整体加引号的实现，只支持单段字段名；
/// yang-db 提供原生 nulls 排序 API 后替换为此处一行调用。
fn pg_nulls_order_expr(
    field: &str,
    order: SortOrder,
    nulls_first: bool,
) -> Result<yang_db::FieldRef, BaseError> {
    if field.is_empty() || field.contains('.') {
        return Err(BaseError::UnsupportedCapability(format!(
            "NULLS 排序仅支持单段字段名: {field}"
        )));
    }
    let dir = match order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };
    let nulls = if nulls_first {
        "NULLS FIRST"
    } else {
        "NULLS LAST"
    };
    Ok(yang_db::FieldRef::__from_validated_owned(format!(
        "{field}\" {dir} {nulls}, \"{field}"
    )))
}

/// 递归收集条件树中所有叶子字段名（逻辑组自身没有字段）。
fn condition_field_names<'a>(condition: &'a WhereCondition, out: &mut Vec<&'a str>) {
    match condition {
        WhereCondition::And { conditions } | WhereCondition::Or { conditions } => {
            for child in conditions {
                condition_field_names(child, out);
            }
        }
        leaf => {
            if let Some(field) = leaf.field() {
                out.push(field);
            }
        }
    }
}

impl TableQuery {
    /// 基于当前连接池构造 PostgreSQL `Database` 包装。
    ///
    /// `yang_db::postgres::QueryBuilder` 的公开构造入口是 `Database::table()`，
    /// 因此这里以默认配置重建轻量包装（内部仍共享同一 `PgPool`）。
    pub(super) fn pg_database(&self) -> Result<yang_db::postgres::Database, BaseError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;
        let pg_pool = pool.as_postgres().ok_or_else(|| {
            BaseError::UnsupportedCapability(
                "当前连接池不是 PostgreSQL 后端，无法执行 PostgreSQL 查询计划".to_string(),
            )
        })?;
        yang_db::postgres::Database::from_pool(
            pg_pool.clone(),
            yang_db::postgres::DatabaseConfig::default(),
        )
        .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 在 builder 上登记单个字段的 PostgreSQL 绑定类型。
    pub(super) fn register_pg_field<'a>(
        &self,
        query: PgQueryBuilder<'a>,
        name: &str,
    ) -> Result<PgQueryBuilder<'a>, BaseError> {
        let field = self.field_ref(name)?;
        let config = self.table_config.get_field(name).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), name.to_string())
        })?;
        Ok(query.field_type(field, pg_field_type(&config.field_type)))
    }

    /// 批量登记字段类型；重复登记同一字段无副作用。
    pub(super) fn register_pg_fields<'a, I>(
        &self,
        mut query: PgQueryBuilder<'a>,
        names: I,
    ) -> Result<PgQueryBuilder<'a>, BaseError>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        for name in names {
            query = self.register_pg_field(query, name.as_ref())?;
        }
        Ok(query)
    }

    /// 读路径计划：投影 + WHERE/软删守卫 + 排序 + 分页。
    pub(super) fn apply_pg_read_plan<'a>(
        &self,
        mut query: PgQueryBuilder<'a>,
    ) -> Result<PgQueryBuilder<'a>, BaseError> {
        let selected = self.query_params.fields.as_ref().map_or_else(
            || {
                self.default_read_fields()
                    .map(|values| values.into_iter().map(str::to_string).collect())
            },
            |values| Ok(values.clone()),
        )?;
        for name in selected {
            query = self.register_pg_field(query, &name)?;
            let field = self.field_ref(&name)?;
            query = query.field(field);
        }
        query = self.apply_pg_where_guards(query)?;
        let orders = if self.query_params.order_by.is_empty() {
            &self.table_config.default_order
        } else {
            &self.query_params.order_by
        };
        for (name, order) in orders {
            let db_order = match order {
                SortOrder::Asc => yang_db::SortOrder::Asc,
                SortOrder::Desc => yang_db::SortOrder::Desc,
            };
            if let Some(nulls_first) = self.nulls_orders.get(name) {
                query = query.order(&pg_nulls_order_expr(name, *order, *nulls_first)?, db_order);
            } else {
                let field = self.field_ref(name)?;
                query = query.order(field, db_order);
            }
        }
        if let Some(page_size) = self.query_params.page_size {
            let page = self.query_params.page.unwrap_or(1).max(1);
            let limit = u64::try_from(page_size).map_err(|_| {
                BaseError::ParamInvalid("page_size".to_string(), "分页大小超出范围".to_string())
            })?;
            let offset = u64::try_from((page - 1).saturating_mul(page_size)).map_err(|_| {
                BaseError::ParamInvalid("page".to_string(), "分页偏移超出范围".to_string())
            })?;
            query = query.limit(limit).offset(offset);
        }
        Ok(query)
    }

    /// 写路径计划：只保留 WHERE/软删守卫。
    pub(super) fn apply_pg_write_plan<'a>(
        &self,
        query: PgQueryBuilder<'a>,
    ) -> Result<PgQueryBuilder<'a>, BaseError> {
        self.apply_pg_where_guards(query)
    }

    /// 把受保护 WHERE 条件编译为带原生类型值的 `PgCondition` 树。
    fn pg_condition(&self, condition: &WhereCondition) -> Result<PgCondition, BaseError> {
        match condition {
            WhereCondition::And { conditions } => Ok(PgCondition::And(
                conditions
                    .iter()
                    .map(|child| self.pg_condition(child))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            WhereCondition::Or { conditions } => Ok(PgCondition::Or(
                conditions
                    .iter()
                    .map(|child| self.pg_condition(child))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            leaf => {
                let name = leaf.field().ok_or_else(|| {
                    BaseError::ParamInvalid(
                        "condition".to_string(),
                        "条件节点缺少字段名".to_string(),
                    )
                })?;
                let config = self.table_config.get_field(name).ok_or_else(|| {
                    BaseError::FieldNotFound(self.table_config.table_name.clone(), name.to_string())
                })?;
                let field_type = &config.field_type;
                let value = |value: &Value| pg_condition_value(name, field_type, value);
                Ok(match leaf {
                    WhereCondition::Eq { value: v, .. } => {
                        PgCondition::Eq(name.to_string(), value(v)?)
                    }
                    WhereCondition::Ne { value: v, .. } => {
                        PgCondition::Ne(name.to_string(), value(v)?)
                    }
                    WhereCondition::Gt { value: v, .. } => {
                        PgCondition::Gt(name.to_string(), value(v)?)
                    }
                    WhereCondition::Gte { value: v, .. } => {
                        PgCondition::Gte(name.to_string(), value(v)?)
                    }
                    WhereCondition::Lt { value: v, .. } => {
                        PgCondition::Lt(name.to_string(), value(v)?)
                    }
                    WhereCondition::Lte { value: v, .. } => {
                        PgCondition::Lte(name.to_string(), value(v)?)
                    }
                    WhereCondition::Like { pattern, .. } => {
                        PgCondition::Like(name.to_string(), pattern.clone())
                    }
                    WhereCondition::In { values, .. } => PgCondition::In(
                        name.to_string(),
                        values.iter().map(&value).collect::<Result<Vec<_>, _>>()?,
                    ),
                    WhereCondition::NotIn { values, .. } => PgCondition::NotIn(
                        name.to_string(),
                        values.iter().map(&value).collect::<Result<Vec<_>, _>>()?,
                    ),
                    WhereCondition::Between { lo, hi, .. } => {
                        PgCondition::Between(name.to_string(), value(lo)?, value(hi)?)
                    }
                    WhereCondition::IsNull { .. } => PgCondition::IsNull(name.to_string()),
                    WhereCondition::IsNotNull { .. } => PgCondition::IsNotNull(name.to_string()),
                    WhereCondition::And { .. } | WhereCondition::Or { .. } => {
                        unreachable!("逻辑组已在 match 外层处理")
                    }
                })
            }
        }
    }

    /// WHERE 条件 + 软删守卫（读写路径共享）。
    ///
    /// 与 MySQL 路径逐条等价：条件先经权限/类型校验（构建期已完成），这里编译为
    /// 带原生类型值的条件树后注入；未 `with_trashed()` 时追加软删字段 `IS NULL`。
    ///
    /// 注：`yang_db::postgres::QueryBuilder` 只提供 `where_not` 接受任意
    /// `Condition` 树，因此以「双重取反」注入（`NOT (NOT tree)` 语义不变，
    /// PostgreSQL 规划器会化简）；不能改走 `where_predicate`，否则 Date /
    /// TIMESTAMPTZ / NUMERIC / JSONB 的值会按 TEXT 绑定并触发 42883。
    pub(super) fn apply_pg_where_guards<'a>(
        &self,
        mut query: PgQueryBuilder<'a>,
    ) -> Result<PgQueryBuilder<'a>, BaseError> {
        for condition in &self.query_params.where_conditions {
            let mut names = Vec::new();
            condition_field_names(condition, &mut names);
            query = self.register_pg_fields(query, names)?;
            let tree = self.pg_condition(condition)?;
            query = query.where_not(PgCondition::Not(Box::new(tree)));
        }
        // `where_any`：整个列表作为一个数组参数绑定为 `field = ANY($n)`。
        for (name, values) in &self.any_conditions {
            query = self.register_pg_field(query, name)?;
            let config = self.table_config.get_field(name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
            })?;
            let field_type = &config.field_type;
            let bound = values
                .iter()
                .map(|value| pg_condition_value(name, field_type, value))
                .collect::<Result<Vec<_>, _>>()?;
            let field = self.field_ref(name)?;
            query = query.where_any(field, bound);
        }
        if !self.include_trashed {
            if let Some(name) = &self.table_config.soft_delete_field {
                query = self.register_pg_field(query, name)?;
                let field = self.field_ref(name)?;
                query = query.where_null(field);
            }
        }
        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::AnyPool;
    use crate::table::{Field, Table};
    use serde_json::json;
    use std::sync::Arc;

    fn lazy_pg_query(table: Table) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres:postgres@127.0.0.1:55432/test")
            .expect("惰性 PG 连接池应创建成功");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(vec!["admin".to_string()]),
            Some(AnyPool::Postgres(Arc::new(pool))),
        )
    }

    /// 类型映射必须与 PostgreSQL 绑定语义一致：字符串族走 TEXT、DateTime 走
    /// TIMESTAMPTZ、Unix 时间戳走 BIGINT、JSON 走 JSONB，其余由服务端按列推导。
    #[test]
    fn field_type_mapping_matches_postgres_binding_semantics() {
        assert_eq!(
            pg_field_type(&FieldType::String { max_length: 50 }),
            PgFieldType::Text
        );
        assert_eq!(pg_field_type(&FieldType::Text), PgFieldType::Text);
        assert_eq!(
            pg_field_type(&FieldType::Enum {
                values: vec!["a".to_string()]
            }),
            PgFieldType::Text
        );
        assert_eq!(pg_field_type(&FieldType::Integer), PgFieldType::Standard);
        assert_eq!(pg_field_type(&FieldType::BigInt), PgFieldType::Standard);
        assert_eq!(pg_field_type(&FieldType::Float), PgFieldType::Standard);
        assert_eq!(pg_field_type(&FieldType::Double), PgFieldType::Standard);
        assert_eq!(pg_field_type(&FieldType::Boolean), PgFieldType::Bool);
        assert_eq!(
            pg_field_type(&FieldType::Decimal {
                precision: 10,
                scale: 2
            }),
            PgFieldType::Decimal
        );
        assert_eq!(pg_field_type(&FieldType::Date), PgFieldType::Date);
        assert_eq!(
            pg_field_type(&FieldType::DateTime),
            PgFieldType::TimestampTz
        );
        assert_eq!(pg_field_type(&FieldType::Timestamp), PgFieldType::Timestamp);
        assert_eq!(pg_field_type(&FieldType::Json), PgFieldType::Jsonb);
    }

    /// UUID / BYTEA / 数组必须登记为原生绑定类型，避免参数按 TEXT 发送触发 42883。
    #[test]
    fn new_field_types_map_to_native_postgres_bindings() {
        assert_eq!(pg_field_type(&FieldType::Uuid), PgFieldType::Uuid);
        assert_eq!(pg_field_type(&FieldType::Bytes), PgFieldType::Blob);
        assert_eq!(
            pg_field_type(&FieldType::array(FieldType::Integer)),
            PgFieldType::Array(Box::new(PgFieldType::Standard))
        );
        assert_eq!(
            pg_field_type(&FieldType::array(FieldType::Text)),
            PgFieldType::Array(Box::new(PgFieldType::Text))
        );
        // 布尔数组元素必须保留为 Bool，空/全 NULL 数组才能绑定为 bool[]。
        assert_eq!(
            pg_field_type(&FieldType::array(FieldType::Boolean)),
            PgFieldType::Array(Box::new(PgFieldType::Bool))
        );
    }

    /// 条件值转换：UUID 解析、BYTEA Base64 解码、数组逐元素递归（含 NULL）。
    #[test]
    fn condition_values_convert_uuid_bytes_and_arrays() {
        let uuid = pg_condition_value(
            "uid",
            &FieldType::Uuid,
            &json!("11111111-1111-1111-1111-111111111111"),
        )
        .expect("合法 UUID 应可转换");
        assert!(matches!(uuid, PgSqlValue::Uuid(_)));
        assert!(pg_condition_value("uid", &FieldType::Uuid, &json!("not-a-uuid")).is_err());

        let bytes = pg_condition_value("payload", &FieldType::Bytes, &json!("3q2+7w=="))
            .expect("Base64 应可转换");
        assert!(matches!(
            bytes,
            PgSqlValue::Bytes(ref value) if value == &[0xde, 0xad, 0xbe, 0xef]
        ));

        let numbers = pg_condition_value(
            "numbers",
            &FieldType::array(FieldType::Integer),
            &json!([1, null, 3]),
        )
        .expect("整数数组应可转换");
        match numbers {
            PgSqlValue::TypedArray {
                element_type,
                values,
            } => {
                assert_eq!(element_type, "int4", "int4[] 必须按 int4 元素类型绑定");
                assert_eq!(values.len(), 3);
                assert!(matches!(values[0], PgSqlValue::Int(1)));
                assert!(matches!(values[1], PgSqlValue::Null));
                assert!(matches!(values[2], PgSqlValue::Int(3)));
            }
            other => panic!("期望带元素类型的数组值，实得 {other:?}"),
        }

        assert!(
            pg_condition_value("numbers", &FieldType::array(FieldType::Integer), &json!(1))
                .is_err()
        );
        // int4 越界必须显式报错，不能静默绑定成 NULL。
        assert!(pg_condition_value(
            "numbers",
            &FieldType::array(FieldType::Integer),
            &json!([i64::from(i32::MAX) + 1])
        )
        .is_err());
    }

    /// 读计划必须在生成 SQL 前完成：投影、WHERE、租户条件、软删守卫、分页。
    #[tokio::test]
    async fn read_plan_renders_projection_guards_tenant_and_pagination() {
        let query = lazy_pg_query(Table::new("pg_plan_rows").fields([
            Field::id("id"),
            Field::string("name", 50).required(),
            Field::bigint("tenant_id").tenant_key(),
            Field::soft_delete("deleted_at"),
        ]))
        .where_eq("name", json!("alice"))
        .expect("合法字段筛选应通过")
        .scope_tenant("tenant_id", json!(7))
        .expect("租户字段应通过校验")
        .page(2, 10)
        .expect("分页参数应有效");

        let database = query.pg_database().expect("惰性池应可构造 PG Database");
        let sql = query
            .apply_pg_read_plan(database.table(&query.table_config.table_ref))
            .expect("PG 读计划应编译成功")
            .try_to_sql()
            .expect("PG 读计划应能渲染 SQL");

        assert!(sql.contains("\"name\""), "投影应包含字段: {sql}");
        assert!(sql.contains("= $1"), "WHERE 应使用编号占位符: {sql}");
        assert!(sql.contains("= $2"), "租户条件应使用编号占位符: {sql}");
        assert!(
            sql.contains("\"deleted_at\" IS NULL"),
            "软删守卫缺失: {sql}"
        );
        assert!(sql.contains("LIMIT 10"), "分页 LIMIT 缺失: {sql}");
        assert!(sql.contains("OFFSET 10"), "分页 OFFSET 缺失: {sql}");
    }

    /// `order_nulls` 必须在 PostgreSQL 上渲染原生 `NULLS FIRST/LAST`。
    #[tokio::test]
    async fn read_plan_renders_nulls_first_and_last() {
        let first = lazy_pg_query(
            Table::new("pg_nulls_first").fields([Field::id("id"), Field::integer("bucket")]),
        )
        .order_nulls("bucket", SortOrder::Asc, true)
        .expect("order_nulls 应通过校验");
        let database = first.pg_database().expect("惰性池应可构造 PG Database");
        let sql = first
            .apply_pg_read_plan(database.table(&first.table_config.table_ref))
            .expect("PG 读计划应编译成功")
            .try_to_sql()
            .expect("PG 读计划应能渲染 SQL");
        assert!(
            sql.contains("\"bucket\" ASC NULLS FIRST, \"bucket\" ASC"),
            "NULLS FIRST 渲染缺失: {sql}"
        );

        let last = lazy_pg_query(
            Table::new("pg_nulls_last").fields([Field::id("id"), Field::integer("bucket")]),
        )
        .order_nulls("bucket", SortOrder::Desc, false)
        .expect("order_nulls 应通过校验");
        let database = last.pg_database().expect("惰性池应可构造 PG Database");
        let sql = last
            .apply_pg_read_plan(database.table(&last.table_config.table_ref))
            .expect("PG 读计划应编译成功")
            .try_to_sql()
            .expect("PG 读计划应能渲染 SQL");
        assert!(
            sql.contains("\"bucket\" DESC NULLS LAST, \"bucket\" DESC"),
            "NULLS LAST 渲染缺失: {sql}"
        );
    }

    /// `where_any` 必须渲染为单个数组参数的 `= ANY($n)`。
    #[tokio::test]
    async fn where_any_renders_single_array_parameter() {
        let query = lazy_pg_query(
            Table::new("pg_any_rows").fields([Field::id("id"), Field::string("name", 50)]),
        )
        .where_any("id", vec![json!(1), json!(2), json!(3)])
        .expect("where_any 应通过校验");
        let database = query.pg_database().expect("惰性池应可构造 PG Database");
        let sql = query
            .apply_pg_read_plan(database.table(&query.table_config.table_ref))
            .expect("PG 读计划应编译成功")
            .try_to_sql()
            .expect("PG 读计划应能渲染 SQL");
        assert!(sql.contains("= ANY($1)"), "ANY 数组参数缺失: {sql}");
    }

    /// 分页必须在业务排序之后追加主键 tiebreaker。
    #[tokio::test]
    async fn pagination_appends_primary_key_tiebreaker() {
        let query = lazy_pg_query(
            Table::new("pg_stable_rows").fields([Field::id("id"), Field::integer("bucket")]),
        )
        .order_by("bucket", SortOrder::Asc)
        .expect("排序字段应合法")
        .page(1, 10)
        .expect("分页参数应合法");
        let (query, page, page_size) = query.with_effective_pagination().expect("分页参数应有效");
        assert_eq!((page, page_size), (1, 10));
        assert_eq!(
            query.query_params.order_by,
            vec![
                ("bucket".to_string(), SortOrder::Asc),
                ("id".to_string(), SortOrder::Asc),
            ],
            "分页排序必须追加主键 tiebreaker"
        );
    }

    /// `with_trashed()` 必须真正移除软删守卫（与 MySQL 行为一致）。
    #[tokio::test]
    async fn with_trashed_removes_soft_delete_guard() {
        let query = lazy_pg_query(
            Table::new("pg_trashed_rows")
                .fields([Field::id("id"), Field::soft_delete("deleted_at")]),
        )
        .with_trashed();

        let database = query.pg_database().expect("惰性池应可构造 PG Database");
        let sql = query
            .apply_pg_read_plan(database.table(&query.table_config.table_ref))
            .expect("PG 读计划应编译成功")
            .try_to_sql()
            .expect("PG 读计划应能渲染 SQL");

        assert!(
            !sql.contains("IS NULL"),
            "with_trashed 不应带软删守卫: {sql}"
        );
    }
}
