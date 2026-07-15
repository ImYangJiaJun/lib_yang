use crate::error::DbError;
use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::field::FieldType;
use crate::mysql::query_builder::QueryBuilder;
use log;
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
    ///
    /// ⚠️ 安全警告：此方法接受裸 SQL 字符串，不进行参数化处理。调用方必须确保 SQL
    /// 字符串不包含用户输入，否则存在 SQL 注入风险。请优先使用 [`execute_with_params`]。
    #[deprecated(
        since = "0.1.0",
        note = "使用 execute_with_params 替代，避免 SQL 注入风险"
    )]
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

    /// 执行带参数的原生 SQL（参数化查询，防止 SQL 注入）
    ///
    /// # 参数
    /// - sql: SQL 语句，使用 `?` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(DbError): 执行失败错误
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let mut tx = db.transaction().await?;
    /// let params = vec![json!("张三"), json!("张三@example.com")];
    /// tx.execute_with_params("INSERT INTO users (name, email) VALUES (?, ?)", params).await?;
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_with_params(
        &mut self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<u64, DbError> {
        if self.enable_logging {
            log::debug!("事务中执行参数化语句: {}, 参数数量: {}", sql, params.len());
        }

        if let Some(tx) = &mut self.tx {
            // 构建查询并逐一绑定参数
            let mut query = sqlx::query(sql);
            for param in &params {
                query = bind_json_param_tx(query, param);
            }
            let result = query.execute(&mut **tx).await?;
            Ok(result.rows_affected())
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }

    /// 执行带参数的原生 SELECT 查询（参数化查询，防止 SQL 注入）
    ///
    /// # 参数
    /// - sql: SQL 查询语句，使用 `?` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询结果列表
    /// - Err(DbError): 查询失败错误
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let mut tx = db.transaction().await?;
    /// let params = vec![json!(1i64)];
    /// // let users: Vec<User> = tx.query_with_params("SELECT * FROM users WHERE id = ?", params).await?;
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_with_params<T>(
        &mut self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("事务中执行参数化查询: {}, 参数数量: {}", sql, params.len());
        }

        if let Some(tx) = &mut self.tx {
            // 构建查询并逐一绑定参数
            let mut query = sqlx::query_as::<_, T>(sql);
            for param in &params {
                query = bind_json_param_as_tx(query, param);
            }
            let rows = query.fetch_all(&mut **tx).await?;
            Ok(rows)
        } else {
            Err(DbError::TransactionError("事务已提交或回滚".to_string()))
        }
    }

    /// 在当前事务中执行普通 QueryBuilder SELECT。
    pub async fn select<T>(&mut self, query: QueryBuilder<'_>) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.select_with_optional_lock(query, None).await
    }

    /// 在当前事务中执行 `FOR UPDATE` SELECT。
    pub async fn select_for_update<T>(&mut self, query: QueryBuilder<'_>) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.select_locked(query, crate::RowLock::ForUpdate).await
    }

    /// 在当前事务中执行 `FOR SHARE` SELECT。
    pub async fn select_for_share<T>(&mut self, query: QueryBuilder<'_>) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.select_locked(query, crate::RowLock::ForShare).await
    }

    /// 在当前事务中按指定模式执行行锁 SELECT。
    pub async fn select_locked<T>(
        &mut self,
        query: QueryBuilder<'_>,
        lock: crate::RowLock,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.select_with_optional_lock(query, Some(lock)).await
    }

    async fn select_with_optional_lock<T>(
        &mut self,
        query: QueryBuilder<'_>,
        lock: Option<crate::RowLock>,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let (sql, params) = query.render_for_transaction(lock)?;
        if self.enable_logging {
            log::debug!("事务中执行 SELECT: {}, 参数: {:?}", sql, params);
        }
        let tx = self
            .tx
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))?;
        let mut sql_query = sqlx::query_as::<_, T>(&sql);
        for param in &params {
            sql_query = crate::mysql::query_builder::bind_param(sql_query, param);
        }
        Ok(sql_query.fetch_all(&mut **tx).await?)
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

    /// 借出底层 sqlx 连接（受控逃生舱，供同 workspace 上层在事务内执行自构建的参数化语句）
    ///
    /// 上层（如 yang-base 受保护层）已持有自己构建好的、参数全部 `?` 绑定的
    /// SQL 与参数，需要在**同一事务**里执行以保证原子性。`Transaction` 自身
    /// 仅提供 `insert/update/delete` 三个高层方法，无法承载受保护层的权限/软删/
    /// 校验语义，故开放此逃生舱让上层直接拿到 `&mut MySqlConnection` 执行。
    ///
    /// 标记 `#[doc(hidden)]`：非稳定公开 API，仅供 workspace 内部桥接使用，
    /// 调用方必须保证 SQL 标识符安全（值侧由 `?` 绑定，标识符由调用方转义）。
    ///
    /// # 返回
    /// - `Some(&mut MySqlConnection)`：事务仍处于活动状态
    /// - `None`：事务已 `commit`/`rollback`，连接不再可用
    // SAFETY: 此方法暴露底层数据库连接，调用方可执行任意 SQL。
    // 仅限 workspace 内部桥接使用，外部代码不得调用。
    #[doc(hidden)]
    pub fn executor(&mut self) -> Option<&mut sqlx::MySqlConnection> {
        // `SqlxTransaction` 通过 DerefMut 解引用为底层 MySqlConnection
        self.tx.as_deref_mut()
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if self.tx.is_some() {
            log::warn!(
                "事务被丢弃而未提交/回滚——底层将自动回滚。请显式调用 commit() 或 rollback()。"
            );
        }
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
    /// 延迟错误：链式 setter（如 `where_and`）无法返回 `Result`，
    /// 故将首个错误暂存于此，在终端方法（insert/update/delete）统一返回。
    error: Option<DbError>,
}

impl<'a> TransactionQueryBuilder<'a> {
    /// 创建新的事务查询构建器
    fn new(tx: &'a mut Transaction, table_name: &str) -> Self {
        Self {
            tx,
            table: table_name.to_string(),
            conditions: Vec::new(),
            field_types: HashMap::new(),
            error: None,
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
    ///
    /// 遇到不支持的操作符时不会 panic，而是将错误暂存，由终端方法
    /// （insert/update/delete）返回 `Err(DbError::UnsupportedOperator)`。
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
            _ => {
                // 仅记录首个错误，保持链式调用可继续
                if self.error.is_none() {
                    self.error = Some(DbError::UnsupportedOperator(op.to_string()));
                }
                return self;
            }
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
        // 先返回链式调用中暂存的错误（如不支持的操作符）
        if let Some(err) = self.error {
            return Err(err);
        }

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
        // 先返回链式调用中暂存的错误（如不支持的操作符）
        if let Some(err) = self.error {
            return Err(err);
        }

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

    /// 在当前事务中原子增加字段值。
    pub async fn increment(self, field: &str, amount: i64) -> Result<u64, DbError> {
        self.execute_arithmetic_update(
            field,
            amount,
            crate::mysql::query_builder::ArithmeticOperator::Add,
        )
        .await
    }

    /// 在当前事务中原子减少字段值。
    pub async fn decrement(self, field: &str, amount: i64) -> Result<u64, DbError> {
        self.execute_arithmetic_update(
            field,
            amount,
            crate::mysql::query_builder::ArithmeticOperator::Subtract,
        )
        .await
    }

    async fn execute_arithmetic_update(
        self,
        field: &str,
        amount: i64,
        operator: crate::mysql::query_builder::ArithmeticOperator,
    ) -> Result<u64, DbError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let mut generator = crate::mysql::query_builder::SqlGenerator::new();
        generator.build_arithmetic_update(
            &self.table,
            field,
            operator,
            amount,
            &self.conditions,
        )?;
        let mut query = sqlx::query(generator.get_sql());
        for param in generator.get_params() {
            query = bind_execute_param(query, param);
        }
        let tx = self
            .tx
            .tx
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))?;
        Ok(query.execute(&mut **tx).await?.rows_affected())
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
        // 先返回链式调用中暂存的错误（如不支持的操作符）
        if let Some(err) = self.error {
            return Err(err);
        }

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

/// 将 `serde_json::Value` 参数绑定到事务执行查询（用于参数化 INSERT/UPDATE/DELETE）
///
/// # 参数
/// - query: sqlx 执行查询对象
/// - param: JSON 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_json_param_tx<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    match param {
        // 字符串类型直接绑定
        serde_json::Value::String(s) => query.bind(s.clone()),
        // 数字类型转为 i64 绑定
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                // 浮点数转为字符串绑定，避免精度丢失
                query.bind(f.to_string())
            } else {
                query.bind(Option::<String>::None)
            }
        }
        // 布尔类型绑定
        serde_json::Value::Bool(b) => query.bind(*b),
        // NULL 类型绑定为 None
        serde_json::Value::Null => query.bind(Option::<String>::None),
        // 数组和对象类型序列化为 JSON 字符串绑定
        other => query.bind(other.to_string()),
    }
}

/// 将 `serde_json::Value` 参数绑定到事务 `query_as` 查询（用于参数化 SELECT）
///
/// # 参数
/// - query: sqlx query_as 查询对象
/// - param: JSON 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_json_param_as_tx<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>,
{
    match param {
        // 字符串类型直接绑定
        serde_json::Value::String(s) => query.bind(s.clone()),
        // 数字类型转为 i64 绑定
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                // 浮点数转为字符串绑定，避免精度丢失
                query.bind(f.to_string())
            } else {
                query.bind(Option::<String>::None)
            }
        }
        // 布尔类型绑定
        serde_json::Value::Bool(b) => query.bind(*b),
        // NULL 类型绑定为 None
        serde_json::Value::Null => query.bind(Option::<String>::None),
        // 数组和对象类型序列化为 JSON 字符串绑定
        other => query.bind(other.to_string()),
    }
}
