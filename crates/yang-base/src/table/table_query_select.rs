//! TableQuery 的 SELECT 查询实现
//!
//! 本模块实现 TableQuery 的 select<T>() 方法，用于执行 SELECT 查询操作。

use crate::error::BaseError;
use crate::table::{QueryParams, SortOrder, TableQuery, WhereCondition};
use serde_json::Value;
use sqlx::mysql::MySqlPool;

impl TableQuery {
    /// 执行 SELECT 查询操作
    ///
    /// 使用 sqlx 构建 SELECT 语句，应用已配置的字段选择、WHERE 条件和排序规则，
    /// 执行查询并将结果反序列化为指定的泛型类型 T。
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(Vec<T>)`：查询成功，返回结果列表
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["user".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 执行查询
    /// let users: Vec<User> = query
    ///     .select_fields(&["id", "name", "email"])?
    ///     .where_eq("status", serde_json::json!("active"))?
    ///     .order_by("created_at", SortOrder::Desc)?
    ///     .select()
    ///     .await?;
    ///
    /// println!("找到 {} 个用户", users.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn select<T>(self) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| BaseError::DatabaseNotInitialized)?;

        // 2. 构建 SQL 语句
        let (sql, params) = self.build_select_sql()?;

        // 3. 创建查询
        let mut query = sqlx::query_as::<_, T>(&sql);

        // 4. 绑定参数
        for param in params {
            query = bind_param(query, &param);
        }

        // 5. 执行查询
        let results = query
            .fetch_all(pool.as_ref())
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))?;

        Ok(results)
    }

    /// 构建 SELECT SQL 语句
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    fn build_select_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        // 1. 字段列表
        if let Some(fields) = &self.query_params.fields {
            if fields.is_empty() {
                sql.push_str("*");
            } else {
                sql.push_str(&fields.join(", "));
            }
        } else {
            sql.push_str("*");
        }

        // 2. FROM 子句
        sql.push_str(&format!(" FROM {}", self.table_config.table_name));

        // 3. WHERE 子句
        if !self.query_params.where_conditions.is_empty() {
            sql.push_str(" WHERE ");
            let mut first = true;

            for condition in &self.query_params.where_conditions {
                if !first {
                    sql.push_str(" AND ");
                }
                first = false;

                match condition {
                    WhereCondition::Eq { field, value } => {
                        sql.push_str(&format!("{} = ?", field));
                        params.push(SqlParam::from_json(value)?);
                    }
                    WhereCondition::In { field, values } => {
                        let placeholders = vec!["?"; values.len()].join(", ");
                        sql.push_str(&format!("{} IN ({})", field, placeholders));
                        for value in values {
                            params.push(SqlParam::from_json(value)?);
                        }
                    }
                    WhereCondition::Like { field, pattern } => {
                        sql.push_str(&format!("{} LIKE ?", field));
                        params.push(SqlParam::String(pattern.clone()));
                    }
                    WhereCondition::Gt { field, value } => {
                        sql.push_str(&format!("{} > ?", field));
                        params.push(SqlParam::from_json(value)?);
                    }
                    WhereCondition::Gte { field, value } => {
                        sql.push_str(&format!("{} >= ?", field));
                        params.push(SqlParam::from_json(value)?);
                    }
                    WhereCondition::Lt { field, value } => {
                        sql.push_str(&format!("{} < ?", field));
                        params.push(SqlParam::from_json(value)?);
                    }
                    WhereCondition::Lte { field, value } => {
                        sql.push_str(&format!("{} <= ?", field));
                        params.push(SqlParam::from_json(value)?);
                    }
                    WhereCondition::IsNull { field } => {
                        sql.push_str(&format!("{} IS NULL", field));
                    }
                    WhereCondition::IsNotNull { field } => {
                        sql.push_str(&format!("{} IS NOT NULL", field));
                    }
                }
            }
        }

        // 4. ORDER BY 子句
        if !self.query_params.order_by.is_empty() {
            sql.push_str(" ORDER BY ");
            let order_clauses: Vec<String> = self
                .query_params
                .order_by
                .iter()
                .map(|(field, order)| {
                    let direction = match order {
                        SortOrder::Asc => "ASC",
                        SortOrder::Desc => "DESC",
                    };
                    format!("{} {}", field, direction)
                })
                .collect();
            sql.push_str(&order_clauses.join(", "));
        }

        // 5. LIMIT 和 OFFSET 子句
        if let (Some(page), Some(page_size)) =
            (self.query_params.page, self.query_params.page_size)
        {
            let offset = (page - 1) * page_size;
            sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
        }

        Ok((sql, params))
    }
}

/// SQL 参数类型
///
/// 用于表示 SQL 查询中的参数值
#[derive(Debug, Clone)]
enum SqlParam {
    /// 空值
    Null,
    /// 布尔值
    Bool(bool),
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
}

impl SqlParam {
    /// 从 JSON 值创建 SQL 参数
    ///
    /// # 参数
    ///
    /// - `value`：JSON 值
    ///
    /// # 返回值
    ///
    /// - `Ok(SqlParam)`：转换成功
    /// - `Err(BaseError)`：转换失败
    fn from_json(value: &Value) -> Result<Self, BaseError> {
        match value {
            Value::Null => Ok(SqlParam::Null),
            Value::Bool(b) => Ok(SqlParam::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqlParam::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlParam::Float(f))
                } else {
                    Err(BaseError::DatabaseQueryFailed(format!(
                        "不支持的数字类型: {}",
                        n
                    )))
                }
            }
            Value::String(s) => Ok(SqlParam::String(s.clone())),
            _ => Err(BaseError::DatabaseQueryFailed(format!(
                "不支持的值类型: {:?}",
                value
            ))),
        }
    }
}

/// 绑定参数到查询
///
/// # 参数
///
/// - `query`：sqlx 查询对象
/// - `param`：SQL 参数值
///
/// # 返回值
///
/// 绑定参数后的查询对象
fn bind_param<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &SqlParam,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    match param {
        SqlParam::Null => query.bind(Option::<i32>::None),
        SqlParam::Bool(b) => query.bind(*b),
        SqlParam::Int(i) => query.bind(*i),
        SqlParam::Float(f) => query.bind(*f),
        SqlParam::String(s) => query.bind(s.clone()),
    }
}
