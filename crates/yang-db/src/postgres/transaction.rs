use crate::error::DbError;
use crate::isolation::IsolationLevel;
use crate::postgres::query_builder::QueryBuilder;
use sqlx::Transaction as SqlxTransaction;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// 流式游标默认每批拉取的行数。
pub(crate) const DEFAULT_STREAM_BATCH_SIZE: usize = 1000;

/// 进程内游标名计数器。游标名只由本 crate 生成，不含外部输入。
static STREAM_CURSOR_COUNTER: AtomicU64 = AtomicU64::new(1);

/// 生成进程内唯一的游标名。
pub(crate) fn next_cursor_name() -> String {
    format!(
        "yang_cursor_{}",
        STREAM_CURSOR_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// 规整流式查询 SQL：去掉首尾空白与末尾分号，避免 `DECLARE ... CURSOR FOR` 语法错误。
pub(crate) fn normalize_stream_sql(sql: &str) -> Result<&str, DbError> {
    let trimmed = sql.trim().trim_end_matches(';').trim_end();
    if trimmed.is_empty() {
        return Err(DbError::InvalidArgument(
            "流式查询的 SQL 不能为空".to_string(),
        ));
    }
    Ok(trimmed)
}

/// 生成保存点控制语句；`name` 必须是合法 SQL 标识符。
fn savepoint_statement(prefix: &str, name: &str) -> Result<String, DbError> {
    Ok(format!(
        "{prefix} {}",
        crate::postgres::identifier::quote_identifier(name)?
    ))
}

/// 两阶段提交 GID 的字节上限（PostgreSQL `gid` 字段长度）。
const MAX_PREPARED_GID_BYTES: usize = 200;

/// 校验两阶段提交 GID。
///
/// `PREPARE TRANSACTION` / `COMMIT PREPARED` 的 GID 语法要求字符串常量，
/// **不能**用绑定参数；因此只允许 `[A-Za-z0-9_.:-]` 白名单字符，杜绝把任意
/// 文本拼进 SQL。
pub(crate) fn validate_prepared_gid(gid: &str) -> Result<(), DbError> {
    if gid.is_empty() || gid.len() > MAX_PREPARED_GID_BYTES {
        return Err(DbError::InvalidArgument(format!(
            "两阶段提交 GID 长度必须在 1..={MAX_PREPARED_GID_BYTES} 字节，当前 {} 字节",
            gid.len()
        )));
    }
    if !gid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
    {
        return Err(DbError::InvalidArgument(format!(
            "两阶段提交 GID 只允许 ASCII 字母、数字与 _ . - : 字符: {gid:?}"
        )));
    }
    Ok(())
}

/// 校验并引用会话 GUC 名（如 `app.tenant_id`）。
///
/// 键名不能参数化，必须严格校验：每一段都必须是 `[A-Za-z_][A-Za-z0-9_]*`，
/// 最多三段。返回整串作为单引号字符串常量（`'app.tenant_id'`），供
/// `set_config('key', $1, true)` 使用。
fn quote_guc_name(key: &str) -> Result<String, DbError> {
    if key.is_empty() || key.len() > 128 {
        return Err(DbError::InvalidArgument(format!(
            "会话 GUC 名长度必须在 1..=128 字节: {key:?}"
        )));
    }
    let parts = key
        .split('.')
        .map(crate::sql_types::Identifier::parse)
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() || parts.len() > 3 {
        return Err(DbError::InvalidArgument(format!(
            "会话 GUC 名最多三段（`prefix.name`）: {key:?}"
        )));
    }
    // 每段已通过严格标识符校验，整串只会含 `[A-Za-z0-9_.]`，可直接作为
    // 单引号字符串常量（无内嵌单引号）。
    Ok(format!("'{key}'"))
}

/// 表锁模式（`LOCK TABLE ... IN <mode> MODE`）。
///
/// 只暴露 PostgreSQL 文档定义的 8 种表级锁模式，避免把任意字符串拼进 SQL。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TableLockMode {
    /// `ACCESS SHARE`
    AccessShare,
    /// `ROW SHARE`
    RowShare,
    /// `ROW EXCLUSIVE`
    RowExclusive,
    /// `SHARE UPDATE EXCLUSIVE`
    ShareUpdateExclusive,
    /// `SHARE`
    Share,
    /// `SHARE ROW EXCLUSIVE`
    ShareRowExclusive,
    /// `EXCLUSIVE`
    Exclusive,
    /// `ACCESS EXCLUSIVE`
    AccessExclusive,
}

impl TableLockMode {
    /// 返回 SQL 关键字串。
    pub fn as_sql(self) -> &'static str {
        match self {
            Self::AccessShare => "ACCESS SHARE",
            Self::RowShare => "ROW SHARE",
            Self::RowExclusive => "ROW EXCLUSIVE",
            Self::ShareUpdateExclusive => "SHARE UPDATE EXCLUSIVE",
            Self::Share => "SHARE",
            Self::ShareRowExclusive => "SHARE ROW EXCLUSIVE",
            Self::Exclusive => "EXCLUSIVE",
            Self::AccessExclusive => "ACCESS EXCLUSIVE",
        }
    }
}

/// 生成 `LOCK TABLE` 语句；表名按 `table` / `schema.table` 口径校验并加引号。
fn lock_table_statement(table: &str, mode: TableLockMode) -> Result<String, DbError> {
    let quoted = crate::postgres::identifier::quote_table(table)?;
    Ok(format!("LOCK TABLE {quoted} IN {} MODE", mode.as_sql()))
}

/// PostgreSQL 事务属性（Phase B）。
///
/// 与 [`IsolationLevel`] 组合使用：`isolation` 为 `None` 时不发
/// `SET TRANSACTION ISOLATION LEVEL`，沿用服务端默认（通常是 READ COMMITTED）。
///
/// 标注 `#[non_exhaustive]`：未来新增事务属性不构成破坏性变更；跨 crate 请用
/// [`TransactionOptions::default`] / [`TransactionOptions::new`] 加链式 setter 构造。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TransactionOptions {
    /// 隔离级别；`None` 表示沿用服务端默认。
    pub isolation: Option<IsolationLevel>,
    /// 是否声明为只读事务（`READ ONLY`）。只读事务内写入由服务端拒绝（SQLSTATE 25006）。
    pub read_only: bool,
    /// 是否声明 `DEFERRABLE`（只对 SERIALIZABLE READ ONLY 事务有实际意义）。
    pub deferrable: bool,
}

impl TransactionOptions {
    /// 全默认选项（等价于 `Database::transaction`）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 只读事务选项。
    pub fn read_only() -> Self {
        Self {
            read_only: true,
            ..Self::default()
        }
    }

    /// 设置隔离级别（链式）。
    pub fn with_isolation(mut self, isolation: IsolationLevel) -> Self {
        self.isolation = Some(isolation);
        self
    }

    /// 设置只读属性（链式）。
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// 设置 `DEFERRABLE` 属性（链式）。
    pub fn with_deferrable(mut self, deferrable: bool) -> Self {
        self.deferrable = deferrable;
        self
    }

    /// 生成 `SET TRANSACTION ...` 语句；全默认时返回 `None`（不发语句）。
    pub(crate) fn set_transaction_sql(&self) -> Option<String> {
        let mut modes = Vec::new();
        if let Some(isolation) = self.isolation {
            modes.push(format!("ISOLATION LEVEL {}", isolation.as_sql()));
        }
        if self.read_only {
            modes.push("READ ONLY".to_string());
        }
        if self.deferrable {
            modes.push("DEFERRABLE".to_string());
        }
        if modes.is_empty() {
            None
        } else {
            Some(format!("SET TRANSACTION {}", modes.join(", ")))
        }
    }
}

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
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, user_id)?
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

    /// 在当前事务内创建保存点（`SAVEPOINT`）。
    ///
    /// `name` 必须是合法 SQL 标识符（首字符字母/下划线，其余字母/数字/下划线）；
    /// 非法名称返回 [`DbError::InvalidArgument`]，不会被拼接进 SQL。
    pub async fn savepoint(&mut self, name: &str) -> Result<(), DbError> {
        let sql = savepoint_statement("SAVEPOINT", name)?;
        self.execute_control(&sql).await
    }

    /// 回滚到指定保存点（`ROLLBACK TO SAVEPOINT`）。
    ///
    /// 只撤销保存点之后的工作；保存点之前的外层事务与更早的保存点不受影响。
    pub async fn rollback_to(&mut self, name: &str) -> Result<(), DbError> {
        let sql = savepoint_statement("ROLLBACK TO SAVEPOINT", name)?;
        self.execute_control(&sql).await
    }

    /// 释放保存点（`RELEASE SAVEPOINT`）。
    pub async fn release_savepoint(&mut self, name: &str) -> Result<(), DbError> {
        let sql = savepoint_statement("RELEASE SAVEPOINT", name)?;
        self.execute_control(&sql).await
    }

    /// 设置事务级会话参数（`SET LOCAL` 语义，随事务提交/回滚自动失效）。
    ///
    /// `key` 必须是白名单标识符（允许点号，如 `app.tenant_id`），非法名返回
    /// [`DbError::InvalidArgument`]；`value` 通过绑定参数传入。
    ///
    /// 实现说明：PostgreSQL 的 `SET` 语法在扩展查询协议下不接受参数占位符
    /// （`SET LOCAL "k" = $1` 会报语法错误），因此这里用等价的
    /// `set_config('k', $1, true)`：键名经严格校验后以字符串常量进入，
    /// 值仍走绑定参数，安全性不变。
    pub async fn set_local(&mut self, key: &str, value: &str) -> Result<(), DbError> {
        let quoted = quote_guc_name(key)?;
        let sql = format!("SELECT set_config({quoted}, $1, true)");
        if self.enable_logging {
            log::debug!("设置事务级会话参数: {}", key);
        }
        let tx = self.active_tx()?;
        sqlx::query(&sql).bind(value).execute(&mut **tx).await?;
        Ok(())
    }

    /// 在当前事务内获取表级锁（`LOCK TABLE ... IN <mode> MODE`）。
    ///
    /// 表名按 `table` / `schema.table` 口径校验并加引号；锁在事务结束时自动释放。
    /// 模式见 [`TableLockMode`]。
    pub async fn lock_table(
        &mut self,
        table: &crate::TableRef,
        mode: TableLockMode,
    ) -> Result<(), DbError> {
        let sql = lock_table_statement(table.as_str(), mode)?;
        self.execute_control(&sql).await
    }

    /// 将当前事务准备为两阶段提交（`PREPARE TRANSACTION '<gid>'`），消费事务句柄。
    ///
    /// 成功后事务进入服务端 prepared 状态，需在任意连接上调用
    /// [`Database::commit_prepared`](crate::postgres::Database::commit_prepared) 或
    /// [`Database::rollback_prepared`](crate::postgres::Database::rollback_prepared)
    /// 收尾。服务端 `max_prepared_transactions = 0` 时返回明确的
    /// [`DbError::TransactionError`]，而非仅透传驱动报文。
    ///
    /// GID 不能参数化，只接受 `[A-Za-z0-9_.:-]` 白名单字符（最长 200 字节）。
    pub async fn prepare(mut self, gid: &str) -> Result<(), DbError> {
        validate_prepared_gid(gid)?;
        let mut tx = self
            .tx
            .take()
            .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))?;
        let sql = format!("PREPARE TRANSACTION '{gid}'");
        if self.enable_logging {
            log::debug!("准备两阶段提交: gid={gid}");
        }
        let result = sqlx::query(&sql).execute(&mut *tx).await;
        match result {
            Ok(_) => {
                // PREPARE 已结束服务端事务；主动 rollback 清空 sqlx 的事务状态，
                // 服务端只会回 25P01 警告，连接可安全归还连接池。
                let _ = tx.rollback().await;
                Ok(())
            }
            Err(error) => {
                let error = DbError::from(error);
                let _ = tx.rollback().await;
                if error.sqlstate() == Some("55000") {
                    return Err(DbError::TransactionError(
                        "两阶段提交不可用：服务端 max_prepared_transactions 为 0；请启用后重试"
                            .to_string(),
                    ));
                }
                Err(error)
            }
        }
    }

    /// 获取事务级 advisory lock（`pg_advisory_xact_lock`），事务结束自动释放。
    ///
    /// 该调用会阻塞直到拿到锁；需要非阻塞语义请用
    /// [`try_advisory_xact_lock`](Self::try_advisory_xact_lock)。
    pub async fn advisory_xact_lock(&mut self, key: i64) -> Result<(), DbError> {
        let tx = self.active_tx()?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// 尝试获取事务级 advisory lock（`pg_try_advisory_xact_lock`），不阻塞。
    ///
    /// 返回 `true` 表示已持有锁，`false` 表示锁被其他会话占用。
    pub async fn try_advisory_xact_lock(&mut self, key: i64) -> Result<bool, DbError> {
        let tx = self.active_tx()?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(key)
            .fetch_one(&mut **tx)
            .await?;
        Ok(acquired)
    }

    /// 在当前事务内以服务端游标流式读取结果集（默认每批 1000 行）。
    ///
    /// 游标通过 `DECLARE ... CURSOR FOR` + 分批 `FETCH` 实现，大结果集不会一次性
    /// 进入内存。游标借出事务的可变引用，使用期间不能再操作同一事务；未读尽即丢弃
    /// 时，服务端游标随事务结束（提交/回滚）一并释放。
    ///
    /// # 错误
    ///
    /// SQL 为空或 `batch_size` 为 0 时返回 [`DbError::InvalidArgument`]。
    pub async fn stream_query(
        &mut self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<TxRowCursor<'_>, DbError> {
        self.stream_query_with_batch_size(sql, params, DEFAULT_STREAM_BATCH_SIZE)
            .await
    }

    /// 同 [`stream_query`](Self::stream_query)，可指定每批拉取行数。
    pub async fn stream_query_with_batch_size(
        &mut self,
        sql: &str,
        params: Vec<serde_json::Value>,
        batch_size: usize,
    ) -> Result<TxRowCursor<'_>, DbError> {
        if batch_size == 0 {
            return Err(DbError::InvalidArgument(
                "流式查询 batch_size 必须大于 0".to_string(),
            ));
        }
        let body = normalize_stream_sql(sql)?;
        let cursor_name = next_cursor_name();
        let declare = format!(
            "DECLARE {} CURSOR FOR {}",
            crate::postgres::identifier::quote_identifier(&cursor_name)?,
            body
        );
        let enable_logging = self.enable_logging;
        {
            let tx = self.active_tx()?;
            if enable_logging {
                log::debug!("事务内声明游标: {}", declare);
            }
            let mut query = sqlx::query(&declare);
            for param in &params {
                query = bind_json_param_tx(query, param);
            }
            query.execute(&mut **tx).await?;
        }
        let tx = self.active_tx()?;
        Ok(TxRowCursor::new(
            tx,
            cursor_name,
            batch_size,
            enable_logging,
        ))
    }

    /// 执行事务控制语句（保存点等），不做参数绑定。
    async fn execute_control(&mut self, sql: &str) -> Result<(), DbError> {
        if self.enable_logging {
            log::debug!("事务控制语句: {}", sql);
        }
        let tx = self.active_tx()?;
        sqlx::query(sql).execute(&mut **tx).await?;
        Ok(())
    }

    /// 借出仍处于活动状态的事务句柄。
    fn active_tx(&mut self) -> Result<&mut SqlxTransaction<'static, sqlx::Postgres>, DbError> {
        self.tx
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))
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

/// 事务内的服务端游标（`DECLARE ... CURSOR` + 分批 `FETCH`）。
///
/// 逐行消费：缓冲区为空时才向后端拉取下一批，内存占用以 `batch_size` 为上限。
/// 游标未读尽即被丢弃时无需显式清理——服务端游标随所属事务的提交/回滚一并释放。
pub struct TxRowCursor<'a> {
    conn: &'a mut sqlx::PgConnection,
    cursor_name: String,
    batch_size: usize,
    buffer: VecDeque<sqlx::postgres::PgRow>,
    exhausted: bool,
    closed: bool,
    enable_logging: bool,
}

impl<'a> TxRowCursor<'a> {
    fn new(
        conn: &'a mut sqlx::PgConnection,
        cursor_name: String,
        batch_size: usize,
        enable_logging: bool,
    ) -> Self {
        Self {
            conn,
            cursor_name,
            batch_size,
            buffer: VecDeque::with_capacity(batch_size.min(1024)),
            exhausted: false,
            closed: false,
            enable_logging,
        }
    }

    /// 每批拉取的最大行数。
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 取下一条结果；流结束返回 `Ok(None)`。
    pub async fn next(&mut self) -> Result<Option<sqlx::postgres::PgRow>, DbError> {
        loop {
            if let Some(row) = self.buffer.pop_front() {
                return Ok(Some(row));
            }
            if self.exhausted {
                self.close_inner().await?;
                return Ok(None);
            }
            self.fetch_batch().await?;
        }
    }

    /// 提前关闭游标，丢弃尚未消费的缓冲行。
    pub async fn close(mut self) -> Result<(), DbError> {
        self.buffer.clear();
        self.close_inner().await
    }

    async fn fetch_batch(&mut self) -> Result<(), DbError> {
        let sql = format!(
            "FETCH FORWARD {} FROM {}",
            self.batch_size,
            crate::postgres::identifier::quote_identifier(&self.cursor_name)?
        );
        if self.enable_logging {
            log::debug!("事务内游标拉取: {}", sql);
        }
        let rows = sqlx::query(&sql).fetch_all(&mut *self.conn).await?;
        if rows.len() < self.batch_size {
            self.exhausted = true;
        }
        self.buffer.extend(rows);
        Ok(())
    }

    async fn close_inner(&mut self) -> Result<(), DbError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.exhausted = true;
        let sql = format!(
            "CLOSE {}",
            crate::postgres::identifier::quote_identifier(&self.cursor_name)?
        );
        sqlx::query(&sql).execute(&mut *self.conn).await?;
        Ok(())
    }
}

fn bind_json_param_tx<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match param {
        serde_json::Value::String(value) => query.bind(value.clone()),
        serde_json::Value::Number(value) => match crate::json_param::classify_json_number(value) {
            Some(crate::json_param::JsonNumber::Int(i)) => query.bind(i),
            Some(crate::json_param::JsonNumber::Uint(u)) => query.bind(u.to_string()),
            Some(crate::json_param::JsonNumber::Float(f)) => query.bind(f),
            None => query.bind(Option::<String>::None),
        },
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
        serde_json::Value::Number(value) => match crate::json_param::classify_json_number(value) {
            Some(crate::json_param::JsonNumber::Int(i)) => query.bind(i),
            Some(crate::json_param::JsonNumber::Uint(u)) => query.bind(u.to_string()),
            Some(crate::json_param::JsonNumber::Float(f)) => query.bind(f),
            None => query.bind(Option::<String>::None),
        },
        serde_json::Value::Bool(value) => query.bind(*value),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.clone()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn transaction_options_default_emits_no_set_transaction() {
        assert_eq!(TransactionOptions::default().set_transaction_sql(), None);
        assert_eq!(TransactionOptions::new().set_transaction_sql(), None);
    }

    #[test]
    fn transaction_options_renders_isolation_read_only_and_deferrable() {
        let isolation_only =
            TransactionOptions::default().with_isolation(IsolationLevel::Serializable);
        assert_eq!(
            isolation_only.set_transaction_sql().as_deref(),
            Some("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        );

        assert_eq!(
            TransactionOptions::read_only()
                .set_transaction_sql()
                .as_deref(),
            Some("SET TRANSACTION READ ONLY")
        );

        let full = TransactionOptions::default()
            .with_isolation(IsolationLevel::RepeatableRead)
            .with_read_only(true)
            .with_deferrable(true);
        assert_eq!(
            full.set_transaction_sql().as_deref(),
            Some("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY, DEFERRABLE")
        );
    }

    #[test]
    fn savepoint_statements_quote_names_and_reject_injection() {
        assert_eq!(
            savepoint_statement("SAVEPOINT", "sp_1").unwrap(),
            "SAVEPOINT \"sp_1\""
        );
        assert_eq!(
            savepoint_statement("ROLLBACK TO SAVEPOINT", "sp_1").unwrap(),
            "ROLLBACK TO SAVEPOINT \"sp_1\""
        );
        assert_eq!(
            savepoint_statement("RELEASE SAVEPOINT", "sp_1").unwrap(),
            "RELEASE SAVEPOINT \"sp_1\""
        );

        for invalid in ["", "1sp", "sp; DROP TABLE t", "sp\"x"] {
            assert!(
                matches!(
                    savepoint_statement("SAVEPOINT", invalid),
                    Err(DbError::InvalidArgument(_))
                ),
                "保存点名应拒绝 {invalid:?}"
            );
        }
    }

    #[test]
    fn stream_sql_is_normalized_and_empty_is_rejected() {
        assert_eq!(normalize_stream_sql("  SELECT 1;  ").unwrap(), "SELECT 1");
        assert_eq!(normalize_stream_sql("SELECT 1;;").unwrap(), "SELECT 1");
        assert!(matches!(
            normalize_stream_sql("   ;  "),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn cursor_names_are_unique_and_identifier_safe() {
        let first = next_cursor_name();
        let second = next_cursor_name();
        assert_ne!(first, second);
        assert!(crate::postgres::identifier::is_valid_identifier(&first));
        assert!(crate::postgres::identifier::is_valid_identifier(&second));
    }

    #[test]
    fn guc_names_allow_dotted_identifiers_and_reject_injection() {
        assert_eq!(quote_guc_name("app.tenant_id").unwrap(), "'app.tenant_id'");
        assert_eq!(
            quote_guc_name("statement_timeout").unwrap(),
            "'statement_timeout'"
        );

        for invalid in [
            "",
            ".app",
            "app.",
            "app..tenant",
            "a.b.c.d",
            "app.tenant_id = 'x'",
            "app.tenant_id; DROP TABLE t",
            "app tenant",
            "app\"tenant\"",
        ] {
            assert!(
                matches!(quote_guc_name(invalid), Err(DbError::InvalidArgument(_))),
                "GUC 名应拒绝 {invalid:?}"
            );
        }
    }

    #[test]
    fn prepared_gid_validation_is_strict_whitelist() {
        assert!(validate_prepared_gid("phase_c_gid-1.2:3").is_ok());
        assert!(validate_prepared_gid(&"a".repeat(MAX_PREPARED_GID_BYTES)).is_ok());

        for invalid in [
            "",
            "gid with space",
            "gid'; DROP TABLE t; --",
            "gid\"x",
            "gid\nx",
        ] {
            assert!(
                matches!(
                    validate_prepared_gid(invalid),
                    Err(DbError::InvalidArgument(_))
                ),
                "GID 应拒绝 {invalid:?}"
            );
        }
        assert!(matches!(
            validate_prepared_gid(&"a".repeat(MAX_PREPARED_GID_BYTES + 1)),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn lock_table_statement_quotes_table_and_uses_controlled_mode() {
        assert_eq!(
            lock_table_statement("phase_c.locks", TableLockMode::AccessExclusive).unwrap(),
            "LOCK TABLE \"phase_c\".\"locks\" IN ACCESS EXCLUSIVE MODE"
        );
        assert_eq!(
            lock_table_statement("locks", TableLockMode::ShareRowExclusive).unwrap(),
            "LOCK TABLE \"locks\" IN SHARE ROW EXCLUSIVE MODE"
        );
        assert!(matches!(
            lock_table_statement("locks; DROP TABLE t", TableLockMode::Share),
            Err(DbError::InvalidArgument(_))
        ));

        let modes = [
            (TableLockMode::AccessShare, "ACCESS SHARE"),
            (TableLockMode::RowShare, "ROW SHARE"),
            (TableLockMode::RowExclusive, "ROW EXCLUSIVE"),
            (
                TableLockMode::ShareUpdateExclusive,
                "SHARE UPDATE EXCLUSIVE",
            ),
            (TableLockMode::Share, "SHARE"),
            (TableLockMode::ShareRowExclusive, "SHARE ROW EXCLUSIVE"),
            (TableLockMode::Exclusive, "EXCLUSIVE"),
            (TableLockMode::AccessExclusive, "ACCESS EXCLUSIVE"),
        ];
        for (mode, expected) in modes {
            assert_eq!(mode.as_sql(), expected);
        }
    }
}
