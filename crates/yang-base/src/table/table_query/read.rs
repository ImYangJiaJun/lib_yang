//! 读取执行（`mysql` feature）：select/all/optional/one/count/paginate 及事务内变体。

#![cfg(feature = "mysql")]

use super::TableQuery;
use crate::error::BaseError;
use serde::Serialize;

/// 数据库执行方法（需要启用 `mysql` feature）
impl TableQuery {
    /// 执行分页查询操作
    ///
    /// 执行分页查询，包括以下步骤：
    /// 1. 执行 COUNT(*) 查询获取总记录数
    /// 2. 计算 LIMIT 和 OFFSET
    /// 3. 执行数据查询
    /// 4. 计算总页数
    /// 5. 构建并返回 PaginatedResult
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` 和 `Serialize` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(PaginatedResult<T>)`：查询成功，返回分页结果
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    pub(crate) fn with_effective_pagination(self) -> Result<(Self, usize, usize), BaseError> {
        let page = self.query_params.page.unwrap_or(1);
        let page_size = self
            .query_params
            .page_size
            .unwrap_or(crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE);
        let query = self.page(page, page_size)?;

        Ok((query, page, page_size))
    }

    /// 执行分页查询。
    ///
    /// 未显式设置分页时会使用默认页码和默认每页大小，并确保数据查询带有 `LIMIT/OFFSET`。
    pub(crate) async fn paginate<T>(self) -> Result<crate::table::PaginatedResult<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin + Serialize,
    {
        // 1. 检查数据库连接池是否存在
        let _pool = self
            .pool
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
        let data = query.select().await?;

        // 6. 构建并返回 PaginatedResult
        Ok(crate::table::PaginatedResult::new(
            data, total, page, page_size,
        ))
    }

    /// 执行分页查询并返回 schema-first [`Record`](crate::table::Record)。
    pub async fn paginate_records(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        self.paginate::<crate::table::Record>().await
    }

    /// 执行 COUNT 查询获取总记录数（内部方法，供 paginate 使用）
    ///
    /// 构建 COUNT(*) SQL 语句，应用已配置的 WHERE 条件，执行查询并返回总记录数。
    /// 返回 `usize` 以与 `PaginatedResult::new` 的 `total: usize` 参数直接匹配；
    /// 公开接口 `count()` 通过 `as u64` 适配。
    ///
    /// # 返回值
    ///
    /// - `Ok(usize)`：查询成功，返回总记录数
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    async fn count_internal(&self) -> Result<usize, BaseError> {
        // COUNT 会泄露表基数，因此与 SELECT 使用相同的可读权限守卫。
        self.ensure_readable_projection()?;
        // 分页只约束当前页数据，不应改变总记录数。尤其是 OFFSET > 0 时，
        // MySQL 会把 COUNT(*) 的唯一结果行跳过，进而把非空结果误判为 0。
        let mut count_query = self.clone();
        count_query.query_params.page = None;
        count_query.query_params.page_size = None;
        let count = count_query
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

    /// 执行 COUNT 查询获取总记录数
    ///
    /// 构建 COUNT(*) SQL 语句，应用已配置的 WHERE 条件，执行查询并返回总记录数。
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：查询成功，返回总记录数
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let total = query
    ///     .where_eq("status", serde_json::json!("active"))?
    ///     .count()
    ///     .await?;
    ///
    /// println!("总记录数: {}", total);
    /// ```
    pub async fn count(self) -> Result<u64, BaseError> {
        self.count_internal().await.map(|n| n as u64)
    }

    /// 执行 SELECT 查询操作
    ///
    /// 使用 sqlx 构建 SELECT 语句，应用已配置的字段选择、WHERE 条件和排序规则，
    /// 执行查询并将结果反序列化为指定的泛型类型 T。
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(Vec<T>)`：查询成功，返回结果列表
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    pub(crate) async fn select<T>(self) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.compile_db_query()?
            .select::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询全部匹配记录。
    pub async fn all(self) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select::<crate::table::Record>().await
    }

    /// 在事务中执行 SELECT 查询并返回多条记录
    ///
    /// 与 [`TableQuery::select`] 完全一致的建句与权限/软删逻辑，但在调用方提供的
    /// `yang_db::Transaction` 内执行，使「读-改-写」场景的读取与后续写入处于同一
    /// 事务、看到一致快照。
    ///
    /// # 参数
    ///
    /// - `tx`：由 [`ActionContext::begin_transaction`](crate::action::ActionContext::begin_transaction)
    ///   或 [`Tools`](crate::tools::Tools) 所有数据库实例创建的活动事务
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚，连接不可用
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    pub(crate) async fn select_in_tx<T>(
        self,
        tx: &mut yang_db::Transaction,
    ) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .select::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 在事务中查询全部匹配记录。
    pub async fn all_in_tx(
        self,
        tx: &mut yang_db::Transaction,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select_in_tx::<crate::table::Record>(tx).await
    }

    /// 执行查询并返回可选的单条记录
    ///
    /// 执行 SELECT 查询，返回第一条匹配记录，如果没有匹配记录则返回 None。
    /// 通常与 `where_eq` 等条件方法配合使用，用于按主键查询单条记录。
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(T))`：找到匹配记录
    /// - `Ok(None)`：没有匹配记录
    /// - `Err(BaseError)`：查询失败
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// // 按主键查询单条记录
    /// let row: Option<Record> = query
    ///     .where_eq("id", serde_json::json!(1))?
    ///     .fetch_optional()
    ///     .await?;
    ///
    /// match row {
    ///     Some(r) => println!("找到记录: {:?}", r.columns),
    ///     None => println!("记录不存在"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn fetch_optional<T>(self) -> Result<Option<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.compile_db_query()?
            .find::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询可选单条记录。
    pub async fn optional(self) -> Result<Option<crate::table::Record>, BaseError> {
        self.fetch_optional::<crate::table::Record>().await
    }

    /// 查询单条记录；没有匹配记录时返回 [`BaseError::RecordNotFound`]。
    pub async fn one(self) -> Result<crate::table::Record, BaseError> {
        let table_name = self.table_config.table_name.clone();
        self.optional()
            .await?
            .ok_or_else(|| BaseError::RecordNotFound(format!("表 {table_name} 中没有匹配记录")))
    }
}
