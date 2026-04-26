use crate::error::DbError;
use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::field::FieldType;
use sqlx::Transaction as SqlxTransaction;
use std::collections::HashMap;

/// 数据库事务
pub struct Transaction {
    tx: Option<SqlxTransaction<'static, sqlx::MySql>>,
    enable_logging: bool,
}

impl Transaction {
    /// 创建新的事务实例
    pub(crate) fn new(tx: SqlxTransaction<'static, sqlx::MySql>, enable_logging: bool) -> Self {
        Self {
            tx: Some(tx),
            enable_logging,
        }
    }

    /// 提交事务
    pub async fn commit(mut self) -> Result<(), DbError> {
        if self.enable_logging {
            log::debug!("提交事务");
        }

        if let Some(tx) = self.tx.take() {
            tx.commit().await?;
        }

        Ok(())
    }

    /// 回滚事务
    pub async fn rollback(mut self) -> Result<(), DbError> {
        if self.enable_logging {
            log::debug!("回滚事务");
        }

        if let Some(tx) = self.tx.take() {
            tx.rollback().await?;
        }

        Ok(())
    }

    /// 执行原生 SQL
    pub async fn execute(&mut self, sql: &str) -> Result<u64, DbError> {
        if self.enable_logging {
            log::debug!("事务中执行: {}", sql);
        }

        if let Some(tx) = &mut self.tx {
            let result = sqlx::query(sql).execute(&mut **tx).await?;
            Ok(result.rows_affected())
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }

    /// 选择表，返回事务中的查询构建器
    ///
    /// # 参数
    /// - table_name: 表名
    ///
    /// # 返回
    /// - TransactionQueryBuilder: 事务查询构建器
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let mut tx = db.transaction().await?;
    ///
    /// // 在事务中插入数据
    /// let user_data = json!({"name": "张三", "email": "zhangsan@example.com"});
    /// let user_id = tx.table("users").insert(&user_data).await?;
    ///
    /// // 在事务中更新数据
    /// let update_data = json!({"status": 1});
    /// tx.table("users")
    ///     .where_and("id", "=", user_id)
    ///     .update(&update_data)
    ///     .await?;
    ///
    /// // 提交事务
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn table(&mut self, table_name: &str) -> TransactionQueryBuilder<'_> {
        TransactionQueryBuilder::new(self, table_name)
    }
}

/// 事务查询构建器
///
/// 用于在事务上下文中构建和执行查询
pub struct TransactionQueryBuilder<'a> {
    tx: &'a mut Transaction,
    table: String,
    conditions: Vec<Condition>,
    field_types: HashMap<String, FieldType>,
}

impl<'a> TransactionQueryBuilder<'a> {
    /// 创建新的事务查询构建器
    fn new(tx: &'a mut Transaction, table_name: &str) -> Self {
        Self {
            tx,
            table: table_name.to_string(),
            conditions: Vec::new(),
            field_types: HashMap::new(),
        }
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

    /// 添加 AND 条件
    pub fn where_and<V>(mut self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<SqlValue>,
    {
        let sql_value = value.into();
        let condition = match op {
            "=" => Condition::Eq(field.to_string(), sql_value),
            "!=" => Condition::Ne(field.to_string(), sql_value),
            ">" => Condition::Gt(field.to_string(), sql_value),
            "<" => Condition::Lt(field.to_string(), sql_value),
            ">=" => Condition::Gte(field.to_string(), sql_value),
            "<=" => Condition::Lte(field.to_string(), sql_value),
            "like" | "LIKE" => {
                if let SqlValue::String(s) = sql_value {
                    Condition::Like(field.to_string(), s)
                } else {
                    Condition::Like(field.to_string(), format!("{:?}", sql_value))
                }
            }
            _ => panic!("不支持的操作符: {}", op),
        };

        self.conditions.push(condition);
        self
    }

    /// 插入数据
    ///
    /// 在事务中执行 INSERT 操作
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
    pub async fn insert<T>(self, data: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 insert() 操作，表: {}", self.table);
        }

        // 将数据序列化为 JSON
        let json_data = serde_json::to_value(data)
            .map_err(|e| DbError::SerializationError(format!("数据序列化失败: {}", e)))?;

        // 生成 INSERT 语句
        let mut generator = crate::mysql::query_builder::SqlGenerator::new();
        generator.build_insert(&self.table, &json_data, &self.field_types)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 insert() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行插入
        if let Some(tx) = &mut self.tx.tx {
            let result = query.execute(&mut **tx).await?;
            let last_insert_id = result.last_insert_id();

            if self.tx.enable_logging {
                log::debug!("事务中 insert() 成功，插入 ID: {}", last_insert_id);
            }

            Ok(last_insert_id)
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }

    /// 更新数据
    ///
    /// 在事务中执行 UPDATE 操作
    /// 为了防止误操作，必须提供 WHERE 条件，否则会返回错误
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
    pub async fn update<T>(self, data: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 update() 操作，表: {}", self.table);
        }

        // 检查是否有 WHERE 条件
        if self.conditions.is_empty() {
            log::warn!("事务中 update() 操作缺少 WHERE 条件，禁止全表更新");
            return Err(DbError::MissingWhereClause);
        }

        // 将数据序列化为 JSON
        let json_data = serde_json::to_value(data)
            .map_err(|e| DbError::SerializationError(format!("数据序列化失败: {}", e)))?;

        // 生成 UPDATE 语句
        let mut generator = crate::mysql::query_builder::SqlGenerator::new();
        generator.build_update(&self.table, &json_data, &self.field_types, &self.conditions)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 update() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行更新
        if let Some(tx) = &mut self.tx.tx {
            let result = query.execute(&mut **tx).await?;
            let rows_affected = result.rows_affected();

            if self.tx.enable_logging {
                log::debug!("事务中 update() 成功，影响 {} 行", rows_affected);
            }

            Ok(rows_affected)
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }

    /// 删除数据
    ///
    /// 在事务中执行 DELETE 操作
    /// 为了防止误操作，必须提供 WHERE 条件，否则会返回错误
    ///
    /// # 返回
    /// - Ok(u64): 删除成功，返回受影响的行数
    /// - Err(DbError): 删除失败
    pub async fn delete(self) -> Result<u64, DbError> {
        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 delete() 操作，表: {}", self.table);
        }

        // 检查是否有 WHERE 条件
        if self.conditions.is_empty() {
            log::warn!("事务中 delete() 操作缺少 WHERE 条件，禁止全表删除");
            return Err(DbError::MissingWhereClause);
        }

        // 生成 DELETE 语句
        let mut generator = crate::mysql::query_builder::SqlGenerator::new();
        generator.build_delete(&self.table, &self.conditions)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.tx.enable_logging {
            log::debug!("事务中执行 delete() SQL: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query(sql);

        // 绑定参数
        for param in params {
            query = bind_execute_param(query, param);
        }

        // 执行删除
        if let Some(tx) = &mut self.tx.tx {
            let result = query.execute(&mut **tx).await?;
            let rows_affected = result.rows_affected();

            if self.tx.enable_logging {
                log::debug!("事务中 delete() 成功，影响 {} 行", rows_affected);
            }

            Ok(rows_affected)
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }
}

/// 绑定参数到执行查询（用于事务中的 INSERT/UPDATE/DELETE）
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
    match param {
        SqlValue::Null => query.bind(Option::<i32>::None),
        SqlValue::Bool(b) => query.bind(*b),
        SqlValue::Int(i) => query.bind(*i),
        SqlValue::Float(f) => query.bind(*f),
        SqlValue::String(s) => query.bind(s.clone()),
        SqlValue::Bytes(b) => query.bind(b.clone()),
        SqlValue::Json(j) => query.bind(j.to_string()),
        SqlValue::DateTime(dt) => query.bind(*dt),
        SqlValue::Timestamp(ts) => query.bind(*ts),
    }
}
