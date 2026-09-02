//! 测试专用 SQL 文本渲染（`cfg(test)`）：`build_*_sql` 系列、WHERE 子句
//! 递归渲染与标识符转义，以及慢查询计时的 `timed` 关联函数。
//!
//! 注意：本模块仅在测试构建中参与编译；生产执行路径由 `plan.rs` 的
//! `yang_db::QueryBuilder` 承担。

#![cfg(all(test, feature = "mysql"))]

use super::sql_param::SqlParam;
use super::TableQuery;
use crate::error::BaseError;
use crate::table::{SortOrder, WhereCondition};
use serde_json::Value;

impl TableQuery {
    /// 对字段名进行反引号转义
    ///
    /// 对合法标识符添加反引号，内部反引号转义为双反引号。
    /// 非法字段名返回 `BaseError::FieldNotFound`。
    ///
    /// # 参数
    ///
    /// - `field`: 字段名
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 转义后的字段名，如 `` `field_name` ``
    /// - `Err(BaseError)`: 字段名非法
    #[cfg(feature = "mysql")]
    #[cfg(test)]
    fn quote_identifier(&self, field: &str) -> Result<String, BaseError> {
        self.table_config
            .get_field_ref(field)
            .map(|field| field.mysql_quoted().to_string())
            .ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
            })
    }

    /// 对表名进行反引号转义（与字段名走同一条校验/转义路径）
    ///
    /// 防止非法表名破坏引号边界。表名虽来自开发者配置而非终端用户输入，
    /// 但统一转义可消除字段名与表名处理的不一致（防御纵深）。
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 转义后的表名，如 `` `users` ``
    /// - `Err(BaseError)`: 表名非法
    #[cfg(feature = "mysql")]
    #[cfg(test)]
    fn quoted_table_name(&self) -> Result<String, BaseError> {
        Ok(format!("`{}`", self.table_config.table_ref.as_str()))
    }

    /// 统一拼接 WHERE 子句到 SQL 字符串
    ///
    /// 集中处理 12 种叶子条件（Eq/Ne/In/NotIn/Like/Gt/Gte/Lt/Lte/Between/IsNull/IsNotNull）
    /// 与两种逻辑组（And/Or）的 SQL 拼接：顶层 `where_conditions` 列表仍以隐式 AND 连接
    /// （向后兼容），逻辑组递归渲染并用括号包裹。所有字段名通过 `quote_identifier`
    /// 反引号转义，防止 SQL 注入。
    ///
    /// # 参数
    ///
    /// - `sql`：目标 SQL 字符串（追加模式）
    /// - `params`：参数列表（追加模式）
    /// - `apply_soft_delete`：是否追加软删过滤（仅读路径）
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：拼接成功
    /// - `Err(BaseError)`：字段名非法、参数转换失败或嵌套层数超限
    #[cfg(test)]
    fn append_where_to_sql(
        &self,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        apply_soft_delete: bool,
    ) -> Result<(), BaseError> {
        // 计算是否需要追加软删过滤子句（仅读路径，且未显式 with_trashed）
        let has_soft_delete = apply_soft_delete
            && !self.include_trashed
            && self.table_config.soft_delete_field.is_some();

        // 既无 WHERE 条件也无需软删过滤，直接返回
        if self.query_params.where_conditions.is_empty() && !has_soft_delete {
            return Ok(());
        }

        sql.push_str(" WHERE ");
        let mut first = true;

        // 顶层条件以隐式 AND 连接；每个条件递归渲染（组节点自带括号）
        for condition in &self.query_params.where_conditions {
            if !first {
                sql.push_str(" AND ");
            }
            first = false;
            self.render_condition(condition, sql, params, 0)?;
        }

        // 读路径软删过滤：追加 `软删字段 IS NULL`（与已有条件用 AND 连接）
        if has_soft_delete {
            if let Some(field) = &self.table_config.soft_delete_field {
                let quoted = self.quote_identifier(field)?;
                if !first {
                    sql.push_str(" AND ");
                }
                sql.push_str(&format!("{} IS NULL", quoted));
            }
        }

        Ok(())
    }

    /// 递归渲染单个 WHERE 条件到 SQL（叶子直接拼接，And/Or 组括号包裹后递归）。
    ///
    /// `depth` 从 0 起递增，超过 [`Self::MAX_WHERE_DEPTH`] 返回 `ParamInvalid`
    /// 而非 panic，保证受保护层不因深嵌套输入崩溃。
    #[cfg(test)]
    fn render_condition(
        &self,
        condition: &WhereCondition,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        depth: usize,
    ) -> Result<(), BaseError> {
        if depth > Self::MAX_WHERE_DEPTH {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                format!("嵌套布尔条件层数超过上限 {}", Self::MAX_WHERE_DEPTH),
            ));
        }

        match condition {
            WhereCondition::Eq { field, value } => {
                let quoted = self.quote_identifier(field)?;
                if value.is_null() {
                    sql.push_str(&format!("{} IS NULL", quoted));
                } else {
                    sql.push_str(&format!("{} = ?", quoted));
                    params.push(SqlParam::from_json(value)?);
                }
            }
            WhereCondition::In { field, values } => {
                // QRY-2 安全网：渲染期再次校验 IN 列表元素数上限
                if values.len() > Self::MAX_IN_LIST_SIZE {
                    return Err(BaseError::ParamInvalid(
                        "values".to_string(),
                        format!(
                            "IN 列表元素数 {} 超过上限 {}",
                            values.len(),
                            Self::MAX_IN_LIST_SIZE
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                if values.is_empty() {
                    // 空 IN 集合：等价于恒假，避免拼出非法的 `IN ()`
                    sql.push_str("1=0");
                } else {
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
            }
            WhereCondition::Like { field, pattern } => {
                // QRY-1 安全网：渲染期再次校验 LIKE pattern 长度上限
                if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
                    return Err(BaseError::ParamInvalid(
                        "pattern".to_string(),
                        format!(
                            "LIKE pattern 长度 {} 超过上限 {}",
                            pattern.len(),
                            Self::MAX_LIKE_PATTERN_LEN
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} LIKE ?", quoted));
                params.push(SqlParam::String(pattern.clone()));
            }
            WhereCondition::Gt { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} > ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Gte { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} >= ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Lt { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} < ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Lte { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} <= ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::IsNull { field } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} IS NULL", quoted));
            }
            WhereCondition::IsNotNull { field } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} IS NOT NULL", quoted));
            }
            WhereCondition::Ne { field, value } => {
                let quoted = self.quote_identifier(field)?;
                if value.is_null() {
                    sql.push_str(&format!("{} IS NOT NULL", quoted));
                } else {
                    sql.push_str(&format!("{} <> ?", quoted));
                    params.push(SqlParam::from_json(value)?);
                }
            }
            WhereCondition::Between { field, lo, hi } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} BETWEEN ? AND ?", quoted));
                params.push(SqlParam::from_json(lo)?);
                params.push(SqlParam::from_json(hi)?);
            }
            WhereCondition::NotIn { field, values } => {
                // QRY-2 安全网：渲染期再次校验 NOT IN 列表元素数上限
                if values.len() > Self::MAX_IN_LIST_SIZE {
                    return Err(BaseError::ParamInvalid(
                        "values".to_string(),
                        format!(
                            "NOT IN 列表元素数 {} 超过上限 {}",
                            values.len(),
                            Self::MAX_IN_LIST_SIZE
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                if values.is_empty() {
                    // 空 NOT IN 集合：等价于恒真，不排除任何行
                    sql.push_str("1=1");
                } else {
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} NOT IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
            }
            // 逻辑组：括号包裹，子条件以 AND/OR 连接，递归渲染（深度 +1）
            WhereCondition::And { conditions } => {
                self.render_group(conditions, " AND ", "1=1", sql, params, depth)?;
            }
            WhereCondition::Or { conditions } => {
                self.render_group(conditions, " OR ", "1=0", sql, params, depth)?;
            }
        }

        Ok(())
    }

    /// 渲染逻辑组：`(c1 <sep> c2 <sep> ...)`；空组用 `empty_literal` 兜底
    /// （And→`1=1` 恒真，Or→`1=0` 恒假），避免拼出非法空括号。
    #[cfg(test)]
    fn render_group(
        &self,
        conditions: &[WhereCondition],
        separator: &str,
        empty_literal: &str,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        depth: usize,
    ) -> Result<(), BaseError> {
        if conditions.is_empty() {
            sql.push_str(empty_literal);
            return Ok(());
        }

        sql.push('(');
        let mut first = true;
        for cond in conditions {
            if !first {
                sql.push_str(separator);
            }
            first = false;
            self.render_condition(cond, sql, params, depth + 1)?;
        }
        sql.push(')');
        Ok(())
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
    #[cfg(test)]
    pub(crate) fn build_select_sql(
        &self,
        hard_limit: Option<usize>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        // 1. 字段列表（通过 quote_identifier 转义字段名）
        if let Some(fields) = &self.query_params.fields {
            if fields.is_empty() {
                let quoted_fields = self
                    .default_read_fields()?
                    .into_iter()
                    .map(|field| self.quote_identifier(field))
                    .collect::<Result<Vec<_>, _>>()?;
                sql.push_str(&quoted_fields.join(", "));
            } else {
                // 对每个字段名进行反引号转义
                let quoted_fields: Result<Vec<String>, BaseError> = fields
                    .iter()
                    .map(|f| {
                        self.validate_read_field(f)?;
                        self.quote_identifier(f)
                    })
                    .collect();
                sql.push_str(&quoted_fields?.join(", "));
            }
        } else {
            let quoted_fields = self
                .default_read_fields()?
                .into_iter()
                .map(|field| self.quote_identifier(field))
                .collect::<Result<Vec<_>, _>>()?;
            sql.push_str(&quoted_fields.join(", "));
        }

        // 2. FROM 子句（表名走统一转义路径）
        sql.push_str(&format!(" FROM {}", self.quoted_table_name()?));

        // 3. 通过统一方法拼接 WHERE 子句（读路径，应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, true)?;

        // 4. ORDER BY 子句：显式 order_by 优先，否则回退到表配置的 default_order
        let order_source = if !self.query_params.order_by.is_empty() {
            Some(&self.query_params.order_by)
        } else if !self.table_config.default_order.is_empty() {
            Some(&self.table_config.default_order)
        } else {
            None
        };
        if let Some(orders) = order_source {
            sql.push_str(" ORDER BY ");
            let order_clauses: Result<Vec<String>, BaseError> = orders
                .iter()
                .map(|(field, order)| {
                    self.validate_order_field(field)?;
                    let direction = match order {
                        SortOrder::Asc => "ASC",
                        SortOrder::Desc => "DESC",
                    };
                    self.quote_identifier(field)
                        .map(|quoted| format!("{} {}", quoted, direction))
                })
                .collect();
            sql.push_str(&order_clauses?.join(", "));
        }

        // 5. LIMIT 和 OFFSET 子句（分页与 hard_limit 互斥，避免拼出两段 LIMIT）
        if let (Some(page), Some(page_size)) = (self.query_params.page, self.query_params.page_size)
        {
            // 使用 saturating_sub 防止 page==0 时 usize 下溢（纵深防御，
            // 主校验在 page() 入口；直接构造 query_params 的调用方也安全）
            let offset = page.saturating_sub(1).saturating_mul(page_size);
            sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
        } else if let Some(limit) = hard_limit {
            // 无分页时应用硬上限（如 fetch_optional 仅需 1 行）
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        Ok((sql, params))
    }

    /// 构建 INSERT SQL 语句
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    fn build_insert_sql(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut fields = Vec::with_capacity(data.len());
        let mut placeholders = Vec::with_capacity(data.len());
        let mut params = Vec::with_capacity(data.len());

        // 遍历数据，构建字段列表和参数列表
        for (field_name, value) in data {
            // 检查字段是否存在于表配置中
            if !self.table_config.fields.contains_key(field_name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                ));
            }

            // 写入权限已在 prepare_and_validate_insert 集中校验，此处不再二次跳过，
            // 保证 data 中所有字段一致入列。

            // 对字段名进行反引号转义，防止 SQL 注入
            let quoted = self.quote_identifier(field_name)?;
            fields.push(quoted);
            placeholders.push("?".to_string());
            params.push(SqlParam::from_json(value)?);
        }

        // 构建 SQL 语句（表名走统一转义路径）
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.quoted_table_name()?,
            fields.join(", "),
            placeholders.join(", ")
        );

        Ok((sql, params))
    }

    /// 构建 UPDATE SQL 语句
    ///
    /// # 参数
    ///
    /// - `data`：要更新的数据
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_update_sql(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_update_sql_impl(data)
    }

    /// 构建 UPDATE SQL 语句的实际实现
    #[cfg(test)]
    fn build_update_sql_impl(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 0. 拒绝空更新：用户未提供任何可更新字段
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "无可更新字段".to_string(),
            ));
        }

        // 1. 自动刷新 updated_at（若已配置且列存在于表），仅在需要时克隆
        let need_timestamp = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|ts| ts.updated_at.as_ref())
            .is_some_and(|updated_at| self.table_config.fields.contains_key(updated_at));

        let mut owned_data;
        let working: &std::collections::HashMap<String, Value> = if need_timestamp {
            owned_data = data.clone();
            // SAFETY: need_timestamp 为 true 意味着 timestamp_fields 和 updated_at 一定存在
            let ts = self.table_config.timestamp_fields.as_ref().unwrap();
            let updated_at = ts.updated_at.as_ref().unwrap();
            let now = chrono::Utc::now().timestamp();
            owned_data.insert(updated_at.clone(), Value::Number(now.into()));
            &owned_data
        } else {
            data
        };

        let mut set_clauses = Vec::with_capacity(working.len());
        let mut params = Vec::with_capacity(working.len());

        // 2. 构建 SET 子句（通过 quote_identifier 转义字段名）
        for (field_name, value) in working {
            // 检查字段是否存在于表配置中
            if !self.table_config.fields.contains_key(field_name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                ));
            }

            // 对字段名进行反引号转义
            let quoted = self.quote_identifier(field_name)?;
            set_clauses.push(format!("{} = ?", quoted));
            params.push(SqlParam::from_json(value)?);
        }

        // 3. 构建基本 UPDATE 语句（表名走统一转义路径）
        let mut sql = format!(
            "UPDATE {} SET {}",
            self.quoted_table_name()?,
            set_clauses.join(", ")
        );

        // 4. WHERE 守卫：无 WHERE 且未显式放行全表，拒绝全表更新
        if self.query_params.where_conditions.is_empty() {
            return Err(BaseError::MissingWhereClause("UPDATE".to_string()));
        }

        // 5. 通过统一方法拼接 WHERE 子句（写路径，不应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, false)?;

        Ok((sql, params))
    }

    /// 构建 DELETE SQL 语句
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_delete_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_delete_sql_impl()
    }

    /// 构建 SELECT SQL 语句（仅供测试使用）
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_select_sql_for_test(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_select_sql(None)
    }

    /// 构建 INSERT SQL 语句（仅供测试使用）
    ///
    /// 先经 `prepare_and_validate_insert` 补默认值/时间戳并校验，再拼 SQL，
    /// 等价于 `insert`/`insert_returning_id` 的建句路径，但无需数据库连接。
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_insert_sql_for_test(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let prepared = self.prepare_and_validate_insert(data)?;
        self.build_insert_sql(&prepared)
    }

    /// 构建 DELETE SQL 语句的实际实现
    #[cfg(test)]
    fn build_delete_sql_impl(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 表名走统一转义路径
        let mut sql = format!("DELETE FROM {}", self.quoted_table_name()?);
        let mut params = Vec::new();

        // WHERE 守卫：无 WHERE 且未显式放行全表，拒绝全表物理删除
        if self.query_params.where_conditions.is_empty() {
            return Err(BaseError::MissingWhereClause("DELETE".to_string()));
        }

        // 通过统一方法拼接 WHERE 子句（写路径，不应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, false)?;

        Ok((sql, params))
    }

    /// 执行边界计时（慢查询观测，C4）。
    ///
    /// 包裹一次数据库执行 `fut`：`threshold` 为 `None`（默认）时仅一次
    /// `Instant::now()`，无额外分配、无日志；为 `Some(d)` 且实际耗时超过 `d` 时
    /// 发一条 `tracing::warn!`（含表名、操作、耗时毫秒、request_id）。
    ///
    /// 设计为关联函数（不借用 `&self`），以便在消费 `self` 的终端方法里，与
    /// 借用 `self.pool`/`sql`/`params` 的执行 future 共存而不冲突借用检查。
    /// SQL 文本**默认不记**（防泄漏 + 防高基数）。`op` 为静态操作名。
    #[cfg(test)]
    pub(crate) async fn timed<F, R>(
        threshold: Option<std::time::Duration>,
        request_id: Option<crate::action::RequestId>,
        table: &str,
        op: &'static str,
        fut: F,
    ) -> R
    where
        F: std::future::Future<Output = R>,
    {
        // 阈值未配置：直接 await，热路径仅一次分支判断，无 Instant
        let threshold = match threshold {
            Some(t) => t,
            None => return fut.await,
        };
        let start = std::time::Instant::now();
        let result = fut.await;
        let elapsed = start.elapsed();
        if elapsed >= threshold {
            let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
            match request_id {
                Some(rid) => tracing::warn!(
                    table = %table,
                    op,
                    elapsed_ms,
                    request_id = %rid,
                    "慢查询",
                ),
                None => tracing::warn!(table = %table, op, elapsed_ms, "慢查询"),
            }
        }
        result
    }
}
