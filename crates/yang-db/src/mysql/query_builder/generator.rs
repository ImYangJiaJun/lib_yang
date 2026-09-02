//! SQL 生成器（内部使用）：把 `QueryBuilder` 的状态渲染为 SQL 文本与参数列表。

use std::collections::HashMap;

use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::field::{FieldType, JoinClause, OrderClause};

use super::{ArithmeticOperator, QueryBuilder, UnionOperator};

/// SQL 生成器（内部使用）
#[allow(dead_code)]
pub(crate) struct SqlGenerator {
    /// 生成的 SQL 语句
    pub(super) sql: String,
    /// SQL 参数列表
    pub(super) params: Vec<SqlValue>,
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
    pub(crate) fn append(&mut self, fragment: &str) {
        self.sql.push_str(fragment);
    }

    /// 添加参数
    pub(crate) fn add_param(&mut self, param: SqlValue) {
        self.params.push(param);
    }

    fn append_condition(&mut self, condition: Condition) -> Result<(), crate::error::DbError> {
        let rendered = crate::mysql::condition::render_condition_checked(condition)?;
        self.sql.push_str(&rendered.sql);
        self.params.extend(rendered.params);
        Ok(())
    }

    /// 清空生成器（保留已分配容量，避免重复分配）
    pub(crate) fn clear(&mut self) {
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
    pub(crate) fn build_select(
        &mut self,
        builder: &QueryBuilder,
    ) -> Result<(), crate::error::DbError> {
        self.clear();
        self.build_compound_select(builder)
    }

    fn build_compound_select(
        &mut self,
        builder: &QueryBuilder,
    ) -> Result<(), crate::error::DbError> {
        // SELECT 子句
        self.append("SELECT ");

        // DISTINCT 关键字
        if builder.distinct {
            self.append("DISTINCT ");
        }

        // 字段列表（普通投影字段 + 受控服务端表达式投影）
        if builder.fields.is_empty() && builder.select_exprs.is_empty() {
            self.append("*");
        } else {
            // 验证需求: ID-1 — field() 按设计接受 SQL 表达式，标识符转义由调用方负责
            let mut has_projection = false;
            if !builder.fields.is_empty() {
                self.append(&builder.fields.join(", "));
                has_projection = true;
            }
            // 受控服务端表达式投影：SQL 片段为 SqlExpr 白名单固定文本，别名经
            // 标识符校验转义，动态部分（如偏移秒数）以绑定参数进入参数列表。
            // 投影先于 WHERE 渲染，参数顺序与占位符顺序一致。
            for (expression, alias) in &builder.select_exprs {
                if has_projection {
                    self.append(", ");
                }
                has_projection = true;
                let (fragment, param) = expression.mysql_render();
                self.append(fragment);
                self.append(" AS ");
                self.append(&crate::mysql::identifier::quote_qualified(alias)?);
                if let Some(seconds) = param {
                    self.add_param(SqlValue::Int(seconds));
                }
            }
        }

        // FROM 子句
        self.append(" FROM ");
        self.append(&crate::mysql::identifier::quote_identifier(&builder.table)?);

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

        for (operator, branch) in &builder.unions {
            match operator {
                UnionOperator::Distinct => self.append(" UNION "),
                UnionOperator::All => self.append(" UNION ALL "),
            }
            self.append("(");
            self.build_compound_select(branch)?;
            self.append(")");
        }

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

        if conditions.len() == 1 {
            self.append_condition(conditions[0].clone())?;
        } else {
            // 内联拼接：逐条 condition_to_sql 直接写入 self.sql，
            // 避免 to_vec() 克隆全部条件 + Condition::And 包装 + parts Vec 中间分配。
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
            self.append_condition(conditions[0].clone())?;
        } else {
            // 与 build_where 多条件路径对齐：内联拼接避免 to_vec + And 包装 + parts Vec。
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
    /// - expr_assignments: 受控服务端表达式写入值（追加在 JSON 数据列之后）
    pub(crate) fn build_insert(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        expr_assignments: &[(String, crate::SqlExpr)],
    ) -> Result<(), crate::error::DbError> {
        // 清空之前的内容
        self.clear();

        // 确保 data 是一个对象
        let obj = data.as_object().ok_or_else(|| {
            crate::error::DbError::SerializationError("插入数据必须是 JSON 对象".to_string())
        })?;

        if obj.is_empty() && expr_assignments.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "插入数据不能为空".to_string(),
            ));
        }

        // 提取字段名和值（列名经 quote_identifier 校验+转义，杜绝注入；DB-1）
        let mut fields = Vec::with_capacity(obj.len());
        let mut placeholders = Vec::with_capacity(obj.len());

        for (key, value) in obj.iter() {
            fields.push(crate::mysql::identifier::quote_identifier(key)?);
            placeholders.push("?".to_string());

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.add_param(sql_value);
        }

        // 受控服务端表达式列：片段固定，动态部分（如偏移秒数）走绑定参数
        for (field, expression) in expr_assignments {
            fields.push(crate::mysql::identifier::quote_identifier(field)?);
            let (fragment, param) = expression.mysql_render();
            placeholders.push(fragment.to_string());
            if let Some(seconds) = param {
                self.add_param(SqlValue::Int(seconds));
            }
        }

        // 构建 INSERT 语句（表名亦 quote）
        self.append("INSERT INTO ");
        self.append(&crate::mysql::identifier::quote_identifier(table)?);
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

        // 列集一致性校验（NEW-9）：批量 VALUES 共用首条记录的列集，若后续记录列集不同
        // 会静默丢列 / 填 NULL。这里要求所有记录列集与首条完全一致，否则返回 InvalidArgument，
        // 避免异构数据被悄悄写错。
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

        // 构建 INSERT 语句头部（表名与列名经 quote_identifier 校验+转义；DB-1。
        // 注意 fields 仍为原始 JSON 键，用于后续 obj.get 查值；列头单独 quote。）
        let quoted_fields: Result<Vec<String>, crate::error::DbError> = fields
            .iter()
            .map(|f| crate::mysql::identifier::quote_identifier(f))
            .collect();
        let quoted_fields = quoted_fields?;
        self.append("INSERT INTO ");
        self.append(&crate::mysql::identifier::quote_identifier(table)?);
        self.append(" (");
        self.append(&quoted_fields.join(", "));
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
    /// - expr_assignments: 受控服务端表达式赋值（追加在 JSON 数据列之后）
    pub(crate) fn build_update(
        &mut self,
        table: &str,
        data: &serde_json::Value,
        field_types: &HashMap<String, FieldType>,
        conditions: &[Condition],
        expr_assignments: &[(String, crate::SqlExpr)],
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

        if obj.is_empty() && expr_assignments.is_empty() {
            return Err(crate::error::DbError::SerializationError(
                "更新数据不能为空".to_string(),
            ));
        }

        // 构建 UPDATE 语句（表名 quote；DB-1）
        self.append("UPDATE ");
        self.append(&crate::mysql::identifier::quote_identifier(table)?);
        self.append(" SET ");

        // 构建 SET 子句（列名 quote）
        let mut set_clauses = Vec::with_capacity(obj.len());

        for (key, value) in obj.iter() {
            set_clauses.push(format!(
                "{} = ?",
                crate::mysql::identifier::quote_identifier(key)?
            ));

            // 根据字段类型转换值
            let sql_value = self.json_value_to_sql_value(value, field_types.get(key))?;
            self.add_param(sql_value);
        }

        // 受控服务端表达式赋值：如 `used_at` = UNIX_TIMESTAMP()，动态部分走绑定参数
        for (field, expression) in expr_assignments {
            let (fragment, param) = expression.mysql_render();
            set_clauses.push(format!(
                "{} = {}",
                crate::mysql::identifier::quote_identifier(field)?,
                fragment
            ));
            if let Some(seconds) = param {
                self.add_param(SqlValue::Int(seconds));
            }
        }

        self.append(&set_clauses.join(", "));

        // 添加 WHERE 子句
        self.build_where(conditions)?;

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
        let table = crate::mysql::identifier::quote_identifier(table)?;
        let field = crate::mysql::identifier::quote_identifier(field)?;
        self.sql.push_str("UPDATE ");
        self.sql.push_str(&table);
        self.sql.push_str(" SET ");
        self.sql.push_str(&field);
        self.sql.push_str(" = ");
        self.sql.push_str(&field);
        self.sql.push(' ');
        self.sql.push_str(operator.as_sql());
        self.sql.push_str(" ?");
        self.params.push(SqlValue::Int(amount));
        self.build_where(conditions)
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

        // 构建 DELETE 语句（表名 quote；DB-1）
        self.append("DELETE FROM ");
        self.append(&crate::mysql::identifier::quote_identifier(table)?);

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

        // 列集一致性校验（NEW-9）：CASE WHEN 批量更新对所有记录套用首条的字段集，
        // 异构记录会静默丢列 / 生成 WHEN id=NULL（永不匹配）。要求每条记录都含 id_field
        // 且其非主键列集与首条完全一致，否则返回 InvalidArgument。
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

        // 写入 UPDATE ... SET 头部（表名 quote；DB-1。id_field/列名在各发射点单独 quote，
        // 原始值保留用于 record.get 查值。）
        let quoted_id = crate::mysql::identifier::quote_identifier(id_field)?;
        self.sql.push_str("UPDATE ");
        self.sql
            .push_str(&crate::mysql::identifier::quote_identifier(table)?);
        self.sql.push_str(" SET ");

        // 为每个字段生成 CASE WHEN 子句，直接追加到 self.sql
        // 消除了原来的 Vec<String> set_parts 和 Vec<String> when_parts 中间分配
        for (field_idx, field) in update_fields.iter().enumerate() {
            // 字段之间用 ", " 分隔
            if field_idx > 0 {
                self.sql.push_str(", ");
            }

            // 写入 "字段名 = CASE "（列名 quote）
            self.sql
                .push_str(&crate::mysql::identifier::quote_identifier(field)?);
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
                self.sql.push_str(&quoted_id);
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
        self.sql.push_str(&quoted_id);
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
        // 列名 quote（DB-1）；fields 保留原始键用于 obj.get 查值。
        let quoted_fields: Result<Vec<String>, crate::error::DbError> = fields
            .iter()
            .map(|f| crate::mysql::identifier::quote_identifier(f))
            .collect();
        let quoted_fields = quoted_fields?;

        // 统一用 push_str 风格拼接，避免 format! 的额外分配
        self.sql.push_str("INSERT INTO ");
        self.sql
            .push_str(&crate::mysql::identifier::quote_identifier(table)?);
        self.sql.push_str(" (");
        self.sql.push_str(&quoted_fields.join(", "));
        self.sql.push_str(") VALUES (");
        self.sql.push_str(&placeholders.join(", "));
        self.sql.push(')');

        for field in &fields {
            let val = obj.get(field.as_str()).unwrap_or(&serde_json::Value::Null);
            self.add_param(self.json_value_to_sql_value(val, field_types.get(field.as_str()))?);
        }

        self.sql.push_str(" ON DUPLICATE KEY UPDATE ");
        for (i, qf) in quoted_fields.iter().enumerate() {
            if i > 0 {
                self.sql.push_str(", ");
            }
            self.sql.push_str(qf);
            self.sql.push_str("=VALUES(");
            self.sql.push_str(qf);
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
                    // TIMESTAMP 类型：期望整数
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
                    // DECIMAL/NUMERIC：JSON 数字经 f64 会丢任意精度（NG-3）。仅当值能被
                    // f64 精确表示（|v| < 2^53）时走 Float（性能优）；超出安全整数范围、
                    // 高精度小数或字符串数字降级为字符串，由 MySQL 隐式转换/CAST 保精度。
                    const SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
                    if let Some(i) = value.as_i64() {
                        if (i.unsigned_abs() as f64) < SAFE_INT {
                            return Ok(SqlValue::Float(i as f64));
                        }
                        return Ok(SqlValue::String(i.to_string()));
                    }
                    if let Some(f) = value.as_f64() {
                        if f.is_finite() && f.abs() < SAFE_INT {
                            return Ok(SqlValue::Float(f));
                        }
                    }
                    if value.is_null() {
                        return Ok(SqlValue::Null);
                    }
                    // 数字（高精度/超大）或字符串数字：以字符串保精度绑定
                    if value.is_number() {
                        return Ok(SqlValue::String(value.to_string()));
                    }
                    if let Some(s) = value.as_str() {
                        return Ok(SqlValue::String(s.to_string()));
                    }
                    return Err(crate::error::DbError::TypeConversionError(format!(
                        "Decimal 字段期望数字或数字字符串，实得 {value}"
                    )));
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
                    } else if value.is_null() {
                        return Ok(SqlValue::Null);
                    } else {
                        return Err(crate::error::DbError::TypeConversionError(format!(
                            "Blob 字段期望字符串，实得 {value}"
                        )));
                    }
                }
                FieldType::Text => {
                    // TEXT 类型：转换为字符串
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
