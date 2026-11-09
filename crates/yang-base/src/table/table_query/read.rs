//! 读取执行：select/all/optional/one/count/paginate 及事务内变体。
//!
//! 公共入口按连接池后端运行时分派：MySQL 分支保持原有 `yang_db::QueryBuilder`
//! 行为，PostgreSQL 分支由 `read_pg.rs` 提供（`postgres` feature）。

#![cfg(any(feature = "mysql", feature = "postgres"))]

use super::TableQuery;
use crate::database::AnyTransaction;
use crate::error::BaseError;

/// 数据库执行方法（需要启用 `mysql` 或 `postgres` feature）
impl TableQuery {
    /// 为分页查询补齐默认页码/每页大小并写回查询参数。
    pub(crate) fn with_effective_pagination(self) -> Result<(Self, usize, usize), BaseError> {
        let page = self.query_params.page.unwrap_or(1);
        let page_size = self
            .query_params
            .page_size
            .unwrap_or(crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE);
        let query = self.page(page, page_size)?;

        Ok((query, page, page_size))
    }

    /// 执行分页查询并返回 schema-first [`Record`](crate::table::Record)。
    ///
    /// 未显式设置分页时会使用默认页码和默认每页大小，并确保数据查询带有 `LIMIT/OFFSET`。
    pub async fn paginate_records(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        self.paginate().await
    }

    /// 执行分页查询（内部方法，供 [`TableQuery::paginate_records`] 使用）。
    pub(crate) async fn paginate(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self.paginate_mysql().await,
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => self.paginate_pg().await,
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL 分页实现：COUNT → LIMIT/OFFSET 数据查询。
    #[cfg(feature = "mysql")]
    async fn paginate_mysql(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        // 1. 检查数据库连接池是否存在
        self.pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 获取分页参数，如果未设置则使用默认值，并写回数据查询
        let (query, page, page_size) = self.with_effective_pagination()?;

        // 3. 执行 COUNT(*) 查询获取总记录数
        let total = query.count_internal().await?;

        // 4. 如果总记录数为 0，直接返回空结果
        if total == 0 {
            return Ok(crate::table::PaginatedResult::empty(page, page_size));
        }

        // 5. 执行数据查询
        let data = query.select_mysql().await?;

        // 6. 构建并返回 PaginatedResult
        Ok(crate::table::PaginatedResult::new(
            data, total, page, page_size,
        ))
    }

    /// PostgreSQL 分页实现：与 MySQL 相同的 COUNT + LIMIT/OFFSET 语义。
    #[cfg(feature = "postgres")]
    async fn paginate_pg(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        self.pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        let (query, page, page_size) = self.with_effective_pagination()?;

        let total = query.count_internal().await?;

        if total == 0 {
            return Ok(crate::table::PaginatedResult::empty(page, page_size));
        }

        let data = query.select_pg().await?;

        Ok(crate::table::PaginatedResult::new(
            data, total, page, page_size,
        ))
    }

    /// 执行 COUNT 查询获取总记录数（内部方法，供 paginate 使用）
    ///
    /// COUNT 会泄露表基数，因此与 SELECT 使用相同的可读权限守卫；分页只约束当前页
    /// 数据，不改变总记录数，因此先清除分页参数再计数。
    async fn count_internal(&self) -> Result<usize, BaseError> {
        self.ensure_readable_projection()?;
        let mut count_query = self.clone();
        count_query.query_params.page = None;
        count_query.query_params.page_size = None;

        match count_query.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => count_query.count_mysql().await,
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => count_query.count_pg().await,
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL COUNT 实现。
    #[cfg(feature = "mysql")]
    async fn count_mysql(&self) -> Result<usize, BaseError> {
        let count = self
            .compile_db_query()?
            .count()
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        usize::try_from(count).map_err(|_| {
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError(
                "COUNT 结果超出 usize 范围".to_string(),
            ))
        })
    }

    /// PostgreSQL COUNT 实现。
    ///
    /// 只应用 WHERE/软删守卫：PostgreSQL 的 `SELECT COUNT(*) ... ORDER BY 非分组列`
    /// 会被拒绝，且计数本身不需要投影、排序与分页。
    #[cfg(feature = "postgres")]
    async fn count_pg(&self) -> Result<usize, BaseError> {
        let database = self.pg_database()?;
        let count = self
            .apply_pg_where_guards(database.table(&self.table_config.table_ref))?
            .count()
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        usize::try_from(count).map_err(|_| {
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError(
                "COUNT 结果为负数，超出 usize 范围".to_string(),
            ))
        })
    }

    /// 执行 COUNT 查询获取总记录数
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：查询成功，返回总记录数
    /// - `Err(BaseError)`：查询失败，返回错误
    pub async fn count(self) -> Result<u64, BaseError> {
        self.count_internal().await.map(|n| n as u64)
    }

    /// 执行 SELECT 查询操作（内部方法）
    async fn select(self) -> Result<Vec<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self.select_mysql().await,
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => self.select_pg().await,
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL SELECT 实现。
    #[cfg(feature = "mysql")]
    async fn select_mysql(self) -> Result<Vec<crate::table::Record>, BaseError> {
        self.compile_db_query()?
            .select::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// PostgreSQL SELECT 实现（`Record` 实现 `PgRow` 解码）。
    #[cfg(feature = "postgres")]
    async fn select_pg(self) -> Result<Vec<crate::table::Record>, BaseError> {
        let database = self.pg_database()?;
        self.apply_pg_read_plan(database.table(&self.table_config.table_ref))?
            .select::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询全部匹配记录。
    pub async fn all(self) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select().await
    }

    /// 在事务中执行 SELECT 查询并返回多条记录
    ///
    /// 与 [`TableQuery::all`] 完全一致的建句与权限/软删逻辑，但在调用方提供的
    /// [`AnyTransaction`] 内执行，使「读-改-写」场景的读取与后续写入处于同一
    /// 事务、看到一致快照。
    pub(crate) async fn select_in_tx(
        self,
        tx: &mut AnyTransaction,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                self.apply_read_plan(query)?
                    .select::<crate::table::Record>()
                    .await
                    .map_err(BaseError::DatabaseQueryFailed)
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                self.apply_pg_read_plan(query)?
                    .select::<crate::table::Record>()
                    .await
                    .map_err(BaseError::DatabaseQueryFailed)
            }
        }
    }

    /// 在事务中查询全部匹配记录。
    pub async fn all_in_tx(
        self,
        tx: &mut AnyTransaction,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select_in_tx(tx).await
    }

    /// 执行查询并返回可选的单条记录（内部方法）
    async fn fetch_optional(self) -> Result<Option<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self.fetch_optional_mysql().await,
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => self.fetch_optional_pg().await,
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL 单条查询实现。
    #[cfg(feature = "mysql")]
    async fn fetch_optional_mysql(self) -> Result<Option<crate::table::Record>, BaseError> {
        self.compile_db_query()?
            .find::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// PostgreSQL 单条查询实现。
    #[cfg(feature = "postgres")]
    async fn fetch_optional_pg(self) -> Result<Option<crate::table::Record>, BaseError> {
        let database = self.pg_database()?;
        self.apply_pg_read_plan(database.table(&self.table_config.table_ref))?
            .find::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询可选单条记录。
    pub async fn optional(self) -> Result<Option<crate::table::Record>, BaseError> {
        self.fetch_optional().await
    }

    /// 查询单条记录；没有匹配记录时返回 [`BaseError::RecordNotFound`]。
    pub async fn one(self) -> Result<crate::table::Record, BaseError> {
        let table_name = self.table_config.table_name.clone();
        self.optional()
            .await?
            .ok_or_else(|| BaseError::RecordNotFound(format!("表 {table_name} 中没有匹配记录")))
    }
}
