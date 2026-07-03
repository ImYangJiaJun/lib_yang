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
use crate::plugin::{Plugin, PluginManager};
use std::sync::Arc;
use yang_db::Database;

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
                    return Err(BaseError::PluginInitFailed(name.to_string(), e.to_string()));
                }
            }

            // 执行迁移
            self.run_migrations_in_tx(&mut tx, plugin.as_ref()).await?;

            // 调用初始化回调
            if let Err(e) = plugin.on_init().await {
                log::error!("插件 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::PluginInitFailed(name.to_string(), e.to_string()));
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
                    return Err(BaseError::PluginInitFailed(name.to_string(), e.to_string()));
                }
            }

            // 执行迁移
            self.run_migrations(plugin.as_ref()).await?;

            // 调用初始化回调
            if let Err(e) = plugin.on_init().await {
                log::error!("插件 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::PluginInitFailed(name.to_string(), e.to_string()));
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
                executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '执行时间',
                UNIQUE KEY unique_migration (module_name, version),
                INDEX idx_module_name (module_name)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='数据库迁移记录表'
        "#;

        self.db()
            .execute(sql)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;

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
            // 检查迁移是否已执行
            if self.is_migration_executed(module_name, &version).await? {
                log::debug!("迁移 {} v{} 已执行，跳过", module_name, version);
                continue;
            }

            log::info!("执行迁移: {} v{}", module_name, version);

            // 执行迁移 SQL（使用 yang-db::Database::execute）
            self.db().execute(&sql).await.map_err(|e| {
                BaseError::MigrationFailed(module_name.to_string(), version.clone(), e.to_string())
            })?;

            // 记录迁移
            self.record_migration(module_name, &version).await?;
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
            // 检查迁移是否已执行
            if self.is_migration_executed(module_name, &version).await? {
                log::debug!("迁移 {} v{} 已执行，跳过", module_name, version);
                continue;
            }

            log::info!("执行迁移: {} v{}", module_name, version);

            // 执行迁移 SQL（使用 yang-db::Transaction::execute）
            tx.execute(&sql).await.map_err(|e| {
                BaseError::MigrationFailed(module_name.to_string(), version.clone(), e.to_string())
            })?;

            // 记录迁移（使用参数化查询，防止 SQL 注入）
            let record_sql = "INSERT INTO _migrations (module_name, version) VALUES (?, ?)";
            let record_params = vec![
                serde_json::Value::String(module_name.to_string()),
                serde_json::Value::String(version.clone()),
            ];
            tx.execute_with_params(record_sql, record_params)
                .await
                .map_err(BaseError::DatabaseExecuteFailed)?;
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
        let sql = "SELECT COUNT(*) as count FROM _migrations WHERE module_name = ? AND version = ?";
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
    ) -> Result<(), BaseError> {
        // 使用参数占位符，防止 SQL 注入
        let sql = "INSERT INTO _migrations (module_name, version) VALUES (?, ?)";
        let params = vec![
            serde_json::Value::String(module_name.to_string()),
            serde_json::Value::String(version.to_string()),
        ];

        self.db()
            .execute_with_params(sql, params)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;

        Ok(())
    }
}
