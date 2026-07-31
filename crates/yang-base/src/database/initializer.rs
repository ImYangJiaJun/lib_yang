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
use crate::plugin::{Plugin, PluginLifecycleStage, PluginManager};
use crate::table::{SchemaColumn, SchemaValidationReport, TableDefinition};
use sqlx::{pool::PoolConnection, MySql};
use std::sync::Arc;
use yang_db::Database;

const MIGRATION_LOCK_TIMEOUT_SECONDS: i64 = 30;

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
    /// 按显式清单或插件依赖/声明顺序排列的迁移项。
    pub entries: Vec<MigrationPlanEntry>,
}

/// 一条不可变、前向执行的数据库迁移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    version: String,
    sql: String,
    completion_check: Option<MigrationCompletionCheck>,
}

impl Migration {
    /// 声明迁移版本与单条 SQL；完整合法性由 [`MigrationManifest::new`] 统一校验。
    pub fn new(version: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            sql: sql.into(),
            completion_check: None,
        }
    }

    /// 为无法由数据库原子事务包裹的 DDL 声明完成状态探针。
    ///
    /// 探针必须完整代表 SQL 的最终效果；执行器只会在探针精确匹配时跳过 SQL 或恢复
    /// `running` 记录。普通幂等 SQL 无需声明探针。
    pub fn with_completion_check(
        mut self,
        completion_check: impl Into<MigrationCompletionCheck>,
    ) -> Self {
        self.completion_check = Some(completion_check.into());
        self
    }

    /// 返回稳定迁移版本。
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 返回冻结的迁移 SQL。
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// 返回可选的完成状态探针。
    pub fn completion_check(&self) -> Option<&MigrationCompletionCheck> {
        self.completion_check.as_ref()
    }
}

/// `ADD COLUMN` 类 DDL 的精确完成状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationColumnCheck {
    table: String,
    column: String,
    column_type: String,
    nullable: bool,
    default: Option<String>,
}

impl MigrationColumnCheck {
    /// 声明目标列的完整稳定形状。
    ///
    /// `column_type` 对应 MySQL `information_schema.columns.column_type`，例如
    /// `bigint`、`varchar(64)`；`default` 对应 `column_default`。
    pub fn new(
        table: impl Into<String>,
        column: impl Into<String>,
        column_type: impl Into<String>,
        nullable: bool,
        default: Option<&str>,
    ) -> Self {
        Self {
            table: table.into(),
            column: column.into(),
            column_type: column_type.into(),
            nullable,
            default: default.map(ToOwned::to_owned),
        }
    }

    /// 返回目标表名。
    pub fn table(&self) -> &str {
        &self.table
    }

    /// 返回目标列名。
    pub fn column(&self) -> &str {
        &self.column
    }

    /// 返回预期 MySQL 列类型。
    pub fn column_type(&self) -> &str {
        &self.column_type
    }

    /// 返回预期可空性。
    pub fn nullable(&self) -> bool {
        self.nullable
    }

    /// 返回预期默认值；`None` 表示 SQL `NULL`。
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }
}

/// 数据库迁移的类型化完成状态探针。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationCompletionCheck {
    /// 精确匹配一个 MySQL 列的类型、可空性和默认值。
    Column(MigrationColumnCheck),
    /// 精确匹配一个 MySQL CHECK 的名称、表达式和强制执行状态。
    CheckConstraint(MigrationCheckConstraint),
}

impl From<MigrationColumnCheck> for MigrationCompletionCheck {
    fn from(value: MigrationColumnCheck) -> Self {
        Self::Column(value)
    }
}

/// `ADD CONSTRAINT ... CHECK` DDL 的精确完成状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationCheckConstraint {
    table: String,
    constraint: String,
    expression: String,
    enforced: bool,
}

impl MigrationCheckConstraint {
    /// 声明目标 CHECK 的表、约束名、表达式和强制执行状态。
    ///
    /// 表达式比较忽略 MySQL 元数据中的关键字大小写、空白、标识符反引号、字符集
    /// introducer 与冗余外层括号，但不会改变字符串字面量或运算符顺序。
    pub fn new(
        table: impl Into<String>,
        constraint: impl Into<String>,
        expression: impl Into<String>,
        enforced: bool,
    ) -> Self {
        Self {
            table: table.into(),
            constraint: constraint.into(),
            expression: expression.into(),
            enforced,
        }
    }

    /// 返回目标表名。
    pub fn table(&self) -> &str {
        &self.table
    }

    /// 返回目标约束名。
    pub fn constraint(&self) -> &str {
        &self.constraint
    }

    /// 返回预期 CHECK 表达式。
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// 返回约束是否必须由 MySQL 强制执行。
    pub fn enforced(&self) -> bool {
        self.enforced
    }
}

impl From<MigrationCheckConstraint> for MigrationCompletionCheck {
    fn from(value: MigrationCheckConstraint) -> Self {
        Self::CheckConstraint(value)
    }
}

/// 一个数据库演进单元的有序、不可变迁移清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationManifest {
    module: String,
    migrations: Vec<Migration>,
}

impl MigrationManifest {
    /// 构建并校验迁移清单。
    ///
    /// module/version 必须无首尾空白且不超过迁移表列宽，SQL 不能为空；版本必须按
    /// 字典序严格递增，因此重复版本和声明乱序都会在连接数据库前失败。
    pub fn new<I>(module: impl Into<String>, migrations: I) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = Migration>,
    {
        let module = module.into();
        validate_migration_identity("module", &module)?;
        let migrations = migrations.into_iter().collect::<Vec<_>>();
        let mut previous = None;
        for migration in &migrations {
            validate_migration_identity("version", migration.version())?;
            if migration.sql().trim().is_empty() {
                return Err(BaseError::ConfigError(format!(
                    "迁移 {} 的 SQL 不能为空",
                    migration.version()
                )));
            }
            if let Some(check) = migration.completion_check() {
                validate_migration_completion_check(migration.version(), check)?;
            }
            if previous.is_some_and(|version| version >= migration.version()) {
                return Err(BaseError::ConfigError(format!(
                    "迁移版本必须严格递增且唯一: {}",
                    migration.version()
                )));
            }
            previous = Some(migration.version());
        }
        Ok(Self { module, migrations })
    }

    /// 返回迁移命名空间；写入 `_migrations.module_name`。
    pub fn module(&self) -> &str {
        &self.module
    }

    /// 按冻结声明顺序返回迁移。
    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }
}

fn validate_migration_completion_check(
    version: &str,
    check: &MigrationCompletionCheck,
) -> Result<(), BaseError> {
    match check {
        MigrationCompletionCheck::Column(check) => {
            validate_mysql_identifier("table", check.table()).map_err(|reason| {
                BaseError::ConfigError(format!("迁移 {version} 的列完成探针非法: {reason}"))
            })?;
            validate_mysql_identifier("column", check.column()).map_err(|reason| {
                BaseError::ConfigError(format!("迁移 {version} 的列完成探针非法: {reason}"))
            })?;
            if check.column_type().trim().is_empty()
                || check.column_type().trim() != check.column_type()
                || check.column_type().chars().count() > 255
            {
                return Err(BaseError::ConfigError(format!(
                    "迁移 {version} 的列完成探针类型不能为空、包含首尾空白或超过 255 个字符"
                )));
            }
        }
        MigrationCompletionCheck::CheckConstraint(check) => {
            validate_mysql_identifier("table", check.table()).map_err(|reason| {
                BaseError::ConfigError(format!("迁移 {version} 的约束完成探针非法: {reason}"))
            })?;
            validate_mysql_identifier("constraint", check.constraint()).map_err(|reason| {
                BaseError::ConfigError(format!("迁移 {version} 的约束完成探针非法: {reason}"))
            })?;
            if check.expression().trim().is_empty()
                || check.expression().trim() != check.expression()
                || check.expression().chars().count() > 4_096
                || normalize_check_expression(check.expression()).is_none()
            {
                return Err(BaseError::ConfigError(format!(
                    "迁移 {version} 的 CHECK 表达式不能为空、包含首尾空白、语法引号不完整或超过 4096 个字符"
                )));
            }
        }
    }
    Ok(())
}

fn validate_mysql_identifier(kind: &str, value: &str) -> Result<(), String> {
    let mut chars = value.chars();
    if value.chars().count() > 64
        || !chars
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        || !chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(format!(
            "{kind} 必须是 1-64 位 ASCII 字母、数字或下划线，且不能以数字开头"
        ));
    }
    Ok(())
}

fn normalize_check_expression(expression: &str) -> Option<String> {
    let characters = expression.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(expression.len());
    let mut index = 0;
    let mut in_literal = false;
    let mut backslash_delimited_literal = false;
    while index < characters.len() {
        let character = characters[index];
        if in_literal {
            normalized.push(character);
            if backslash_delimited_literal
                && character == '\\'
                && index + 1 < characters.len()
                && characters[index + 1] == '\''
            {
                normalized.pop();
                normalized.push('\'');
                in_literal = false;
                backslash_delimited_literal = false;
                index += 2;
                continue;
            }
            if !backslash_delimited_literal && character == '\\' && index + 1 < characters.len() {
                index += 1;
                normalized.push(characters[index]);
            } else if !backslash_delimited_literal && character == '\'' {
                if index + 1 < characters.len() && characters[index + 1] == '\'' {
                    index += 1;
                    normalized.push('\'');
                } else {
                    in_literal = false;
                }
            }
            index += 1;
            continue;
        }
        if character == '\'' {
            in_literal = true;
            backslash_delimited_literal = false;
            normalized.push(character);
            index += 1;
            continue;
        }
        if character == '\\' && index + 1 < characters.len() && characters[index + 1] == '\'' {
            in_literal = true;
            backslash_delimited_literal = true;
            normalized.push('\'');
            index += 2;
            continue;
        }
        if character.is_whitespace() || character == '`' {
            index += 1;
            continue;
        }
        if character == '_' {
            let mut end = index + 1;
            while end < characters.len()
                && (characters[end] == '_' || characters[end].is_ascii_alphanumeric())
            {
                end += 1;
            }
            if end < characters.len() && characters[end] == '\'' {
                index = end;
                continue;
            }
            if end + 1 < characters.len() && characters[end] == '\\' && characters[end + 1] == '\''
            {
                in_literal = true;
                backslash_delimited_literal = true;
                normalized.push('\'');
                index = end + 2;
                continue;
            }
        }
        normalized.push(character.to_ascii_lowercase());
        index += 1;
    }
    if in_literal || normalized.is_empty() {
        return None;
    }
    while expression_has_redundant_outer_parentheses(&normalized) {
        normalized = normalized.strip_prefix('(')?.strip_suffix(')')?.to_string();
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn expression_has_redundant_outer_parentheses(expression: &str) -> bool {
    if !expression.starts_with('(') || !expression.ends_with(')') {
        return false;
    }
    let characters = expression.chars().collect::<Vec<_>>();
    let mut depth = 0_i64;
    let mut in_literal = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if in_literal {
            if character == '\\' && index + 1 < characters.len() {
                index += 2;
                continue;
            }
            if character == '\'' {
                if index + 1 < characters.len() && characters[index + 1] == '\'' {
                    index += 2;
                    continue;
                }
                in_literal = false;
            }
            index += 1;
            continue;
        }
        match character {
            '\'' => in_literal = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + 1 != characters.len() {
                    return false;
                }
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        index += 1;
    }
    !in_literal && depth == 0
}

fn validate_migration_identity(kind: &str, value: &str) -> Result<(), BaseError> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(BaseError::ConfigError(format!(
            "迁移 {kind} 不能为空或包含首尾空白"
        )));
    }
    if value.chars().count() > 255 {
        return Err(BaseError::ConfigError(format!(
            "迁移 {kind} 不能超过 255 个字符"
        )));
    }
    Ok(())
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

fn update_migration_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// 计算原始迁移 SQL 的稳定 FNV-1a 64 位校验和。
fn migration_sql_checksum(sql: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    update_migration_hash(&mut hash, sql.as_bytes());
    format!("{hash:016x}")
}

/// 计算 SQL 与完成状态契约的稳定校验和。
///
/// 无探针迁移只哈希 SQL，保持历史 `_migrations` 校验和兼容。
fn migration_checksum(migration: &Migration) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    update_migration_hash(&mut hash, migration.sql().as_bytes());
    if let Some(check) = migration.completion_check() {
        match check {
            MigrationCompletionCheck::Column(check) => {
                update_migration_hash(&mut hash, b"\0column");
                for value in [check.table(), check.column(), check.column_type()] {
                    update_migration_hash(&mut hash, b"\0");
                    update_migration_hash(&mut hash, value.as_bytes());
                }
                update_migration_hash(&mut hash, &[u8::from(check.nullable())]);
                match check.default() {
                    Some(default) => {
                        update_migration_hash(&mut hash, b"\x01");
                        update_migration_hash(&mut hash, default.as_bytes());
                    }
                    None => update_migration_hash(&mut hash, b"\x00"),
                }
            }
            MigrationCompletionCheck::CheckConstraint(check) => {
                update_migration_hash(&mut hash, b"\0check-constraint");
                for value in [check.table(), check.constraint(), check.expression()] {
                    update_migration_hash(&mut hash, b"\0");
                    update_migration_hash(&mut hash, value.as_bytes());
                }
                update_migration_hash(&mut hash, &[u8::from(check.enforced())]);
            }
        }
    }
    format!("{hash:016x}")
}

fn migration_lock_name(database_name: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in database_name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("yang_base_migration_{hash:016x}")
}

fn migration_execution_error(
    module: &str,
    version: &str,
    checksum: &str,
    source: yang_db::DbError,
) -> BaseError {
    BaseError::MigrationExecutionFailed {
        module: module.to_string(),
        version: version.to_string(),
        checksum: checksum.to_string(),
        source,
    }
}

/// 数据库初始化器
///
/// 负责执行插件的数据库初始化脚本和迁移。
/// 所有数据库操作都通过 yang-db::Database 提供的方法实现。
///
/// # 字段
///
/// - `db`: 调用方显式传入并交由初始化器持有的数据库实例
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
    /// 显式拥有的数据库实例。
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
    /// let initializer = DatabaseInitializer::new(db, true);
    ///
    /// // 创建非事务模式的初始化器
    /// let db2 = Database::connect("mysql://root:password@localhost/test").await?;
    /// let initializer = DatabaseInitializer::new(db2, false);
    /// ```
    pub fn new(db: Database, use_transaction: bool) -> Self {
        Self {
            db,
            use_transaction,
        }
    }

    /// 返回底层数据库实例的引用
    ///
    pub(crate) fn db(&self) -> &Database {
        &self.db
    }

    /// 只读生成单个插件的迁移计划，不创建迁移表、不执行 SQL、不写迁移记录。
    pub async fn plan_migrations(&self, plugin: &dyn Plugin) -> Result<MigrationPlan, BaseError> {
        let migrations = plugin
            .migration_sql()
            .into_iter()
            .map(|(version, sql)| Migration::new(version, sql))
            .collect::<Vec<_>>();
        self.plan_declared_migrations(plugin.name(), &migrations)
            .await
    }

    /// 只读生成显式清单的迁移计划，不创建迁移表、不执行 SQL、不写迁移记录。
    pub async fn plan_manifest(
        &self,
        manifest: &MigrationManifest,
    ) -> Result<MigrationPlan, BaseError> {
        self.plan_declared_migrations(manifest.module(), manifest.migrations())
            .await
    }

    async fn plan_declared_migrations(
        &self,
        module: &str,
        migrations: &[Migration],
    ) -> Result<MigrationPlan, BaseError> {
        let table_exists = self
            .db()
            .table_exists(yang_db::table!("_migrations"))
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        let mut entries = Vec::new();
        for migration in migrations {
            let checksum = migration_checksum(migration);
            let record = if table_exists {
                self.load_migration_record(module, migration.version())
                    .await?
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
                module: module.to_string(),
                version: migration.version().to_string(),
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

    /// 从 MySQL information_schema 读取当前列并验证表定义的运行期字段契约。
    ///
    /// 本方法只读，不生成或执行 ALTER；数据库额外列不视为问题。
    pub async fn validate_table_definition(
        &self,
        table: &TableDefinition,
    ) -> Result<SchemaValidationReport, BaseError> {
        #[derive(sqlx::FromRow)]
        struct ColumnRow {
            column_name: String,
            data_type: String,
            column_type: String,
            is_nullable: String,
            character_maximum_length: Option<i64>,
            column_default: Option<String>,
            extra: String,
        }

        let rows: Vec<ColumnRow> = self
            .db()
            .query_with_params(
                "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length, CAST(COLUMN_DEFAULT AS CHAR) AS column_default, CAST(EXTRA AS CHAR) AS extra FROM information_schema.columns WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
                vec![serde_json::Value::String(table.name().to_string())],
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
                    row.column_default,
                )
                .with_auto_increment(
                    row.extra
                        .split_whitespace()
                        .any(|part| part.eq_ignore_ascii_case("auto_increment")),
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
        let migrations = plugin
            .migration_sql()
            .into_iter()
            .map(|(version, sql)| Migration::new(version, sql))
            .collect::<Vec<_>>();
        self.run_declared_migrations(plugin.name(), &migrations)
            .await
    }

    /// 在数据库级 advisory lock 内创建迁移记录表并执行显式迁移清单。
    ///
    /// 该入口面向独立部署作业；同一数据库的显式清单会串行执行。取得锁后，校验和
    /// 一致的 `running` 记录必然来自已经失去数据库连接的中断作业，因此会删除预留并
    /// 重跑迁移。迁移 SQL 必须自行保证幂等；MySQL DDL 仍遵循前向语义，不承诺事务回滚。
    ///
    /// advisory lock 会独占一个池连接，清单执行使用另一个连接，因此连接池上限至少为 2。
    pub async fn apply_manifest(&self, manifest: &MigrationManifest) -> Result<(), BaseError> {
        let (mut lock_connection, lock_name) =
            self.acquire_migration_lock(manifest.module()).await?;
        let operation = async {
            self.create_migration_table().await?;
            self.recover_interrupted_manifest_reservations(manifest)
                .await?;
            self.run_declared_migrations(manifest.module(), manifest.migrations())
                .await
        }
        .await;
        let release =
            Self::release_migration_lock(&mut lock_connection, &lock_name, manifest.module()).await;
        match (operation, release) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(release_error)) => {
                log::error!(
                    "迁移失败后释放数据库锁也失败: module={}, lock={}, error={}",
                    manifest.module(),
                    lock_name,
                    release_error
                );
                Err(error)
            }
        }
    }

    async fn acquire_migration_lock(
        &self,
        module: &str,
    ) -> Result<(PoolConnection<MySql>, String), BaseError> {
        if self.db().pool_status().max_size < 2 {
            return Err(BaseError::DatabaseMigrationFailed(
                module.to_string(),
                "显式迁移需要至少 2 个 MySQL 池连接以持有数据库 advisory lock".to_string(),
            ));
        }
        let mut connection = self
            .db()
            .pool()
            .acquire()
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        let database_name: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        let database_name = database_name.ok_or_else(|| {
            BaseError::DatabaseMigrationFailed(
                module.to_string(),
                "迁移连接没有选择数据库".to_string(),
            )
        })?;
        let lock_name = migration_lock_name(&database_name);
        let acquired: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK(?, ?)")
            .bind(&lock_name)
            .bind(MIGRATION_LOCK_TIMEOUT_SECONDS)
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        if acquired != Some(1) {
            return Err(BaseError::DatabaseMigrationFailed(
                module.to_string(),
                format!("等待数据库迁移锁 {lock_name} 超过 {MIGRATION_LOCK_TIMEOUT_SECONDS} 秒"),
            ));
        }
        Ok((connection, lock_name))
    }

    async fn release_migration_lock(
        connection: &mut PoolConnection<MySql>,
        lock_name: &str,
        module: &str,
    ) -> Result<(), BaseError> {
        let released: Option<i64> = sqlx::query_scalar("SELECT RELEASE_LOCK(?)")
            .bind(lock_name)
            .fetch_one(&mut **connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        if released != Some(1) {
            return Err(BaseError::DatabaseMigrationFailed(
                module.to_string(),
                format!("数据库迁移锁 {lock_name} 未被当前连接持有"),
            ));
        }
        Ok(())
    }

    async fn recover_interrupted_manifest_reservations(
        &self,
        manifest: &MigrationManifest,
    ) -> Result<(), BaseError> {
        for migration in manifest.migrations() {
            let checksum = migration_checksum(migration);
            let Some(record) = self
                .load_migration_record(manifest.module(), migration.version())
                .await?
            else {
                continue;
            };
            if record.status != "running" {
                continue;
            }
            if record.checksum.as_deref() != Some(checksum.as_str()) {
                return Err(BaseError::MigrationChecksumMismatch {
                    module: manifest.module().to_string(),
                    version: migration.version().to_string(),
                    expected: checksum,
                    actual: record.checksum,
                });
            }
            if self.migration_completion_check_matches(migration).await? {
                self.mark_migration_applied(
                    manifest.module(),
                    migration.version(),
                    checksum.as_str(),
                )
                .await?;
                log::warn!(
                    "完成探针确认中断迁移已生效: {} v{}",
                    manifest.module(),
                    migration.version()
                );
                continue;
            }
            let affected = self
                .delete_migration_reservation(
                    manifest.module(),
                    migration.version(),
                    checksum.as_str(),
                )
                .await?;
            if affected != 1 {
                return Err(BaseError::DatabaseMigrationFailed(
                    manifest.module().to_string(),
                    format!(
                        "迁移 v{} 的中断预留在数据库锁内发生变化",
                        migration.version()
                    ),
                ));
            }
            log::warn!(
                "恢复中断迁移预留并重跑幂等 SQL: {} v{}",
                manifest.module(),
                migration.version()
            );
        }
        Ok(())
    }

    async fn migration_completion_check_matches(
        &self,
        migration: &Migration,
    ) -> Result<bool, BaseError> {
        let Some(check) = migration.completion_check() else {
            return Ok(false);
        };
        match check {
            MigrationCompletionCheck::Column(check) => {
                #[derive(sqlx::FromRow)]
                struct ColumnMetadata {
                    column_type: String,
                    is_nullable: String,
                    column_default: Option<String>,
                }

                let metadata = sqlx::query_as::<_, ColumnMetadata>(
                    "SELECT CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(COLUMN_DEFAULT AS CHAR) AS column_default FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
                )
                .bind(check.table())
                .bind(check.column())
                .fetch_optional(self.db().pool())
                .await
                .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                Ok(metadata.is_some_and(|metadata| {
                    metadata
                        .column_type
                        .eq_ignore_ascii_case(check.column_type())
                        && (metadata.is_nullable == "YES") == check.nullable()
                        && metadata.column_default.as_deref() == check.default()
                }))
            }
            MigrationCompletionCheck::CheckConstraint(check) => {
                #[derive(sqlx::FromRow)]
                struct CheckMetadata {
                    check_clause: String,
                    enforced: String,
                }

                let metadata = sqlx::query_as::<_, CheckMetadata>(
                    "SELECT CAST(cc.CHECK_CLAUSE AS CHAR) AS check_clause, \
                            CAST(tc.ENFORCED AS CHAR) AS enforced \
                     FROM information_schema.table_constraints AS tc \
                     INNER JOIN information_schema.check_constraints AS cc \
                       ON cc.CONSTRAINT_SCHEMA = tc.CONSTRAINT_SCHEMA \
                      AND cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
                     WHERE tc.CONSTRAINT_SCHEMA = DATABASE() \
                       AND tc.TABLE_NAME = ? \
                       AND tc.CONSTRAINT_NAME = ? \
                       AND tc.CONSTRAINT_TYPE = 'CHECK'",
                )
                .bind(check.table())
                .bind(check.constraint())
                .fetch_optional(self.db().pool())
                .await
                .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                let expected_expression = normalize_check_expression(check.expression());
                Ok(metadata.is_some_and(|metadata| {
                    normalize_check_expression(&metadata.check_clause) == expected_expression
                        && metadata.enforced.eq_ignore_ascii_case(if check.enforced() {
                            "YES"
                        } else {
                            "NO"
                        })
                }))
            }
        }
    }

    #[allow(deprecated)]
    async fn run_declared_migrations(
        &self,
        module: &str,
        migrations: &[Migration],
    ) -> Result<(), BaseError> {
        for migration in migrations {
            let checksum = migration_checksum(migration);
            if self.validate_migration_record(
                module,
                migration.version(),
                &checksum,
                self.load_migration_record(module, migration.version())
                    .await?,
            )? {
                continue;
            }

            if let Err(reservation_error) = self
                .record_migration_with_checksum(module, migration.version(), &checksum, "running")
                .await
            {
                let record = self
                    .load_migration_record(module, migration.version())
                    .await?;
                if self.validate_migration_record(module, migration.version(), &checksum, record)? {
                    continue;
                }
                return Err(reservation_error);
            }

            if self.migration_completion_check_matches(migration).await? {
                self.mark_migration_applied(module, migration.version(), &checksum)
                    .await?;
                log::info!(
                    "完成探针确认迁移已存在，跳过 SQL: {} v{}",
                    module,
                    migration.version()
                );
                continue;
            }

            log::info!("执行迁移: {} v{}", module, migration.version());

            if let Err(source) = self.db().execute(migration.sql()).await {
                let _ = self
                    .delete_migration_reservation(module, migration.version(), &checksum)
                    .await;
                return Err(migration_execution_error(
                    module,
                    migration.version(),
                    &checksum,
                    source,
                ));
            }
            if migration.completion_check().is_some()
                && !self.migration_completion_check_matches(migration).await?
            {
                let _ = self
                    .delete_migration_reservation(module, migration.version(), &checksum)
                    .await;
                return Err(BaseError::DatabaseMigrationFailed(
                    module.to_string(),
                    format!("迁移 v{} 执行后未满足声明的完成状态", migration.version()),
                ));
            }
            self.mark_migration_applied(module, migration.version(), &checksum)
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
            let checksum = migration_sql_checksum(&sql);
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
                    &checksum,
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
    pub async fn record_migration_with_checksum(
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
    ) -> Result<u64, BaseError> {
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
            .map_err(BaseError::DatabaseExecuteFailed)
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
            &migration_sql_checksum(sql),
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
                assert_eq!(checksum, &migration_sql_checksum(sql));
                assert_ne!(checksum, &migration_sql_checksum("ALTER TABLE users"));
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

    #[test]
    fn migration_manifest_preserves_order_and_exposes_immutable_entries() {
        let manifest = MigrationManifest::new(
            "yang-system",
            [
                Migration::new("202607260001", "CREATE TABLE first_table (id BIGINT)"),
                Migration::new(
                    "202607260002",
                    "ALTER TABLE first_table ADD COLUMN name VARCHAR(64)",
                ),
            ],
        )
        .unwrap_or_else(|error| panic!("有序迁移清单应有效: {error}"));

        assert_eq!(manifest.module(), "yang-system");
        assert_eq!(
            manifest
                .migrations()
                .iter()
                .map(Migration::version)
                .collect::<Vec<_>>(),
            ["202607260001", "202607260002"]
        );
        assert_eq!(
            manifest.migrations()[0].sql(),
            "CREATE TABLE first_table (id BIGINT)"
        );
        assert!(manifest.migrations()[0].completion_check().is_none());
    }

    #[test]
    fn migration_manifest_rejects_invalid_identity_sql_duplicates_and_order() {
        let cases = [
            MigrationManifest::new(" ", [Migration::new("202607260001", "SELECT 1")]),
            MigrationManifest::new("yang-system", [Migration::new(" ", "SELECT 1")]),
            MigrationManifest::new("yang-system", [Migration::new("202607260001", " ")]),
            MigrationManifest::new(
                "yang-system",
                [
                    Migration::new("202607260001", "SELECT 1"),
                    Migration::new("202607260001", "SELECT 2"),
                ],
            ),
            MigrationManifest::new(
                "yang-system",
                [
                    Migration::new("202607260002", "SELECT 2"),
                    Migration::new("202607260001", "SELECT 1"),
                ],
            ),
        ];

        for result in cases {
            assert!(result.is_err(), "非法迁移清单必须在执行前被拒绝");
        }
    }

    #[test]
    fn migration_completion_check_is_validated_and_changes_only_opt_in_checksum() {
        let sql = "ALTER TABLE users ADD COLUMN authz_version BIGINT NOT NULL DEFAULT 1";
        let plain = Migration::new("202607260003", sql);
        let checked = Migration::new("202607260003", sql).with_completion_check(
            MigrationColumnCheck::new("users", "authz_version", "bigint", false, Some("1")),
        );
        assert_eq!(migration_checksum(&plain), migration_sql_checksum(sql));
        assert_ne!(migration_checksum(&checked), migration_checksum(&plain));

        let constraint_checked = Migration::new(
            "202607260004",
            "ALTER TABLE users ADD CONSTRAINT chk_users_status CHECK (status IN ('active','disabled'))",
        )
        .with_completion_check(MigrationCheckConstraint::new(
            "users",
            "chk_users_status",
            "status IN ('active','disabled')",
            true,
        ));
        let constraint_drift = Migration::new(
            "202607260004",
            "ALTER TABLE users ADD CONSTRAINT chk_users_status CHECK (status IN ('active','disabled'))",
        )
        .with_completion_check(MigrationCheckConstraint::new(
            "users",
            "chk_users_status",
            "status = 'active'",
            true,
        ));
        assert_ne!(
            migration_checksum(&constraint_checked),
            migration_checksum(&constraint_drift),
            "CHECK 表达式必须进入迁移校验和"
        );

        let invalid =
            MigrationManifest::new(
                "yang-system",
                [Migration::new("202607260003", sql).with_completion_check(
                    MigrationColumnCheck::new(
                        "users;DROP",
                        "authz_version",
                        "bigint",
                        false,
                        Some("1"),
                    ),
                )],
            );
        assert!(invalid.is_err(), "非法标识符必须在数据库连接前失败");

        let invalid_constraint = MigrationManifest::new(
            "yang-system",
            [
                Migration::new("202607260004", "SELECT 1").with_completion_check(
                    MigrationCheckConstraint::new("users", "chk users", "status = 'active", true),
                ),
            ],
        );
        assert!(
            invalid_constraint.is_err(),
            "非法约束名或未闭合字符串必须在数据库连接前失败"
        );
    }

    #[test]
    fn check_expression_normalization_preserves_literal_semantics() {
        assert_eq!(
            normalize_check_expression(
                r"((`status` IN (_utf8mb4\'active\', _utf8mb4\'disabled\')))"
            ),
            normalize_check_expression("status in ('active','disabled')")
        );
        assert_ne!(
            normalize_check_expression("status = 'ACTIVE'"),
            normalize_check_expression("status = 'active'"),
            "字符串字面量大小写不可被归一化"
        );
        assert!(normalize_check_expression("status = 'active").is_none());
    }
}
