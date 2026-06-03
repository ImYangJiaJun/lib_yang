// PostgreSQL 数据库初始化模块
// 主要功能已在 database.rs 中实现
// 此模块预留用于未来扩展，如迁移管理等

/// 数据库迁移配置（PostgreSQL）
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// 迁移脚本目录
    pub migrations_dir: String,
    /// 是否自动运行迁移
    pub auto_migrate: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            migrations_dir: "migrations".to_string(),
            auto_migrate: false,
        }
    }
}
