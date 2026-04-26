use crate::error::DbError;
use crate::mysql::query_builder::QueryBuilder;
use crate::mysql::transaction::Transaction;
use sqlx::mysql::MySqlPool;

/// 数据库配置
///
/// 用于配置数据库连接池的参数
///
/// # 示例
///
/// ```rust
/// use yang_db::DatabaseConfig;
///
/// // 使用默认配置
/// let config = DatabaseConfig::default();
///
/// // 自定义配置
/// let config = DatabaseConfig {
///     max_connections: 20,
///     connect_timeout: 10,
///     idle_timeout: 300,
///     enable_logging: true,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 空闲连接超时时间（秒）
    pub idle_timeout: u64,
    /// 是否启用日志
    pub enable_logging: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            connect_timeout: 30,
            idle_timeout: 600,
            enable_logging: false,
        }
    }
}

/// 数据库连接管理器
///
/// 管理 MySQL 数据库连接池，提供查询构建和执行的入口点
///
/// # 示例
///
/// ```rust,no_run
/// use yang_db::Database;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // 连接数据库
///     let db = Database::connect("mysql://user:password@localhost:3306/test").await?;
///     
///     // 使用查询构建器
///     let builder = db.table("users")
///         .field("id")
///         .field("name");
///     
///     // 执行查询（需要实现 select 方法）
///     // let users = builder.select::<User>().await?;
///     
///     Ok(())
/// }
/// ```
pub struct Database {
    pool: MySqlPool,
    config: DatabaseConfig,
}

impl Database {
    /// 创建数据库连接
    ///
    /// # 参数
    /// - url: 数据库连接字符串，格式：mysql://user:password@host:port/database
    ///
    /// # 返回
    /// - Ok(Database): 成功创建的数据库实例
    /// - Err(DbError): 连接失败错误
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        Self::connect_with_config(url, DatabaseConfig::default()).await
    }

    /// 使用自定义配置创建数据库连接
    pub async fn connect_with_config(url: &str, config: DatabaseConfig) -> Result<Self, DbError> {
        use sqlx::mysql::MySqlPoolOptions;
        use std::time::Duration;

        // 使用配置参数创建连接池
        let pool = MySqlPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout))
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .connect(url)
            .await?;

        Ok(Self { pool, config })
    }

    /// 选择表，返回查询构建器
    pub fn table(&self, table_name: &str) -> QueryBuilder<'_> {
        QueryBuilder::new(&self.pool, table_name, self.config.enable_logging)
    }

    /// 执行原生 SELECT 查询
    pub async fn query<T>(&self, sql: &str) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        if self.config.enable_logging {
            log::debug!("执行原生查询: {}", sql);
        }

        let rows = sqlx::query_as::<_, T>(sql).fetch_all(&self.pool).await?;

        Ok(rows)
    }

    /// 执行原生 INSERT/UPDATE/DELETE 查询
    pub async fn execute(&self, sql: &str) -> Result<u64, DbError> {
        if self.config.enable_logging {
            log::debug!("执行原生语句: {}", sql);
        }

        let result = sqlx::query(sql).execute(&self.pool).await?;

        Ok(result.rows_affected())
    }

    /// 开始事务
    pub async fn transaction(&self) -> Result<Transaction, DbError> {
        let tx = self.pool.begin().await?;
        Ok(Transaction::new(tx, self.config.enable_logging))
    }

    /// 初始化数据库（执行 SQL 脚本）
    pub async fn init(&self, sql_script: &str) -> Result<(), DbError> {
        // 按分号分割多个 SQL 语句
        let statements: Vec<&str> = sql_script
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for statement in statements {
            self.execute(statement).await?;
        }

        Ok(())
    }

    /// 创建表
    pub async fn create_table(&self, create_sql: &str) -> Result<(), DbError> {
        self.execute(create_sql).await?;
        Ok(())
    }

    /// 删除表
    pub async fn drop_table(&self, table_name: &str) -> Result<(), DbError> {
        let sql = format!("DROP TABLE IF EXISTS `{}`", table_name);
        self.execute(&sql).await?;
        Ok(())
    }

    /// 检查表是否存在
    pub async fn table_exists(&self, table_name: &str) -> Result<bool, DbError> {
        let sql = format!(
            "SELECT COUNT(*) as count FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = '{}'",
            table_name
        );

        let row: (i64,) = sqlx::query_as(&sql).fetch_one(&self.pool).await?;

        Ok(row.0 > 0)
    }
}
