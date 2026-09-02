//! 链式构建方法：字段选择、条件、JOIN、分组、排序、分页与 UNION。

use crate::mysql::condition::Condition;
use crate::mysql::field::FieldType;

use super::predicate::predicate_condition;
use super::{QueryBuilder, UnionOperator};

impl<'a> QueryBuilder<'a> {
    /// 选择字段
    ///
    /// # 安全
    ///
    /// 本方法按设计接受 SQL 表达式（如 `COUNT(*) AS c`、`users.id`、`YEAR(d)`），
    /// 故**不**对入参做标识符校验/转义——属可信输入。若需把外部输入当列名，请先用
    /// [`is_valid_identifier`](crate::mysql::is_valid_identifier) /
    /// [`quote_identifier`](crate::mysql::quote_identifier) 校验或转义后再传入。
    /// 写入路径（INSERT/UPDATE/UPSERT 的列名与各 DML 表名）已在生成层强制 quote。
    pub fn field(mut self, field: &crate::FieldRef) -> Self {
        self.fields.push(field.mysql_quoted().to_string());
        self
    }

    /// 添加受控聚合表达式。
    pub fn expr(mut self, expression: crate::SelectExpr) -> Self {
        self.fields.push(expression.mysql_sql());
        self
    }

    /// 在 SELECT 投影中追加受控服务端标量表达式，并以受控字段引用作为输出别名。
    ///
    /// 渲染形如 ``UNIX_TIMESTAMP() AS `now```；表达式片段是 [`crate::SqlExpr`]
    /// 白名单内的固定文本，别名经标识符校验转义，动态部分（如偏移秒数）以绑定
    /// 参数传递。可与 [`crate::mysql::Transaction::select_for_update`] 等行锁查询
    /// 组合，覆盖「读取行的同时取服务端当前时间」的场景。
    pub fn select_expr(mut self, expression: crate::SqlExpr, alias: &crate::FieldRef) -> Self {
        self.select_exprs
            .push((expression, alias.as_str().to_string()));
        self
    }

    /// 把字段的写入值设为受控服务端表达式，UPDATE 的 SET 子句与 INSERT 的
    /// VALUES 子句同样生效（如 `used_at = UNIX_TIMESTAMP()`）。
    ///
    /// 与 JSON 数据列混用时表达式列追加在其后；表达式只能由 [`crate::SqlExpr`]
    /// 的白名单构造函数创建，动态部分一律走绑定参数，调用方无法注入 SQL 片段。
    /// `insert`/`update` 时若 JSON 数据为空对象但存在表达式赋值，语句仍然合法。
    pub fn set_expr(mut self, field: &crate::FieldRef, expression: crate::SqlExpr) -> Self {
        self.expr_assignments
            .push((field.as_str().to_string(), expression));
        self
    }

    /// 选择多个字段
    pub fn fields(mut self, fields: &[&crate::FieldRef]) -> Self {
        for field in fields {
            self.fields.push(field.mysql_quoted().to_string());
        }
        self
    }

    /// 标记字段为 JSON 类型
    pub fn json(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Json);
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

    /// 标记字段为 DECIMAL 类型
    pub fn decimal(mut self, field: &crate::FieldRef) -> Self {
        self.field_types
            .insert(field.as_str().to_string(), FieldType::Decimal);
        self
    }

    /// 标记字段为 BLOB 类型
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

    /// 去重
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// 使用 UNION 组合一个显式投影且输出列数相同的查询。
    pub fn union(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_union(other, UnionOperator::Distinct)
    }

    /// 使用 UNION ALL 组合一个显式投影且输出列数相同的查询。
    pub fn union_all(self, other: QueryBuilder<'a>) -> Result<Self, crate::error::DbError> {
        self.add_union(other, UnionOperator::All)
    }

    fn add_union(
        mut self,
        other: QueryBuilder<'a>,
        operator: UnionOperator,
    ) -> Result<Self, crate::error::DbError> {
        if self.fields.is_empty() || other.fields.is_empty() {
            return Err(crate::error::DbError::InvalidArgument(
                "UNION 各分支必须显式声明输出字段，不能使用未知列数的 *".to_string(),
            ));
        }
        if self.fields.len() != other.fields.len() {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "UNION 输出列数不一致：左侧 {} 列，右侧 {} 列",
                self.fields.len(),
                other.fields.len()
            )));
        }
        self.unions.push((operator, Box::new(other)));
        Ok(self)
    }

    /// 添加 AND 条件
    ///
    /// 遇到不支持的操作符时返回 `Err(DbError::UnsupportedOperator)`。
    /// 支持的操作符：`=`、`!=`、`>`、`<`、`>=`、`<=`、`like`、`LIKE`。
    ///
    /// # 参数
    /// - `field`: 字段名
    /// - `op`: 比较操作符
    /// - `value`: 比较值
    ///
    /// # 返回
    /// - `Ok(Self)`: 操作符合法，条件已添加
    /// - `Err(DbError::UnsupportedOperator)`: 操作符不在支持集合中
    pub fn where_and<V>(mut self, field: &crate::FieldRef, op: crate::CompareOp, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::{Condition, SqlValue};

        let field = field.as_str().to_string();
        let value = value.into();
        let condition = match op {
            crate::CompareOp::Eq => Condition::Eq(field, value),
            crate::CompareOp::Ne => Condition::Ne(field, value),
            crate::CompareOp::Gt => Condition::Gt(field, value),
            crate::CompareOp::Lt => Condition::Lt(field, value),
            crate::CompareOp::Gte => Condition::Gte(field, value),
            crate::CompareOp::Lte => Condition::Lte(field, value),
            crate::CompareOp::Like => Condition::Like(
                field,
                match value {
                    SqlValue::String(value) => value,
                    other => format!("{other:?}"),
                },
            ),
        };
        self.conditions.push(condition);
        self
    }

    /// 添加 OR 条件
    ///
    /// 遇到不支持的操作符时返回 `Err(DbError::UnsupportedOperator)`。
    /// 支持的操作符：`=`、`!=`、`>`、`<`、`>=`、`<=`、`like`、`LIKE`。
    ///
    /// # 参数
    /// - `field`: 字段名
    /// - `op`: 比较操作符
    /// - `value`: 比较值
    ///
    /// # 返回
    /// - `Ok(Self)`: 操作符合法，条件已添加
    /// - `Err(DbError::UnsupportedOperator)`: 操作符不在支持集合中
    pub fn where_or<V>(mut self, field: &crate::FieldRef, op: crate::CompareOp, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::{Condition, SqlValue};

        let field = field.as_str().to_string();
        let value = value.into();
        let condition = match op {
            crate::CompareOp::Eq => Condition::Eq(field, value),
            crate::CompareOp::Ne => Condition::Ne(field, value),
            crate::CompareOp::Gt => Condition::Gt(field, value),
            crate::CompareOp::Lt => Condition::Lt(field, value),
            crate::CompareOp::Gte => Condition::Gte(field, value),
            crate::CompareOp::Lte => Condition::Lte(field, value),
            crate::CompareOp::Like => Condition::Like(
                field,
                match value {
                    SqlValue::String(value) => value,
                    other => format!("{other:?}"),
                },
            ),
        };

        // 如果已有条件，将新条件与现有条件用 OR 组合
        if !self.conditions.is_empty() {
            let mut existing = std::mem::take(&mut self.conditions);
            self.conditions.push(Condition::Or(vec![
                if existing.len() == 1 {
                    // remove(0) 在 len == 1 时安全，不会 panic
                    existing.remove(0)
                } else {
                    Condition::And(existing)
                },
                condition,
            ]));
        } else {
            self.conditions.push(condition);
        }

        self
    }

    /// 添加 IN 条件
    pub fn where_in<V>(mut self, field: &crate::FieldRef, values: Vec<V>) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::Condition;

        let sql_values: Vec<_> = values.into_iter().map(|v| v.into()).collect();
        self.conditions
            .push(Condition::In(field.as_str().to_string(), sql_values));
        self
    }

    /// 添加受控 EXISTS 子查询。
    pub fn where_exists(mut self, subquery: crate::mysql::Subquery) -> Self {
        self.conditions.push(Condition::Exists(Box::new(subquery)));
        self
    }

    /// 添加受控 NOT EXISTS 子查询。
    pub fn where_not_exists(mut self, subquery: crate::mysql::Subquery) -> Self {
        self.conditions
            .push(Condition::NotExists(Box::new(subquery)));
        self
    }

    /// 添加受控 IN 子查询，外层字段必须是合法的单段或两段标识符。
    pub fn where_in_subquery(
        mut self,
        field: &crate::FieldRef,
        subquery: crate::mysql::Subquery,
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
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::Condition;

        self.conditions.push(Condition::Between(
            field.as_str().to_string(),
            start.into(),
            end.into(),
        ));
        self
    }

    /// 添加 IS NULL 条件
    ///
    /// 生成 `field IS NULL` 子句，用于查询字段值为 NULL 的记录。
    ///
    /// # 参数
    /// - `field`: 字段名
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::Database;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// # #[derive(serde::Deserialize, sqlx::FromRow)] struct User { id: i64 }
    /// let users = db.table(yang_db::table!("users"))
    ///     .where_null(yang_db::field!("deleted_at"))
    ///     .select::<User>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn where_null(mut self, field: &crate::FieldRef) -> Self {
        self.conditions
            .push(Condition::IsNull(field.as_str().to_string()));
        self
    }

    /// 添加 IS NOT NULL 条件
    ///
    /// 生成 `field IS NOT NULL` 子句，用于查询字段值不为 NULL 的记录。
    ///
    /// # 参数
    /// - `field`: 字段名
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

    /// 添加字段与受控服务端表达式的 AND 比较条件（如 `expires_at > UNIX_TIMESTAMP()`）。
    ///
    /// `op` 仅支持六个比较操作符；传入 [`crate::CompareOp::Like`] 时返回
    /// `Err(DbError::UnsupportedOperator)`。表达式右值由 [`crate::SqlExpr`] 白名单
    /// 构造函数创建，动态部分（如偏移秒数）以绑定参数传递。
    pub fn where_expr(
        mut self,
        field: &crate::FieldRef,
        op: crate::CompareOp,
        expression: crate::SqlExpr,
    ) -> Result<Self, crate::error::DbError> {
        use crate::mysql::condition::ComparisonOperator;

        let operator = match op {
            crate::CompareOp::Eq => ComparisonOperator::Eq,
            crate::CompareOp::Ne => ComparisonOperator::Ne,
            crate::CompareOp::Gt => ComparisonOperator::Gt,
            crate::CompareOp::Lt => ComparisonOperator::Lt,
            crate::CompareOp::Gte => ComparisonOperator::Gte,
            crate::CompareOp::Lte => ComparisonOperator::Lte,
            crate::CompareOp::Like => {
                return Err(crate::error::DbError::UnsupportedOperator(
                    "LIKE 不支持服务端表达式右值".to_string(),
                ));
            }
        };
        self.conditions.push(Condition::ColumnExprComparison(
            field.as_str().to_string(),
            operator,
            expression,
        ));
        Ok(self)
    }

    /// 添加 HAVING 条件
    ///
    /// 对 GROUP BY 分组后的结果进行过滤。必须与 `group()` 方法配合使用，
    /// 否则查询执行时返回 `MissingGroupByClause` 错误。
    /// 多次调用将以 AND 连接所有条件。
    ///
    /// # 参数
    /// - `field`: 聚合字段或聚合表达式（如 `"cnt"`）
    /// - `op`: 比较运算符（"="、"!="、">"、"<"、">="、"<="）
    /// - `value`: 比较值（参数化，防 SQL 注入）
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::Database;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// # #[derive(serde::Deserialize, sqlx::FromRow)] struct OrderSummary { user_id: i64, cnt: i64 }
    /// let result = db.table(yang_db::table!("orders"))
    ///     .field(yang_db::field!("user_id"))
    ///     .expr(yang_db::SelectExpr::count_all().alias(yang_db::field!("cnt")))
    ///     .group(yang_db::field!("user_id"))
    ///     .having_cond(yang_db::field!("cnt"), yang_db::CompareOp::Gt, 5i64)
    ///     .select::<OrderSummary>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn having_cond<V>(mut self, field: &crate::FieldRef, op: crate::CompareOp, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        let field = field.as_str().to_string();
        let value = value.into();
        let condition = match op {
            crate::CompareOp::Eq => Condition::Eq(field, value),
            crate::CompareOp::Ne => Condition::Ne(field, value),
            crate::CompareOp::Gt => Condition::Gt(field, value),
            crate::CompareOp::Lt => Condition::Lt(field, value),
            crate::CompareOp::Gte => Condition::Gte(field, value),
            crate::CompareOp::Lte => Condition::Lte(field, value),
            crate::CompareOp::Like => Condition::Like(
                field,
                match value {
                    crate::mysql::condition::SqlValue::String(value) => value,
                    other => format!("{other:?}"),
                },
            ),
        };
        self.having_clause.push(condition);
        self
    }

    /// 使用可信表/ON 表达式的 INNER JOIN。
    ///
    /// 外部标识符的等值连接请使用 [`Self::join_on_identifiers`]。
    pub fn join(
        mut self,
        table: &crate::TableRef,
        left: &crate::FieldRef,
        right: &crate::FieldRef,
    ) -> Self {
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: format!("`{}`", table.as_str()),
            on: format!("{} = {}", left.mysql_quoted(), right.mysql_quoted()),
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
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: format!("`{}`", table.as_str()),
            on: format!("{} = {}", left.mysql_quoted(), right.mysql_quoted()),
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
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Right,
            table: format!("`{}`", table.as_str()),
            on: format!("{} = {}", left.mysql_quoted(), right.mysql_quoted()),
        });
        self
    }

    /// 按可信 SQL 表达式排序；外部列名请使用 [`Self::order_identifier`]。
    pub fn order(mut self, field: &crate::FieldRef, order: crate::SortOrder) -> Self {
        use crate::mysql::field::OrderClause;

        self.order_by.push(OrderClause {
            field: field.mysql_quoted().to_string(),
            asc: order.is_ascending(),
        });
        self
    }

    /// 按可信 SQL 表达式分组；外部列名请使用 [`Self::group_identifier`]。
    pub fn group(mut self, field: &crate::FieldRef) -> Self {
        self.group_by.push(field.mysql_quoted().to_string());
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
}
