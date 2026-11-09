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
    /// 批量 UPSERT。MySQL 是 `INSERT ... ON DUPLICATE KEY UPDATE`；
    /// PostgreSQL 是 `INSERT ... ON CONFLICT (...) DO UPDATE SET ...`（形状不同）。
    BatchUpsert,
    /// PostgreSQL `RETURNING`。
    Returning,
    /// PostgreSQL 显式冲突列 `ON CONFLICT (...)`。
    ExplicitConflictTarget,
    /// 事务保存点（`SAVEPOINT` / `ROLLBACK TO SAVEPOINT`）。
    Savepoint,
    /// 行锁跳过已锁定行（`FOR UPDATE SKIP LOCKED`）。
    SkipLocked,
    /// 批量 COPY 导入（PostgreSQL `COPY ... FROM STDIN`）。
    Copy,
    /// LISTEN / NOTIFY 通知通道。
    ListenNotify,
    /// WITH / WITH RECURSIVE 公共表表达式。
    Cte,
    /// 窗口函数（`OVER (...)`）。
    Window,
    /// PostgreSQL 数组类型与 `= ANY(...)` / `= ALL(...)` 谓词。
    Arrays,
    /// PostgreSQL 范围类型（`int4range` 等）。
    Range,
    /// PostgreSQL 多范围类型（`int4multirange` 等）。
    Multirange,
    /// jsonpath 查询（`jsonb_path_exists` / `jsonb_path_match`）。
    JsonPath,
    /// 全文检索（`to_tsvector` / `to_tsquery`）。
    FullText,
    /// MERGE（PostgreSQL 15+，执行前按 `server_version_num` 门控）。
    Merge,
    /// 查询计划与锁等待诊断（`EXPLAIN`、`pg_stat_activity` / `pg_locks`）。
    Diagnostics,
    /// 两阶段提交（`PREPARE TRANSACTION` / `COMMIT PREPARED` / `ROLLBACK PREPARED`）。
    TwoPhaseCommit,
    /// 表级锁（`LOCK TABLE`）。
    TableLock,
    /// 事务级会话配置（`SET LOCAL`）。
    TransactionLocalConfig,
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
    BackendCapability::BatchUpsert,
];

// PostgreSQL 已实现的全部能力。事务专属能力（保存点、两阶段提交、表级锁、
// SET LOCAL）声明在 [`POSTGRES_CAPABILITIES`] 的 `transaction` 切片中，
// [`BackendCapabilities::supports`] 对两个切片一视同仁。
//
// MySQL 与 Redis 不得声明 PG 专属能力：能力矩阵是调用方机读契约，
// 虚报会让下游在错误 SQL 处才暴露问题。
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
    BackendCapability::BatchUpsert,
    BackendCapability::Returning,
    BackendCapability::ExplicitConflictTarget,
    BackendCapability::SkipLocked,
    BackendCapability::Copy,
    BackendCapability::ListenNotify,
    BackendCapability::Cte,
    BackendCapability::Window,
    BackendCapability::Arrays,
    BackendCapability::Range,
    BackendCapability::Multirange,
    BackendCapability::JsonPath,
    BackendCapability::FullText,
    BackendCapability::Merge,
    BackendCapability::Diagnostics,
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
        BackendCapability::Savepoint,
        BackendCapability::TwoPhaseCommit,
        BackendCapability::TableLock,
        BackendCapability::TransactionLocalConfig,
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
    fn both_backends_declare_batch_upsert() {
        // 形状不同（MySQL ODKU / PG ON CONFLICT + EXCLUDED），但两家都已实现。
        assert!(MYSQL_CAPABILITIES.supports(BackendCapability::BatchUpsert));
        assert!(POSTGRES_CAPABILITIES.supports(BackendCapability::BatchUpsert));
    }

    #[test]
    fn postgres_capabilities_match_implemented_features() {
        // P1-26：Phase C 已实现的能力必须全部声明。
        for capability in [
            BackendCapability::Savepoint,
            BackendCapability::Cte,
            BackendCapability::Window,
            BackendCapability::SkipLocked,
            BackendCapability::Returning,
            BackendCapability::ExplicitConflictTarget,
            BackendCapability::Copy,
            BackendCapability::ListenNotify,
            BackendCapability::BatchUpsert,
            BackendCapability::Arrays,
            BackendCapability::Range,
            BackendCapability::Multirange,
            BackendCapability::JsonPath,
            BackendCapability::FullText,
            BackendCapability::Merge,
            BackendCapability::Diagnostics,
            BackendCapability::TwoPhaseCommit,
            BackendCapability::TableLock,
            BackendCapability::TransactionLocalConfig,
        ] {
            assert!(
                POSTGRES_CAPABILITIES.supports(capability),
                "PostgreSQL 已实现 {capability:?}，必须声明"
            );
        }

        // 可信表达式入口与标识符入口在 API 上显式分离。
        assert!(
            POSTGRES_CAPABILITIES.enforces(SafetyConstraint::ExplicitTrustedExpressions),
            "PostgreSQL 必须声明 ExplicitTrustedExpressions 安全约束"
        );

        // 仅 PostgreSQL 实现的差异能力不得被 MySQL/Redis 提前声明。
        for capability in [
            BackendCapability::Copy,
            BackendCapability::ListenNotify,
            BackendCapability::Arrays,
            BackendCapability::Range,
            BackendCapability::Multirange,
            BackendCapability::JsonPath,
            BackendCapability::FullText,
            BackendCapability::Merge,
            BackendCapability::Diagnostics,
            BackendCapability::TwoPhaseCommit,
            BackendCapability::TableLock,
            BackendCapability::TransactionLocalConfig,
        ] {
            assert!(
                !MYSQL_CAPABILITIES.supports(capability),
                "MySQL 未实现 {capability:?}，不得声明"
            );
            assert!(
                !REDIS_CAPABILITIES.supports(capability),
                "Redis 未实现 {capability:?}，不得声明"
            );
        }
    }

    #[test]
    fn non_postgres_backends_do_not_claim_postgres_only_capabilities() {
        // 能力矩阵是机读契约；任一 PG 专属能力被 MySQL/Redis 声明都视为回归。
        for capability in [
            BackendCapability::Returning,
            BackendCapability::ExplicitConflictTarget,
            BackendCapability::SkipLocked,
            BackendCapability::Savepoint,
            BackendCapability::Cte,
            BackendCapability::Window,
            BackendCapability::Arrays,
            BackendCapability::Range,
            BackendCapability::Multirange,
            BackendCapability::JsonPath,
            BackendCapability::FullText,
            BackendCapability::Merge,
            BackendCapability::Diagnostics,
            BackendCapability::TwoPhaseCommit,
            BackendCapability::TableLock,
            BackendCapability::TransactionLocalConfig,
        ] {
            assert!(!MYSQL_CAPABILITIES.supports(capability), "{capability:?}");
            assert!(!REDIS_CAPABILITIES.supports(capability), "{capability:?}");
        }
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
