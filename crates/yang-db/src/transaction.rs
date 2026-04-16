use crate::error::DbError;
use sqlx::Transaction as SqlxTransaction;

/// 数据库事务
pub struct Transaction {
    tx: Option<SqlxTransaction<'static, sqlx::MySql>>,
    enable_logging: bool,
}

impl Transaction {
    /// 创建新的事务实例
    pub(crate) fn new(tx: SqlxTransaction<'static, sqlx::MySql>, enable_logging: bool) -> Self {
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
}
