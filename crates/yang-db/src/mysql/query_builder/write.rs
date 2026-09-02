//! 写路径执行：`insert` / `update` / `delete` / `upsert` / 批量与原子增减。

use std::collections::HashMap;

use crate::mysql::field::FieldType;

use super::bind::bind_execute_param;
use super::{ArithmeticOperator, QueryBuilder, QueryExecutor, SqlGenerator};

/// 批量插入的默认批次大小
///
/// 为了避免单次插入过多数据导致 SQL 语句过大或超时，
/// 批量插入操作会自动将数据分批处理，每批最多插入 INSERT_BATCH_SIZE 条记录。
const INSERT_BATCH_SIZE: usize = 500;

/// 批量更新的默认批次大小
const UPDATE_BATCH_SIZE: usize = 1000;

impl<'a> QueryBuilder<'a> {
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
    /// let user_id = db.table(yang_db::table!("users"))
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
    /// let order_id = db.table(yang_db::table!("orders"))
    ///     .json(yang_db::field!("items"))  // 标记 items 字段为 JSON 类型
    ///     .insert(&order_data)
    ///     .await?;
    ///
    /// println!("订单插入成功，订单 ID: {}", order_id);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
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
        generator.build_insert(
            &self.table,
            &json_data,
            &self.field_types,
            &self.expr_assignments,
        )?;

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
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };

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

    /// 插入单条数据并显式返回 MySQL 自增主键（`LAST_INSERT_ID()`）。
    ///
    /// 语义与 [`Self::insert`] 的返回值一致（`insert` 历来返回自增 ID 而非受影响
    /// 行数），本方法以更明确的名字供「插入后立刻拿主键」的场景使用（如唯一哨兵
    /// 的原子 claim）。表没有 AUTO_INCREMENT 列时返回 0。
    ///
    /// # 示例
    /// ```no_run
    /// # use yang_db::Database;
    /// # use serde_json::json;
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// # let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let id = db.table(yang_db::table!("users"))
    ///     .insert_returning_id(&json!({"name": "张三"}))
    ///     .await?;
    /// assert!(id > 0);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn insert_returning_id<T>(self, data: &T) -> Result<u64, crate::error::DbError>
    where
        T: serde::Serialize,
    {
        self.insert(data).await
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
    /// let affected_rows = db.table(yang_db::table!("users"))
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
    /// let affected_rows = db.table(yang_db::table!("orders"))
    ///     .json(yang_db::field!("items"))  // 标记 items 字段为 JSON 类型
    ///     .insert_batch(&orders)
    ///     .await?;
    ///
    /// println!("批量插入订单成功，影响 {} 行", affected_rows);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
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
    /// let affected_rows = db.table(yang_db::table!("users"))
    ///     .insert_batch_with_size(&users, 100)
    ///     .await?;
    ///
    /// println!("批量插入成功，影响 {} 行", affected_rows);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "insert", db.collection = %self.table, otel.kind = "client")
    )]
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

        // 单批次：直接走 pool 执行，免事务开销。
        // 多批次：用单个事务包裹所有 chunk，任一批失败整体回滚（DB-4，比照 update_batch；
        // 此前每 chunk 独立 execute(pool)，第 N 批失败时前 N-1 批已落库无法回滚）。
        // PERF-12: 用 div_ceil 计算批次数，避免仅为取 len 而 collect 整个 Vec。
        let chunk_count = data.len().div_ceil(batch_size);
        if chunk_count == 1 {
            let enable_logging = self.enable_logging;
            let affected = self.insert_chunk(data).await?;
            if enable_logging {
                log::debug!("insert_batch_with_size() 单批完成，影响 {} 行", affected);
            }
            return Ok(affected);
        }

        let mut total_affected = 0u64;
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for (batch_index, chunk) in data.chunks(batch_size).enumerate() {
                    let result = execute_insert_chunk(
                        &mut transaction,
                        &self.table,
                        chunk,
                        &self.field_types,
                    )
                    .await?;
                    total_affected += result;
                    if self.enable_logging {
                        log::debug!("第 {} 批插入成功，影响 {} 行", batch_index + 1, result);
                    }
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for (batch_index, chunk) in data.chunks(batch_size).enumerate() {
                    let result = execute_insert_chunk(
                        &mut *connection,
                        &self.table,
                        chunk,
                        &self.field_types,
                    )
                    .await?;
                    total_affected += result;
                    if self.enable_logging {
                        log::debug!("第 {} 批插入成功，影响 {} 行", batch_index + 1, result);
                    }
                }
            }
        }

        if self.enable_logging {
            log::debug!(
                "insert_batch_with_size() 多批事务提交完成，总共影响 {} 行",
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
    async fn insert_chunk<T>(self, data: &[T]) -> Result<u64, crate::error::DbError>
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
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };

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
    /// let rows_affected = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
    ///     .update(&update_data)
    ///     .await?;
    ///
    /// println!("更新了 {} 行数据", rows_affected);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
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
        generator.build_update(
            &self.table,
            &json_data,
            &self.field_types,
            &self.conditions,
            &self.expr_assignments,
        )?;

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
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };

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

    /// 原子增加字段值；增量使用绑定参数，且必须提供 WHERE。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn increment(
        self,
        field: &crate::FieldRef,
        amount: i64,
    ) -> Result<u64, crate::error::DbError> {
        self.execute_arithmetic_update(field, amount, ArithmeticOperator::Add)
            .await
    }

    /// 原子减少字段值；增量使用绑定参数，且必须提供 WHERE。
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn decrement(
        self,
        field: &crate::FieldRef,
        amount: i64,
    ) -> Result<u64, crate::error::DbError> {
        self.execute_arithmetic_update(field, amount, ArithmeticOperator::Subtract)
            .await
    }

    async fn execute_arithmetic_update(
        self,
        field: &crate::FieldRef,
        amount: i64,
        operator: ArithmeticOperator,
    ) -> Result<u64, crate::error::DbError> {
        let mut generator = SqlGenerator::new();
        generator.build_arithmetic_update(
            &self.table,
            field.as_str(),
            operator,
            amount,
            &self.conditions,
        )?;
        let mut query = sqlx::query(generator.get_sql());
        for param in generator.get_params() {
            query = bind_execute_param(query, param);
        }
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        }?;
        Ok(result.rows_affected())
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
    /// let rows_affected = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
    ///     .delete()
    ///     .await?;
    ///
    /// println!("删除了 {} 行数据", rows_affected);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "delete", db.collection = %self.table, otel.kind = "client")
    )]
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
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        };

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
    /// let affected = db.table(yang_db::table!("users"))
    ///     .update_batch(&records, yang_db::field!("id"))
    ///     .await?;
    /// println!("批量更新了 {} 行", affected);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "update", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn update_batch<T>(
        self,
        records: &[T],
        where_field: &crate::FieldRef,
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

        let mut total = 0u64;
        match self.executor {
            QueryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await.map_err(crate::error::DbError::from)?;
                for chunk in json_records.chunks(UPDATE_BATCH_SIZE) {
                    total += execute_update_chunk(
                        &mut transaction,
                        &self.table,
                        chunk,
                        where_field,
                        &self.field_types,
                    )
                    .await?;
                }
                transaction
                    .commit()
                    .await
                    .map_err(crate::error::DbError::from)?;
            }
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                for chunk in json_records.chunks(UPDATE_BATCH_SIZE) {
                    total += execute_update_chunk(
                        &mut *connection,
                        &self.table,
                        chunk,
                        where_field,
                        &self.field_types,
                    )
                    .await?;
                }
            }
        }
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
    /// let rows = db.table(yang_db::table!("users")).upsert(&data).await?;
    /// if rows == 1 {
    ///     println!("新插入记录");
    /// } else if rows == 2 {
    ///     println!("更新了已有记录");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "upsert", db.collection = %self.table, otel.kind = "client")
    )]
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

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.execute(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.execute(&mut *connection).await
            }
        }
        .map_err(crate::error::DbError::from)?;
        let rows = result.rows_affected();

        if self.enable_logging {
            log::debug!("upsert() 完成，rows_affected: {}", rows);
        }

        Ok(rows)
    }
}

async fn execute_insert_chunk<T>(
    connection: &mut sqlx::MySqlConnection,
    table: &str,
    data: &[T],
    field_types: &HashMap<String, FieldType>,
) -> Result<u64, crate::error::DbError>
where
    T: serde::Serialize,
{
    let json_data_list = data
        .iter()
        .map(|item| {
            serde_json::to_value(item).map_err(|error| {
                crate::error::DbError::SerializationError(format!("数据序列化失败: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut generator = SqlGenerator::new();
    generator.build_insert_batch(table, &json_data_list, field_types)?;
    let mut query = sqlx::query(generator.get_sql());
    for param in generator.get_params() {
        query = bind_execute_param(query, param);
    }
    Ok(query.execute(connection).await?.rows_affected())
}

async fn execute_update_chunk(
    connection: &mut sqlx::MySqlConnection,
    table: &str,
    records: &[serde_json::Value],
    where_field: &crate::FieldRef,
    field_types: &HashMap<String, FieldType>,
) -> Result<u64, crate::error::DbError> {
    let mut generator = SqlGenerator::new();
    generator.build_update_batch(table, records, where_field.as_str(), field_types)?;
    let mut query = sqlx::query(generator.get_sql());
    for param in generator.get_params() {
        query = bind_execute_param(query, param);
    }
    Ok(query.execute(connection).await?.rows_affected())
}
