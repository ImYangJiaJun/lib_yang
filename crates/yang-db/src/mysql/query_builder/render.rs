//! SQL 渲染入口：`to_sql` / `try_to_sql` / 事务行锁渲染。

use crate::mysql::condition::SqlValue;

use super::{QueryBuilder, SqlGenerator};

impl<'a> QueryBuilder<'a> {
    /// 尝试获取生成的 SQL（用于调试）。
    ///
    /// 与 [`Self::to_sql`] 不同，本方法不会吞掉 SQL 生成错误。调用方需要区分非法表名、
    /// 缺少 `GROUP BY` 等生产级配置错误时，应优先使用本方法。
    ///
    /// # 返回
    /// - `Ok(String)`: 生成的完整 SQL 语句字符串
    /// - `Err(DbError)`: SQL 生成失败的真实原因
    pub fn try_to_sql(&self) -> Result<String, crate::error::DbError> {
        let mut generator = SqlGenerator::new();
        generator.build_select(self)?;
        Ok(generator.get_sql().to_string())
    }

    pub(crate) fn render_for_transaction(
        &self,
        lock: Option<crate::RowLock>,
    ) -> Result<(String, Vec<SqlValue>), crate::error::DbError> {
        if lock.is_some()
            && (self.distinct
                || !self.group_by.is_empty()
                || !self.having_clause.is_empty()
                || !self.unions.is_empty())
        {
            return Err(crate::error::DbError::InvalidArgument(
                "行锁不支持 DISTINCT、GROUP BY、HAVING 或 UNION 查询".to_string(),
            ));
        }
        let mut generator = SqlGenerator::new();
        generator.build_select(self)?;
        if let Some(lock) = lock {
            generator.sql.push(' ');
            generator.sql.push_str(lock.as_sql());
        }
        Ok((generator.sql, generator.params))
    }

    /// 获取生成的 SQL（用于调试）
    ///
    /// # 返回
    /// - 生成的完整 SQL 语句字符串
    ///
    /// # 说明
    /// 兼容历史 `String` 返回值；生成失败时返回固定的不可执行哨兵，避免旧降级逻辑把
    /// 未校验表名或不完整查询拼成看似可执行的 SQL。需要错误细节请使用 [`Self::try_to_sql`]。
    pub fn to_sql(&self) -> String {
        match self.try_to_sql() {
            Ok(sql) => sql,
            Err(err) => {
                if self.enable_logging {
                    log::warn!("生成 SELECT SQL 失败: {err}");
                }
                "/* SQL generation failed */".to_string()
            }
        }
    }
}
