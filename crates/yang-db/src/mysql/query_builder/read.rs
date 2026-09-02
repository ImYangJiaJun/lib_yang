//! 读路径执行：`find` / `select` / 标量值读取。

use super::bind::{bind_param, bind_scalar_param};
use super::{QueryBuilder, QueryExecutor, SqlGenerator};

impl<'a> QueryBuilder<'a> {
    /// 查询单条记录
    ///
    /// 自动添加 LIMIT 1 到查询，返回单条记录或 None
    ///
    /// # 类型参数
    /// - T: 结果类型，必须实现 FromRow trait
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回单条记录
    /// - Ok(None): 查询成功，但没有匹配的记录
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let user: Option<User> = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
    ///     .find()
    ///     .await?;
    ///
    /// match user {
    ///     Some(u) => println!("找到用户: {:?}", u),
    ///     None => println!("用户不存在"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn find<T>(mut self) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 自动添加 LIMIT 1
        self.limit = Some(1);

        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 find() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query_as::<_, T>(sql);

        // 绑定参数
        for param in params {
            query = bind_param(query, param);
        }

        // pool 与事务共享同一份查询计划，只在最终执行器上分流。
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_optional(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_optional(&mut *connection).await
            }
        };

        match result {
            Ok(row) => {
                if self.enable_logging {
                    if row.is_some() {
                        log::debug!("find() 查询成功，返回 1 条记录");
                    } else {
                        log::debug!("find() 查询成功，未找到匹配记录");
                    }
                }
                Ok(row)
            }
            Err(e) => {
                log::error!("find() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询多条记录
    ///
    /// 执行 SELECT 查询并返回所有匹配的记录
    ///
    /// # 类型参数
    /// - T: 结果类型，必须实现 FromRow trait
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询成功，返回匹配的记录列表（可能为空）
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let users: Vec<User> = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
    ///     .order(yang_db::field!("name"), yang_db::SortOrder::Asc)
    ///     .select()
    ///     .await?;
    ///
    /// println!("找到 {} 个用户", users.len());
    /// for user in users {
    ///     println!("用户: {:?}", user);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn select<T>(self) -> Result<Vec<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 记录日志
        if self.enable_logging {
            log::debug!("执行 select() 查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // 构建查询
        let mut query = sqlx::query_as::<_, T>(sql);

        // 绑定参数
        for param in params {
            query = bind_param(query, param);
        }

        // pool 与事务共享同一份查询计划，只在最终执行器上分流。
        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_all(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_all(&mut *connection).await
            }
        };

        match result {
            Ok(rows) => {
                if self.enable_logging {
                    log::debug!("select() 查询成功，返回 {} 条记录", rows.len());
                }
                Ok(rows)
            }
            Err(e) => {
                log::error!("select() 查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 标量查询助手（内部）
    ///
    /// 统一 `value` / `sum` / `avg` / `min` / `max` 五个方法的样板：清空字段选择、
    /// 只选 `select_expr`、自动加 `LIMIT 1`、生成 SELECT、绑定参数、执行单行单值查询。
    ///
    /// 类型参数 `C` 为 sqlx 解码目标类型：`value` 传 `T`（NULL 列触发解码错误的语义保持不变），
    /// 聚合函数传 `Option<U>`（外层再 `Option::flatten`）。生成的 SQL 与参数与各方法原实现逐字节一致。
    pub(super) async fn fetch_scalar<C>(
        mut self,
        select_expr: &str,
    ) -> Result<Option<C>, crate::error::DbError>
    where
        C: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 清空现有字段选择，只选择指定表达式
        self.fields.clear();
        self.fields.push(select_expr.to_string());

        // 自动添加 LIMIT 1
        self.limit = Some(1);

        // 生成 SQL 语句
        let mut generator = SqlGenerator::new();
        generator.build_select(&self)?;

        let sql = generator.get_sql();
        let params = generator.get_params();

        if self.enable_logging {
            log::debug!("执行标量查询: {}", sql);
            log::debug!("参数: {:?}", params);
        }

        // query_scalar 直接获取单个值；bind_scalar_param 对任意输出类型通用
        let mut query = sqlx::query_scalar::<_, C>(sql);
        for param in params {
            query = bind_scalar_param(query, param);
        }

        let result = match self.executor {
            QueryExecutor::Pool(pool) => query.fetch_optional(pool).await,
            QueryExecutor::Transaction(transaction) => {
                let connection = transaction.executor().ok_or_else(|| {
                    crate::error::DbError::TransactionError("事务已提交或回滚".to_string())
                })?;
                query.fetch_optional(&mut *connection).await
            }
        };

        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                log::error!("标量查询失败: {}", e);
                Err(crate::error::DbError::from(e))
            }
        }
    }

    /// 查询单个字段值
    ///
    /// 执行 SELECT 查询并返回指定字段的单个值。自动添加 LIMIT 1 到查询。
    ///
    /// # 参数
    /// - field: 要查询的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回字段值
    /// - Ok(None): 查询成功，但没有匹配的记录
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询用户名
    /// let name: Option<String> = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
    ///     .value(yang_db::field!("name"))
    ///     .await?;
    ///
    /// match name {
    ///     Some(n) => println!("用户名: {}", n),
    ///     None => println!("用户不存在"),
    /// }
    ///
    /// // 查询用户 ID
    /// let user_id: Option<i64> = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
    ///     .value(yang_db::field!("id"))
    ///     .await?;
    ///
    /// println!("用户 ID: {:?}", user_id);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn value<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        if self.enable_logging {
            log::debug!("执行 value() 查询，字段: {}", field.as_str());
        }

        // 直接解码为 T（NULL 列触发解码错误的语义保持不变）
        // 验证需求: ID-1 — field 按设计接受 SQL 表达式，与 field() 一致；
        // 若字段来自不可信输入，调用方需先通过 quote_identifier 转义
        let field = crate::mysql::identifier::quote_identifier(field.as_str())?;
        self.fetch_scalar::<T>(&field).await
    }
}
