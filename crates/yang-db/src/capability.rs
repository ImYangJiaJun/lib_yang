//! 后端能力与安全约束的单一事实源。

/// 后端类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackendKind {
    /// MySQL 关系数据库。
    MySql,
    /// PostgreSQL 关系数据库。
    PostgreSql,
    /// Redis 键值数据库。
    Redis,
}

/// SQL 占位符风格；非关系后端使用 `NotApplicable`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlaceholderStyle {
    /// MySQL 的 `?` 占位符。
    QuestionMark,
    /// PostgreSQL 的 `$1`、`$2` 编号占位符。
    NumberedDollar,
    /// 后端不使用 SQL 占位符。
    NotApplicable,
}

/// 可被调用方查询的后端能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackendCapability {
    /// checked identifier QueryBuilder API。
    CheckedIdentifiers,
    /// SELECT/find/value 查询。
    Select,
    /// count/sum/avg/min/max 聚合。
    Aggregate,
    /// INNER/LEFT/RIGHT JOIN。
    Join,
    /// 单行插入。
    Insert,
    /// 批量插入及显式 batch size。
    BatchInsert,
    /// 条件更新。
    Update,
    /// 批量更新。
    BatchUpdate,
    /// 条件删除。
    Delete,
    /// 方言原生 UPSERT。
    Upsert,
    /// PostgreSQL `RETURNING`。
    Returning,
    /// PostgreSQL 显式冲突列 `ON CONFLICT (...)`。
    ExplicitConflictTarget,
    /// SQL 事务内的 insert/update/delete 构建器。
    TransactionCrud,
    /// Redis Pipeline。
    RedisPipeline,
    /// Redis WATCH/MULTI/EXEC 乐观锁事务。
    RedisOptimisticTransaction,
    /// Redis Lua 脚本执行。
    RedisLua,
}

/// 后端公开 API 必须遵守的安全约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SafetyConstraint {
    /// 数据值使用驱动绑定参数，不拼接为 SQL/命令文本。
    ParameterizedValues,
    /// 外部标识符通过 checked identifier API 进入。
    CheckedIdentifiers,
    /// 可信 SQL 表达式入口与标识符入口在 API 上显式分离。
    ExplicitTrustedExpressions,
    /// UPDATE/DELETE 缺少 WHERE 时 fail-closed。
    WriteRequiresWhere,
    /// Redis 命令和值通过结构化参数传给驱动。
    StructuredCommandArguments,
}

/// 单个后端的能力快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackendCapabilities {
    /// 后端类型。
    pub backend: BackendKind,
    /// 是否为关系数据库。
    pub relational: bool,
    /// SQL 占位符风格。
    pub placeholder_style: PlaceholderStyle,
    /// QueryBuilder 或原生命令层能力。
    pub operations: &'static [BackendCapability],
    /// 事务层能力。
    pub transaction: &'static [BackendCapability],
    /// 强制安全约束。
    pub safety: &'static [SafetyConstraint],
}

impl BackendCapabilities {
    /// 判断后端是否声明指定能力。
    pub fn supports(&self, capability: BackendCapability) -> bool {
        self.operations.contains(&capability) || self.transaction.contains(&capability)
    }

    /// 判断后端是否声明指定安全约束。
    pub fn enforces(&self, constraint: SafetyConstraint) -> bool {
        self.safety.contains(&constraint)
    }
}

const MYSQL_OPERATIONS: &[BackendCapability] = &[
    BackendCapability::CheckedIdentifiers,
    BackendCapability::Select,
    BackendCapability::Aggregate,
    BackendCapability::Join,
    BackendCapability::Insert,
    BackendCapability::BatchInsert,
    BackendCapability::Update,
    BackendCapability::BatchUpdate,
    BackendCapability::Delete,
    BackendCapability::Upsert,
];

const POSTGRES_OPERATIONS: &[BackendCapability] = &[
    BackendCapability::CheckedIdentifiers,
    BackendCapability::Select,
    BackendCapability::Aggregate,
    BackendCapability::Join,
    BackendCapability::Insert,
    BackendCapability::BatchInsert,
    BackendCapability::Update,
    BackendCapability::BatchUpdate,
    BackendCapability::Delete,
    BackendCapability::Upsert,
    BackendCapability::Returning,
    BackendCapability::ExplicitConflictTarget,
];

const SQL_SAFETY: &[SafetyConstraint] = &[
    SafetyConstraint::ParameterizedValues,
    SafetyConstraint::CheckedIdentifiers,
    SafetyConstraint::ExplicitTrustedExpressions,
    SafetyConstraint::WriteRequiresWhere,
];

/// MySQL 能力契约。
pub const MYSQL_CAPABILITIES: BackendCapabilities = BackendCapabilities {
    backend: BackendKind::MySql,
    relational: true,
    placeholder_style: PlaceholderStyle::QuestionMark,
    operations: MYSQL_OPERATIONS,
    transaction: &[BackendCapability::TransactionCrud],
    safety: SQL_SAFETY,
};

/// PostgreSQL 能力契约。
pub const POSTGRES_CAPABILITIES: BackendCapabilities = BackendCapabilities {
    backend: BackendKind::PostgreSql,
    relational: true,
    placeholder_style: PlaceholderStyle::NumberedDollar,
    operations: POSTGRES_OPERATIONS,
    transaction: &[
        BackendCapability::TransactionCrud,
        BackendCapability::Returning,
    ],
    safety: SQL_SAFETY,
};

/// Redis 能力契约。
pub const REDIS_CAPABILITIES: BackendCapabilities = BackendCapabilities {
    backend: BackendKind::Redis,
    relational: false,
    placeholder_style: PlaceholderStyle::NotApplicable,
    operations: &[
        BackendCapability::RedisPipeline,
        BackendCapability::RedisLua,
    ],
    transaction: &[BackendCapability::RedisOptimisticTransaction],
    safety: &[SafetyConstraint::StructuredCommandArguments],
};

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "mysql", feature = "postgres", feature = "redis"))]
    use std::future::Future;

    #[cfg(any(feature = "mysql", feature = "postgres", feature = "redis"))]
    fn assert_future<T>(_future: impl Future<Output = T>) {}

    #[cfg(feature = "mysql")]
    fn mysql_management_contract(db: &crate::mysql::Database) {
        let _: &'static BackendCapabilities = crate::mysql::Database::capabilities();
        let _: crate::PoolStatus = db.pool_status();
        let _: bool = db.is_closed();
        assert_future::<Result<bool, crate::DbError>>(db.health_check());
        assert_future::<()>(db.close());
    }

    #[cfg(feature = "postgres")]
    fn postgres_management_contract(db: &crate::postgres::Database) {
        let _: &'static BackendCapabilities = crate::postgres::Database::capabilities();
        let _: crate::PoolStatus = db.pool_status();
        let _: bool = db.is_closed();
        assert_future::<Result<bool, crate::DbError>>(db.health_check());
        assert_future::<()>(db.close());
    }

    #[cfg(feature = "redis")]
    fn redis_management_contract(client: &crate::redis::RedisClient) {
        let _: &'static BackendCapabilities = crate::redis::RedisClient::capabilities();
        let _: crate::PoolStatus = client.pool_status();
        let _: bool = client.is_closed();
        assert_future::<Result<bool, crate::DbError>>(client.health_check());
        assert_future::<()>(client.close());
    }

    #[test]
    fn management_contract_signatures_compile() {
        #[cfg(feature = "mysql")]
        let _ = mysql_management_contract;
        #[cfg(feature = "postgres")]
        let _ = postgres_management_contract;
        #[cfg(feature = "redis")]
        let _ = redis_management_contract;
    }

    #[test]
    fn sql_backends_share_common_contract_and_expose_dialect_differences() {
        for capability in [
            BackendCapability::CheckedIdentifiers,
            BackendCapability::Select,
            BackendCapability::Aggregate,
            BackendCapability::Join,
            BackendCapability::Insert,
            BackendCapability::BatchInsert,
            BackendCapability::Update,
            BackendCapability::BatchUpdate,
            BackendCapability::Delete,
            BackendCapability::Upsert,
            BackendCapability::TransactionCrud,
        ] {
            assert!(MYSQL_CAPABILITIES.supports(capability));
            assert!(POSTGRES_CAPABILITIES.supports(capability));
        }

        assert!(!MYSQL_CAPABILITIES.supports(BackendCapability::Returning));
        assert!(POSTGRES_CAPABILITIES.supports(BackendCapability::Returning));
        assert!(!MYSQL_CAPABILITIES.supports(BackendCapability::ExplicitConflictTarget));
        assert!(POSTGRES_CAPABILITIES.supports(BackendCapability::ExplicitConflictTarget));
    }

    #[test]
    fn redis_is_explicitly_non_relational() {
        let redis = std::hint::black_box(REDIS_CAPABILITIES);

        assert!(!redis.relational);
        assert_eq!(redis.placeholder_style, PlaceholderStyle::NotApplicable);
        assert!(!redis.supports(BackendCapability::Select));
        assert!(redis.supports(BackendCapability::RedisPipeline));
        assert!(redis.supports(BackendCapability::RedisOptimisticTransaction));
        assert!(redis.supports(BackendCapability::RedisLua));
    }

    #[test]
    fn sql_safety_contract_is_symmetric() {
        for constraint in SQL_SAFETY {
            assert!(MYSQL_CAPABILITIES.enforces(*constraint));
            assert!(POSTGRES_CAPABILITIES.enforces(*constraint));
        }
        assert!(REDIS_CAPABILITIES.enforces(SafetyConstraint::StructuredCommandArguments));
        assert!(!REDIS_CAPABILITIES.enforces(SafetyConstraint::WriteRequiresWhere));
    }
}
