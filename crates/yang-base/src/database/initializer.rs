//! 数据库初始化器
//!
//! 负责执行插件的数据库初始化脚本和迁移。
//!
//! # 设计说明
//!
//! DatabaseInitializer 使用 yang-db::Database 的方法执行 SQL，支持事务和非事务两种初始化模式。
//! 所有数据库操作都通过 yang-db 库实现，确保类型安全和统一的数据库访问接口。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::DatabaseInitializer;
//! use yang_base::plugin::PluginManager;
//! use yang_db::Database;
//!
//! // 创建数据库连接
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//!
//! // 创建插件管理器并注册插件
//! let manager = PluginManager::new();
//! manager.register(MyPlugin).await?;
//!
//! // 创建数据库初始化器（启用事务模式）
//! let initializer = DatabaseInitializer::new(db, true);
//!
//! // 初始化所有插件的数据库
//! initializer.initialize_all(&manager).await?;
//! ```

use crate::database::GlobalDatabase;
use crate::error::BaseError;
use crate::plugin::{Plugin, PluginLifecycleStage, PluginManager};
use crate::table::{SchemaColumn, SchemaValidationReport, TableConfig};
use std::sync::Arc;
use yang_db::Database;

/// 迁移 dry-run 中单项的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationPlanStatus {
    /// 尚未执行。
    Pending,
    /// 已执行且校验和一致。
    Applied,
    /// 相同 module/version 的内容已变化或历史记录不可验证。
    ChecksumMismatch,
    /// 另一个初始化器已预留并正在执行。
    InProgress,
}

/// 一条可审计的迁移计划记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlanEntry {
    /// 插件/模块名称。
    pub module: String,
    /// 迁移版本。
    pub version: String,
    /// 当前 SQL 内容的稳定校验和。
    pub checksum: String,
    /// 与数据库记录比较后的状态。
    pub status: MigrationPlanStatus,
}

/// dry-run 迁移计划；生成过程只读数据库，不创建表或写记录。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationPlan {
    /// 按插件依赖顺序及插件声明顺序排列的迁移项。
    pub entries: Vec<MigrationPlanEntry>,
}

#[derive(sqlx::FromRow)]
struct MigrationRecord {
    checksum: Option<String>,
    status: String,
}

fn classify_migration_record(
    record: Option<(Option<&str>, &str)>,
    expected_checksum: &str,
) -> MigrationPlanStatus {
    match record {
        None => MigrationPlanStatus::Pending,
        Some((_, status)) if status != "applied" => MigrationPlanStatus::InProgress,
        Some((Some(actual), "applied")) if actual == expected_checksum => {
            MigrationPlanStatus::Applied
        }
        Some(_) => MigrationPlanStatus::ChecksumMismatch,
    }
}

/// 计算迁移 SQL 的稳定 FNV-1a 64 位校验和。
fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn migration_execution_error(
    module: &str,
    version: &str,
    sql: &str,
    source: yang_db::DbError,
) -> BaseError {
    BaseError::MigrationExecutionFailed {
        module: module.to_string(),
        version: version.to_string(),
        checksum: migration_checksum(sql),
        source,
    }
}

/// 数据库引用
///
/// 用于让 [`DatabaseInitializer`] 既能持有调用方传入的 owned [`Database`]，
/// 也能引用进程级全局单例（`'static` 引用），二者共用同一套初始化逻辑。
enum DbRef {
    /// 调用方传入并交由初始化器持有的数据库实例
    Owned(Database),
    /// 指向全局单例 [`GlobalDatabase`] 的 `'static` 引用
    Global(&'static Database),
}

impl DbRef {
    /// 返回底层数据库实例的引用
    ///
    /// 两个变体统一收敛为 `&Database`，调用处无需关心数据库来源。
    fn db(&self) -> &Database {
        match self {
            DbRef::Owned(db) => db,
            DbRef::Global(db) => db,
        }
    }
}

/// 数据库初始化器
///
/// 负责执行插件的数据库初始化脚本和迁移。
/// 所有数据库操作都通过 yang-db::Database 提供的方法实现。
///
/// # 字段
///
/// - `db`: 数据库引用（owned 实例或全局单例的 `'static` 引用）
/// - `use_transaction`: 是否启用事务模式
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::database::DatabaseInitializer;
/// use yang_db::Database;
///
/// let db = Database::connect("mysql://root:password@localhost/test").await?;
/// let initializer = DatabaseInitializer::new(db, true);
/// ```
pub struct DatabaseInitializer {
    /// 数据库引用（owned 实例或全局单例的 `'static` 引用）
    db: DbRef,

    /// 是否启用事务模式
    use_transaction: bool,
}

impl DatabaseInitializer {
    /// 创建新的数据库初始化器
    ///
    /// # 参数
    ///
    /// - `db`: yang-db::Database 实例
    /// - `use_transaction`: 是否启用事务模式
    ///
    /// # 返回
    ///
    /// - DatabaseInitializer 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::database::DatabaseInitializer;
    /// use yang_db::Database;
    ///
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    ///
    /// // 创建事务模式的初始化器
    /// let initializer = DatabaseInitializer::new(db, true);
    ///
    /// // 创建非事务模式的初始化器
    /// let db2 = Database::connect("mysql://root:password@localhost/test").await?;
    /// let initializer = DatabaseInitializer::new(db2, false);
    /// ```
    pub fn new(db: Database, use_transaction: bool) -> Self {
        Self {
            db: DbRef::Owned(db),
            use_transaction,
        }
    }

    /// 基于全局数据库单例创建初始化器
    ///
    /// 直接引用 [`GlobalDatabase`] 持有的进程级单例，无需调用方再传入 owned 实例，
    /// 适合应用已通过 [`GlobalDatabase::init`] 或 `DatabaseBundle::init` 完成初始化的场景。
    ///
    /// # 参数
    ///
    /// - `use_transaction`: 是否启用事务模式
    ///
    /// # 返回
    ///
    /// - `Ok(DatabaseInitializer)`: 引用全局单例的初始化器
    /// - `Err(BaseError)`: 全局数据库尚未初始化
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 全局数据库未初始化，需要先调用 `init`
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::database::DatabaseInitializer;
    ///
    /// let initializer = DatabaseInitializer::from_global(true)?;
    /// ```
    pub fn from_global(use_transaction: bool) -> Result<Self, BaseError> {
        Ok(Self {
            db: DbRef::Global(GlobalDatabase::get()?),
            use_transaction,
        })
    }

    /// 返回底层数据库实例的引用
    ///
    /// 收敛 [`DbRef`] 的两个变体，初始化逻辑无需关心数据库来源（owned 或全局单例）。
    fn db(&self) -> &Database {
        self.db.db()
    }

    /// 只读生成单个插件的迁移计划，不创建迁移表、不执行 SQL、不写迁移记录。
    pub async fn plan_migrations(&self, plugin: &dyn Plugin) -> Result<MigrationPlan, BaseError> {
        let table_exists = self
            .db()
            .table_exists("_migrations")
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        let mut entries = Vec::new();
        for (version, sql) in plugin.migration_sql() {
            let checksum = migration_checksum(&sql);
            let record = if table_exists {
                self.load_migration_record(plugin.name(), &version).await?
            } else {
                None
            };
            let status = classify_migration_record(
                record
                    .as_ref()
                    .map(|record| (record.checksum.as_deref(), record.status.as_str())),
                &checksum,
            );
            entries.push(MigrationPlanEntry {
                module: plugin.name().to_string(),
                version,
                checksum,
                status,
            });
        }
        Ok(MigrationPlan { entries })
    }

    /// 只读生成全部插件的迁移计划。
    pub async fn plan_all(
        &self,
        plugin_manager: &PluginManager,
    ) -> Result<MigrationPlan, BaseError> {
        let mut plan = MigrationPlan::default();
        for plugin in plugin_manager.get_all().await {
            plan.entries
                .extend(self.plan_migrations(plugin.as_ref()).await?.entries);
        }
        Ok(plan)
    }

    /// 从 MySQL information_schema 读取当前列并验证 TableConfig 的运行期字段契约。
    ///
    /// 本方法只读，不生成或执行 ALTER；数据库额外列不视为问题。
    pub async fn validate_table_config(
        &self,
        table: &TableConfig,
    ) -> Result<SchemaValidationReport, BaseError> {
        #[derive(sqlx::FromRow)]
        struct ColumnRow {
            column_name: String,
            data_type: String,
            column_type: String,
            is_nullable: String,
            character_maximum_length: Option<i64>,
        }

        let rows: Vec<ColumnRow> = self
            .db()
            .query_with_params(
                "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length FROM information_schema.columns WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
                vec![serde_json::Value::String(table.table_name.clone())],
            )
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        let columns: Vec<_> = rows
            .into_iter()
            .map(|row| {
                SchemaColumn::new(
                    row.column_name,
                    row.data_type,
                    row.column_type,
                    row.is_nullable.eq_ignore_ascii_case("YES"),
                    row.character_maximum_length
                        .and_then(|length| u64::try_from(length).ok()),
                )
            })
            .collect();
        Ok(table.validate_schema(&columns))
    }

    async fn load_migration_record(
        &self,
        module: &str,
        version: &str,
    ) -> Result<Option<MigrationRecord>, BaseError> {
        let sql = "SELECT checksum, status FROM _migrations WHERE module_name = ? AND version = ? LIMIT 1";
        let records: Vec<MigrationRecord> = self
            .db()
            .query_with_params(
                sql,
                vec![
                    serde_json::Value::String(module.to_string()),
                    serde_json::Value::String(version.to_string()),
                ],
            )
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        Ok(records.into_iter().next())
    }

    fn validate_migration_record(
        &self,
        module: &str,
        version: &str,
        expected_checksum: &str,
        record: Option<MigrationRecord>,
    ) -> Result<bool, BaseError> {
        match classify_migration_record(
            record
                .as_ref()
                .map(|record| (record.checksum.as_deref(), record.status.as_str())),
            expected_checksum,
        ) {
            MigrationPlanStatus::Pending => Ok(false),
            MigrationPlanStatus::Applied => Ok(true),
            MigrationPlanStatus::ChecksumMismatch => Err(BaseError::MigrationChecksumMismatch {
                module: module.to_string(),
                version: version.to_string(),
                expected: expected_checksum.to_string(),
                actual: record.and_then(|record| record.checksum),
            }),
            MigrationPlanStatus::InProgress => Err(BaseError::MigrationInProgress {
                module: module.to_string(),
                version: version.to_string(),
            }),
        }
    }

    /// 初始化所有插件的数据库
    ///
    /// # 参数
    ///
    /// - `plugin_manager`: 插件管理器
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::execute 和 yang-db::Database::transaction 方法。
    /// 根据 use_transaction 标志选择事务或非事务模式。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::database::DatabaseInitializer;
    /// use yang_base::plugin::PluginManager;
    /// use yang_db::Database;
    ///
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let manager = PluginManager::new();
    /// manager.register(MyPlugin).await?;
    ///
    /// let initializer = DatabaseInitializer::new(db, true);
    /// initializer.initialize_all(&manager).await?;
    /// ```
    pub async fn initialize_all(&self, plugin_manager: &PluginManager) -> Result<(), BaseError> {
        log::info!("开始初始化数据库...");

        // 创建迁移记录表（使用 yang-db::Database::execute）
        self.create_migration_table().await?;

        // 获取所有插件（已按依赖关系排序）
        let plugins = plugin_manager.get_all().await;

        if self.use_transaction {
            // 事务模式：所有插件在一个事务中初始化（使用 yang-db::Transaction）
            self.initialize_with_transaction(&plugins).await?;
        } else {
            // 非事务模式：逐个插件初始化（使用 yang-db::Database::execute）
            self.initialize_without_transaction(&plugins).await?;
        }

        log::info!("数据库初始化完成");
        Ok(())
    }

    /// 使用事务模式初始化
    ///
    /// # 参数
    ///
    /// - `plugins`: 插件列表（已按依赖关系排序）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::transaction 创建事务。
    /// 使用 yang-db::Transaction::execute 执行 SQL。
    /// 使用 yang-db::Transaction::commit 提交事务。
    #[allow(deprecated)]
    async fn initialize_with_transaction(
        &self,
        plugins: &[Arc<dyn Plugin>],
    ) -> Result<(), BaseError> {
        let mut tx = self
            .db()
            .transaction()
            .await
            .map_err(BaseError::DatabaseTransactionFailed)?;

        for plugin in plugins {
            let name = plugin.name();
            log::info!("初始化插件数据库: {}", name);

            // 执行初始化 SQL（使用 yang-db::Transaction::execute）
            for sql in plugin.init_sql() {
                if let Err(e) = tx.execute(&sql).await {
                    log::error!("插件 {} 初始化失败: {}", name, e);
                    return Err(BaseError::PluginLifecycleFailed {
                        plugin: name.to_string(),
                        stage: PluginLifecycleStage::Initialize,
                        source: Box::new(e),
                    });
                }
            }

            // 执行迁移
            self.run_migrations_in_tx(&mut tx, plugin.as_ref()).await?;

            // 调用初始化回调
            if let Err(e) = plugin.on_init().await {
                log::error!("插件 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::PluginLifecycleFailed {
                    plugin: name.to_string(),
                    stage: PluginLifecycleStage::Initialize,
                    source: e,
                });
            }
        }

        tx.commit()
            .await
            .map_err(BaseError::DatabaseTransactionFailed)?;

        Ok(())
    }

    /// 不使用事务模式初始化
    ///
    /// # 参数
    ///
    /// - `plugins`: 插件列表（已按依赖关系排序）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::execute 执行 SQL。
    #[allow(deprecated)]
    async fn initialize_without_transaction(
        &self,
        plugins: &[Arc<dyn Plugin>],
    ) -> Result<(), BaseError> {
        for plugin in plugins {
            let name = plugin.name();
            log::info!("初始化插件数据库: {}", name);

            // 执行初始化 SQL（使用 yang-db::Database::execute）
            for sql in plugin.init_sql() {
                if let Err(e) = self.db().execute(&sql).await {
                    log::error!("插件 {} 初始化失败: {}", name, e);
                    return Err(BaseError::PluginLifecycleFailed {
                        plugin: name.to_string(),
                        stage: PluginLifecycleStage::Initialize,
                        source: Box::new(e),
                    });
                }
            }

            // 执行迁移
            self.run_migrations(plugin.as_ref()).await?;

            // 调用初始化回调
            if let Err(e) = plugin.on_init().await {
                log::error!("插件 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::PluginLifecycleFailed {
                    plugin: name.to_string(),
                    stage: PluginLifecycleStage::Initialize,
                    source: e,
                });
            }
        }

        Ok(())
    }

    /// 创建迁移记录表
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 创建成功
    /// - `Err(BaseError)`: 创建失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::execute 执行 SQL。
    #[allow(deprecated)]
    pub async fn create_migration_table(&self) -> Result<(), BaseError> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id INT AUTO_INCREMENT PRIMARY KEY,
                module_name VARCHAR(255) NOT NULL COMMENT '模块名称',
                version VARCHAR(255) NOT NULL COMMENT '迁移版本',
                checksum CHAR(16) NULL COMMENT 'FNV-1a 迁移内容校验和',
                status VARCHAR(16) NOT NULL DEFAULT 'applied' COMMENT 'running/applied',
                executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '执行时间',
                UNIQUE KEY unique_migration (module_name, version),
                INDEX idx_module_name (module_name)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='数据库迁移记录表'
        "#;

        self.db()
            .execute(sql)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;

        self.ensure_migration_column(
            "checksum",
            "ALTER TABLE _migrations ADD COLUMN checksum CHAR(16) NULL AFTER version",
        )
        .await?;
        self.ensure_migration_column(
            "status",
            "ALTER TABLE _migrations ADD COLUMN status VARCHAR(16) NOT NULL DEFAULT 'applied' AFTER checksum",
        )
        .await?;

        Ok(())
    }

    async fn ensure_migration_column(
        &self,
        column: &str,
        alter_sql: &str,
    ) -> Result<(), BaseError> {
        #[derive(sqlx::FromRow)]
        struct CountResult {
            count: i64,
        }
        let rows: Vec<CountResult> = self
            .db()
            .query_with_params(
                "SELECT COUNT(*) AS count FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = '_migrations' AND column_name = ?",
                vec![serde_json::Value::String(column.to_string())],
            )
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        if rows.first().map(|row| row.count).unwrap_or(0) == 0 {
            #[allow(deprecated)]
            self.db()
                .execute(alter_sql)
                .await
                .map_err(BaseError::DatabaseExecuteFailed)?;
        }
        Ok(())
    }

    /// 执行迁移（非事务模式）
    ///
    /// # 参数
    ///
    /// - `plugin`: 插件实例
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 迁移成功
    /// - `Err(BaseError)`: 迁移失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::execute 执行 SQL。
    #[allow(deprecated)]
    pub async fn run_migrations(&self, plugin: &dyn Plugin) -> Result<(), BaseError> {
        let module_name = plugin.name();

        for (version, sql) in plugin.migration_sql() {
            let checksum = migration_checksum(&sql);
            if self.validate_migration_record(
                module_name,
                &version,
                &checksum,
                self.load_migration_record(module_name, &version).await?,
            )? {
                continue;
            }

            if let Err(reservation_error) = self
                .record_migration(module_name, &version, &checksum, "running")
                .await
            {
                let record = self.load_migration_record(module_name, &version).await?;
                if self.validate_migration_record(module_name, &version, &checksum, record)? {
                    continue;
                }
                return Err(reservation_error);
            }

            log::info!("执行迁移: {} v{}", module_name, version);

            if let Err(source) = self.db().execute(&sql).await {
                let _ = self
                    .delete_migration_reservation(module_name, &version, &checksum)
                    .await;
                return Err(migration_execution_error(
                    module_name,
                    &version,
                    &sql,
                    source,
                ));
            }
            self.mark_migration_applied(module_name, &version, &checksum)
                .await?;
        }

        Ok(())
    }

    /// 执行迁移（事务模式）
    ///
    /// # 参数
    ///
    /// - `tx`: yang-db 事务对象
    /// - `plugin`: 插件实例
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 迁移成功
    /// - `Err(BaseError)`: 迁移失败
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Transaction::execute 执行 SQL。
    #[allow(deprecated)]
    pub async fn run_migrations_in_tx(
        &self,
        tx: &mut yang_db::Transaction,
        plugin: &dyn Plugin,
    ) -> Result<(), BaseError> {
        let module_name = plugin.name();

        for (version, sql) in plugin.migration_sql() {
            let checksum = migration_checksum(&sql);
            let record_sql = "SELECT checksum, status FROM _migrations WHERE module_name = ? AND version = ? LIMIT 1";
            let record_params = vec![
                serde_json::Value::String(module_name.to_string()),
                serde_json::Value::String(version.clone()),
            ];
            let records: Vec<MigrationRecord> = tx
                .query_with_params(record_sql, record_params)
                .await
                .map_err(BaseError::DatabaseQueryFailed)?;
            if self.validate_migration_record(
                module_name,
                &version,
                &checksum,
                records.into_iter().next(),
            )? {
                continue;
            }

            let reserve_sql = "INSERT INTO _migrations (module_name, version, checksum, status) VALUES (?, ?, ?, 'running')";
            let reserve_params = vec![
                serde_json::Value::String(module_name.to_string()),
                serde_json::Value::String(version.clone()),
                serde_json::Value::String(checksum.clone()),
            ];
            tx.execute_with_params(reserve_sql, reserve_params)
                .await
                .map_err(BaseError::DatabaseExecuteFailed)?;

            log::info!("执行迁移: {} v{}", module_name, version);

            if let Err(source) = tx.execute(&sql).await {
                let cleanup_sql = "DELETE FROM _migrations WHERE module_name = ? AND version = ? AND checksum = ? AND status = 'running'";
                let cleanup_params = vec![
                    serde_json::Value::String(module_name.to_string()),
                    serde_json::Value::String(version.clone()),
                    serde_json::Value::String(checksum.clone()),
                ];
                let _ = tx.execute_with_params(cleanup_sql, cleanup_params).await;
                return Err(migration_execution_error(
                    module_name,
                    &version,
                    &sql,
                    source,
                ));
            }

            let applied_sql = "UPDATE _migrations SET status = 'applied', executed_at = CURRENT_TIMESTAMP WHERE module_name = ? AND version = ? AND checksum = ? AND status = 'running'";
            let applied_params = vec![
                serde_json::Value::String(module_name.to_string()),
                serde_json::Value::String(version.clone()),
                serde_json::Value::String(checksum),
            ];
            let affected = tx
                .execute_with_params(applied_sql, applied_params)
                .await
                .map_err(BaseError::DatabaseExecuteFailed)?;
            if affected != 1 {
                return Err(BaseError::DatabaseMigrationFailed(
                    module_name.to_string(),
                    format!("迁移 v{version} 的 running 预留已丢失"),
                ));
            }
        }

        Ok(())
    }

    /// 检查迁移是否已执行
    ///
    /// # 参数
    ///
    /// - `module_name`: 模块名称
    /// - `version`: 迁移版本
    ///
    /// # 返回
    ///
    /// - `Ok(bool)`: true 表示已执行，false 表示未执行
    /// - `Err(BaseError)`: 查询失败
    ///
    /// # 说明
    ///
    /// 使用参数化查询，防止 SQL 注入攻击。
    pub async fn is_migration_executed(
        &self,
        module_name: &str,
        version: &str,
    ) -> Result<bool, BaseError> {
        // 定义查询结果结构体
        #[derive(sqlx::FromRow)]
        struct CountResult {
            count: i64,
        }

        // 使用参数占位符，防止 SQL 注入
        let sql = "SELECT COUNT(*) as count FROM _migrations WHERE module_name = ? AND version = ? AND status = 'applied'";
        let params = vec![
            serde_json::Value::String(module_name.to_string()),
            serde_json::Value::String(version.to_string()),
        ];

        // 使用 yang-db::Database::query_with_params 方法
        let results: Vec<CountResult> = self
            .db()
            .query_with_params(sql, params)
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;

        Ok(results.first().map(|r| r.count > 0).unwrap_or(false))
    }

    /// 记录迁移
    ///
    /// # 参数
    ///
    /// - `module_name`: 模块名称
    /// - `version`: 迁移版本
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 记录成功
    /// - `Err(BaseError)`: 记录失败
    ///
    /// # 说明
    ///
    /// 使用参数化查询，防止 SQL 注入攻击。
    pub async fn record_migration(
        &self,
        module_name: &str,
        version: &str,
        checksum: &str,
        status: &str,
    ) -> Result<(), BaseError> {
        if !matches!(status, "running" | "applied") {
            return Err(BaseError::DatabaseMigrationFailed(
                module_name.to_string(),
                format!("非法迁移状态: {status}"),
            ));
        }
        let sql =
            "INSERT INTO _migrations (module_name, version, checksum, status) VALUES (?, ?, ?, ?)";
        let params = vec![
            serde_json::Value::String(module_name.to_string()),
            serde_json::Value::String(version.to_string()),
            serde_json::Value::String(checksum.to_string()),
            serde_json::Value::String(status.to_string()),
        ];

        self.db()
            .execute_with_params(sql, params)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;

        Ok(())
    }

    async fn mark_migration_applied(
        &self,
        module: &str,
        version: &str,
        checksum: &str,
    ) -> Result<(), BaseError> {
        let affected = self
            .db()
            .execute_with_params(
                "UPDATE _migrations SET status = 'applied', executed_at = CURRENT_TIMESTAMP WHERE module_name = ? AND version = ? AND checksum = ? AND status = 'running'",
                vec![
                    serde_json::Value::String(module.to_string()),
                    serde_json::Value::String(version.to_string()),
                    serde_json::Value::String(checksum.to_string()),
                ],
            )
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        if affected != 1 {
            return Err(BaseError::DatabaseMigrationFailed(
                module.to_string(),
                format!("迁移 v{version} 的 running 预留已丢失"),
            ));
        }
        Ok(())
    }

    async fn delete_migration_reservation(
        &self,
        module: &str,
        version: &str,
        checksum: &str,
    ) -> Result<(), BaseError> {
        self.db()
            .execute_with_params(
                "DELETE FROM _migrations WHERE module_name = ? AND version = ? AND checksum = ? AND status = 'running'",
                vec![
                    serde_json::Value::String(module.to_string()),
                    serde_json::Value::String(version.to_string()),
                    serde_json::Value::String(checksum.to_string()),
                ],
            )
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn migration_execution_error_preserves_identity_checksum_and_source() {
        let sql = "ALTER TABLE users ADD COLUMN status INT";
        let error = migration_execution_error(
            "accounts",
            "202607150001",
            sql,
            yang_db::DbError::SqlSyntaxError("bad ddl".into()),
        );

        match &error {
            BaseError::MigrationExecutionFailed {
                module,
                version,
                checksum,
                ..
            } => {
                assert_eq!(module, "accounts");
                assert_eq!(version, "202607150001");
                assert_eq!(checksum, &migration_checksum(sql));
                assert_ne!(checksum, &migration_checksum("ALTER TABLE users"));
            }
            other => panic!("期望 MigrationExecutionFailed，得到: {other:?}"),
        }
        assert!(error.source().is_some());
    }

    #[test]
    fn migration_record_classification_detects_drift_and_in_progress() {
        assert_eq!(
            classify_migration_record(None, "new-checksum"),
            MigrationPlanStatus::Pending
        );
        assert_eq!(
            classify_migration_record(Some((Some("same"), "applied")), "same"),
            MigrationPlanStatus::Applied
        );
        assert_eq!(
            classify_migration_record(Some((Some("old"), "applied")), "new"),
            MigrationPlanStatus::ChecksumMismatch
        );
        assert_eq!(
            classify_migration_record(Some((None, "applied")), "new"),
            MigrationPlanStatus::ChecksumMismatch
        );
        assert_eq!(
            classify_migration_record(Some((Some("same"), "running")), "same"),
            MigrationPlanStatus::InProgress
        );
    }
}
