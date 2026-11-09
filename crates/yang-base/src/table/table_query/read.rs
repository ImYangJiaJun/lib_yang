//! 读取执行：select/all/optional/one/count/paginate 及事务内变体。
//!
//! 公共入口按连接池后端运行时分派：MySQL 分支保持原有 `yang_db::QueryBuilder`
//! 行为，PostgreSQL 分支由 `read_pg.rs` 提供（`postgres` feature）。

#![cfg(any(feature = "mysql", feature = "postgres"))]

use super::TableQuery;
use crate::database::AnyTransaction;
use crate::error::BaseError;

/// 分页窗口计数在结果行中的别名；读取后从 `Record` 中剥离。
#[cfg(feature = "postgres")]
const TOTAL_COUNT_ALIAS: &str = "__yang_total";

#[cfg(feature = "postgres")]
fn strip_total_count(mut record: crate::table::Record) -> crate::table::Record {
    // 不能用 into_map() + Record::from 重建：那会丢弃 JSON null sidecar。
    record.remove_column(TOTAL_COUNT_ALIAS);
    record
}

/// 数据库执行方法（需要启用 `mysql` 或 `postgres` feature）
impl TableQuery {
    /// 为分页查询补齐默认页码/每页大小并写回查询参数。
    ///
    /// 同时为分页结果追加主键 tiebreaker：非唯一排序键（或默认排序）下，仅靠
    /// 业务排序无法保证跨页稳定，主键提供确定性的最终排序。
    pub(crate) fn with_effective_pagination(self) -> Result<(Self, usize, usize), BaseError> {
        let page = self.query_params.page.unwrap_or(1);
        let page_size = self
            .query_params
            .page_size
            .unwrap_or(crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE);
        let mut query = self.page(page, page_size)?;

        // 未显式排序时沿用表定义默认排序，再追加主键。
        if query.query_params.order_by.is_empty() {
            query.query_params.order_by = query.table_config.default_order.clone();
        }
        let primary_key = query.table_config.primary_key.clone();
        if !query
            .query_params
            .order_by
            .iter()
            .any(|(field, _)| field == &primary_key)
        {
            query
                .query_params
                .order_by
                .push((primary_key, crate::table::SortOrder::Asc));
        }

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
            other => Err(BaseError::UnsupportedCapability(format!(
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

    /// PostgreSQL 分页实现：单查询窗口 `COUNT(*) OVER()` 同时返回当前页与总数。
    ///
    /// 窗口计数在同一条语句内完成，`total` 与 `items` 天然处于同一快照；仅当
    /// 当前页为空（OFFSET 越界）无法从结果行取到 total 时，才回退到额外 COUNT。
    #[cfg(feature = "postgres")]
    async fn paginate_pg(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        self.pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        let (query, page, page_size) = self.with_effective_pagination()?;
        let database = query.pg_database()?;
        let total_alias = yang_db::FieldRef::new(TOTAL_COUNT_ALIAS)
            .map_err(|error| BaseError::ConfigError(format!("窗口计数别名不合法: {error}")))?;
        let builder = query
            .apply_pg_read_plan(database.table(&query.table_config.table_ref))?
            .window_expr(
                yang_db::postgres::query_builder::WindowFunction::count_all(),
                yang_db::postgres::query_builder::WindowSpec::new(),
                &total_alias,
            );
        let rows = builder
            .select::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;

        let total = rows
            .first()
            .and_then(|row| row.get(TOTAL_COUNT_ALIAS))
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as usize);
        let data = rows.into_iter().map(strip_total_count).collect();
        let total = match total {
            Some(total) => total,
            // OFFSET 越界时结果为空，窗口计数也随之丢失，回退到 COUNT。
            None => query.count_internal().await?,
        };

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
            other => Err(BaseError::UnsupportedCapability(format!(
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

    /// 判断是否存在至少一条匹配记录（`SELECT 1 ... LIMIT 1` 语义）。
    ///
    /// 与 `count() == 0` 相比不扫描全部匹配行，适合权限/去重前置判断。
    pub async fn exists(self) -> Result<bool, BaseError> {
        // 与 COUNT 一样，存在性也会泄露表基数，必须经过同一可读权限守卫。
        self.ensure_readable_projection()?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => Ok(self
                .compile_db_query()?
                .limit(1)
                .find::<crate::table::Record>()
                .await
                .map_err(BaseError::DatabaseQueryFailed)?
                .is_some()),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let database = self.pg_database()?;
                let primary_key = self.table_config.primary_key.clone();
                let key = self.field_ref(&primary_key)?;
                let query = self
                    .apply_pg_where_guards(database.table(&self.table_config.table_ref))?
                    .field(key);
                Ok(query
                    .limit(1)
                    .find::<crate::table::Record>()
                    .await
                    .map_err(BaseError::DatabaseQueryFailed)?
                    .is_some())
            }
            other => Err(BaseError::UnsupportedCapability(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 执行 SELECT 查询操作（内部方法）
    async fn select(self) -> Result<Vec<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self.select_mysql().await,
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => self.select_pg().await,
            other => Err(BaseError::UnsupportedCapability(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL SELECT 实现。
    #[cfg(feature = "mysql")]
    async fn select_mysql(self) -> Result<Vec<crate::table::Record>, BaseError> {
        let query = if self.nulls_orders.is_empty() {
            self.compile_db_query()?
        } else {
            self.compile_mysql_query_with_nulls()?
        };
        query
            .select::<crate::table::Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// MySQL 读计划（含 NULLS FIRST/LAST 模拟）。
    ///
    /// `yang-db` 的排序入口无法表达 `field IS NULL` 表达式，因此仅在有 nulls 排序
    /// 需求时重建一次读计划，与 [`super::plan::TableQuery::apply_read_plan`] 的
    /// 投影/守卫/分页语义保持一致。
    #[cfg(feature = "mysql")]
    fn compile_mysql_query_with_nulls(&self) -> Result<yang_db::QueryBuilder<'_>, BaseError> {
        let pool = self
            .pool
            .as_ref()
            .and_then(|pool| pool.as_mysql())
            .ok_or(BaseError::DatabaseNotInitialized)?;
        let mut query = yang_db::QueryBuilder::from_pool(pool, &self.table_config.table_ref);
        let selected = self.query_params.fields.as_ref().map_or_else(
            || {
                self.default_read_fields()
                    .map(|values| values.into_iter().map(str::to_string).collect())
            },
            |values| Ok(values.clone()),
        )?;
        for name in selected {
            query = query.field(self.field_ref(&name)?);
        }
        for condition in &self.query_params.where_conditions {
            query = query
                .where_predicate(&self.compile_predicate(condition)?)
                .map_err(BaseError::DatabaseQueryFailed)?;
        }
        if !self.include_trashed {
            if let Some(name) = &self.table_config.soft_delete_field {
                query = query.where_null(self.field_ref(name)?);
            }
        }
        let orders = if self.query_params.order_by.is_empty() {
            &self.table_config.default_order
        } else {
            &self.query_params.order_by
        };
        for (name, order) in orders {
            let db_order = match order {
                crate::table::SortOrder::Asc => yang_db::SortOrder::Asc,
                crate::table::SortOrder::Desc => yang_db::SortOrder::Desc,
            };
            if let Some(nulls_first) = self.nulls_orders.get(name) {
                // MySQL 没有 NULLS 关键字：`field IS NULL`（1/0）前置到主排序键。
                let nulls_dir = if *nulls_first { "DESC" } else { "ASC" };
                let expr = yang_db::FieldRef::__from_validated_owned(format!(
                    "{name}` IS NULL {nulls_dir}, `{name}"
                ));
                query = query.order(&expr, db_order);
            } else {
                query = query.order(self.field_ref(name)?, db_order);
            }
        }
        if let Some(page_size) = self.query_params.page_size {
            let page = self.query_params.page.unwrap_or(1).max(1);
            let limit = u64::try_from(page_size).map_err(|_| {
                BaseError::ParamInvalid("page_size".to_string(), "分页大小超出范围".to_string())
            })?;
            let offset = u64::try_from((page - 1).saturating_mul(page_size)).map_err(|_| {
                BaseError::ParamInvalid("page".to_string(), "分页偏移超出范围".to_string())
            })?;
            query = query.limit(limit).offset(offset);
        }
        Ok(query)
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
            other => Err(BaseError::UnsupportedCapability(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// MySQL 单条查询实现。
    #[cfg(feature = "mysql")]
    async fn fetch_optional_mysql(self) -> Result<Option<crate::table::Record>, BaseError> {
        let query = if self.nulls_orders.is_empty() {
            self.compile_db_query()?
        } else {
            self.compile_mysql_query_with_nulls()?
        };
        query
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

#[cfg(all(test, feature = "mysql"))]
mod tests {
    use super::*;
    use crate::database::AnyPool;
    use crate::table::{Field, SortOrder, Table};
    use std::sync::Arc;

    fn lazy_mysql_query(table: Table) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        let pool = sqlx::MySqlPool::connect_lazy("mysql://root@127.0.0.1:3306/test")
            .expect("惰性 MySQL 连接池应创建成功");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(vec!["admin".to_string()]),
            Some(AnyPool::MySql(Arc::new(pool))),
        )
    }

    /// MySQL 没有 `NULLS FIRST/LAST`：用 `field IS NULL` 排序表达式模拟。
    #[tokio::test]
    async fn mysql_nulls_ordering_uses_is_null_expression() {
        let nulls_last = lazy_mysql_query(
            Table::new("tq_mysql_nulls").fields([Field::id("id"), Field::integer("bucket")]),
        )
        .order_nulls("bucket", SortOrder::Asc, false)
        .expect("order_nulls 应通过校验")
        .page(1, 10)
        .expect("分页参数应合法");
        let sql = nulls_last
            .compile_mysql_query_with_nulls()
            .expect("MySQL 读计划应编译成功")
            .try_to_sql()
            .expect("MySQL 读计划应能渲染 SQL");
        assert!(
            sql.contains("`bucket` IS NULL ASC, `bucket` ASC"),
            "NULLS LAST 模拟缺失: {sql}"
        );

        let nulls_first = lazy_mysql_query(
            Table::new("tq_mysql_nulls2").fields([Field::id("id"), Field::integer("bucket")]),
        )
        .order_nulls("bucket", SortOrder::Desc, true)
        .expect("order_nulls 应通过校验")
        .page(1, 10)
        .expect("分页参数应合法");
        let sql = nulls_first
            .compile_mysql_query_with_nulls()
            .expect("MySQL 读计划应编译成功")
            .try_to_sql()
            .expect("MySQL 读计划应能渲染 SQL");
        assert!(
            sql.contains("`bucket` IS NULL DESC, `bucket` DESC"),
            "NULLS FIRST 模拟缺失: {sql}"
        );
    }
}
