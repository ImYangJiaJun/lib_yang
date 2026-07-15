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
}

impl RowLock {
    pub(crate) const fn as_sql(self) -> &'static str {
        match self {
            Self::ForUpdate => "FOR UPDATE",
            Self::ForShare => "FOR SHARE",
        }
    }
}
