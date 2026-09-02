//! 聚合查询：`count` / `sum` / `avg` / `min` / `max`。

use super::QueryBuilder;

impl<'a> QueryBuilder<'a> {
    /// 统计记录数量
    ///
    /// 执行 COUNT(*) 查询并返回匹配条件的记录数量。
    ///
    /// # 返回
    /// - Ok(i64): 查询成功，返回记录数量
    /// - Err(DbError): 查询执行失败
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 统计所有用户数量
    /// let total_users = db.table(yang_db::table!("users"))
    ///     .count()
    ///     .await?;
    /// println!("总用户数: {}", total_users);
    ///
    /// // 统计活跃用户数量
    /// let active_users = db.table(yang_db::table!("users"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
    ///     .count()
    ///     .await?;
    /// println!("活跃用户数: {}", active_users);
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn count(self) -> Result<i64, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 count() 查询");
        }

        // 使用 value() 方法查询 COUNT(*)
        let result = self.fetch_scalar::<i64>("COUNT(*)").await?;

        // COUNT(*) 总是返回一个值（至少是 0），所以这里 unwrap_or(0) 是安全的
        Ok(result.unwrap_or(0))
    }

    /// 计算字段总和
    ///
    /// 执行 SUM(field) 查询并返回指定字段的总和。
    ///
    /// # 参数
    /// - field: 要求和的字段名
    ///
    /// # 返回
    /// - Ok(Some(f64)): 查询成功，返回字段总和
    /// - Ok(None): 查询成功，但没有匹配的记录或字段值全为 NULL
    /// - Err(DbError): 查询执行失败
    ///
    /// # 注意
    /// MySQL 的 SUM() 函数对于整数字段返回 DECIMAL 类型，对于浮点数字段返回 DOUBLE 类型。
    /// 本方法使用 CAST 将结果转换为 DOUBLE，以统一返回类型。
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 计算所有订单总金额
    /// let total_amount = db.table(yang_db::table!("orders"))
    ///     .sum(yang_db::field!("amount"))
    ///     .await?;
    ///
    /// match total_amount {
    ///     Some(sum) => println!("订单总金额: {:.2}", sum),
    ///     None => println!("没有订单或金额全为 NULL"),
    /// }
    ///
    /// // 计算已完成订单的总金额
    /// let completed_amount = db.table(yang_db::table!("orders"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, "completed")
    ///     .sum(yang_db::field!("amount"))
    ///     .await?;
    ///
    /// println!("已完成订单总金额: {:.2}", completed_amount.unwrap_or(0.0));
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn sum(self, field: &crate::FieldRef) -> Result<Option<f64>, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 sum() 查询，字段: {}", field.as_str());
        }

        // 构建 SUM(field) 表达式，并使用 CAST 转换为 DOUBLE
        // 这样可以统一处理整数和浮点数字段的求和结果
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = crate::mysql::identifier::quote_identifier(field.as_str())?;
        let sum_expr = format!("CAST(SUM({quoted_field}) AS DOUBLE)");

        // 用 Option<f64> 解码处理 NULL；fetch_scalar 外层 Option 表示有无行，
        // flatten 后与原 fetch_optional + match 的返回完全一致
        self.fetch_scalar::<Option<f64>>(&sum_expr)
            .await
            .map(Option::flatten)
    }

    /// 计算字段平均值
    ///
    /// 执行 AVG 聚合函数，计算指定字段的平均值。
    ///
    /// # 参数
    /// - field: 要计算平均值的字段名
    ///
    /// # 返回
    /// - Ok(Some(f64)): 计算成功，返回平均值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 只对数值类型字段有效
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与计算）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 计算所有用户的平均年龄
    /// let avg_age = db.table(yang_db::table!("users"))
    ///     .avg(yang_db::field!("age"))
    ///     .await?;
    ///
    /// if let Some(age) = avg_age {
    ///     println!("平均年龄: {:.1}", age);
    /// } else {
    ///     println!("没有数据");
    /// }
    ///
    /// // 计算已完成订单的平均金额
    /// let avg_amount = db.table(yang_db::table!("orders"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, "completed")
    ///     .avg(yang_db::field!("amount"))
    ///     .await?;
    ///
    /// println!("已完成订单平均金额: {:.2}", avg_amount.unwrap_or(0.0));
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn avg(self, field: &crate::FieldRef) -> Result<Option<f64>, crate::error::DbError> {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 avg() 查询，字段: {}", field.as_str());
        }

        // 构建 AVG(field) 表达式，并使用 CAST 转换为 DOUBLE
        // 这样可以统一处理整数和浮点数字段的平均值结果
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = crate::mysql::identifier::quote_identifier(field.as_str())?;
        let avg_expr = format!("CAST(AVG({quoted_field}) AS DOUBLE)");

        // 用 Option<f64> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<f64>>(&avg_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最小值
    ///
    /// 执行 MIN 聚合函数，获取指定字段的最小值。
    ///
    /// # 参数
    /// - field: 要查询最小值的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///   支持的类型包括：i32, i64, f32, f64, String, chrono::NaiveDateTime 等
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回最小值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 对数值类型字段返回数值最小值
    /// - 对字符串类型字段返回字典序最小值
    /// - 对日期时间类型字段返回最早时间
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与比较）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询最低价格（浮点数）
    /// let min_price: Option<f64> = db.table(yang_db::table!("products"))
    ///     .min(yang_db::field!("price"))
    ///     .await?;
    ///
    /// if let Some(price) = min_price {
    ///     println!("最低价格: {:.2}", price);
    /// } else {
    ///     println!("没有产品数据");
    /// }
    ///
    /// // 查询最小库存数量（整数）
    /// let min_stock: Option<i32> = db.table(yang_db::table!("products"))
    ///     .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
    ///     .min(yang_db::field!("stock"))
    ///     .await?;
    ///
    /// println!("最小库存: {}", min_stock.unwrap_or(0));
    ///
    /// // 查询最早注册时间（字符串）
    /// let earliest_date: Option<String> = db.table(yang_db::table!("users"))
    ///     .min(yang_db::field!("created_at"))
    ///     .await?;
    ///
    /// if let Some(date) = earliest_date {
    ///     println!("最早注册时间: {}", date);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn min<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 min() 查询，字段: {}", field.as_str());
        }

        // 构建 MIN(field) 表达式
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = crate::mysql::identifier::quote_identifier(field.as_str())?;
        let min_expr = format!("MIN({quoted_field})");

        // 用 Option<T> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<T>>(&min_expr)
            .await
            .map(Option::flatten)
    }

    /// 获取字段最大值
    ///
    /// 执行 MAX 聚合函数，获取指定字段的最大值。
    ///
    /// # 参数
    /// - field: 要查询最大值的字段名
    ///
    /// # 类型参数
    /// - T: 字段值类型，必须实现 sqlx::Decode 和 sqlx::Type trait
    ///   支持的类型包括：i32, i64, f32, f64, String, chrono::NaiveDateTime 等
    ///
    /// # 返回
    /// - Ok(Some(T)): 查询成功，返回最大值
    /// - Ok(None): 没有匹配记录或所有字段值为 NULL
    /// - Err(DbError): 查询失败
    ///
    /// # 注意
    /// - 对数值类型字段返回数值最大值
    /// - 对字符串类型字段返回字典序最大值
    /// - 对日期时间类型字段返回最晚时间
    /// - 空结果集返回 None
    /// - NULL 值会被忽略（不参与比较）
    /// - 可以与 WHERE 条件组合使用
    /// - 可以与 GROUP BY 组合使用
    ///
    /// # 示例
    /// ```no_run
    /// use yang_db::Database;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 查询最高价格（浮点数）
    /// let max_price: Option<f64> = db.table(yang_db::table!("products"))
    ///     .max(yang_db::field!("price"))
    ///     .await?;
    ///
    /// if let Some(price) = max_price {
    ///     println!("最高价格: {:.2}", price);
    /// } else {
    ///     println!("没有产品数据");
    /// }
    ///
    /// // 查询最高分数（整数）
    /// let max_score: Option<i32> = db.table(yang_db::table!("scores"))
    ///     .where_and(yang_db::field!("exam_id"), yang_db::CompareOp::Eq, 1)
    ///     .max(yang_db::field!("score"))
    ///     .await?;
    ///
    /// println!("最高分: {}", max_score.unwrap_or(0));
    ///
    /// // 查询最新更新时间（字符串）
    /// let latest_date: Option<String> = db.table(yang_db::table!("articles"))
    ///     .max(yang_db::field!("updated_at"))
    ///     .await?;
    ///
    /// if let Some(date) = latest_date {
    ///     println!("最新更新时间: {}", date);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[tracing::instrument(
        name = "db.query",
        skip_all,
        fields(db.system = "mysql", db.operation = "select", db.collection = %self.table, otel.kind = "client")
    )]
    pub async fn max<T>(self, field: &crate::FieldRef) -> Result<Option<T>, crate::error::DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
    {
        // 记录日志
        if self.enable_logging {
            log::debug!("执行 max() 查询，字段: {}", field.as_str());
        }

        // 构建 MAX(field) 表达式
        // 验证需求: ID-1 — 聚合方法对标识符做转义，杜绝注入
        let quoted_field = crate::mysql::identifier::quote_identifier(field.as_str())?;
        let max_expr = format!("MAX({quoted_field})");

        // 用 Option<T> 解码处理 NULL；flatten 后与原实现返回完全一致
        self.fetch_scalar::<Option<T>>(&max_expr)
            .await
            .map(Option::flatten)
    }
}
