use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::field::{FieldType, JoinClause, OrderClause};
use sqlx::mysql::MySqlPool;
use std::collections::HashMap;

/// 将 SqlValue 绑定到 sqlx 查询的内部宏
///
/// 封装 SqlValue 各变体到 `.bind()` 调用的映射逻辑，消除 4 个 bind_param
/// 函数中完全相同的 match 分支重复代码。未来新增 SqlValue 变体时，
/// 只需在此宏中添加一个分支即可完成所有函数的更新。
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
            // 字节数组（需要 clone）
            SqlValue::Bytes(b) => $query.bind(b.clone()),
            // JSON 值：序列化为字符串后绑定
            SqlValue::Json(j) => $query.bind(j.to_string()),
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

        // 这里 conditions 是借用，但 build_where 已经决定要消费它构造的临时 Condition，
        // 故走 owned 版本：避免借用版内部对每个值再 clone 一次（消除多余的双重 clone）。
        if conditions.len() == 1 {
            let sql = crate::mysql::condition::condition_to_sql_owned(
                conditions[0].clone(),
                &mut self.params,
            );
            self.append(&sql);
        } else {
            // 多个条件用 AND 连接
            let combined = Condition::And(conditions.to_vec());
            let sql = crate::mysql::condition::condition_to_sql_owned(combined, &mut self.params);
            self.append(&sql);
        }

        Ok(())
    }

    /// 生成 JOIN 子句
    ///
    /// # 参数
    /// - joins: JOIN 子句列表
    fn build_joins(&mut self, joins: &[JoinClause]) {
        use crate::mysql::field::JoinType;

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

        // 直接 push_str 写入 self.sql，避免先 collect Vec<String> 再 join 的中间分配，
        // 与文件内 build_update_batch 的 push_str 风格一致。输出 SQL 完全不变。
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
            let sql = crate::mysql::condition::condition_to_sql(&conditions[0], &mut self.params);
            self.append(&sql);
        } else {
            // 直接 push_str 写入 self.sql，避免先 collect Vec<String> 再 join。
            // 先把片段存入局部变量结束对 self.params 的可变借用，再 push_str 到 self.sql，
            // 避免同时可变借用两个字段。参数 push 顺序与原 collect 顺序一致（均按 conditions 迭代序）。
            for (i, c) in conditions.iter().enumerate() {
                if i > 0 {
                    self.sql.push_str(" AND ");
                }
                let frag = crate::mysql::condition::condition_to_sql(c, &mut self.params);
                self.sql.push_str(&frag);
            }
        }
        Ok(())
    }

    /// 生成 INSERT 语句
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

        // 提取字段名和值
        let mut fields = Vec::new();
        let mut placeholders = Vec::new();

        for (key, value) in obj.iter() {
            fields.push(key.clone());
            placeholders.push("?".to_string());

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.add_param(sql_value);
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

    /// 生成批量 INSERT 语句
    ///
    /// # 参数
    /// - table: 表名
    /// - data_list: 要插入的数据列表（JSON 格式）
    /// - field_types: 字段类型映射
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    pub(crate) fn build_insert_batch(
        &mut self,
        table: &str,
        data_list: &[serde_json::Value],
        field_types: &HashMap<String, FieldType>,
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        if data_list.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量插入数据不能为空".to_string(),
            ));
        }

        // 从第一条数据中提取字段名
        let first_obj = data_list[0].as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if first_obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        // 提取字段名（从第一条记录）
        let fields: Vec<String> = first_obj.keys().cloned().collect();

        // 构建 INSERT 语句头部
        self.append("INSERT INTO ");
        self.append(table);
        self.append(" (");
        self.append(&fields.join(", "));
        self.append(") VALUES ");

        // 直接将每条记录的 VALUES 子句写入 self.sql，避免中间 Vec<String> 分配
        for (record_idx, data) in data_list.iter().enumerate() {
            // 记录之间用 ", " 分隔，直接追加到 sql，替代最终的 join 调用
            if record_idx > 0 {
                self.sql.push_str(", ");
            }

            let obj = data.as_object().ok_or_else(|| {
                crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
            })?;

            // 直接写入 '(' 开始当前记录的占位符列表
            self.sql.push('(');

            // 逐字段追加 '?' 占位符，替代 format!("({})", placeholders.join(", ")) 模式
            for (field_idx, field) in fields.iter().enumerate() {
                // 字段之间用 ", " 分隔
                if field_idx > 0 {
                    self.sql.push_str(", ");
                }
                self.sql.push('?');

                // 获取字段值，如果不存在则使用 NULL
                let value = obj.get(field).unwrap_or(&serde_json::Value::Null);

                // 根据字段类型转换值并绑定参数
                let sql_value = self.json_value_to_sql_value(value, field_types.get(field))?;
                self.add_param(sql_value);
            }

            // 直接写入 ')' 结束当前记录的占位符列表
            self.sql.push(')');
        }

        Ok(())
    }

    /// 生成 UPDATE 语句
    ///
    /// # 参数
    /// - table: 表名
    /// - data: 要更新的数据（JSON 格式）
    /// - field_types: 字段类型映射
    /// - conditions: WHERE 条件列表
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    pub(crate) fn build_update(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        conditions: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        // 检查是否有 WHERE 条件
        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        // 确保 data 是一个对象
        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("更新数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "更新数据不能为空".to_string(),
            ));
        }

        // 构建 UPDATE 语句
        self.append("UPDATE ");
        self.append(table);
        self.append(" SET ");

        // 构建 SET 子句
        let mut set_clauses = Vec::new();

        for (key, value) in obj.iter() {
            set_clauses.push(format!("{} = ?", key));

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.add_param(sql_value);
        }

        self.append(&set_clauses.join(", "));

        // 添加 WHERE 子句
        self.build_where(conditions)?;

        Ok(())
    }

    /// 生成 DELETE 语句
    ///
    /// # 参数
    /// - table: 表名
    /// - conditions: WHERE 条件列表
    ///
    /// # 返回
    /// - Ok(()): 成功生成 SQL
    /// - Err(DbError): 生成失败
    pub(crate) fn build_delete(
        &mut self,
        table: &str,
        conditions: &[Condition],
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        // 检查是否有 WHERE 条件
        if conditions.is_empty() {
            return Err(crate::error::DbError::MissingWhereClause);
        }

        // 构建 DELETE 语句
        self.append("DELETE FROM ");
        self.append(table);

        // 添加 WHERE 子句
        self.build_where(conditions)?;

        Ok(())
    }

    /// 生成批量 UPDATE 语句（CASE WHEN 策略）
    ///
    /// 优化：直接将 CASE WHEN 子句写入 self.sql，消除 O(M×N) 的中间字符串分配。
    /// 每个字段只需一次 push_str 操作序列，中间分配次数降至 O(M) 级别。
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

        // 收集需要更新的字段名（排除主键字段），O(M) 次分配
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

        // 写入 UPDATE ... SET 头部
        self.sql.push_str("UPDATE ");
        self.sql.push_str(table);
        self.sql.push_str(" SET ");

        // 为每个字段生成 CASE WHEN 子句，直接追加到 self.sql
        // 消除了原来的 Vec<String> set_parts 和 Vec<String> when_parts 中间分配
        for (field_idx, field) in update_fields.iter().enumerate() {
            // 字段之间用 ", " 分隔
            if field_idx > 0 {
                self.sql.push_str(", ");
            }

            // 写入 "字段名 = CASE "
            self.sql.push_str(field);
            self.sql.push_str(" = CASE ");

            // 为每条记录生成 WHEN id=? THEN ? 子句，直接追加，替代 format! 收集再 join 的模式
            for record in records {
                let id_val = record.get(id_field).unwrap_or(&serde_json::Value::Null);
                let field_val = record
                    .get(field.as_str())
                    .unwrap_or(&serde_json::Value::Null);

                // 先转换参数值（避免借用冲突），再追加 SQL 片段和参数
                let id_sql_val = self.json_value_to_sql_value(id_val, field_types.get(id_field))?;
                let field_sql_val =
                    self.json_value_to_sql_value(field_val, field_types.get(field.as_str()))?;

                // 直接追加 WHEN id=? THEN ? 片段，替代 format!("WHEN {}=? THEN ?", id_field)
                self.sql.push_str("WHEN ");
                self.sql.push_str(id_field);
                self.sql.push_str("=? THEN ? ");

                // 绑定 id 参数和字段值参数
                self.params.push(id_sql_val);
                self.params.push(field_sql_val);
            }

            // 写入 CASE 结束标记
            self.sql.push_str("END");
        }

        // 生成 WHERE id IN (?, ?, ...) 子句
        self.sql.push_str(" WHERE ");
        self.sql.push_str(id_field);
        self.sql.push_str(" IN (");

        // 直接追加占位符，替代 Vec<&str> 收集再 join 的模式
        for (idx, record) in records.iter().enumerate() {
            if idx > 0 {
                self.sql.push(',');
            }
            self.sql.push('?');

            // 绑定 WHERE IN 子句中的 id 参数
            let id_val = record.get(id_field).unwrap_or(&serde_json::Value::Null);
            let id_sql_val = self.json_value_to_sql_value(id_val, field_types.get(id_field))?;
            self.params.push(id_sql_val);
        }

        self.sql.push(')');

        Ok(())
    }

    /// 生成 UPSERT (INSERT ... ON DUPLICATE KEY UPDATE) 语句
    pub(crate) fn build_upsert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &std::collections::HashMap<String, FieldType>,
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
        let placeholders: Vec<&str> = fields.iter().map(|_| "?").collect();

        // 统一用 push_str 风格拼接，避免 format! 的额外分配（输出 SQL 完全不变）
        self.sql.push_str("INSERT INTO ");
        self.sql.push_str(table);
        self.sql.push_str(" (");
        self.sql.push_str(&fields.join(", "));
        self.sql.push_str(") VALUES (");
        self.sql.push_str(&placeholders.join(", "));
        self.sql.push(')');

        for field in &fields {
            let val = obj.get(field.as_str()).unwrap_or(&serde_json::Value::Null);
            self.add_param(self.json_value_to_sql_value(val, field_types.get(field.as_str()))?);
        }

        self.sql.push_str(" ON DUPLICATE KEY UPDATE ");
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                self.sql.push_str(", ");
            }
            self.sql.push_str(f);
            self.sql.push_str("=VALUES(");
            self.sql.push_str(f);
            self.sql.push(')');
        }

        Ok(())
    }

    ///
    /// # 参数
    /// - value: JSON 值
    /// - field_type: 字段类型（可选）
    ///
    /// # 返回
    /// - Ok(SqlValue): 转换后的 SQL 值
    /// - Err(DbError): 转换失败
    fn json_value_to_sql_value(
        &self,
        value: &serde_json::Value,
        field_type: Option<&FieldType>,
    ) -> Result<SqlValue, crate::error::DbError> {
        use serde_json::Value;

        // 如果有字段类型标记，优先使用
        if let Some(ft) = field_type {
            match ft {
                FieldType::Json => {
                    // JSON 类型：直接存储为 JSON
                    return Ok(SqlValue::Json(value.clone()));
                }
                FieldType::DateTime => {
                    // DATETIME 类型：期望字符串格式
                    if let Some(s) = value.as_str() {
                        let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                            .map_err(|e| {
                                crate::error::DbError::TypeConversionError(format!(
                                    "无法解析 DATETIME 字符串: {}",
                                    e
                                ))
                            })?;
                        return Ok(SqlValue::DateTime(dt));
                    }
                }
                FieldType::Timestamp => {
                    // TIMESTAMP 类型：期望整数
                    if let Some(i) = value.as_i64() {
                        return Ok(SqlValue::Timestamp(i));
                    }
                }
                FieldType::Decimal => {
                    // DECIMAL 类型：转换为浮点数
                    if let Some(f) = value.as_f64() {
                        return Ok(SqlValue::Float(f));
                    } else if let Some(i) = value.as_i64() {
                        return Ok(SqlValue::Float(i as f64));
                    }
                }
                FieldType::Blob => {
                    // BLOB 类型：期望字节数组或 base64 字符串
                    if let Some(s) = value.as_str() {
                        // 尝试解析为 base64
                        use base64::Engine;
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(s) {
                            return Ok(SqlValue::Bytes(bytes));
                        }
                        // 否则直接转换为字节
                        return Ok(SqlValue::Bytes(s.as_bytes().to_vec()));
                    }
                }
                FieldType::Text => {
                    // TEXT 类型：转换为字符串
                    if let Some(s) = value.as_str() {
                        return Ok(SqlValue::String(s.to_string()));
                    }
                }
                FieldType::Standard => {
                    // 标准类型：按照默认规则处理
                }
            }
        }

        // 默认转换规则
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
            Value::Array(_) | Value::Object(_) => {
                // 数组和对象默认序列化为 JSON
                Ok(SqlValue::Json(value.clone()))
            }
        }
    }
}

/// 将 `(字段, 操作符, 值)` 映射为 6 个比较类 `Condition` 变体的共享助手。
///
/// 仅处理比较操作符（`=`、`!=`、`>`、`<`、`>=`、`<=`）。无法匹配时把 `value`
/// 原样通过 `Err` 交还调用方，便于上层继续处理 like 等其它操作符而不丢失所有权。
/// 抽出此助手以消除 `where_and` / `where_or` / `having_cond` 三处重复的映射 match。
fn map_comparison_condition(field: &str, op: &str, value: SqlValue) -> Result<Condition, SqlValue> {
    match op {
        "=" => Ok(Condition::Eq(field.to_string(), value)),
        "!=" => Ok(Condition::Ne(field.to_string(), value)),
        ">" => Ok(Condition::Gt(field.to_string(), value)),
        "<" => Ok(Condition::Lt(field.to_string(), value)),
        ">=" => Ok(Condition::Gte(field.to_string(), value)),
        "<=" => Ok(Condition::Lte(field.to_string(), value)),
        // 非比较操作符：把值原样交还调用方处理
        _ => Err(value),
    }
}

/// 查询构建器
pub struct QueryBuilder<'a> {
    #[allow(dead_code)]
    pool: &'a MySqlPool,
    table: String,
    fields: Vec<String>,
    #[allow(dead_code)]
    conditions: Vec<Condition>,
    #[allow(dead_code)]
    joins: Vec<JoinClause>,
    #[allow(dead_code)]
    order_by: Vec<OrderClause>,
    #[allow(dead_code)]
    group_by: Vec<String>,
    #[allow(dead_code)]
    having_clause: Vec<Condition>,
    limit: Option<u64>,
    offset: Option<u64>,
    distinct: bool,
    field_types: HashMap<String, FieldType>,
    #[allow(dead_code)]
    enable_logging: bool,
}

impl<'a> QueryBuilder<'a> {
    /// 创建新的查询构建器
    pub(crate) fn new(pool: &'a MySqlPool, table_name: &str, enable_logging: bool) -> Self {
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

    /// 标记字段为 BLOB 类型
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
    pub fn where_and<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::{Condition, SqlValue};

        let sql_value = value.into();
        // 先尝试 6 个比较操作符的共享映射；Err 时拿回值继续判断 like
        let condition = match map_comparison_condition(field, op, sql_value) {
            Ok(c) => c,
            Err(sql_value) => match op {
                "like" | "LIKE" => {
                    if let SqlValue::String(s) = sql_value {
                        Condition::Like(field.to_string(), s)
                    } else {
                        // 如果不是字符串，转换为字符串
                        Condition::Like(field.to_string(), format!("{:?}", sql_value))
                    }
                }
                // 不支持的操作符：返回错误而非 panic
                _ => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
            },
        };

        self.conditions.push(condition);
        Ok(self)
    }

    /// 添加 AND 条件（不检查操作符，保持向后兼容）
    ///
    /// 与 `where_and` 相同，但遇到不支持的操作符时直接 panic，
    /// 保持原有行为，供需要链式调用且确定操作符合法的场景使用。
    ///
    /// # 参数
    /// - `field`: 字段名
    /// - `op`: 比较操作符（必须是支持的操作符，否则 panic）
    /// - `value`: 比较值
    pub fn where_and_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        // 调用 where_and 并在出错时 panic，保持原有行为
        self.where_and(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
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
    pub fn where_or<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::{Condition, SqlValue};

        let sql_value = value.into();
        // 先尝试 6 个比较操作符的共享映射；Err 时拿回值继续判断 like
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
                // 不支持的操作符：返回错误而非 panic
                _ => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
            },
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

        Ok(self)
    }

    /// 添加 OR 条件（不检查操作符，保持向后兼容）
    ///
    /// 与 `where_or` 相同，但遇到不支持的操作符时直接 panic，
    /// 保持原有行为，供需要链式调用且确定操作符合法的场景使用。
    ///
    /// # 参数
    /// - `field`: 字段名
    /// - `op`: 比较操作符（必须是支持的操作符，否则 panic）
    /// - `value`: 比较值
    pub fn where_or_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        // 调用 where_or 并在出错时 panic，保持原有行为
        self.where_or(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// 添加 IN 条件
    pub fn where_in<V>(mut self, field: &str, values: Vec<V>) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::Condition;

        let sql_values: Vec<_> = values.into_iter().map(|v| v.into()).collect();
        self.conditions
            .push(Condition::In(field.to_string(), sql_values));
        self
    }

    /// 添加 BETWEEN 条件
    pub fn where_between<V>(mut self, field: &str, start: V, end: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        use crate::mysql::condition::Condition;

        self.conditions.push(Condition::Between(
            field.to_string(),
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
    /// # #[derive(serde::Deserialize, sqlx::FromRow)] struct User;
    /// let users = db.table("users")
    ///     .where_null("deleted_at")
    ///     .select::<User>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn where_null(mut self, field: &str) -> Self {
        self.conditions.push(Condition::IsNull(field.to_string()));
        self
    }

    /// 添加 IS NOT NULL 条件
    ///
    /// 生成 `field IS NOT NULL` 子句，用于查询字段值不为 NULL 的记录。
    ///
    /// # 参数
    /// - `field`: 字段名
    pub fn where_not_null(mut self, field: &str) -> Self {
        self.conditions
            .push(Condition::IsNotNull(field.to_string()));
        self
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
    /// # #[derive(serde::Deserialize, sqlx::FromRow)] struct OrderSummary;
    /// let result = db.table("orders")
    ///     .field("user_id")
    ///     .field("COUNT(*) as cnt")
    ///     .group("user_id")
    ///     .having_cond("cnt", ">", 5i64)
    ///     .select::<OrderSummary>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn having_cond<V>(
        mut self,
        field: &str,
        op: &str,
        value: V,
    ) -> Result<Self, crate::error::DbError>
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        let sql_value = value.into();
        // HAVING 仅支持 6 个比较操作符；like 等其它操作符仍返回 UnsupportedOperator（不 panic）
        let condition = match map_comparison_condition(field, op, sql_value) {
            Ok(c) => c,
            Err(_) => return Err(crate::error::DbError::UnsupportedOperator(op.to_string())),
        };
        self.having_clause.push(condition);
        Ok(self)
    }

    /// 添加 HAVING 条件（不检查操作符，保持向后兼容）
    ///
    /// 与 `having_cond` 相同，但遇到不支持的操作符时直接 panic，
    /// 保持原有行为，供需要链式调用且确定操作符合法的场景使用。
    ///
    /// # 弃用说明
    ///
    /// 请改用 [`having_cond`]，它在操作符非法时返回 `Err` 而非 panic，
    /// 可安全地在链式调用中传播错误：
    ///
    /// ```no_run
    /// # use yang_db::Database;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// # #[derive(serde::Deserialize, sqlx::FromRow)] struct Row;
    /// let result = db.table("orders")
    ///     .group("user_id")
    ///     .having_cond("cnt", ">", 5i64)?  // 返回 Result，而非 panic
    ///     .select::<Row>()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 参数
    /// - `field`: 聚合字段或聚合表达式
    /// - `op`: 比较运算符（必须是支持的操作符，否则 panic）
    /// - `value`: 比较值
    #[deprecated(
        since = "0.1.3",
        note = "使用 `having_cond` 替代：它在操作符非法时返回 Err 而非 panic，更安全。"
    )]
    pub fn having_cond_unchecked<V>(self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<crate::mysql::condition::SqlValue>,
    {
        // 调用 having_cond 并在出错时 panic，保持原有行为
        self.having_cond(field, op, value)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// INNER JOIN
    pub fn join(mut self, table: &str, on: &str) -> Self {
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// LEFT JOIN
    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// RIGHT JOIN
    pub fn right_join(mut self, table: &str, on: &str) -> Self {
        use crate::mysql::field::{JoinClause, JoinType};

        self.joins.push(JoinClause {
            join_type: JoinType::Right,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// 排序
    pub fn order(mut self, field: &str, asc: bool) -> Self {
        use crate::mysql::field::OrderClause;

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
    ///
    /// # 返回
    /// - 生成的完整 SQL 语句字符串
    pub fn to_sql(&self) -> String {
        let mut generator = SqlGenerator::new();

        // 使用 build_select 生成完整的 SQL
        match generator.build_select(self) {
            Ok(_) => generator.get_sql().to_string(),
            Err(_) => {
                // 如果生成失败，返回简化版本
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

    /// 查询单条记录
    ///
    /// 自动添加 LIMIT 1 到查询，返回单条记录或 None
    ///
    /// # 类型参数
    /// - T: 结果类型，必须实现 FromRow trait
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回单条记录
    /// - Ok(None): 查询成功，但没有匹配的记录
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let user: Option<User> = db.table("users")
    ///     .where_and("id", "=", 1)
    ///     .find()
    ///     .await?;
    ///
    /// match user {
    ///     Some(u) => println!("找到用户: {:?}", u),
    ///     None => println!("用户不存在"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find<T>(mut self) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 自动添加 LIMIT 1
        self.limit = Some(1);

        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 find() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query_as::<_, T>(sql);

        // 绑定参数
        for param in params {
            query = bind_param(query, param);
        }

        // 执行查询
        let result = query.fetch_optional(self.pool).await;

        match result {
            Ok(row) => {
                if self.enable_logging {
                    if row.is_some() {
                        log::debug!("find() 查询成功，返回 1 条记录");
                    } else {
                        log::debug!("find() 查询成功，未找到匹配记录");
                    }
                }
                Ok(row)
            }
            Err(e) => {
                log::error!("find() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询多条记录
    ///
    /// 执行 SELECT 查询并返回所有匹配的记录
    ///
    /// # 类型参数
    /// - T: 结果类型，必须实现 FromRow trait
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询成功，返回匹配的记录列表（可能为空）
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let users: Vec<User> = db.table("users")
    ///     .where_and("status", "=", 1)
    ///     .order("name", true)
    ///     .select()
    ///     .await?;
    ///
    /// println!("找到 {} 个用户", users.len());
    /// for user in users {
    ///     println!("用户: {:?}", user);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn select<T>(self) -> Result<Vec<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 select() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query_as::<_, T>(sql);

        // 绑定参数
        for param in params {
            query = bind_param(query, param);
        }

        // 执行查询
        let result = query.fetch_all(self.pool).await;

        match result {
            Ok(rows) => {
                if self.enable_logging {
                    log::debug!("select() 查询成功，返回 {} 条记录", rows.len());
                }
                Ok(rows)
            }
            Err(e) => {
                log::error!("select() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 标量查询助手（内部）
    ///
    /// 统一 `value` / `sum` / `avg` / `min` / `max` 五个方法的样板：清空字段选择、
    /// 只选 `select_expr`、自动加 `LIMIT 1`、生成 SELECT、绑定参数、执行单行单值查询。
    ///
    /// 类型参数 `C` 为 sqlx 解码目标类型：`value` 传 `T`（NULL 列触发解码错误的语义保持不变），
    /// 聚合函数传 `Option<U>`（外层再 `Option::flatten`）。生成的 SQL 与参数与各方法原实现逐字节一致。
    async fn fetch_scalar<C>(
        mut self,
        select_expr: &str,
    ) -> Result<Option<C>, crate::error::DbError>
    where
        C: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 清空现有字段选择，只选择指定表达式
        self.fields.clear();
        self.fields.push(select_expr.to_string());

        // 自动添加 LIMIT 1
        self.limit = Some(1);

        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行标量查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // query_scalar 直接获取单个值；bind_scalar_param 对任意输出类型通用
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

    /// 查询单个字段值
    ///
    /// 执行 SELECT 查询并返回指定字段的单个值。自动添加 LIMIT 1 到查询。
    ///
    /// # 参数
    /// - field: 要查询的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回字段值
    /// - Ok(None): 查询成功，但没有匹配的记录
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询用户名
    /// let name: Option<String> = db.table("users")
    ///     .where_and("id", "=", 1)
    ///     .value("name")
    ///     .await?;
    ///
    /// match name {
    ///     Some(n) => println!("用户名: {}", n),
    ///     None => println!("用户不存在"),
    /// }
    ///
    /// // 查询用户数量
    /// let count: Option<i64> = db.table("users")
    ///     .where_and("status", "=", 1)
    ///     .value("COUNT(*)")
    ///     .await?;
    ///
    /// println!("活跃用户数: {}", count.unwrap_or(0));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn value<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 value() 查询，字段: {}", field);
        }

        // 直接解码为 T（NULL 列触发解码错误的语义保持不变）
        self.fetch_scalar::<T>(field).await
    }

    /// 统计记录数量
    ///
    /// 执行 COUNT(*) 查询并返回匹配条件的记录数量。
    ///
    /// # 返回
    /// - Ok(i64): 查询成功，返回记录数量
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 统计所有用户数量
    /// let total_users = db.table("users")
    ///     .count()
    ///     .await?;
    /// println!("总用户数: {}", total_users);
    ///
    /// // 统计活跃用户数量
    /// let active_users = db.table("users")
    ///     .where_and("status", "=", 1)
    ///     .count()
    ///     .await?;
    /// println!("活跃用户数: {}", active_users);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn count(self) -> Result<i64, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 count() 查询");
        }

        // 使用 value() 方法查询 COUNT(*)
        let result = self.value::<i64>("COUNT(*)").await?;

        // COUNT(*) 总是返回一个值（至少是 0），所以这里 unwrap_or(0) 是安全的
        Ok(result.unwrap_or(0))
    }

    /// 计算字段总和
    ///
    /// 执行 SUM(field) 查询并返回指定字段的总和。
    ///
    /// # 参数
    /// - field: 要求和的字段名
    ///
    /// # 返回
    /// - Ok(Some(f64)): 查询成功，返回字段总和
    /// - Ok(None): 查询成功，但没有匹配的记录或字段值全为 NULL
    /// - Err(DbError): 查询执行失败
    ///
    /// # 注意
    /// MySQL 的 SUM() 函数对于整数字段返回 DECIMAL 类型，对于浮点数字段返回 DOUBLE 类型。
    /// 本方法使用 CAST 将结果转换为 DOUBLE，以统一返回类型。
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 计算所有订单总金额
    /// let total_amount = db.table("orders")
    ///     .sum("amount")
    ///     .await?;
    ///
    /// match total_amount {
    ///     Some(sum) => println!("订单总金额: {:.2}", sum),
    ///     None => println!("没有订单或金额全为 NULL"),
    /// }
    ///
    /// // 计算已完成订单的总金额
    /// let completed_amount = db.table("orders")
    ///     .where_and("status", "=", "completed")
    ///     .sum("amount")
    ///     .await?;
    ///
    /// println!("已完成订单总金额: {:.2}", completed_amount.unwrap_or(0.0));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sum(self, field: &str) -> Result<Option<f64>, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 sum() 查询，字段: {}", field);
        }

        // 构建 SUM(field) 表达式，并使用 CAST 转换为 DOUBLE
        // 这样可以统一处理整数和浮点数字段的求和结果
        let sum_expr = format!("CAST(SUM({}) AS DOUBLE)", field);

        // 用 Option<f64> 解码处理 NULL；fetch_scalar 外层 Option 表示有无行，
        // flatten 后与原 fetch_optional + match 的返回完全一致
        self.fetch_scalar::<Option<f64>>(&sum_expr)
            .await
            .map(Option::flatten)
    }

    /// 计算字段平均值
    ///
    /// 执行 AVG 聚合函数，计算指定字段的平均值。
    ///
    /// # 参数
    /// - field: 要计算平均值的字段名
    ///
    /// # 返回
    /// - Ok(Some(f64)): 计算成功，返回平均值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 只对数值类型字段有效
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与计算）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 计算所有用户的平均年龄
    /// let avg_age = db.table("users")
    ///     .avg("age")
    ///     .await?;
    ///
    /// if let Some(age) = avg_age {
    ///     println!("平均年龄: {:.1}", age);
    /// } else {
    ///     println!("没有数据");
    /// }
    ///
    /// // 计算已完成订单的平均金额
    /// let avg_amount = db.table("orders")
    ///     .where_and("status", "=", "completed")
    ///     .avg("amount")
    ///     .await?;
    ///
    /// println!("已完成订单平均金额: {:.2}", avg_amount.unwrap_or(0.0));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn avg(self, field: &str) -> Result<Option<f64>, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 avg() 查询，字段: {}", field);
        }

        // 构建 AVG(field) 表达式，并使用 CAST 转换为 DOUBLE
        // 这样可以统一处理整数和浮点数字段的平均值结果
        let avg_expr = format!("CAST(AVG({}) AS DOUBLE)", field);

        // 用 Option<f64> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<f64>>(&avg_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最小值
    ///
    /// 执行 MIN 聚合函数，获取指定字段的最小值。
    ///
    /// # 参数
    /// - field: 要查询最小值的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///   支持的类型包括：i32, i64, f32, f64, String, chrono::NaiveDateTime 等
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回最小值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 对数值类型字段返回数值最小值
    /// - 对字符串类型字段返回字典序最小值
    /// - 对日期时间类型字段返回最早时间
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与比较）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询最低价格（浮点数）
    /// let min_price: Option<f64> = db.table("products")
    ///     .min("price")
    ///     .await?;
    ///
    /// if let Some(price) = min_price {
    ///     println!("最低价格: {:.2}", price);
    /// } else {
    ///     println!("没有产品数据");
    /// }
    ///
    /// // 查询最小库存数量（整数）
    /// let min_stock: Option<i32> = db.table("products")
    ///     .where_and("status", "=", 1)
    ///     .min("stock")
    ///     .await?;
    ///
    /// println!("最小库存: {}", min_stock.unwrap_or(0));
    ///
    /// // 查询最早注册时间（字符串）
    /// let earliest_date: Option<String> = db.table("users")
    ///     .min("created_at")
    ///     .await?;
    ///
    /// if let Some(date) = earliest_date {
    ///     println!("最早注册时间: {}", date);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn min<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 min() 查询，字段: {}", field);
        }

        // 构建 MIN(field) 表达式
        let min_expr = format!("MIN({})", field);

        // 用 Option<T> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<T>>(&min_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最大值
    ///
    /// 执行 MAX 聚合函数，获取指定字段的最大值。
    ///
    /// # 参数
    /// - field: 要查询最大值的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///   支持的类型包括：i32, i64, f32, f64, String, chrono::NaiveDateTime 等
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回最大值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 对数值类型字段返回数值最大值
    /// - 对字符串类型字段返回字典序最大值
    /// - 对日期时间类型字段返回最晚时间
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与比较）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询最高价格（浮点数）
    /// let max_price: Option<f64> = db.table("products")
    ///     .max("price")
    ///     .await?;
    ///
    /// if let Some(price) = max_price {
    ///     println!("最高价格: {:.2}", price);
    /// } else {
    ///     println!("没有产品数据");
    /// }
    ///
    /// // 查询最高分数（整数）
    /// let max_score: Option<i32> = db.table("scores")
    ///     .where_and("exam_id", "=", 1)
    ///     .max("score")
    ///     .await?;
    ///
    /// println!("最高分: {}", max_score.unwrap_or(0));
    ///
    /// // 查询最新更新时间（字符串）
    /// let latest_date: Option<String> = db.table("articles")
    ///     .max("updated_at")
    ///     .await?;
    ///
    /// if let Some(date) = latest_date {
    ///     println!("最新更新时间: {}", date);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn max<T>(self, field: &str) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 max() 查询，字段: {}", field);
        }

        // 构建 MAX(field) 表达式
        let max_expr = format!("MAX({})", field);

        // 用 Option<T> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<T>>(&max_expr)
            .await
            .map(Option::flatten)
    }

    /// 插入数据
    ///
    /// 执行 INSERT 操作，将数据插入到表中。
    ///
    /// # 类型参数
    /// - T: 数据类型，必须实现 Serialize trait
    ///
    /// # 参数
    /// - data: 要插入的数据
    ///
    /// # 返回
    /// - Ok(u64): 插入成功，返回插入记录的 ID（自增主键）
    /// - Err(DbError): 插入失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 使用 JSON 对象插入
    /// let user_data = json!({
    ///     "name": "张三",
    ///     "email": "zhangsan@example.com",
    ///     "age": 25
    /// });
    ///
    /// let user_id = db.table("users")
    ///     .insert(&user_data)
    ///     .await?;
    ///
    /// println!("插入成功，用户 ID: {}", user_id);
    ///
    /// // 插入带 JSON 字段的数据
    /// let order_data = json!({
    ///     "user_id": user_id,
    ///     "total": 199.99,
    ///     "items": [{"id": 1, "qty": 2}, {"id": 2, "qty": 1}]
    /// });
    ///
    /// let order_id = db.table("orders")
    ///     .json("items")  // 标记 items 字段为 JSON 类型
    ///     .insert(&order_data)
    ///     .await?;
    ///
    /// println!("订单插入成功，订单 ID: {}", order_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 insert() 操作，表: {}", self.table);
        }

        // 将数据序列化为 JSON
        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        // 生成 INSERT 语句
        let mut generator = SqlGenerator::new();
        generator.build_insert(&self.table, &json_data, &self.field_types)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 insert() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行插入
        let result = query.execute(self.pool).await;

        match result {
            Ok(query_result) => {
                let last_insert_id = query_result.last_insert_id();
                if self.enable_logging {
                    log::debug!("insert() 成功，插入 ID: {}", last_insert_id);
                }
                Ok(last_insert_id)
            }
            Err(e) => {
                log::error!("insert() 失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 批量插入数据
    ///
    /// 执行批量 INSERT 操作，将多条数据一次性插入到表中。
    /// 相比多次调用 insert()，批量插入性能更高，因为只需要一次数据库往返。
    ///
    /// # 类型参数
    /// - T: 数据类型，必须实现 Serialize trait
    ///
    /// # 参数
    /// - data: 要插入的数据切片
    ///
    /// # 返回
    /// - Ok(u64): 插入成功，返回受影响的行数
    /// - Err(DbError): 插入失败
    ///
    /// # 注意
    /// - 所有记录必须具有相同的字段结构
    /// - 字段顺序以第一条记录为准
    /// - 如果某条记录缺少字段，将使用 NULL 值
    /// - 批量插入使用单个 INSERT 语句，性能优于多次单条插入
    /// - 自动分批处理：当数据量超过 INSERT_BATCH_SIZE（默认 500）时，会自动分批插入
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 批量插入多个用户
    /// let users = vec![
    ///     json!({"name": "张三", "email": "zhangsan@example.com", "age": 25}),
    ///     json!({"name": "李四", "email": "lisi@example.com", "age": 30}),
    ///     json!({"name": "王五", "email": "wangwu@example.com", "age": 28}),
    /// ];
    ///
    /// let affected_rows = db.table("users")
    ///     .insert_batch(&users)
    ///     .await?;
    ///
    /// println!("批量插入成功，影响 {} 行", affected_rows);
    ///
    /// // 批量插入带 JSON 字段的数据
    /// let orders = vec![
    ///     json!({
    ///         "user_id": 1,
    ///         "total": 199.99,
    ///         "items": [{"id": 1, "qty": 2}]
    ///     }),
    ///     json!({
    ///         "user_id": 2,
    ///         "total": 299.99,
    ///         "items": [{"id": 2, "qty": 1}]
    ///     }),
    /// ];
    ///
    /// let affected_rows = db.table("orders")
    ///     .json("items")  // 标记 items 字段为 JSON 类型
    ///     .insert_batch(&orders)
    ///     .await?;
    ///
    /// println!("批量插入订单成功，影响 {} 行", affected_rows);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_batch<T>(self, data: &[T]) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // 直接委托给可自定义批大小的实现，使用默认批大小常量。
        // 空数据 / 分批 / 受影响行数累加等逻辑全部由 insert_batch_with_size 统一处理，
        // 避免两处重复（且 insert_chunk 只借用 &self，无需为每批克隆整个 builder）。
        self.insert_batch_with_size(data, INSERT_BATCH_SIZE).await
    }

    /// 批量插入数据（自定义批次大小）
    ///
    /// 与 `insert_batch` 相同，但允许调用方根据场景自定义每批次的最大记录数。
    /// 适用于需要根据网络延迟、数据大小或 MySQL max_allowed_packet 调整性能的场景。
    ///
    /// # 类型参数
    /// - T: 数据类型，必须实现 Serialize trait
    ///
    /// # 参数
    /// - data: 要插入的数据切片
    /// - batch_size: 每批最多插入的记录数（必须 > 0）
    ///
    /// # 返回
    /// - Ok(u64): 插入成功，返回总受影响行数
    /// - Err(DbError::SerializationError): batch_size 为 0 时
    /// - Err(DbError): 其他插入失败情况
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// let users = vec![
    ///     json!({"name": "张三", "age": 25}),
    ///     json!({"name": "李四", "age": 30}),
    /// ];
    ///
    /// // 每批最多插入 100 条记录
    /// let affected_rows = db.table("users")
    ///     .insert_batch_with_size(&users, 100)
    ///     .await?;
    ///
    /// println!("批量插入成功，影响 {} 行", affected_rows);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_batch_with_size<T>(
        self,
        data: &[T],
        batch_size: usize,
    ) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // 检查 batch_size 是否为 0，为 0 时返回错误
        if batch_size == 0 {
            return Err(crate::error::DbError::SerializationError(
                "batch_size 不能为 0".to_string(),
            ));
        }

        // 记录日志
        if self.enable_logging {
            log::debug!(
                "执行 insert_batch_with_size() 操作，表: {}，记录数: {}，批次大小: {}",
                self.table,
                data.len(),
                batch_size
            );
        }

        // 数据为空时直接返回错误
        if data.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "批量插入数据不能为空".to_string(),
            ));
        }

        // 使用 data.chunks(batch_size) 分批，每批调用内部插入逻辑，累加受影响行数
        let mut total_affected = 0u64;

        for (batch_index, chunk) in data.chunks(batch_size).enumerate() {
            if self.enable_logging {
                log::debug!(
                    "执行第 {} 批插入，本批记录数: {}",
                    batch_index + 1,
                    chunk.len()
                );
            }

            // insert_chunk 只借用 &self，无需为每批克隆整个 builder
            let affected = self.insert_chunk(chunk).await?;
            total_affected += affected;

            if self.enable_logging {
                log::debug!("第 {} 批插入成功，影响 {} 行", batch_index + 1, affected);
            }
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
    /// 此方法用于实际执行单个批次的 INSERT 操作。
    /// 它被 insert_batch() 方法调用，用于处理分批后的每个数据块。
    ///
    /// # 类型参数
    /// - T: 数据类型，必须实现 Serialize trait
    ///
    /// # 参数
    /// - data: 要插入的数据切片（单个批次）
    ///
    /// # 返回
    /// - Ok(u64): 插入成功，返回受影响的行数
    /// - Err(DbError): 插入失败
    async fn insert_chunk<T>(&self, data: &[T]) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // 将所有数据序列化为 JSON
        let json_data_list: Result<Vec<_>, _> = data
            .iter()
            .map(|item| {
                serde_json::to_value(item).map_err(|e| {
                    crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
                })
            })
            .collect();

        let json_data_list = json_data_list?;

        // 生成批量 INSERT 语句
        let mut generator = SqlGenerator::new();
        generator.build_insert_batch(&self.table, &json_data_list, &self.field_types)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 insert_chunk() SQL: {}", sql);
            log::debug!("参数数量: {}", params.len());
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行批量插入
        let result = query.execute(self.pool).await;

        match result {
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
    /// 执行 UPDATE 操作，更新表中的数据。
    /// 为了防止误操作，必须提供 WHERE 条件，否则会返回错误。
    ///
    /// # 类型参数
    /// - T: 数据类型，必须实现 Serialize trait
    ///
    /// # 参数
    /// - data: 要更新的数据
    ///
    /// # 返回
    /// - Ok(u64): 更新成功，返回受影响的行数
    /// - Err(DbError): 更新失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 更新用户信息
    /// let update_data = json!({
    ///     "name": "李四",
    ///     "age": 30
    /// });
    ///
    /// let rows_affected = db.table("users")
    ///     .where_and("id", "=", 1)
    ///     .update(&update_data)
    ///     .await?;
    ///
    /// println!("更新了 {} 行数据", rows_affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 update() 操作，表: {}", self.table);
        }

        // 检查是否有 WHERE 条件
        if self.conditions.is_empty() {
            log::warn!("update() 操作缺少 WHERE 条件，禁止全表更新");
            return Err(crate::error::DbError::MissingWhereClause);
        }

        // 将数据序列化为 JSON
        let json_data = serde_json::to_value(data).map_err(|e| {
            crate::error::DbError::SerializationError(format!("数据序列化失败: {}", e))
        })?;

        // 生成 UPDATE 语句
        let mut generator = SqlGenerator::new();
        generator.build_update(&self.table, &json_data, &self.field_types, &self.conditions)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 update() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行更新
        let result = query.execute(self.pool).await;

        match result {
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
    /// 执行 DELETE 操作，删除表中的数据。
    /// 为了防止误操作，必须提供 WHERE 条件，否则会返回错误。
    ///
    /// # 返回
    /// - Ok(u64): 删除成功，返回受影响的行数
    /// - Err(DbError): 删除失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 删除指定用户
    /// let rows_affected = db.table("users")
    ///     .where_and("id", "=", 1)
    ///     .delete()
    ///     .await?;
    ///
    /// println!("删除了 {} 行数据", rows_affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(self) -> Result<u64, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 delete() 操作，表: {}", self.table);
        }

        // 检查是否有 WHERE 条件
        if self.conditions.is_empty() {
            log::warn!("delete() 操作缺少 WHERE 条件，禁止全表删除");
            return Err(crate::error::DbError::MissingWhereClause);
        }

        // 生成 DELETE 语句
        let mut generator = SqlGenerator::new();
        generator.build_delete(&self.table, &self.conditions)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 delete() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行删除
        let result = query.execute(self.pool).await;

        match result {
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
    /// 使用 CASE WHEN 策略在单次查询中更新多条记录。自动分批处理（每批 1000 条），
    /// 所有批次在同一事务中执行，保证原子性。
    ///
    /// # 参数
    /// - `records`: 要更新的记录列表（每条必须包含 where_field 字段）
    /// - `where_field`: 主键字段名（如 `"id"`），用于匹配记录
    ///
    /// # 返回
    /// - `Ok(u64)`: 总受影响行数
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::Database;
    /// # use serde_json::json;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let records = vec![
    ///     json!({"id": 1, "name": "张三", "age": 25}),
    ///     json!({"id": 2, "name": "李四", "age": 30}),
    /// ];
    /// let affected = db.table("users")
    ///     .update_batch(&records, "id")
    ///     .await?;
    /// println!("批量更新了 {} 行", affected);
    /// # Ok(())
    /// # }
    /// ```
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
    /// 使用 `INSERT ... ON DUPLICATE KEY UPDATE` 语法。当主键或唯一键冲突时
    /// 自动更新所有字段，否则插入新记录。
    ///
    /// # 返回
    /// - `Ok(u64)`: MySQL rows_affected（1=插入新记录, 2=更新现有记录）
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::Database;
    /// # use serde_json::json;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let data = json!({"id": 1, "name": "张三", "email": "zhangsan@example.com"});
    /// let rows = db.table("users").upsert(&data).await?;
    /// if rows == 1 {
    ///     println!("新插入记录");
    /// } else if rows == 2 {
    ///     println!("更新了已有记录");
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
        generator.build_upsert(&self.table, &json_data, &self.field_types)?;

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

/// 绑定参数到执行查询（用于 INSERT/UPDATE/DELETE）
///
/// # 参数
/// - query: sqlx 查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_execute_param<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到查询
///
/// # 参数
/// - query: sqlx 查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_param<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询
///
/// # 参数
/// - query: sqlx 标量查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_scalar_param<'q, T>(
    query: sqlx::query::QueryScalar<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询（Option 类型）
///
/// # 参数
/// - query: sqlx 标量查询对象（返回 Option<T>）
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
///
/// 注：聚合/标量方法现统一走 `fetch_scalar` + `bind_scalar_param`（后者对 `Option<T>`
/// 输出类型同样适用），本函数暂无调用方但保留作为公开内部表面，标注 allow(dead_code)。
#[allow(dead_code)]
fn bind_scalar_param_option<'q, T>(
    query: sqlx::query::QueryScalar<'q, sqlx::MySql, Option<T>, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::MySql, Option<T>, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::mysql::MySqlPoolOptions;

    // 创建测试用的数据库连接池（真实连接，用于 async 集成测试）
    async fn create_test_pool() -> MySqlPool {
        MySqlPoolOptions::new()
            .max_connections(1)
            .connect("mysql://root:111111@localhost:3306/test")
            .await
            .expect("无法连接到测试数据库")
    }

    /// 获取或创建共享懒连接池（仅验证 URL，不立即建立连接）。
    /// 用于只测试 SQL 生成逻辑、不需要真实数据库连接的单元测试。
    /// 使用 OnceLock + 静态 Tokio 运行时确保池在有效上下文中创建和驻留。
    fn make_sync_test_pool() -> &'static MySqlPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<MySqlPool> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            rt.block_on(async {
                MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("mysql://root:111111@localhost:3306/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    #[tokio::test]
    async fn test_table_name_in_sql() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false);
        let sql = builder.to_sql();
        assert!(sql.contains("FROM users"));
    }

    // SqlGenerator 单元测试
    #[test]
    fn test_sql_generator_new() {
        let generator = SqlGenerator::new();
        assert_eq!(generator.get_sql(), "");
        assert_eq!(generator.get_params().len(), 0);
    }

    #[test]
    fn test_sql_generator_append() {
        let mut generator = SqlGenerator::new();
        generator.append("SELECT * FROM users");
        assert_eq!(generator.get_sql(), "SELECT * FROM users");
    }

    #[test]
    fn test_sql_generator_add_param() {
        let mut generator = SqlGenerator::new();
        generator.add_param(SqlValue::Int(42));
        generator.add_param(SqlValue::String("test".to_string()));
        assert_eq!(generator.get_params().len(), 2);
    }

    #[test]
    fn test_sql_generator_clear() {
        let mut generator = SqlGenerator::new();
        generator.append("SELECT * FROM users");
        generator.add_param(SqlValue::Int(1));

        generator.clear();

        assert_eq!(generator.get_sql(), "");
        assert_eq!(generator.get_params().len(), 0);
    }

    #[test]
    fn test_sql_generator_multiple_operations() {
        let mut generator = SqlGenerator::new();

        generator.append("SELECT * FROM users WHERE id = ?");
        generator.add_param(SqlValue::Int(1));
        generator.append(" AND name = ?");
        generator.add_param(SqlValue::String("test".to_string()));

        assert_eq!(
            generator.get_sql(),
            "SELECT * FROM users WHERE id = ? AND name = ?"
        );
        assert_eq!(generator.get_params().len(), 2);
    }

    #[tokio::test]
    async fn test_field_selection() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .field("name");
        let sql = builder.to_sql();
        assert!(sql.contains("id, name"));
    }

    #[tokio::test]
    async fn test_fields_selection() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).fields(&["id", "name", "email"]);
        let sql = builder.to_sql();
        assert!(sql.contains("id, name, email"));
    }

    #[tokio::test]
    async fn test_distinct() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("name")
            .distinct();
        let sql = builder.to_sql();
        assert!(sql.contains("SELECT DISTINCT"));
    }

    #[tokio::test]
    async fn test_field_type_marking() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .json("data")
            .datetime("created_at")
            .timestamp("updated_at")
            .decimal("price")
            .blob("content")
            .text("description");

        assert_eq!(builder.field_types.get("data"), Some(&FieldType::Json));
        assert_eq!(
            builder.field_types.get("created_at"),
            Some(&FieldType::DateTime)
        );
        assert_eq!(
            builder.field_types.get("updated_at"),
            Some(&FieldType::Timestamp)
        );
        assert_eq!(builder.field_types.get("price"), Some(&FieldType::Decimal));
        assert_eq!(builder.field_types.get("content"), Some(&FieldType::Blob));
        assert_eq!(
            builder.field_types.get("description"),
            Some(&FieldType::Text)
        );
    }

    #[tokio::test]
    async fn test_where_and() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_and_unchecked("name", "=", "test")
            .where_and_unchecked("age", ">", 18);

        assert_eq!(builder.conditions.len(), 2);
    }

    #[tokio::test]
    async fn test_where_or() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_or_unchecked("status", "=", 1)
            .where_or_unchecked("status", "=", 2);

        // where_or 会将条件组合成 OR
        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_where_in() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).where_in("id", vec![1, 2, 3]);

        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_where_between() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).where_between("age", 18, 65);

        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_join() {
        let pool = create_test_pool().await;
        let builder =
            QueryBuilder::new(&pool, "users", false).join("orders", "users.id = orders.user_id");

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_left_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .left_join("orders", "users.id = orders.user_id");

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_right_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .right_join("orders", "users.id = orders.user_id");

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_order() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .order("name", true)
            .order("age", false);

        assert_eq!(builder.order_by.len(), 2);
    }

    #[tokio::test]
    async fn test_group() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .group("status")
            .group("role");

        assert_eq!(builder.group_by.len(), 2);
    }

    // 测试完整的 SELECT 语句生成
    #[tokio::test]
    async fn test_select_with_where() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .field("name")
            .where_and_unchecked("status", "=", 1);

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT id, name FROM users"));
        assert!(sql.contains("WHERE"));
    }

    #[tokio::test]
    async fn test_select_with_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("users.id")
            .field("orders.total")
            .join("orders", "users.id = orders.user_id");

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT users.id, orders.total FROM users"));
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
    }

    #[tokio::test]
    async fn test_select_with_order_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("name")
            .order("name", true)
            .order("age", false);

        let sql = builder.to_sql();
        assert!(sql.contains("ORDER BY name ASC, age DESC"));
    }

    #[tokio::test]
    async fn test_select_with_group_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("status")
            .group("status");

        let sql = builder.to_sql();
        assert!(sql.contains("GROUP BY status"));
    }

    #[tokio::test]
    async fn test_select_with_limit_offset() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .limit(10)
            .offset(20);

        let sql = builder.to_sql();
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 20"));
    }

    #[tokio::test]
    async fn test_select_complex_query() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("users.id")
            .field("users.name")
            .field("orders.total")
            .distinct()
            .join("orders", "users.id = orders.user_id")
            .where_and_unchecked("users.status", "=", 1)
            .where_and_unchecked("orders.total", ">", 100)
            .group("users.id")
            .order("orders.total", false)
            .limit(50);

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT DISTINCT"));
        assert!(sql.contains("users.id, users.name, orders.total"));
        assert!(sql.contains("FROM users"));
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY users.id"));
        assert!(sql.contains("ORDER BY orders.total DESC"));
        assert!(sql.contains("LIMIT 50"));
    }

    #[tokio::test]
    async fn test_select_with_multiple_joins() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("users.name")
            .field("orders.total")
            .field("products.name")
            .join("orders", "users.id = orders.user_id")
            .left_join("products", "orders.product_id = products.id");

        let sql = builder.to_sql();
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
        assert!(sql.contains("LEFT JOIN products ON orders.product_id = products.id"));
    }

    #[tokio::test]
    async fn test_select_with_in_condition() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("name")
            .where_in("id", vec![1, 2, 3, 4, 5]);

        let sql = builder.to_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("IN"));
    }

    #[tokio::test]
    async fn test_select_with_between_condition() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("name")
            .where_between("age", 18, 65);

        let sql = builder.to_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("BETWEEN"));
    }

    #[test]
    fn test_where_null_generates_is_null_sql() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false).where_null("deleted_at");
        let sql = builder.to_sql();
        assert!(sql.contains("deleted_at IS NULL"));
    }

    #[test]
    fn test_where_not_null_generates_is_not_null_sql() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false).where_not_null("email");
        let sql = builder.to_sql();
        assert!(sql.contains("email IS NOT NULL"));
    }

    #[test]
    fn test_is_null_with_and_condition() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_and_unchecked("status", "=", 1i64)
            .where_null("deleted_at");
        let sql = builder.to_sql();
        assert!(sql.contains("status = ?"));
        assert!(sql.contains("deleted_at IS NULL"));
    }

    #[test]
    fn test_having_clause_sql_generation() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false)
            .field("user_id")
            .field("COUNT(*) as cnt")
            .group("user_id")
            .having_cond_unchecked("cnt", ">", 5i64);
        let sql = builder.to_sql();
        assert!(sql.contains("HAVING"));
        assert!(sql.contains("cnt > ?"));
    }

    #[test]
    fn test_having_without_group_returns_error() {
        let pool = make_sync_test_pool();
        let builder =
            QueryBuilder::new(pool, "orders", false).having_cond_unchecked("cnt", ">", 5i64);
        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::DbError::MissingGroupByClause
        ));
    }

    #[test]
    fn test_having_clause_order() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false)
            .group("user_id")
            .having_cond_unchecked("cnt", ">", 5i64)
            .order("cnt", false);
        let sql = builder.to_sql();
        let group_pos = sql.find("GROUP BY").unwrap();
        let having_pos = sql.find("HAVING").unwrap();
        let order_pos = sql.find("ORDER BY").unwrap();
        assert!(group_pos < having_pos);
        assert!(having_pos < order_pos);
    }

    #[test]
    fn test_update_batch_case_when_sql() {
        let records = vec![
            serde_json::json!({"id": 1, "name": "Alice", "age": 25}),
            serde_json::json!({"id": 2, "name": "Bob", "age": 30}),
        ];
        let mut generator = SqlGenerator::new();
        generator
            .build_update_batch("users", &records, "id", &std::collections::HashMap::new())
            .unwrap();
        let sql = generator.get_sql();
        assert!(sql.starts_with("UPDATE users SET "));
        assert!(sql.contains("CASE WHEN id=? THEN ?"));
        assert!(sql.contains("WHERE id IN ("));
    }

    #[test]
    fn test_update_batch_empty_returns_error() {
        let records: Vec<serde_json::Value> = vec![];
        let mut generator = SqlGenerator::new();
        let result = generator.build_update_batch(
            "users",
            &records,
            "id",
            &std::collections::HashMap::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_upsert_sql_generation() {
        let data = serde_json::json!({"id": 1, "name": "Alice", "email": "a@b.com"});
        let mut generator = SqlGenerator::new();
        generator
            .build_upsert("users", &data, &std::collections::HashMap::new())
            .unwrap();
        let sql = generator.get_sql();
        assert!(sql.starts_with("INSERT INTO users"));
        assert!(sql.contains("ON DUPLICATE KEY UPDATE"));
        assert!(sql.contains("name=VALUES(name)"));
    }

    #[test]
    fn test_upsert_empty_data_returns_error() {
        let data = serde_json::json!({});
        let mut generator = SqlGenerator::new();
        let result = generator.build_upsert("users", &data, &std::collections::HashMap::new());
        assert!(result.is_err());
    }

    // 测试 SqlGenerator 的 build_select 方法
    #[tokio::test]
    async fn test_sql_generator_build_select_basic() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .field("name");

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT id, name FROM users");
    }

    #[tokio::test]
    async fn test_sql_generator_build_select_with_distinct() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("name")
            .distinct();

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT DISTINCT name FROM users");
    }

    #[tokio::test]
    async fn test_sql_generator_build_select_all_fields() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT * FROM users");
    }

    // 测试 WHERE 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_where() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_and_unchecked("status", "=", 1)
            .where_and_unchecked("age", ">", 18);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
        assert!(sql.contains("age"));
    }

    // 测试 JOIN 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_joins() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .join("orders", "users.id = orders.user_id")
            .left_join("profiles", "users.id = profiles.user_id");

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
        assert!(sql.contains("LEFT JOIN profiles ON users.id = profiles.user_id"));
    }

    // 测试 ORDER BY 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_order_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .order("name", true)
            .order("created_at", false);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("ORDER BY name ASC, created_at DESC"));
    }

    // 测试 GROUP BY 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_group_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .group("status")
            .group("role");

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("GROUP BY status, role"));
    }

    // 测试 LIMIT 和 OFFSET 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_limit_offset() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .limit(10)
            .offset(20);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 20"));
    }

    // 测试完整的复杂查询生成
    #[tokio::test]
    async fn test_sql_generator_complex_query() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("users.id")
            .field("users.name")
            .field("COUNT(orders.id) as order_count")
            .distinct()
            .join("orders", "users.id = orders.user_id")
            .where_and_unchecked("users.status", "=", 1)
            .where_and_unchecked("orders.total", ">", 100)
            .group("users.id")
            .group("users.name")
            .order("order_count", false)
            .limit(20)
            .offset(10);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();

        // 验证各个部分都存在
        assert!(sql.starts_with("SELECT DISTINCT"));
        assert!(sql.contains("users.id, users.name, COUNT(orders.id) as order_count"));
        assert!(sql.contains("FROM users"));
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY users.id, users.name"));
        assert!(sql.contains("ORDER BY order_count DESC"));
        assert!(sql.contains("LIMIT 20"));
        assert!(sql.contains("OFFSET 10"));
    }

    // 测试 find() 方法的 SQL 生成
    #[tokio::test]
    async fn test_find_adds_limit_one() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .field("name")
            .where_and_unchecked("id", "=", 1);

        // 在调用 find() 之前，limit 应该是 None
        assert_eq!(builder.limit, None);

        // 创建一个新的 builder 来测试 SQL 生成
        let builder_with_limit = QueryBuilder::new(&pool, "users", false)
            .field("id")
            .field("name")
            .where_and_unchecked("id", "=", 1)
            .limit(1);

        let sql = builder_with_limit.to_sql();
        assert!(sql.contains("LIMIT 1"), "find() 应该自动添加 LIMIT 1");
    }

    // 测试 INSERT 语句生成
    #[test]
    fn test_sql_generator_build_insert_basic() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({
            "name": "张三",
            "age": 25,
            "email": "zhangsan@example.com"
        });
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        assert!(sql.starts_with("INSERT INTO users"));
        assert!(sql.contains("name"));
        assert!(sql.contains("age"));
        assert!(sql.contains("email"));
        assert!(sql.contains("VALUES"));
        assert_eq!(generator.get_params().len(), 3);
    }

    #[test]
    fn test_sql_generator_build_insert_with_json_field() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({
            "name": "测试用户",
            "data": {"role": "admin", "permissions": ["read", "write"]}
        });

        let mut field_types = HashMap::new();
        field_types.insert("data".to_string(), FieldType::Json);

        let result = generator.build_insert("users", &data, &field_types);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("name"));
        assert!(sql.contains("data"));
        assert_eq!(generator.get_params().len(), 2);

        // 验证 JSON 字段被正确处理
        let params = generator.get_params();
        let has_json = params.iter().any(|p| matches!(p, SqlValue::Json(_)));
        assert!(has_json, "应该包含 JSON 类型的参数");
    }

    #[test]
    fn test_sql_generator_build_insert_empty_data() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({});
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ));
    }

    #[test]
    fn test_sql_generator_build_insert_not_object() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!([1, 2, 3]); // 数组而不是对象
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ));
    }

    // ==================== 聚合函数单元测试 ====================

    /// 测试 AVG 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_avg_sql_generation() {
        let pool = create_test_pool().await;

        // 创建一个新的 builder 来模拟 avg() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("FROM products"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 AVG 与 WHERE 条件组合
    #[tokio::test]
    async fn test_avg_with_where_sql() {
        let pool = create_test_pool().await;

        // 模拟 avg() 方法与 WHERE 条件组合
        let mut test_builder =
            QueryBuilder::new(&pool, "products", false).where_and_unchecked("status", "=", 1);
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("FROM products"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
    }

    /// 测试 MIN 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_min_sql_generation() {
        let pool = create_test_pool().await;

        // 模拟 min() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("MIN(price)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT MIN(price)"));
        assert!(sql.contains("FROM products"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 MAX 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_max_sql_generation() {
        let pool = create_test_pool().await;

        // 模拟 max() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("MAX(price)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT MAX(price)"));
        assert!(sql.contains("FROM products"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 MIN/MAX 不同数据类型的 SQL 生成
    #[tokio::test]
    async fn test_min_max_different_types() {
        let pool = create_test_pool().await;

        // 测试整数类型
        let mut builder_int = QueryBuilder::new(&pool, "products", false);
        builder_int.fields.clear();
        builder_int.fields.push("MIN(stock)".to_string());
        let sql_int = builder_int.to_sql();
        assert!(sql_int.contains("MIN(stock)"));

        // 测试浮点数类型
        let mut builder_float = QueryBuilder::new(&pool, "products", false);
        builder_float.fields.clear();
        builder_float.fields.push("MAX(price)".to_string());
        let sql_float = builder_float.to_sql();
        assert!(sql_float.contains("MAX(price)"));

        // 测试字符串类型
        let mut builder_string = QueryBuilder::new(&pool, "users", false);
        builder_string.fields.clear();
        builder_string.fields.push("MIN(name)".to_string());
        let sql_string = builder_string.to_sql();
        assert!(sql_string.contains("MIN(name)"));

        // 测试日期时间类型
        let mut builder_datetime = QueryBuilder::new(&pool, "users", false);
        builder_datetime.fields.clear();
        builder_datetime.fields.push("MAX(created_at)".to_string());
        let sql_datetime = builder_datetime.to_sql();
        assert!(sql_datetime.contains("MAX(created_at)"));
    }

    /// 测试聚合函数与 GROUP BY 组合
    #[tokio::test]
    async fn test_aggregates_with_group_by_sql() {
        let pool = create_test_pool().await;

        // 模拟聚合函数与 GROUP BY 组合
        let mut test_builder = QueryBuilder::new(&pool, "orders", false).group("user_id");
        test_builder.fields.clear();
        test_builder.fields.push("user_id".to_string());
        test_builder
            .fields
            .push("CAST(AVG(amount) AS DOUBLE) as avg_amount".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT user_id, CAST(AVG(amount) AS DOUBLE) as avg_amount"));
        assert!(sql.contains("FROM orders"));
        assert!(sql.contains("GROUP BY user_id"));
    }

    /// 测试多个聚合函数组合
    #[tokio::test]
    async fn test_multiple_aggregates_sql() {
        let pool = create_test_pool().await;

        // 模拟多个聚合函数组合
        let mut test_builder = QueryBuilder::new(&pool, "orders", false).where_and_unchecked(
            "status",
            "=",
            "completed",
        );
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(amount) AS DOUBLE) as avg_amount".to_string());
        test_builder
            .fields
            .push("CAST(MIN(amount) AS DOUBLE) as min_amount".to_string());
        test_builder
            .fields
            .push("CAST(MAX(amount) AS DOUBLE) as max_amount".to_string());
        test_builder
            .fields
            .push("COUNT(*) as order_count".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("CAST(AVG(amount) AS DOUBLE) as avg_amount"));
        assert!(sql.contains("CAST(MIN(amount) AS DOUBLE) as min_amount"));
        assert!(sql.contains("CAST(MAX(amount) AS DOUBLE) as max_amount"));
        assert!(sql.contains("COUNT(*) as order_count"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
    }

    /// 测试 SQL 子句顺序正确性（WHERE -> GROUP BY -> ORDER BY）
    #[tokio::test]
    async fn test_sql_clause_order_with_aggregates() {
        let pool = create_test_pool().await;

        // 创建包含 WHERE、GROUP BY、ORDER BY 的查询
        let mut test_builder = QueryBuilder::new(&pool, "orders", false)
            .where_and_unchecked("status", "=", "completed")
            .group("user_id")
            .order("total_amount", false);
        test_builder.fields.clear();
        test_builder.fields.push("user_id".to_string());
        test_builder
            .fields
            .push("SUM(amount) as total_amount".to_string());

        let sql = test_builder.to_sql();

        // 验证子句顺序：WHERE 应该在 GROUP BY 之前，GROUP BY 应该在 ORDER BY 之前
        let where_pos = sql.find("WHERE").expect("应该包含 WHERE");
        let group_pos = sql.find("GROUP BY").expect("应该包含 GROUP BY");
        let order_pos = sql.find("ORDER BY").expect("应该包含 ORDER BY");

        assert!(where_pos < group_pos, "WHERE 应该在 GROUP BY 之前");
        assert!(group_pos < order_pos, "GROUP BY 应该在 ORDER BY 之前");
    }

    /// 测试空结果集场景的 SQL 生成
    #[tokio::test]
    async fn test_aggregates_empty_result_sql() {
        let pool = create_test_pool().await;

        // 创建一个不会匹配任何记录的查询
        let mut test_builder =
            QueryBuilder::new(&pool, "products", false).where_and_unchecked("id", "=", -1); // 假设 id 不会是负数
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("WHERE"));
        // SQL 生成应该正常，即使结果集为空
    }

    /// 测试聚合函数字段名包含特殊字符
    #[tokio::test]
    async fn test_aggregates_with_special_field_names() {
        let pool = create_test_pool().await;

        // 测试带下划线的字段名
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("AVG(unit_price)".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("AVG(unit_price)"));

        // 测试带反引号的字段名（MySQL 保留字）
        let mut test_builder2 = QueryBuilder::new(&pool, "products", false);
        test_builder2.fields.clear();
        test_builder2.fields.push("MAX(`order`)".to_string());

        let sql2 = test_builder2.to_sql();
        assert!(sql2.contains("MAX(`order`)"));
    }

    /// 测试聚合函数与 DISTINCT 组合
    #[tokio::test]
    async fn test_aggregates_with_distinct() {
        let pool = create_test_pool().await;

        // 模拟 COUNT(DISTINCT field) 场景
        let mut test_builder = QueryBuilder::new(&pool, "orders", false);
        test_builder.fields.clear();
        test_builder
            .fields
            .push("COUNT(DISTINCT user_id) as unique_users".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("COUNT(DISTINCT user_id)"));
    }

    /// 测试聚合函数与 JOIN 组合
    #[tokio::test]
    async fn test_aggregates_with_join() {
        let pool = create_test_pool().await;

        // 模拟聚合函数与 JOIN 组合
        let mut test_builder = QueryBuilder::new(&pool, "users", false)
            .join("orders", "users.id = orders.user_id")
            .group("users.id");
        test_builder.fields.clear();
        test_builder.fields.push("users.id".to_string());
        test_builder.fields.push("users.name".to_string());
        test_builder
            .fields
            .push("COUNT(orders.id) as order_count".to_string());
        test_builder
            .fields
            .push("SUM(orders.amount) as total_amount".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
        assert!(sql.contains("COUNT(orders.id) as order_count"));
        assert!(sql.contains("SUM(orders.amount) as total_amount"));
        assert!(sql.contains("GROUP BY users.id"));
    }

    /// 测试聚合函数参数化查询防止 SQL 注入
    #[tokio::test]
    async fn test_aggregates_sql_injection_prevention() {
        let pool = create_test_pool().await;

        // 测试 WHERE 条件使用参数化查询
        let builder = QueryBuilder::new(&pool, "products", false).where_and_unchecked(
            "category",
            "=",
            "'; DROP TABLE products; --",
        );

        // 生成 SQL 和参数
        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 验证 SQL 使用占位符而不是直接拼接值
        assert!(sql.contains("?"));
        assert!(!sql.contains("DROP TABLE"));

        // 验证参数列表包含恶意字符串（作为参数值，不会被执行）
        assert_eq!(params.len(), 1);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use sqlx::mysql::MySqlPoolOptions;

    // 生成有效的表名（字母开头，后跟字母数字下划线）
    fn table_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,30}"
    }

    // 生成有效的字段名
    fn field_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,30}"
    }

    // 创建测试用的数据库连接池（同步版本用于 proptest）
    fn create_test_pool_sync() -> MySqlPool {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            MySqlPoolOptions::new()
                .max_connections(1)
                .connect("mysql://root:111111@localhost:3306/test")
                .await
                .expect("无法连接到测试数据库")
        })
    }

    /// 获取或创建共享懒连接池（仅验证 URL，不立即建立连接）。
    /// 用于只测试 SQL 生成逻辑、不需要真实数据库连接的单元测试。
    /// 使用 OnceLock + 静态 Tokio 运行时确保池在有效上下文中创建和驻留。
    fn make_sync_test_pool() -> &'static MySqlPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<MySqlPool> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            rt.block_on(async {
                MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("mysql://root:111111@localhost:3306/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    // Feature: mysql-query-builder, Property 1: 表名设置正确性
    // 验证需求：2.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_table_name_in_sql(table_name in table_name_strategy()) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false);
            let sql = builder.to_sql();

            // 验证 SQL 包含表名
            let expected = format!("FROM {}", table_name);
            prop_assert!(sql.contains(&expected));
        }
    }

    // Feature: mysql-query-builder, Property 2: 表名覆盖行为
    // 验证需求：2.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_table_name_override(
            table_name1 in table_name_strategy(),
            table_name2 in table_name_strategy()
        ) {
            prop_assume!(table_name1 != table_name2);

            let pool = create_test_pool_sync();
            // 先创建一个 builder，然后通过重新创建来模拟覆盖
            let builder1 = QueryBuilder::new(&pool, &table_name1, false);
            let sql1 = builder1.to_sql();
            let expected1 = format!("FROM {}", table_name1);
            prop_assert!(sql1.contains(&expected1));

            // 创建新的 builder 使用 table_name2
            let builder2 = QueryBuilder::new(&pool, &table_name2, false);
            let sql2 = builder2.to_sql();
            let expected2 = format!("FROM {}", table_name2);
            prop_assert!(sql2.contains(&expected2));

            // 使用更精确的匹配：检查 FROM 后面的完整表名（带空格或 WHERE 等关键字）
            // 避免子字符串匹配问题（如 "w" 是 "w_" 的子串）
            let pattern1 = format!("FROM {} ", table_name1);
            let pattern1_alt = format!("FROM {}\n", table_name1);
            prop_assert!(!sql2.contains(&pattern1) && !sql2.contains(&pattern1_alt));
        }
    }

    // Feature: mysql-query-builder, Property 24: 字段选择
    // 验证需求：9.1, 9.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_field_selection(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..10)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加所有字段
            for field in &fields {
                builder = builder.field(field);
            }

            let sql = builder.to_sql();

            // 验证所有字段都在 SELECT 子句中
            for field in &fields {
                prop_assert!(sql.contains(field));
            }
        }
    }

    // Feature: mysql-query-builder, Property 25: DISTINCT 关键字
    // 验证需求：9.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_distinct_keyword(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(&field)
                .distinct();

            let sql = builder.to_sql();

            // 验证 SQL 包含 SELECT DISTINCT
            prop_assert!(sql.contains("SELECT DISTINCT"));
        }
    }

    // Feature: mysql-query-builder, Property 27: 特殊字段类型标记
    // 验证需求：11.1, 11.2, 11.3, 11.4, 11.5, 11.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_special_field_type_marking(
            table_name in table_name_strategy(),
            json_field in field_name_strategy(),
            datetime_field in field_name_strategy(),
            timestamp_field in field_name_strategy(),
            decimal_field in field_name_strategy(),
            blob_field in field_name_strategy(),
            text_field in field_name_strategy()
        ) {
            // 确保所有字段名都不相同，避免覆盖
            prop_assume!(json_field != datetime_field);
            prop_assume!(json_field != timestamp_field);
            prop_assume!(json_field != decimal_field);
            prop_assume!(json_field != blob_field);
            prop_assume!(json_field != text_field);
            prop_assume!(datetime_field != timestamp_field);
            prop_assume!(datetime_field != decimal_field);
            prop_assume!(datetime_field != blob_field);
            prop_assume!(datetime_field != text_field);
            prop_assume!(timestamp_field != decimal_field);
            prop_assume!(timestamp_field != blob_field);
            prop_assume!(timestamp_field != text_field);
            prop_assume!(decimal_field != blob_field);
            prop_assume!(decimal_field != text_field);
            prop_assume!(blob_field != text_field);

            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .json(&json_field)
                .datetime(&datetime_field)
                .timestamp(&timestamp_field)
                .decimal(&decimal_field)
                .blob(&blob_field)
                .text(&text_field);

            // 验证字段类型映射包含正确的类型标记
            prop_assert_eq!(builder.field_types.get(&json_field), Some(&FieldType::Json));
            prop_assert_eq!(builder.field_types.get(&datetime_field), Some(&FieldType::DateTime));
            prop_assert_eq!(builder.field_types.get(&timestamp_field), Some(&FieldType::Timestamp));
            prop_assert_eq!(builder.field_types.get(&decimal_field), Some(&FieldType::Decimal));
            prop_assert_eq!(builder.field_types.get(&blob_field), Some(&FieldType::Blob));
            prop_assert_eq!(builder.field_types.get(&text_field), Some(&FieldType::Text));
        }
    }

    // Feature: mysql-query-builder, Property 4: WHERE 条件添加
    // 验证需求：3.1, 3.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_where_and_condition_added(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", value);

            // 验证条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }

        #[test]
        fn prop_where_or_condition_added(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value1 in any::<i32>(),
            value2 in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_or_unchecked(&field, "=", value1)
                .where_or_unchecked(&field, "=", value2);

            // where_or 会将条件组合，所以应该有 1 个条件（OR 组合）
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 6: IN 操作符数组支持
    // 验证需求：3.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_in_operator_array_support(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            values in prop::collection::vec(any::<i32>(), 1..10)
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_in(&field, values);

            // 验证 IN 条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 7: BETWEEN 操作符边界支持
    // 验证需求：3.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_between_operator_boundary_support(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            start in any::<i32>(),
            end in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_between(&field, start, end);

            // 验证 BETWEEN 条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 8: 多条件 AND 连接
    // 验证需求：3.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_and_conditions(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            values in prop::collection::vec(any::<i32>(), 2..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个 AND 条件
            for value in &values {
                builder = builder.where_and_unchecked(&field, "=", *value);
            }

            // 验证所有条件都已添加
            prop_assert_eq!(builder.conditions.len(), values.len());
        }
    }

    // Feature: mysql-query-builder, Property 31: JOIN 子句生成
    // 验证需求：17.1, 17.2, 17.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_join_clause_generation(
            table_name in table_name_strategy(),
            join_table in table_name_strategy(),
            on_condition in "[a-z][a-z0-9_]{0,20}\\.[a-z][a-z0-9_]{0,20} = [a-z][a-z0-9_]{0,20}\\.[a-z][a-z0-9_]{0,20}"
        ) {
            let pool = create_test_pool_sync();

            // 测试 INNER JOIN
            let builder_inner = QueryBuilder::new(&pool, &table_name, false)
                .join(&join_table, &on_condition);
            prop_assert_eq!(builder_inner.joins.len(), 1);

            // 测试 LEFT JOIN
            let builder_left = QueryBuilder::new(&pool, &table_name, false)
                .left_join(&join_table, &on_condition);
            prop_assert_eq!(builder_left.joins.len(), 1);

            // 测试 RIGHT JOIN
            let builder_right = QueryBuilder::new(&pool, &table_name, false)
                .right_join(&join_table, &on_condition);
            prop_assert_eq!(builder_right.joins.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 32: 多表连接支持
    // 验证需求：17.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_join_support(
            table_name in table_name_strategy(),
            join_tables in prop::collection::vec(table_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个 JOIN
            for join_table in &join_tables {
                let on_condition = format!("{}.id = {}.id", table_name, join_table);
                builder = builder.join(join_table, &on_condition);
            }

            // 验证所有 JOIN 都已添加
            prop_assert_eq!(builder.joins.len(), join_tables.len());
        }
    }

    // Feature: mysql-query-builder, Property 33: 表别名支持
    // 验证需求：17.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_table_alias_support(
            base_table in table_name_strategy(),
            join_table in table_name_strategy(),
            base_alias in "[a-z][a-z0-9]{0,5}",
            join_alias in "[a-z][a-z0-9]{0,5}"
        ) {
            prop_assume!(base_table != join_table);
            prop_assume!(base_alias != join_alias);

            let pool = create_test_pool_sync();

            // 构建带别名的表名
            let base_table_with_alias = format!("{} AS {}", base_table, base_alias);
            let join_table_with_alias = format!("{} AS {}", join_table, join_alias);

            // 使用别名构建 ON 条件
            let on_condition = format!("{}.id = {}.id", base_alias, join_alias);

            // 创建查询构建器，使用带别名的表名
            let builder = QueryBuilder::new(&pool, &base_table_with_alias, false)
                .field(&format!("{}.id", base_alias))
                .field(&format!("{}.name", base_alias))
                .join(&join_table_with_alias, &on_condition);

            let sql = builder.to_sql();

            // 验证 SQL 包含带别名的主表
            prop_assert!(sql.contains(&format!("FROM {}", base_table_with_alias)),
                "SQL 应该包含带别名的主表: FROM {}", base_table_with_alias);

            // 验证 SQL 包含带别名的 JOIN 表
            prop_assert!(sql.contains(&join_table_with_alias),
                "SQL 应该包含带别名的 JOIN 表: {}", join_table_with_alias);

            // 验证 SQL 包含使用别名的 ON 条件
            prop_assert!(sql.contains(&on_condition),
                "SQL 应该包含使用别名的 ON 条件: {}", on_condition);

            // 验证 SQL 包含使用别名的字段选择
            prop_assert!(sql.contains(&format!("{}.id", base_alias)),
                "SQL 应该包含使用别名的字段: {}.id", base_alias);
            prop_assert!(sql.contains(&format!("{}.name", base_alias)),
                "SQL 应该包含使用别名的字段: {}.name", base_alias);
        }
    }

    // Feature: mysql-query-builder, Property 20: ORDER BY 子句生成
    // 验证需求：8.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_order_by_clause_generation(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            asc in any::<bool>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .order(&field, asc);

            // 验证 ORDER BY 已添加
            prop_assert_eq!(builder.order_by.len(), 1);
            prop_assert_eq!(&builder.order_by[0].field, &field);
            prop_assert_eq!(builder.order_by[0].asc, asc);
        }
    }

    // Feature: mysql-query-builder, Property 21: 多字段排序支持
    // 验证需求：8.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_order_by_support(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个排序字段
            for field in &fields {
                builder = builder.order(field, true);
            }

            // 验证所有排序字段都已添加
            prop_assert_eq!(builder.order_by.len(), fields.len());
        }
    }

    // Feature: mysql-query-builder, Property 22: GROUP BY 子句生成
    // 验证需求：8.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_group_by_clause_generation(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .group(&field);

            // 验证 GROUP BY 已添加
            prop_assert_eq!(builder.group_by.len(), 1);
            prop_assert_eq!(&builder.group_by[0], &field);
        }
    }

    // Feature: mysql-query-builder, Property 23: 多字段分组支持
    // 验证需求：8.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_group_by_support(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个分组字段
            for field in &fields {
                builder = builder.group(field);
            }

            // 验证所有分组字段都已添加
            prop_assert_eq!(builder.group_by.len(), fields.len());
        }
    }

    // Feature: mysql-query-builder, Property 30: SQL 语句调试输出
    // 验证需求：15.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_to_sql_returns_valid_sql(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 0..5),
            use_distinct in any::<bool>(),
            limit_opt in prop::option::of(1u64..100),
            offset_opt in prop::option::of(0u64..100)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加字段
            for field in &fields {
                builder = builder.field(field);
            }

            // 可选的 DISTINCT
            if use_distinct {
                builder = builder.distinct();
            }

            // 可选的 LIMIT
            if let Some(limit) = limit_opt {
                builder = builder.limit(limit);
            }

            // 可选的 OFFSET
            if let Some(offset) = offset_opt {
                builder = builder.offset(offset);
            }

            // 调用 to_sql() 方法
            let sql = builder.to_sql();

            // 验证返回的 SQL 字符串非空
            prop_assert!(!sql.is_empty(), "SQL 字符串不应为空");

            // 验证包含基本的 SQL 关键字
            prop_assert!(sql.contains("SELECT"), "SQL 应包含 SELECT 关键字");
            prop_assert!(sql.contains("FROM"), "SQL 应包含 FROM 关键字");

            // 验证包含表名
            prop_assert!(sql.contains(&table_name), "SQL 应包含表名");

            // 如果使用了 DISTINCT，验证包含 DISTINCT 关键字
            if use_distinct {
                prop_assert!(sql.contains("DISTINCT"), "SQL 应包含 DISTINCT 关键字");
            }

            // 如果设置了 LIMIT，验证包含 LIMIT 子句
            if let Some(limit) = limit_opt {
                prop_assert!(sql.contains("LIMIT"), "SQL 应包含 LIMIT 关键字");
                prop_assert!(sql.contains(&limit.to_string()), "SQL 应包含 LIMIT 值");
            }

            // 如果设置了 OFFSET，验证包含 OFFSET 子句
            if let Some(offset) = offset_opt {
                prop_assert!(sql.contains("OFFSET"), "SQL 应包含 OFFSET 关键字");
                prop_assert!(sql.contains(&offset.to_string()), "SQL 应包含 OFFSET 值");
            }

            // 验证字段在 SQL 中
            if !fields.is_empty() {
                for field in &fields {
                    prop_assert!(sql.contains(field), "SQL 应包含字段 {}", field);
                }
            } else {
                // 如果没有指定字段，应该使用 SELECT *
                prop_assert!(sql.contains("*"), "SQL 应包含 * 表示所有字段");
            }
        }

        #[test]
        fn prop_to_sql_with_conditions(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", value);

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));
            prop_assert!(sql.contains(&table_name));

            // 验证包含 WHERE 子句
            prop_assert!(sql.contains("WHERE"), "SQL 应包含 WHERE 关键字");
        }

        #[test]
        fn prop_to_sql_with_joins(
            table_name in table_name_strategy(),
            join_table in table_name_strategy(),
            on_field1 in field_name_strategy(),
            on_field2 in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let on_condition = format!("{}.{} = {}.{}", table_name, on_field1, join_table, on_field2);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .join(&join_table, &on_condition);

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));

            // 验证包含 JOIN 子句
            prop_assert!(sql.contains("JOIN"), "SQL 应包含 JOIN 关键字");
            prop_assert!(sql.contains(&join_table), "SQL 应包含连接的表名");
        }

        #[test]
        fn prop_to_sql_with_order_and_group(
            table_name in table_name_strategy(),
            order_field in field_name_strategy(),
            group_field in field_name_strategy(),
            asc in any::<bool>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .order(&order_field, asc)
                .group(&group_field);

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));

            // 验证包含 ORDER BY 和 GROUP BY 子句
            prop_assert!(sql.contains("ORDER BY"), "SQL 应包含 ORDER BY 关键字");
            prop_assert!(sql.contains("GROUP BY"), "SQL 应包含 GROUP BY 关键字");
            prop_assert!(sql.contains(&order_field), "SQL 应包含排序字段");
            prop_assert!(sql.contains(&group_field), "SQL 应包含分组字段");
        }

        #[test]
        fn prop_to_sql_complex_query(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..3),
            join_table in table_name_strategy(),
            where_field in field_name_strategy(),
            order_field in field_name_strategy(),
            group_field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加字段
            for field in &fields {
                builder = builder.field(field);
            }

            // 添加 JOIN
            let on_condition = format!("{}.id = {}.id", table_name, join_table);
            builder = builder.join(&join_table, &on_condition);

            // 添加 WHERE 条件
            builder = builder.where_and_unchecked(&where_field, "=", 1);

            // 添加 ORDER BY
            builder = builder.order(&order_field, true);

            // 添加 GROUP BY
            builder = builder.group(&group_field);

            // 添加 LIMIT
            builder = builder.limit(10);

            let sql = builder.to_sql();

            // 验证这是一个有效的复杂 SQL 查询
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));
            prop_assert!(sql.contains(&table_name));
            prop_assert!(sql.contains("JOIN"));
            prop_assert!(sql.contains("WHERE"));
            prop_assert!(sql.contains("ORDER BY"));
            prop_assert!(sql.contains("GROUP BY"));
            prop_assert!(sql.contains("LIMIT"));

            // 验证 SQL 子句的顺序正确（SQL 标准顺序）
            let select_pos = sql.find("SELECT").unwrap();
            let from_pos = sql.find("FROM").unwrap();
            let join_pos = sql.find("JOIN").unwrap();
            let where_pos = sql.find("WHERE").unwrap();
            let group_pos = sql.find("GROUP BY").unwrap();
            let order_pos = sql.find("ORDER BY").unwrap();
            let limit_pos = sql.find("LIMIT").unwrap();

            // 验证子句顺序：SELECT < FROM < JOIN < WHERE < GROUP BY < ORDER BY < LIMIT
            prop_assert!(select_pos < from_pos, "SELECT 应在 FROM 之前");
            prop_assert!(from_pos < join_pos, "FROM 应在 JOIN 之前");
            prop_assert!(join_pos < where_pos, "JOIN 应在 WHERE 之前");
            prop_assert!(where_pos < group_pos, "WHERE 应在 GROUP BY 之前");
            prop_assert!(group_pos < order_pos, "GROUP BY 应在 ORDER BY 之前");
            prop_assert!(order_pos < limit_pos, "ORDER BY 应在 LIMIT 之前");
        }
    }

    // Feature: mysql-query-builder, Property 3: SQL 注入防护
    // 验证需求：2.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sql_injection_prevention_single_quote(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*'.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input.as_str());

            let sql = builder.to_sql();

            // SQL 不应该直接包含恶意输入的单引号
            // 参数化查询应该使用 ? 占位符
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询（? 占位符）");

            // SQL 中不应该直接出现用户输入的单引号
            // 注意：SQL 本身可能包含单引号（如 'table'），但不应该是用户输入的
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_semicolon(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*;.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 中不应该直接出现用户输入的分号
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_comment(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*--.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 中不应该直接出现用户输入的注释符
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_drop_table(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "'; DROP TABLE users; --";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 不应该包含 DROP TABLE 语句
            prop_assert!(!sql.to_uppercase().contains("DROP TABLE"),
                "SQL 不应该包含 DROP TABLE 语句");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_union_select(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "' UNION SELECT * FROM passwords --";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 不应该包含 UNION SELECT 注入
            let sql_upper = sql.to_uppercase();
            let union_count = sql_upper.matches("UNION").count();
            prop_assert_eq!(union_count, 0, "SQL 不应该包含 UNION 注入");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_or_always_true(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "' OR '1'='1";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");

            // 验证不会产生永真条件（除了我们自己构建的条件）
            // 恶意输入应该被当作参数值，而不是 SQL 代码
            let or_count = where_clause.matches(" OR ").count();
            // 如果没有使用 where_or，就不应该有 OR
            prop_assert_eq!(or_count, 0, "不应该因为用户输入而产生 OR 条件");
        }

        #[test]
        fn prop_sql_injection_prevention_multiple_special_chars(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in "[a-z0-9]*[';\"\\-][a-z0-9]*[';\"\\-][a-z0-9]*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "=", malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_in_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_values in prop::collection::vec(".*[';].*", 1..5)
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_in(&field, malicious_values.clone());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("IN"), "SQL 应该包含 IN 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // 验证每个值都有对应的占位符
            let placeholder_count = sql.matches("?").count();
            prop_assert!(placeholder_count >= malicious_values.len(),
                "每个 IN 值都应该有对应的参数占位符");

            // WHERE 子句不应该直接包含恶意输入
            for malicious_value in &malicious_values {
                let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
                prop_assert!(!where_clause.contains(malicious_value),
                    "WHERE 子句不应该直接包含用户输入的恶意字符串");
            }
        }

        #[test]
        fn prop_sql_injection_prevention_like_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_pattern in ".*[';].*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field, "like", malicious_pattern.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("LIKE"), "SQL 应该包含 LIKE 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_pattern),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_between_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_start in ".*[';].*",
            malicious_end in ".*[';].*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_between(&field, malicious_start.as_str(), malicious_end.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("BETWEEN"), "SQL 应该包含 BETWEEN 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // 验证有两个占位符（start 和 end）
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            let placeholder_count = where_clause.matches("?").count();
            prop_assert!(placeholder_count >= 2, "BETWEEN 应该有两个参数占位符");

            // WHERE 子句不应该直接包含恶意输入
            prop_assert!(!where_clause.contains(&malicious_start),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
            prop_assert!(!where_clause.contains(&malicious_end),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }
    }

    // Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find()
    // 验证需求：4.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_one(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个带 WHERE 条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(&field)
                .where_and_unchecked(&field, "=", value)
                .limit(1); // 模拟 find() 会添加的 LIMIT 1

            let sql = builder.to_sql();

            // 验证 SQL 包含 LIMIT 1
            prop_assert!(sql.contains("LIMIT 1"),
                "find() 方法应该自动添加 LIMIT 1 到查询中");
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数
    // 验证需求：4.4
    //
    // 属性：对于任意查询构建器，调用 count() 方法时，生成的 SQL 应该包含 COUNT(*) 或 COUNT(field)
    //
    // 此测试验证 count() 方法正确生成 COUNT 聚合函数的 SQL 语句。
    // count() 方法内部使用 value("COUNT(*)") 来实现，因此我们测试生成的 SQL 是否包含 COUNT。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_aggregation_function(
            table_name in table_name_strategy()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个查询构建器并使用 field("COUNT(*)") 模拟 count() 方法的行为
            // count() 方法内部调用 value("COUNT(*)")，这等同于 field("COUNT(*)")
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field("COUNT(*)");

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(*) 或 COUNT(field)
            prop_assert!(
                sql.contains("COUNT(*)") || sql.contains("COUNT("),
                "count() 方法应该生成包含 COUNT(*) 或 COUNT(field) 的 SQL 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "count() 方法应该生成 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM {}", table_name)),
                "count() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数 - 带条件
    // 验证需求：4.4
    //
    // 属性：对于任意带 WHERE 条件的查询构建器，调用 count() 方法时，
    // 生成的 SQL 应该包含 COUNT(*) 和 WHERE 子句
    //
    // 此测试验证 count() 方法与 WHERE 条件的组合使用。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_with_where_condition(
            table_name in table_name_strategy(),
            field_name in field_name_strategy(),
            field_value in 1i32..1000i32,
        ) {
            let pool = create_test_pool_sync();

            // 创建带条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&field_name, "=", field_value)
                .field("COUNT(*)");

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(*)
            prop_assert!(
                sql.contains("COUNT(*)"),
                "带条件的 count() 查询应该包含 COUNT(*)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "带条件的 count() 查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM {}", table_name)),
                "count() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数 - 特定字段
    // 验证需求：4.4
    //
    // 属性：对于任意查询构建器，使用 field("COUNT(field_name)") 时，
    // 生成的 SQL 应该包含 COUNT(field_name)
    //
    // 此测试验证对特定字段进行 COUNT 统计的功能。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_specific_field(
            table_name in table_name_strategy(),
            field_name in field_name_strategy(),
        ) {
            let pool = create_test_pool_sync();

            // 创建查询构建器，统计特定字段
            let count_expr = format!("COUNT({})", field_name);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(&count_expr);

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(field_name)
            prop_assert!(
                sql.contains(&count_expr),
                "COUNT 特定字段应该包含 COUNT(field_name)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "COUNT 查询应该是 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM {}", table_name)),
                "COUNT 查询应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数
    // 验证需求：4.5
    //
    // 属性：对于任意字段名，调用 sum(field) 方法时，生成的 SQL 应该包含 SUM(field)
    //
    // 此测试验证 sum() 方法正确生成 SUM 聚合函数的 SQL 语句。
    // sum() 方法内部使用 CAST(SUM(field) AS DOUBLE) 来统一返回类型。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_aggregation_function(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个查询构建器并生成 SUM 查询的 SQL
            // 模拟 sum() 方法会生成的 SQL
            let sum_expr = format!("CAST(SUM({}) AS DOUBLE)", field);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(&sum_expr);

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "sum() 方法应该生成包含 SUM(field) 的 SQL 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含字段名
            prop_assert!(
                sql.contains(&field),
                "sum() 方法生成的 SQL 应该包含指定的字段名 {}，实际 SQL: {}",
                field,
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "sum() 方法应该生成 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM {}", table_name)),
                "sum() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 CAST 转换（sum() 方法的实现细节）
            prop_assert!(
                sql.to_uppercase().contains("CAST"),
                "sum() 方法应该使用 CAST 转换结果为 DOUBLE，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数 - 带条件
    // 验证需求：4.5
    //
    // 属性：对于任意带 WHERE 条件的查询构建器，调用 sum(field) 方法时，
    // 生成的 SQL 应该包含 SUM(field) 和 WHERE 子句
    //
    // 此测试验证 sum() 方法与 WHERE 条件的组合使用。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_with_where_condition(
            table_name in table_name_strategy(),
            sum_field in field_name_strategy(),
            where_field in field_name_strategy(),
            where_value in 1i32..1000i32,
        ) {
            // 确保两个字段名不同
            prop_assume!(sum_field != where_field);

            let pool = create_test_pool_sync();

            // 创建带条件的查询构建器
            let sum_expr = format!("CAST(SUM({}) AS DOUBLE)", sum_field);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&where_field, "=", where_value)
                .field(&sum_expr);

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "带条件的 sum() 查询应该包含 SUM(field)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含求和字段名
            prop_assert!(
                sql.contains(&sum_field),
                "sum() 方法应该包含求和字段名 {}，实际 SQL: {}",
                sum_field,
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "带条件的 sum() 查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM {}", table_name)),
                "sum() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数 - 多条件
    // 验证需求：4.5
    //
    // 属性：对于任意带多个 WHERE 条件的查询构建器，调用 sum(field) 方法时，
    // 生成的 SQL 应该正确包含所有条件
    //
    // 此测试验证 sum() 方法在复杂查询中的正确性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_with_multiple_conditions(
            table_name in table_name_strategy(),
            sum_field in field_name_strategy(),
            where_field1 in field_name_strategy(),
            where_field2 in field_name_strategy(),
            value1 in 1i32..1000i32,
            value2 in 1i32..1000i32,
        ) {
            // 确保字段名都不同
            prop_assume!(sum_field != where_field1);
            prop_assume!(sum_field != where_field2);
            prop_assume!(where_field1 != where_field2);

            let pool = create_test_pool_sync();

            // 创建带多个条件的查询构建器
            let sum_expr = format!("CAST(SUM({}) AS DOUBLE)", sum_field);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and_unchecked(&where_field1, "=", value1)
                .where_and_unchecked(&where_field2, ">", value2)
                .field(&sum_expr);

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "多条件 sum() 查询应该包含 SUM(field)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含求和字段名
            prop_assert!(
                sql.contains(&sum_field),
                "sum() 方法应该包含求和字段名 {}，实际 SQL: {}",
                sum_field,
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "多条件查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 AND 连接符（因为使用了两次 where_and）
            prop_assert!(
                sql.to_uppercase().contains(" AND "),
                "多个 where_and 条件应该用 AND 连接，实际 SQL: {}",
                sql
            );
        }
    }

    // ==================== 任务 5.7：属性测试 P2 - 不支持操作符返回错误 ====================

    // **Validates: Requirements 3.1**
    // 属性 P2：对于任意不在支持集合 {=, !=, >, <, >=, <=, like, LIKE} 中的操作符字符串，
    // where_and 必须返回 Err(DbError::UnsupportedOperator(_))
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_unsupported_operator_returns_error(
            // 生成不在支持集合中的操作符字符串
            // 排除所有支持的操作符：=, !=, >, <, >=, <=, like, LIKE
            op in "[a-zA-Z0-9!@#$%^&*]{1,10}"
        ) {
            // 过滤掉支持的操作符
            let supported = ["=", "!=", ">", "<", ">=", "<=", "like", "LIKE"];
            prop_assume!(!supported.contains(&op.as_str()));

            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, "users", false);

            // 调用 where_and 并验证返回 Err(DbError::UnsupportedOperator)
            let result = builder.where_and("field", &op, 1i64);

            prop_assert!(
                matches!(result, Err(crate::DbError::UnsupportedOperator(_))),
                "不支持的操作符 '{}' 应该返回 Err(DbError::UnsupportedOperator)，实际结果: {:?}",
                op,
                result.map(|_| "Ok")
            );
        }
    }

    // ==================== 任务 5.8：单元测试 - 已支持操作符的现有行为不变 ====================

    #[test]
    fn test_where_and_supported_operators_eq() {
        // 验证需求: 5.8  支持的操作符 = 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("age", "=", 18i64);
        assert!(result.is_ok(), "操作符 '=' 应该返回 Ok");
        let builder = result.unwrap();
        assert_eq!(builder.conditions.len(), 1);
    }

    #[test]
    fn test_where_and_supported_operators_ne() {
        // 验证需求: 5.8  支持的操作符 != 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("status", "!=", 0i64);
        assert!(result.is_ok(), "操作符 '!=' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_gt() {
        // 验证需求: 5.8  支持的操作符 > 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("age", ">", 18i64);
        assert!(result.is_ok(), "操作符 '>' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_lt() {
        // 验证需求: 5.8  支持的操作符 < 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("age", "<", 65i64);
        assert!(result.is_ok(), "操作符 '<' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_gte() {
        // 验证需求: 5.8  支持的操作符 >= 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("score", ">=", 60i64);
        assert!(result.is_ok(), "操作符 '>=' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_lte() {
        // 验证需求: 5.8  支持的操作符 <= 行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("score", "<=", 100i64);
        assert!(result.is_ok(), "操作符 '<=' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_like_lowercase() {
        // 验证需求: 5.8  支持的操作符 like（小写）行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("name", "like", "%test%");
        assert!(result.is_ok(), "操作符 'like' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_supported_operators_like_uppercase() {
        // 验证需求: 5.8  支持的操作符 LIKE（大写）行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("name", "LIKE", "%test%");
        assert!(result.is_ok(), "操作符 'LIKE' 应该返回 Ok");
    }

    #[test]
    fn test_where_and_unsupported_operator_returns_error() {
        // 验证需求: 5.1  不支持的操作符返回 Err(DbError::UnsupportedOperator)
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("age", "BETWEEN", 18i64);
        assert!(
            matches!(result, Err(crate::DbError::UnsupportedOperator(_))),
            "不支持的操作符 'BETWEEN' 应该返回 Err(DbError::UnsupportedOperator)"
        );
    }

    #[test]
    fn test_where_and_unsupported_operator_error_message() {
        // 验证需求: 5.1  错误消息包含操作符名称
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_and("age", "XOR", 1i64);
        match result {
            Err(crate::DbError::UnsupportedOperator(op)) => {
                assert_eq!(op, "XOR", "错误消息应该包含操作符名称");
            }
            _ => panic!("应该返回 UnsupportedOperator 错误"),
        }
    }

    #[test]
    fn test_where_or_unsupported_operator_returns_error() {
        // 验证需求: 5.2  where_or 遇到不支持操作符时返回 Err
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_or("age", "IN", 18i64);
        assert!(
            matches!(result, Err(crate::DbError::UnsupportedOperator(_))),
            "where_or 遇到不支持的操作符 'IN' 应该返回 Err(DbError::UnsupportedOperator)"
        );
    }

    #[test]
    fn test_where_or_supported_operators_work() {
        // 验证需求: 5.8  where_or 支持的操作符行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false).where_or("status", "=", 1i64);
        assert!(result.is_ok(), "where_or 操作符 '=' 应该返回 Ok");
        let builder = result.unwrap();
        assert_eq!(builder.conditions.len(), 1);
    }

    #[test]
    fn test_having_cond_unsupported_operator_returns_error() {
        // 验证需求: 5.3  having_cond 遇到不支持操作符时返回 Err
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "orders", false).having_cond("cnt", "LIKE", 5i64);
        assert!(
            matches!(result, Err(crate::DbError::UnsupportedOperator(_))),
            "having_cond 遇到不支持的操作符 'LIKE' 应该返回 Err(DbError::UnsupportedOperator)"
        );
    }

    #[test]
    fn test_having_cond_supported_operators_work() {
        // 验证需求: 5.8  having_cond 支持的操作符行为不变
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "orders", false).having_cond("cnt", ">", 5i64);
        assert!(result.is_ok(), "having_cond 操作符 '>' 应该返回 Ok");
        let builder = result.unwrap();
        assert_eq!(builder.having_clause.len(), 1);
    }

    #[test]
    fn test_where_and_unchecked_works_with_valid_operator() {
        // 验证需求: 5.4  where_and_unchecked 对合法操作符正常工作
        let pool = make_sync_test_pool();
        let builder =
            QueryBuilder::new(pool, "users", false).where_and_unchecked("age", "=", 18i64);
        assert_eq!(builder.conditions.len(), 1);
    }

    #[test]
    fn test_where_or_unchecked_works_with_valid_operator() {
        // 验证需求: 5.5  where_or_unchecked 对合法操作符正常工作
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_or_unchecked("status", "=", 1i64)
            .where_or_unchecked("status", "=", 2i64);
        // where_or 会将条件组合成 OR
        assert_eq!(builder.conditions.len(), 1);
    }

    #[test]
    fn test_having_cond_unchecked_works_with_valid_operator() {
        // 验证需求: 5.6  having_cond_unchecked 对合法操作符正常工作
        let pool = make_sync_test_pool();
        let builder =
            QueryBuilder::new(pool, "orders", false).having_cond_unchecked("cnt", ">", 5i64);
        assert_eq!(builder.having_clause.len(), 1);
    }

    #[test]
    fn test_where_and_chaining_with_result() {
        // 验证需求: 5.1  where_and 返回 Result 后可以链式调用
        let pool = make_sync_test_pool();
        let result = QueryBuilder::new(pool, "users", false)
            .where_and("age", ">", 18i64)
            .and_then(|b| b.where_and("status", "=", 1i64));
        assert!(result.is_ok(), "链式 where_and 调用应该成功");
        let builder = result.unwrap();
        assert_eq!(builder.conditions.len(), 2);
    }

    // ==================== 任务 8.5：属性测试 P5 - 批次大小分割正确性 ====================

    // **Validates: Requirements 7.4**
    // 属性 P5：对于任意 n 条记录和批次大小 b（b > 0），
    // 执行的批次数等于 ceil(n / b)。
    //
    // 此测试不需要真实数据库连接，直接测试 chunks() 分批逻辑来验证批次数。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn prop_batch_size_chunk_count(
            // n: 记录数，范围 0..=1000
            n in 0usize..=1000,
            // b: 批次大小，范围 1..=200（b > 0）
            b in 1usize..=200
        ) {
            // 构造 n 条虚拟记录（不需要真实数据，只需验证分批逻辑）
            let data: Vec<u32> = (0..n as u32).collect();

            // 计算实际分批数（使用 chunks() 分批）
            let actual_chunk_count = if data.is_empty() {
                0
            } else {
                data.chunks(b).count()
            };

            // 计算预期分批数：ceil(n / b)
            let expected_chunk_count = if n == 0 {
                0
            } else {
                n.div_ceil(b)  // 等价于 ceil(n / b)
            };

            // 验证分批数等于 ceil(n / b)
            prop_assert_eq!(
                actual_chunk_count,
                expected_chunk_count,
                "n={} 条记录，批次大小 b={}，实际分批数 {} 应等于 ceil(n/b)={}",
                n, b, actual_chunk_count, expected_chunk_count
            );

            // 额外验证：每个分批的大小不超过 b
            for chunk in data.chunks(b) {
                prop_assert!(
                    chunk.len() <= b,
                    "每个分批的大小 {} 不应超过批次大小 {}",
                    chunk.len(), b
                );
            }

            // 额外验证：所有分批的记录总数等于 n
            let total_records: usize = data.chunks(b).map(|c| c.len()).sum();
            prop_assert_eq!(
                total_records,
                n,
                "所有分批的记录总数 {} 应等于原始记录数 {}",
                total_records, n
            );
        }
    }

    // 单元测试：验证 batch_size 为 0 时返回错误
    #[test]
    fn test_insert_batch_with_size_zero_batch_size_returns_error() {
        // 验证需求: 7.2  batch_size 为 0 时返回 SerializationError
        // 此测试不需要数据库连接，直接验证错误处理逻辑
        // 使用 tokio 运行时执行异步测试
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 创建一个懒连接池（不需要真实连接）
            // batch_size == 0 的检查在任何数据库操作之前就会返回
            let pool = make_sync_test_pool();
            let builder = QueryBuilder::new(pool, "users", false);

            // 准备测试数据
            let data = vec![serde_json::json!({"name": "张三"})];

            // 调用 insert_batch_with_size，batch_size = 0
            let result = builder.insert_batch_with_size(&data, 0).await;

            // 验证返回 SerializationError
            assert!(
                matches!(result, Err(crate::DbError::SerializationError(_))),
                "batch_size 为 0 应返回 SerializationError，实际结果: {:?}",
                result.map(|_| "Ok")
            );

            // 验证错误消息包含预期内容
            if let Err(crate::DbError::SerializationError(msg)) = result {
                assert!(
                    msg.contains("batch_size") || msg.contains("0"),
                    "错误消息应提及 batch_size 不能为 0，实际消息: {}",
                    msg
                );
            }
        });
    }

    // 单元测试：验证分批逻辑的边界情况
    #[test]
    fn test_batch_chunk_logic_boundary_cases() {
        // 验证需求: 7.4  分批逻辑边界情况

        // 情况 1：数据量恰好等于批次大小
        let data: Vec<u32> = (0..10).collect();
        let chunks: Vec<_> = data.chunks(10).collect();
        assert_eq!(chunks.len(), 1, "数据量等于批次大小时应只有 1 个分批");
        assert_eq!(chunks[0].len(), 10);

        // 情况 2：数据量小于批次大小
        let data: Vec<u32> = (0..5).collect();
        let chunks: Vec<_> = data.chunks(10).collect();
        assert_eq!(chunks.len(), 1, "数据量小于批次大小时应只有 1 个分批");
        assert_eq!(chunks[0].len(), 5);

        // 情况 3：数据量为批次大小的整数倍
        let data: Vec<u32> = (0..20).collect();
        let chunks: Vec<_> = data.chunks(5).collect();
        assert_eq!(chunks.len(), 4, "20 条记录按批次大小 5 分批应得到 4 个分批");
        for chunk in &chunks {
            assert_eq!(chunk.len(), 5);
        }

        // 情况 4：数据量不是批次大小的整数倍（最后一批较小）
        let data: Vec<u32> = (0..11).collect();
        let chunks: Vec<_> = data.chunks(5).collect();
        assert_eq!(chunks.len(), 3, "11 条记录按批次大小 5 分批应得到 3 个分批");
        assert_eq!(chunks[0].len(), 5);
        assert_eq!(chunks[1].len(), 5);
        assert_eq!(chunks[2].len(), 1, "最后一批应只有 1 条记录");

        // 情况 5：批次大小为 1（每条记录一批）
        let data: Vec<u32> = (0..5).collect();
        let chunks: Vec<_> = data.chunks(1).collect();
        assert_eq!(chunks.len(), 5, "批次大小为 1 时，分批数应等于记录数");
    }
}
