use crate::error::DbError;
use crate::mysql::query_builder::QueryBuilder;
use log;
use sqlx::Transaction as SqlxTransaction;

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
    /// - QueryBuilder: 与连接池路径共享的事务查询构建器
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
    /// let user_id = tx.table(yang_db::table!("users")).insert(&user_data).await?;
    ///
    /// // 在事务中更新数据
    /// let update_data = json!({"status": 1});
    /// tx.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, user_id)
    ///     .update(&update_data)
    ///     .await?;
    ///
    /// // 提交事务
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn table(&mut self, table: &crate::TableRef) -> QueryBuilder<'_> {
        let enable_logging = self.enable_logging;
        QueryBuilder::new_transaction(self, table.as_str(), enable_logging)
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

fn bind_json_param_tx<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    match param {
        serde_json::Value::String(value) => query.bind(value.clone()),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                query.bind(integer)
            } else if let Some(float) = value.as_f64() {
                query.bind(float.to_string())
            } else {
                query.bind(Option::<String>::None)
            }
        }
        serde_json::Value::Bool(value) => query.bind(*value),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.to_string()),
    }
}

fn bind_json_param_as_tx<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'row> sqlx::FromRow<'row, sqlx::mysql::MySqlRow>,
{
    match param {
        serde_json::Value::String(value) => query.bind(value.clone()),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                query.bind(integer)
            } else if let Some(float) = value.as_f64() {
                query.bind(float.to_string())
            } else {
                query.bind(Option::<String>::None)
            }
        }
        serde_json::Value::Bool(value) => query.bind(*value),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.to_string()),
    }
}
