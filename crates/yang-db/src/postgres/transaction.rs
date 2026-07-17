use crate::error::DbError;
use crate::postgres::query_builder::QueryBuilder;
use sqlx::Transaction as SqlxTransaction;

/// 数据库事务（PostgreSQL）
pub struct Transaction {
    tx: Option<SqlxTransaction<'static, sqlx::Postgres>>,
    enable_logging: bool,
}

impl Transaction {
    /// 创建新的事务实例
    pub(crate) fn new(tx: SqlxTransaction<'static, sqlx::Postgres>, enable_logging: bool) -> Self {
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
    /// # 弃用说明
    ///
    /// 已弃用（与 MySQL `Transaction::execute` 对齐）。请使用参数化查询方法
    /// [`execute_with_params`](Self::execute_with_params) 或
    /// [`query_with_params`](Self::query_with_params) 以防 SQL 注入。
    #[deprecated(
        since = "0.1.0",
        note = "使用 execute_with_params / query_with_params 等参数化方法替代"
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
    /// - sql: SQL 语句，使用 `$N` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(DbError): 执行失败错误
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use yang_db::postgres::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("postgres://postgres:postgres@localhost:5432/test").await?;
    /// let mut tx = db.transaction().await?;
    /// let params = vec![json!("张三"), json!("张三@example.com")];
    /// tx.execute_with_params("INSERT INTO users (name, email) VALUES ($1, $2)", params).await?;
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
    /// - sql: SQL 查询语句，使用 `$N` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询结果列表
    /// - Err(DbError): 查询失败错误
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use yang_db::postgres::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("postgres://postgres:postgres@localhost:5432/test").await?;
    /// let mut tx = db.transaction().await?;
    /// let params = vec![json!(1i64)];
    /// // let users: Vec<User> = tx.query_with_params("SELECT * FROM users WHERE id = $1", params).await?;
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
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
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
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.select_with_optional_lock(query, None).await
    }

    /// 在当前事务中执行 `FOR UPDATE` SELECT。
    pub async fn select_for_update<T>(&mut self, query: QueryBuilder<'_>) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.select_locked(query, crate::RowLock::ForUpdate).await
    }

    /// 在当前事务中执行 `FOR SHARE` SELECT。
    pub async fn select_for_share<T>(&mut self, query: QueryBuilder<'_>) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
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
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        self.select_with_optional_lock(query, Some(lock)).await
    }

    async fn select_with_optional_lock<T>(
        &mut self,
        query: QueryBuilder<'_>,
        lock: Option<crate::RowLock>,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
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
            sql_query = crate::postgres::query_builder::bind_param(sql_query, param);
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
    /// use yang_db::postgres::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("postgres://postgres:postgres@localhost:5432/test").await?;
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
    /// 语义与 MySQL 侧一致：上层已持有参数化好的 SQL 与参数，需要在**同一事务**
    /// 里执行以保证原子性。标记 `#[doc(hidden)]`，非稳定公开 API，仅供 workspace
    /// 内部桥接使用；值侧由 `$N` 绑定，标识符安全由调用方保证。
    ///
    /// # 返回
    /// - `Some(&mut PgConnection)`：事务仍处于活动状态
    /// - `None`：事务已 `commit`/`rollback`，连接不再可用
    #[doc(hidden)]
    pub fn executor(&mut self) -> Option<&mut sqlx::PgConnection> {
        // `SqlxTransaction` 通过 DerefMut 解引用为底层 PgConnection
        self.tx.as_deref_mut()
    }
}

// NEW-39: 与 MySQL 对齐——未提交/未回滚的事务被丢弃时输出诊断日志
impl Drop for Transaction {
    fn drop(&mut self) {
        if self.tx.is_some() {
            log::warn!(
                "PG 事务被丢弃而未提交/回滚——底层将自动回滚。请显式调用 commit() 或 rollback()。"
            );
        }
    }
}

fn bind_json_param_tx<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match param {
        serde_json::Value::String(value) => query.bind(value.clone()),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                query.bind(integer)
            } else if let Some(float) = value.as_f64() {
                query.bind(float)
            } else {
                query.bind(Option::<String>::None)
            }
        }
        serde_json::Value::Bool(value) => query.bind(*value),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.clone()),
    }
}

fn bind_json_param_as_tx<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>,
    param: &serde_json::Value,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>
where
    T: for<'row> sqlx::FromRow<'row, sqlx::postgres::PgRow>,
{
    match param {
        serde_json::Value::String(value) => query.bind(value.clone()),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                query.bind(integer)
            } else if let Some(float) = value.as_f64() {
                query.bind(float)
            } else {
                query.bind(Option::<String>::None)
            }
        }
        serde_json::Value::Bool(value) => query.bind(*value),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.clone()),
    }
}
