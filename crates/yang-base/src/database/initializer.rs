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

use crate::error::BaseError;
use crate::plugin::{Plugin, PluginManager};
use std::sync::Arc;
use yang_db::Database;

/// 数据库初始化器
///
/// 负责执行插件的数据库初始化脚本和迁移。
/// 所有数据库操作都通过 yang-db::Database 提供的方法实现。
///
/// # 字段
///
/// - `db`: yang-db 数据库实例
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
    /// yang-db 数据库实例
    db: Database,

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
    /// let initializer = DatabaseInitializer::new(db.clone(), true);
    ///
    /// // 创建非事务模式的初始化器
    /// let initializer = DatabaseInitializer::new(db, false);
    /// ```
    pub fn new(db: Database, use_transaction: bool) -> Self {
        Self {
            db,
            use_transaction,
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
    async fn initialize_with_transaction(
        &self,
        plugins: &[Arc<dyn Plugin>],
    ) -> Result<(), BaseError> {
        let mut tx = self
            .db
            .transaction()
            .await
            .map_err(|e| BaseError::DatabaseTransactionFailed(e.to_string()))?;

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
            .map_err(|e| BaseError::DatabaseTransactionFailed(e.to_string()))?;

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
    async fn initialize_without_transaction(
        &self,
        plugins: &[Arc<dyn Plugin>],
    ) -> Result<(), BaseError> {
        for plugin in plugins {
            let name = plugin.name();
            log::info!("初始化插件数据库: {}", name);

            // 执行初始化 SQL（使用 yang-db::Database::execute）
            for sql in plugin.init_sql() {
                if let Err(e) = self.db.execute(&sql).await {
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

        self.db
            .execute(sql)
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;

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
            self.db.execute(&sql).await.map_err(|e| {
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

            // 记录迁移
            let record_sql = format!(
                "INSERT INTO _migrations (module_name, version) VALUES ('{}', '{}')",
                module_name, version
            );
            tx.execute(&record_sql)
                .await
                .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;
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
    /// 使用 yang-db::Database::query 方法查询。
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

        let sql = format!(
            "SELECT COUNT(*) as count FROM _migrations WHERE module_name = '{}' AND version = '{}'",
            module_name, version
        );

        // 使用 yang-db::Database::query 方法
        let results: Vec<CountResult> = self
            .db
            .query(&sql)
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))?;

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
    /// 使用 yang-db::Database::execute 执行 SQL。
    pub async fn record_migration(
        &self,
        module_name: &str,
        version: &str,
    ) -> Result<(), BaseError> {
        let sql = format!(
            "INSERT INTO _migrations (module_name, version) VALUES ('{}', '{}')",
            module_name, version
        );

        self.db
            .execute(&sql)
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;

        Ok(())
    }
}
