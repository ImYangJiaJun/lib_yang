/// 事务 SELECT 的行锁模式。
///
/// 锁模式只能交给 MySQL/PostgreSQL 的 `Transaction::select_locked` 或便捷方法使用；
/// 普通 `QueryBuilder` 不暴露加锁入口，避免锁语句在自动提交连接上失去预期语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RowLock {
    /// 排他行锁：`FOR UPDATE`。
    ForUpdate,
    /// 共享行锁：`FOR SHARE`。
    ForShare,
    /// 排他行锁并跳过已锁定行：`FOR UPDATE SKIP LOCKED`。
    ForUpdateSkipLocked,
    /// 共享行锁并跳过已锁定行：`FOR SHARE SKIP LOCKED`。
    ForShareSkipLocked,
}

impl RowLock {
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub(crate) const fn as_sql(self) -> &'static str {
        match self {
            Self::ForUpdate => "FOR UPDATE",
            Self::ForShare => "FOR SHARE",
            Self::ForUpdateSkipLocked => "FOR UPDATE SKIP LOCKED",
            Self::ForShareSkipLocked => "FOR SHARE SKIP LOCKED",
        }
    }
}

#[cfg(all(test, any(feature = "mysql", feature = "postgres")))]
mod tests {
    use super::RowLock;

    #[test]
    fn skip_locked_modes_render_expected_sql() {
        assert_eq!(
            RowLock::ForUpdateSkipLocked.as_sql(),
            "FOR UPDATE SKIP LOCKED"
        );
        assert_eq!(
            RowLock::ForShareSkipLocked.as_sql(),
            "FOR SHARE SKIP LOCKED"
        );
        assert_eq!(RowLock::ForUpdate.as_sql(), "FOR UPDATE");
        assert_eq!(RowLock::ForShare.as_sql(), "FOR SHARE");
    }
}
