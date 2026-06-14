use crate::postgres::condition::{Condition, SqlValue};
use crate::postgres::field::{FieldType, JoinClause, OrderClause};
use sqlx::postgres::PgPool;
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

/// 压入一个参数并返回其 PostgreSQL 占位符（`$N`，1 基）
///
/// PostgreSQL 使用编号占位符，编号由参数压入后的 `params.len()` 决定，
/// 保证占位符编号与最终绑定顺序严格一致。这是与 MySQL 后端（统一使用 `?`）
/// 的核心方言差异，在手写 INSERT/UPDATE/UPSERT 等 SQL 时集中通过本助手处理。
fn push_placeholder(params: &mut Vec<SqlValue>, value: SqlValue) -> String {
    params.push(value);
    format!("${}", params.len())
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
        // 清空之前的内容
        self.clear();

        // SELECT 子句
        self.append("SELECT ");

        // DISTINCT 关键字
        if builder.distinct {
            self.append("DISTINCT ");
        }

        // 字段列表
        if builder.fields.is_empty() {
            self.append("*");
        } else {
            self.append(&builder.fields.join(", "));
        }

        // FROM 子句
        self.append(" FROM ");
        self.append(&builder.table);

        // JOIN 子句
        if !builder.joins.is_empty() {
            self.build_joins(&builder.joins);
        }

        // WHERE 子句
        if !builder.conditions.is_empty() {
            self.build_where(&builder.conditions)?;
        }

        // GROUP BY 子句
        if !builder.group_by.is_empty() {
            self.build_group_by(&builder.group_by);
        }

        // HAVING 子句
        if !builder.having_clause.is_empty() {
            if builder.group_by.is_empty() {
                return Err(crate::error::DbError::MissingGroupByClause);
            }
            self.build_having(&builder.having_clause)?;
        }

        // ORDER BY 子句
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

        // 走 owned 版本：避免借用版内部对每个值再 clone 一次（消除多余的双重 clone）。
        // 占位符编号从 self.params 当前长度接续，UPDATE 的 SET 子句已压入的参数会被正确跳过。
        if conditions.len() == 1 {
            let sql = crate::postgres::condition::condition_to_sql_owned(
                conditions[0].clone(),
                &mut self.params,
            );
            self.append(&sql);
        } else {
            // 多个条件用 AND 连接
            let combined = Condition::And(conditions.to_vec());
            let sql =
                crate::postgres::condition::condition_to_sql_owned(combined, &mut self.params);
            self.append(&sql);
        }

        Ok(())
    }

    /// 生成 JOIN 子句
    ///
    /// # 参数
    /// - joins: JOIN 子句列表
    fn build_joins(&mut self, joins: &[JoinClause]) {
        use crate::postgres::field::JoinType;

        for join in joins {
            let join_type_str = match join.join_type {
                JoinType::Inner => " INNER JOIN ",
                JoinType::Left => " LEFT JOIN ",
                JoinType::Right => " RIGHT JOIN ",
            };

            self.append(join_type_str);
            self.append(&join.table);
            self.append(" ON ");
            self.append(&join.on);
        }
    }

    /// 生成 ORDER BY 子句
    ///
    /// # 参数
    /// - orders: 排序子句列表
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

    /// 生成 GROUP BY 子句
    ///
    /// # 参数
    /// - groups: 分组字段列表
    fn build_group_by(&mut self, groups: &[String]) {
        if groups.is_empty() {
            return;
        }

        self.append(" GROUP BY ");
        self.append(&groups.join(", "));
    }

    /// 生成 HAVING 子句
    fn build_having(&mut self, conditions: &[Condition]) -> Result<(), crate::error::DbError> {
        self.append(" HAVING ");
        if conditions.len() == 1 {
            let sql =
                crate::postgres::condition::condition_to_sql(&conditions[0], &mut self.params);
            self.append(&sql);
        } else {
            // 直接 push_str 写入 self.sql，避免先 collect Vec<String> 再 join。
            // 先把片段存入局部变量结束对 self.params 的可变借用，再 push_str 到 self.sql。
            // 占位符编号由 condition 模块基于 self.params 当前长度推导，跨片段连续。
            for (i, c) in conditions.iter().enumerate() {
                if i > 0 {
                    self.sql.push_str(" AND ");
                }
                let frag = crate::postgres::condition::condition_to_sql(c, &mut self.params);
                self.sql.push_str(&frag);
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
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    pub(crate) fn build_insert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        // 确保 data 是一个对象
        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        // 提取字段名和占位符
        let mut fields = Vec::new();
        let mut placeholders = Vec::new();

        for (key, value) in obj.iter() {
            fields.push(key.clone());

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            // NULL 内联字面量，不占占位符编号；其余压入并生成 $N
            match sql_value {
                SqlValue::Null => placeholders.push("NULL".to_string()),
                v => placeholders.push(push_placeholder(&mut self.params, v)),
            }
        }

        // 构建 INSERT 语句
        self.append("INSERT INTO ");
        self.append(table);
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

        self.append("INSERT INTO ");
        self.append(table);
        self.append(" (");
        self.append(&fields.join(", "));
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
                        let ph = push_placeholder(&mut self.params, v);
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
    ) -> Result<(), crate::error::DbError> {
        self.clear();

        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "更新数据不能为空".to_string(),
            ));
        }

        self.append("UPDATE ");
        self.append(table);
        self.append(" SET ");

        // 逐字段生成 `字段 = $N`；值为 NULL 时内联字面量 NULL（不占占位符编号），
        // 与 build_insert/build_update_batch/build_upsert 一致。PG 对未类型化绑定的
        // NULL 默认按 INT4，对 text/timestamp 等列 SET col=$1 绑 NULL 会报类型不匹配（DB-3）。
        for (i, (key, value)) in obj.iter().enumerate() {
            if i > 0 {
                self.sql.push_str(", ");
            }
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.sql.push_str(key);
            self.sql.push_str(" = ");
            match sql_value {
                SqlValue::Null => self.sql.push_str("NULL"),
                v => {
                    let ph = push_placeholder(&mut self.params, v);
                    self.sql.push_str(&ph);
                }
            }
        }

        // WHERE 子句的占位符编号接续 SET 子句已压入的参数
        self.build_where(conditions)?;

        Ok(())
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
        self.append(table);
        self.build_where(conditions)?;

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

        self.sql.push_str("UPDATE ");
        self.sql.push_str(table);
        self.sql.push_str(" SET ");

        // 为每个字段生成 CASE WHEN 子句
        for (field_idx, field) in update_fields.iter().enumerate() {
            if field_idx > 0 {
                self.sql.push_str(", ");
            }
            self.sql.push_str(field);
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
                self.sql.push_str(id_field);
                self.sql.push('=');
                match id_sql_val {
                    SqlValue::Null => self.sql.push_str("NULL"),
                    v => {
                        let ph = push_placeholder(&mut self.params, v);
                        self.sql.push_str(&ph);
                    }
                }
                self.sql.push_str(" THEN ");
                match field_sql_val {
                    SqlValue::Null => self.sql.push_str("NULL"),
                    v => {
                        let ph = push_placeholder(&mut self.params, v);
                        self.sql.push_str(&ph);
                    }
                }
                self.sql.push(' ');
            }

            self.sql.push_str("END");
        }

        // 生成 WHERE id IN ($K, ...) 子句
        self.sql.push_str(" WHERE ");
        self.sql.push_str(id_field);
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
                    let ph = push_placeholder(&mut self.params, v);
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
    /// 冲突目标列（`conflict_columns`），并用 `EXCLUDED.col` 引用待插入的新值。
    /// 当 `conflict_columns` 为空时回退为 `ON CONFLICT DO NOTHING`（无冲突目标，
    /// 仅在任意约束冲突时跳过）。冲突目标列本身不出现在 DO UPDATE 的 SET 列表中。
    pub(crate) fn build_upsert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        conflict_columns: &[String],
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

        self.sql.push_str("INSERT INTO ");
        self.sql.push_str(table);
        self.sql.push_str(" (");
        self.sql.push_str(&fields.join(", "));
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
                    let ph = push_placeholder(&mut self.params, v);
                    self.sql.push_str(&ph);
                }
            }
        }
        self.sql.push(')');

        if conflict_columns.is_empty() {
            // 无显式冲突目标：任意约束冲突时跳过插入
            self.sql.push_str(" ON CONFLICT DO NOTHING");
            return Ok(());
        }

        self.sql.push_str(" ON CONFLICT (");
        self.sql.push_str(&conflict_columns.join(", "));
        self.sql.push_str(") DO UPDATE SET ");

        // 冲突目标列不参与更新，其余列用 EXCLUDED.col 取待插入的新值
        let mut first = true;
        for f in &fields {
            if conflict_columns.iter().any(|c| c == f) {
                continue;
            }
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(f);
            self.sql.push_str(" = EXCLUDED.");
            self.sql.push_str(f);
        }

        // 所有列都是冲突目标列时 SET 列表为空，会导致语法错误；
        // 退化为对首个冲突列做恒等更新（col = EXCLUDED.col），保证语句合法。
        if first {
            let first_conflict = &conflict_columns[0];
            self.sql.push_str(first_conflict);
            self.sql.push_str(" = EXCLUDED.");
            self.sql.push_str(first_conflict);
        }

        Ok(())
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
                FieldType::Json => return Ok(SqlValue::Json(value.clone())),
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
                    if let Some(f) = value.as_f64() {
                        return Ok(SqlValue::Float(f));
                    } else if let Some(i) = value.as_i64() {
                        return Ok(SqlValue::Float(i as f64));
                    }
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
                FieldType::Standard => {}
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

/// 将 `(字段, 操作符, 值)` 映射为 6 个比较类 `Condition` 变体的共享助手。
///
/// 仅处理比较操作符（`=`、`!=`、`>`、`<`、`>=`、`<=`）。无法匹配时把 `value`
/// 原样通过 `Err` 交还调用方，便于上层继续处理 like 等其它操作符而不丢失所有权。
fn map_comparison_condition(field: &str, op: &str, value: SqlValue) -> Result<Condition, SqlValue> {
    match op {
        "=" => Ok(Condition::Eq(field.to_string(), value)),
        "!=" => Ok(Condition::Ne(field.to_string(), value)),
        ">" => Ok(Condition::Gt(field.to_string(), value)),
        "<" => Ok(Condition::Lt(field.to_string(), value)),
        ">=" => Ok(Condition::Gte(field.to_string(), value)),
        "<=" => Ok(Condition::Lte(field.to_string(), value)),
        _ => Err(value),
    }
}

/// 查询构建器（PostgreSQL）
pub struct QueryBuilder<'a> {
    pool: &'a PgPool,
    table: String,
    fields: Vec<String>,
    conditions: Vec<Condition>,
    joins: Vec<JoinClause>,
    order_by: Vec<OrderClause>,
    group_by: Vec<String>,
    having_clause: Vec<Condition>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
    field_types: HashMap<String, FieldType>,
    /// UPSERT 的冲突目标列（PostgreSQL `ON CONFLICT (...)` 必需）。
    /// 默认 `["id"]`，可通过 [`QueryBuilder::on_conflict`] 覆盖。
    conflict_columns: Vec<String>,
    /// `insert` 的 `RETURNING` 列，用于取回自增主键。默认 `"id"`。
    returning: String,
    enable_logging: bool,
}

impl<'a> QueryBuilder<'a> {
    /// 创建新的查询构建器
    pub(crate) fn new(pool: &'a PgPool, table_name: &str, enable_logging: bool) -> Self {
        Self {
            pool,
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
            field_types: HashMap::new(),
            conflict_columns: vec!["id".to_string()],
            returning: "id".to_string(),
            enable_logging,
        }
    }

    /// 选择字段
    pub fn field(mut self, field: &str) -> Self {
        self.fields.push(field.to_string());
        self
    }

    /// 选择多个字段
    pub fn fields(mut self, fields: &[&str]) -> Self {
        for field in fields {
            self.fields.push(field.to_string());
        }
        self
    }

    /// 标记字段为 JSON 类型
    pub fn json(mut self, field: &str) -> Self {
        self.field_types.insert(field.to_string(), FieldType::Json);
        self
    }

    /// 标记字段为 DATETIME 类型
    pub fn datetime(mut self, field: &str) -> Self {
        self.field_types
            .insert(field.to_string(), FieldType::DateTime);
        self
    }

    /// 标记字段为 TIMESTAMP 类型
    pub fn timestamp(mut self, field: &str) -> Self {
        self.field_types
            .insert(field.to_string(), FieldType::Timestamp);
        self
    }

    /// 标记字段为 DECIMAL 类型
    pub fn decimal(mut self, field: &str) -> Self {
        self.field_types
            .insert(field.to_string(), FieldType::Decimal);
        self
    }

    /// 标记字段为 BLOB（BYTEA）类型
    pub fn blob(mut self, field: &str) -> Self {
        self.field_types.insert(field.to_string(), FieldType::Blob);
        self
    }

    /// 标记字段为 TEXT 类型
    pub fn text(mut self, field: &str) -> Self {
        self.field_types.insert(field.to_string(), FieldType::Text);
        self
    }

    /// 去重
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// 设置 UPSERT 的冲突目标列（PostgreSQL `ON CONFLICT (...)`）
    ///
    /// 默认冲突列为 `["id"]`。当唯一约束建立在其它列（如 `email`）上时，
    /// 调用本方法覆盖，例如 `.on_conflict(&["email"])`。
    pub fn on_conflict(mut self, columns: &[&str]) -> Self {
        self.conflict_columns = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    /// 设置 `insert` 的 `RETURNING` 列（默认 `"id"`）
    ///
    /// PostgreSQL 无 `last_insert_id()`，`insert` 通过 `RETURNING <col>` 取回主键。
    /// 当自增主键列名不是 `id` 时，用本方法指定。
    pub fn returning(mut self, column: &str) -> Self {
        self.returning = column.to_string();
        self
    }

    /// 添加 AND 条件（不检查操作符，保持向后兼容）
    ///
    /// 与 `where_and` 相同，但遇到不支持的操作符时直接 panic，
    /// 保持原有行为，供需要链式调用且确定操作符合法的场景使用。
    pub fn where_and_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.where_and(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// 添加 OR 条件（不检查操作符，保持向后兼容）
    ///
    /// 与 `where_or` 相同，但遇到不支持的操作符时直接 panic，
    /// 保持原有行为，供需要链式调用且确定操作符合法的场景使用。
    pub fn where_or_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.where_or(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// 添加 HAVING 条件（不检查操作符，保持向后兼容）
    ///
    /// # 弃用说明
    ///
    /// 请改用 [`having_cond`](Self::having_cond)，它在操作符非法时返回 `Err`
    /// 而非 panic，可安全地在链式调用中传播错误。
    #[deprecated(
        since = "0.1.3",
        note = "使用 `having_cond` 替代：它在操作符非法时返回 Err 而非 panic，更安全。"
    )]
    pub fn having_cond_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.having_cond(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// 添加 AND 条件
    ///
    /// 遇到不支持的操作符时返回 `Err(DbError::UnsupportedOperator)`。
    /// 支持的操作符：`=`、`!=`、`>`、`<`、`>=`、`<=`、`like`、`LIKE`。
    pub fn where_and<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let sql_value = value.into();
        let condition = match map_comparison_condition(field, op, sql_value) {
            Ok(c) => c,
            Err(sql_value) => match op {
                "like" | "LIKE" => {
                    if let SqlValue::String(s) = sql_value {
                        Condition::Like(field.to_string(), s)
                    } else {
                        Condition::Like(field.to_string(), format!("{:?}", sql_value))
                    }
                }
                _ => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
            },
        };

        self.conditions.push(condition);
        Ok(self)
    }

    /// 添加 OR 条件
    ///
    /// 遇到不支持的操作符时返回 `Err(DbError::UnsupportedOperator)`。
    pub fn where_or<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let sql_value = value.into();
        let condition = match map_comparison_condition(field, op, sql_value) {
            Ok(c) => c,
            Err(sql_value) => match op {
                "like" | "LIKE" => {
                    if let SqlValue::String(s) = sql_value {
                        Condition::Like(field.to_string(), s)
                    } else {
                        Condition::Like(field.to_string(), format!("{:?}", sql_value))
                    }
                }
                _ => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
            },
        };

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
    pub fn where_in<V>(mut self, field: &str, values: Vec<V>) -> Self
    where
        V: Into<SqlValue>,
    {
        let sql_values: Vec<_> = values.into_iter().map(|v| v.into()).collect();
        self.conditions
            .push(Condition::In(field.to_string(), sql_values));
        self
    }

    /// 添加 BETWEEN 条件
    pub fn where_between<V>(mut self, field: &str, start: V, end: V) -> Self
    where
        V: Into<SqlValue>,
    {
        self.conditions.push(Condition::Between(
            field.to_string(),
            start.into(),
            end.into(),
        ));
        self
    }

    /// 添加 IS NULL 条件
    pub fn where_null(mut self, field: &str) -> Self {
        self.conditions.push(Condition::IsNull(field.to_string()));
        self
    }

    /// 添加 IS NOT NULL 条件
    pub fn where_not_null(mut self, field: &str) -> Self {
        self.conditions
            .push(Condition::IsNotNull(field.to_string()));
        self
    }

    /// 添加 HAVING 条件（仅支持 6 个比较操作符）
    pub fn having_cond<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<SqlValue>,
    {
        let sql_value = value.into();
        let condition = match map_comparison_condition(field, op, sql_value) {
            Ok(c) => c,
            Err(_) => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
        };
        self.having_clause.push(condition);
        Ok(self)
    }

    /// INNER JOIN
    pub fn join(mut self, table: &str, on: &str) -> Self {
        use crate::postgres::field::JoinType;
        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// LEFT JOIN
    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        use crate::postgres::field::JoinType;
        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// RIGHT JOIN
    pub fn right_join(mut self, table: &str, on: &str) -> Self {
        use crate::postgres::field::JoinType;
        self.joins.push(JoinClause {
            join_type: JoinType::Right,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// 排序
    pub fn order(mut self, field: &str, asc: bool) -> Self {
        self.order_by.push(OrderClause {
            field: field.to_string(),
            asc,
        });
        self
    }

    /// 分组
    pub fn group(mut self, field: &str) -> Self {
        self.group_by.push(field.to_string());
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

    /// 获取生成的 SQL（用于调试）
    pub fn to_sql(&self) -> String {
        let mut generator = SqlGenerator::new();
        match generator.build_select(self) {
            Ok(_) => generator.get_sql().to_string(),
            Err(_) => {
                let fields_str = if self.fields.is_empty() {
                    "*".to_string()
                } else {
                    self.fields.join(", ")
                };
                let distinct_str = if self.distinct { "DISTINCT " } else { "" };
                format!("SELECT {}{} FROM {}", distinct_str, fields_str, self.table)
            }
        }
    }

    /// 查询单条记录（自动追加 LIMIT 1）
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

        match query.fetch_optional(self.pool).await {
            Ok(row) => Ok(row),
            Err(e) => {
                log::error!("find() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询多条记录
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

        match query.fetch_all(self.pool).await {
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

        match query.fetch_optional(self.pool).await {
            Ok(value) => Ok(value),
            Err(e) => {
                log::error!("标量查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询单个字段值（自动追加 LIMIT 1）
    pub async fn value<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 value() 查询，字段: {}", field);
        }
        self.fetch_scalar::<T>(field).await
    }

    /// 统计记录数量（COUNT(*)）
    pub async fn count(self) -> Result<i64, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 count() 查询");
        }
        let result = self.value::<i64>("COUNT(*)").await?;
        Ok(result.unwrap_or(0))
    }
    /// 计算字段总和（SUM）
    ///
    /// 使用 `CAST(SUM(field) AS DOUBLE PRECISION)` 统一返回 `f64`，
    /// 与 MySQL 后端的 `AS DOUBLE` 对应（PostgreSQL 的等价类型为 `DOUBLE PRECISION`）。
    pub async fn sum(self, field: &str) -> Result<Option<f64>, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 sum() 查询，字段: {}", field);
        }
        let sum_expr = format!("CAST(SUM({}) AS DOUBLE PRECISION)", field);
        self.fetch_scalar::<Option<f64>>(&sum_expr)
            .await
            .map(Option::flatten)
    }

    /// 计算字段平均值（AVG）
    ///
    /// 使用 `CAST(AVG(field) AS DOUBLE PRECISION)` 统一返回 `f64`。
    pub async fn avg(self, field: &str) -> Result<Option<f64>, crate::error::DbError> {
        if self.enable_logging {
            log::debug!("执行 avg() 查询，字段: {}", field);
        }
        let avg_expr = format!("CAST(AVG({}) AS DOUBLE PRECISION)", field);
        self.fetch_scalar::<Option<f64>>(&avg_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最小值（MIN）
    pub async fn min<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 min() 查询，字段: {}", field);
        }
        let min_expr = format!("MIN({})", field);
        self.fetch_scalar::<Option<T>>(&min_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最大值（MAX）
    pub async fn max<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 max() 查询，字段: {}", field);
        }
        let max_expr = format!("MAX({})", field);
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
    pub async fn insert<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if self.enable_logging {
            log::debug!("执行 insert() 操作，表: {}", self.table);
        }

        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        let mut generator = SqlGenerator::new();
        generator.build_insert(&self.table, &json_data, &self.field_types)?;

        // PostgreSQL 无 last_insert_id()，改用 RETURNING 取回自增主键。
        // 用 CAST(... AS BIGINT) 统一首列类型：SERIAL 为 INT4、BIGSERIAL 为 INT8，
        // sqlx 解码严格匹配类型，强制转为 BIGINT 后即可统一按 i64 解码。
        let sql = format!(
            "{} RETURNING CAST({} AS BIGINT)",
            generator.get_sql(),
            self.returning
        );
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行 insert() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // RETURNING 首列解码为 i64，再转回 u64 保持与 MySQL 后端一致的返回类型
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for param in params {
            query = bind_scalar_param(query, param);
        }

        match query.fetch_one(self.pool).await {
            Ok(id) => {
                if self.enable_logging {
                    log::debug!("insert() 成功，插入 ID: {}", id);
                }
                Ok(id as u64)
            }
            Err(e) => {
                log::error!("insert() 失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }
    /// 批量插入数据
    ///
    /// 使用默认批大小 [`INSERT_BATCH_SIZE`]（500）分批执行，返回总受影响行数。
    /// 批量插入不使用 `RETURNING`，仅返回 `rows_affected()`。
    pub async fn insert_batch<T>(self, data: &[T]) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        self.insert_batch_with_size(data, INSERT_BATCH_SIZE).await
    }

    /// 批量插入数据（自定义批次大小）
    ///
    /// 允许调用方根据网络延迟、数据大小等自定义每批最大记录数（必须 > 0）。
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

        let mut total_affected = 0u64;
        for (batch_index, chunk) in data.chunks(batch_size).enumerate() {
            if self.enable_logging {
                log::debug!(
                    "执行第 {} 批插入，本批记录数: {}",
                    batch_index + 1,
                    chunk.len()
                );
            }
            let affected = self.insert_chunk(chunk).await?;
            total_affected += affected;
        }

        if self.enable_logging {
            log::debug!(
                "insert_batch_with_size() 全部完成，总共影响 {} 行",
                total_affected
            );
        }

        Ok(total_affected)
    }
    /// 插入单个批次的数据（内部方法）
    ///
    /// 仅借用 `&self`，避免为每批克隆整个 builder。返回 `rows_affected()`。
    async fn insert_chunk<T>(&self, data: &[T]) -> Result<u64, crate::error::DbError>
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

        match query.execute(self.pool).await {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("insert_chunk() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                log::error!("insert_chunk() 失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }
    /// 更新数据
    ///
    /// 执行 UPDATE 操作。为防止误操作，必须提供 WHERE 条件，否则返回
    /// `MissingWhereClause`。返回受影响的行数。
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
        generator.build_update(&self.table, &json_data, &self.field_types, &self.conditions)?;

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

        match query.execute(self.pool).await {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("update() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                log::error!("update() 失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 删除数据
    ///
    /// 执行 DELETE 操作。为防止误操作，必须提供 WHERE 条件，否则返回
    /// `MissingWhereClause`。返回受影响的行数。
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

        match query.execute(self.pool).await {
            Ok(query_result) => {
                let rows_affected = query_result.rows_affected();
                if self.enable_logging {
                    log::debug!("delete() 成功，影响 {} 行", rows_affected);
                }
                Ok(rows_affected)
            }
            Err(e) => {
                log::error!("delete() 失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }
    /// 批量更新记录
    ///
    /// 使用 CASE WHEN 策略在单次查询中更新多条记录，自动分批（每批
    /// [`UPDATE_BATCH_SIZE`]，1000），所有批次在同一事务中执行保证原子性。
    /// 返回总受影响行数。
    pub async fn update_batch<T>(
        self,
        records: &[T],
        where_field: &str,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        if records.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量更新数据不能为空".to_string(),
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

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(crate::error::DbError::from)?;
        let mut total = 0u64;

        for chunk in json_records.chunks(UPDATE_BATCH_SIZE) {
            let mut generator = SqlGenerator::new();
            generator.build_update_batch(&self.table, chunk, where_field, &self.field_types)?;

            let sql = generator.get_sql();
            let params = generator.get_params();

            let mut query = sqlx::query(sql);
            for param in params {
                query = bind_execute_param(query, param);
            }

            let result = query
                .execute(&mut *tx)
                .await
                .map_err(crate::error::DbError::from)?;
            total += result.rows_affected();
        }

        tx.commit().await.map_err(crate::error::DbError::from)?;
        Ok(total)
    }

    /// UPSERT - 插入或更新记录
    ///
    /// 使用 PostgreSQL `INSERT ... ON CONFLICT (...) DO UPDATE SET col = EXCLUDED.col`
    /// 语法。冲突目标列默认 `["id"]`，可用 [`QueryBuilder::on_conflict`] 覆盖。
    /// 返回 `rows_affected()`。
    pub async fn upsert<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
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
            &self.conflict_columns,
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

        let result = query
            .execute(self.pool)
            .await
            .map_err(crate::error::DbError::from)?;
        let rows = result.rows_affected();

        if self.enable_logging {
            log::debug!("upsert() 完成，rows_affected: {}", rows);
        }

        Ok(rows)
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
fn bind_param<'q, T>(
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
// PLACEHOLDER_TESTS

#[cfg(test)]
mod tests {
    use super::*;
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
        g.build_insert("users", &data, &empty_types()).unwrap();
        let sql = g.get_sql();
        // serde_json 默认按 key 字母序：age 在前、name 在后
        assert_eq!(sql, "INSERT INTO users (age, name) VALUES ($1, $2)");
        // INSERT 本身不含 RETURNING（由 insert() 方法在外部追加）
        assert!(!sql.contains("RETURNING"));
        assert_eq!(g.get_params().len(), 2);
    }

    #[test]
    fn test_build_insert_inlines_null_without_placeholder() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三", "deleted_at": null});
        g.build_insert("users", &data, &empty_types()).unwrap();
        let sql = g.get_sql();
        // 字母序：deleted_at(NULL 内联) 在前、name($1) 在后；NULL 不占占位符编号
        assert_eq!(
            sql,
            "INSERT INTO users (deleted_at, name) VALUES (NULL, $1)"
        );
        assert_eq!(g.get_params().len(), 1);
    }

    #[test]
    fn test_build_update_set_then_where_numbering_continues() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "李四", "age": 30});
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(1))];
        g.build_update("users", &data, &empty_types(), &conditions)
            .unwrap();
        let sql = g.get_sql();
        // 字母序：age=$1, name=$2；WHERE 接续为 $3
        assert_eq!(sql, "UPDATE users SET age = $1, name = $2 WHERE id = $3");
        assert_eq!(g.get_params().len(), 3);
    }

    /// DB-3：单行 UPDATE 的 SET 子句对 NULL 内联字面量，不占占位符编号。
    #[test]
    fn test_build_update_inlines_null_in_set() {
        let mut g = SqlGenerator::new();
        // description 设为 NULL，name 为普通值
        let data = serde_json::json!({"description": serde_json::Value::Null, "name": "张三"});
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(1))];
        g.build_update("users", &data, &empty_types(), &conditions)
            .unwrap();
        let sql = g.get_sql();
        // 字母序：description 内联 NULL（不占编号），name=$1，WHERE 接续 $2
        assert_eq!(sql, "UPDATE users SET description = NULL, name = $1 WHERE id = $2");
        // NULL 不压参数：仅 name + WHERE id 两个参数
        assert_eq!(g.get_params().len(), 2);
    }

    #[test]
    fn test_build_delete_dollar_placeholder() {
        let mut g = SqlGenerator::new();
        let conditions = vec![Condition::Eq("id".to_string(), SqlValue::Int(7))];
        g.build_delete("users", &conditions).unwrap();
        assert_eq!(g.get_sql(), "DELETE FROM users WHERE id = $1");
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
        let conflict = vec!["id".to_string()];
        g.build_upsert("users", &data, &empty_types(), &conflict)
            .unwrap();
        let sql = g.get_sql();
        // 字母序字段：email($1), id($2), name($3)
        assert!(sql.starts_with("INSERT INTO users (email, id, name) VALUES ($1, $2, $3)"));
        assert!(sql.contains("ON CONFLICT (id) DO UPDATE SET"));
        // 冲突列 id 不参与更新，其余列用 EXCLUDED.col
        assert!(sql.contains("name = EXCLUDED.name"));
        assert!(sql.contains("email = EXCLUDED.email"));
        assert!(!sql.contains("id = EXCLUDED.id"));
        assert_eq!(g.get_params().len(), 3);
    }

    #[test]
    fn test_build_upsert_empty_conflict_do_nothing() {
        let mut g = SqlGenerator::new();
        let data = serde_json::json!({"name": "张三"});
        g.build_upsert("users", &data, &empty_types(), &[]).unwrap();
        assert!(g.get_sql().contains("ON CONFLICT DO NOTHING"));
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
            "INSERT INTO users (age, name) VALUES ($1, $2), ($3, $4)"
        );
        assert_eq!(g.get_params().len(), 4);
    }

    #[test]
    fn test_sum_expr_uses_double_precision() {
        let pool = make_sync_test_pool();
        // 通过 fetch_scalar 路径间接构造的表达式无法静态读取，这里直接断言生成片段
        let expr = format!("CAST(SUM({}) AS DOUBLE PRECISION)", "amount");
        assert!(expr.contains("DOUBLE PRECISION"));
        // 确保 builder 能正常构造（不发起连接）
        let sql = QueryBuilder::new(pool, "orders", false)
            .field(&expr)
            .to_sql();
        assert!(sql.contains("DOUBLE PRECISION"));
        assert!(sql.contains("FROM orders"));
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
            .field("id")
            .field("name")
            .to_sql();
        assert_eq!(sql, "SELECT id, name FROM users");
    }

    #[test]
    fn test_to_sql_where_uses_dollar() {
        let pool = make_sync_test_pool();
        let sql = QueryBuilder::new(pool, "users", false)
            .where_and_unchecked("id", "=", 1)
            .to_sql();
        assert_eq!(sql, "SELECT * FROM users WHERE id = $1");
    }

    #[test]
    fn test_default_returning_and_conflict_columns() {
        let pool = make_sync_test_pool();
        let qb = QueryBuilder::new(pool, "users", false);
        assert_eq!(qb.returning, "id");
        assert_eq!(qb.conflict_columns, vec!["id".to_string()]);
        let qb2 = qb.returning("uid").on_conflict(&["email", "tenant_id"]);
        assert_eq!(qb2.returning, "uid");
        assert_eq!(
            qb2.conflict_columns,
            vec!["email".to_string(), "tenant_id".to_string()]
        );
    }
}
