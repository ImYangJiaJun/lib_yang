//! 事务隔离级别（NG-2）。
//!
//! yang-db 的 `transaction()` 此前裸 `pool.begin()`，使用数据库默认隔离级别
//! （MySQL InnoDB 默认 REPEATABLE READ、PostgreSQL 默认 READ COMMITTED），无法按需调整，
//! 跨后端差异静默隐埋竞态。本枚举 + `transaction_with_isolation` 入口提供显式配置。

/// 事务隔离级别（SQL 标准四级）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IsolationLevel {
    /// 读未提交（最低）
    ReadUncommitted,
    /// 读已提交（PostgreSQL 默认）
    ReadCommitted,
    /// 可重复读（MySQL InnoDB 默认）
    RepeatableRead,
    /// 串行化（最高）
    Serializable,
}

impl IsolationLevel {
    /// 级别名（不含方言前缀），用于拼 `SET TRANSACTION ISOLATION LEVEL <name>`。
    ///
    /// 返回 `&'static str` 字面量，**不含**任何外部输入，拼接安全。
    pub fn as_sql(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_sql() {
        assert_eq!(IsolationLevel::ReadUncommitted.as_sql(), "READ UNCOMMITTED");
        assert_eq!(IsolationLevel::ReadCommitted.as_sql(), "READ COMMITTED");
        assert_eq!(IsolationLevel::RepeatableRead.as_sql(), "REPEATABLE READ");
        assert_eq!(IsolationLevel::Serializable.as_sql(), "SERIALIZABLE");
    }
}
