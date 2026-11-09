//! SQL 渲染入口：`to_sql` / `try_to_sql` / 事务行锁渲染。

use crate::mysql::condition::SqlValue;

use super::{QueryBuilder, SqlGenerator};

/// MySQL 只支持标准 `FOR UPDATE` / `FOR SHARE` 及其 `SKIP LOCKED` 变体。
///
/// PostgreSQL 专属的 `NoKeyUpdate` / `KeyShare` 必须在客户端拒绝：若原样下发，
/// MySQL 会把 `FOR NO KEY UPDATE` / `FOR KEY SHARE` 判为语法错误。
fn ensure_mysql_row_lock(lock: crate::RowLock) -> Result<(), crate::error::DbError> {
    match lock {
        crate::RowLock::NoKeyUpdate | crate::RowLock::KeyShare => {
            Err(crate::error::DbError::UnsupportedOperator(format!(
                "MySQL 不支持行锁模式: {}",
                lock.as_sql()
            )))
        }
        _ => Ok(()),
    }
}

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
            ensure_mysql_row_lock(lock)?;
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{ensure_mysql_row_lock, QueryBuilder};
    use crate::error::DbError;

    /// 懒连接池（仅解析 URL，不建立真实连接）；部分 SQL 渲染单元测试共用。
    fn make_sync_test_pool() -> &'static sqlx::mysql::MySqlPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<sqlx::mysql::MySqlPool> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            rt.block_on(async {
                sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("mysql://root:111111@localhost:3306/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    #[test]
    fn postgres_only_row_locks_are_rejected_for_mysql() {
        for lock in [crate::RowLock::NoKeyUpdate, crate::RowLock::KeyShare] {
            match ensure_mysql_row_lock(lock) {
                Err(DbError::UnsupportedOperator(message)) => {
                    assert!(
                        message.contains(lock.as_sql()),
                        "错误消息应指出被拒绝的行锁: {message}"
                    );
                }
                other => panic!("{lock:?} 在 MySQL 下必须返回 UnsupportedOperator: {other:?}"),
            }
        }
    }

    #[test]
    fn standard_row_locks_still_render_for_mysql() {
        for (lock, expected) in [
            (crate::RowLock::ForUpdate, "FOR UPDATE"),
            (crate::RowLock::ForShare, "FOR SHARE"),
            (
                crate::RowLock::ForUpdateSkipLocked,
                "FOR UPDATE SKIP LOCKED",
            ),
            (crate::RowLock::ForShareSkipLocked, "FOR SHARE SKIP LOCKED"),
        ] {
            let builder = QueryBuilder::new(make_sync_test_pool(), "accounts", false)
                .field(crate::field!("id"));
            let (sql, _) = builder
                .render_for_transaction(Some(lock))
                .expect("MySQL 标准行锁应可渲染");
            assert!(sql.ends_with(expected), "{lock:?} 渲染结果异常: {sql}");
        }
    }
}
