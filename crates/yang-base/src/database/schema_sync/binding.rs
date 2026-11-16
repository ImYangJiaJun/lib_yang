//! `_yang_schema_binding` 所有权登记。
//!
//! 同步器用该表区分框架创建的托管表（`managed`）与接管存量表（`adopted`），
//! 并记录最近一次同步后的结构指纹。登记只做附加写入，绝不删除表或列。

use super::driver::{SchemaConnection, SchemaDriver};
use crate::datasource::DatasourceId;
use crate::error::BaseError;
use crate::table::TableDefinition;
use sqlx::FromRow;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// 框架所有权登记表名。
pub(crate) const BINDING_TABLE: &str = "_yang_schema_binding";

#[cfg(feature = "mysql")]
const BINDING_TABLE_DDL_MYSQL: &str = "CREATE TABLE IF NOT EXISTS `_yang_schema_binding` (\
 `datasource` VARCHAR(32) NOT NULL, \
 `table_name` VARCHAR(128) NOT NULL, \
 `mode` VARCHAR(16) NOT NULL, \
 `fingerprint` VARCHAR(64) NOT NULL, \
 `adopted_at` BIGINT NOT NULL, \
 PRIMARY KEY (`datasource`, `table_name`)\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4";

#[cfg(feature = "postgres")]
const BINDING_TABLE_DDL_POSTGRES: &str = "CREATE TABLE IF NOT EXISTS \"_yang_schema_binding\" (\
 \"datasource\" VARCHAR(32) NOT NULL, \
 \"table_name\" VARCHAR(128) NOT NULL, \
 \"mode\" VARCHAR(16) NOT NULL, \
 \"fingerprint\" VARCHAR(64) NOT NULL, \
 \"adopted_at\" BIGINT NOT NULL, \
 PRIMARY KEY (\"datasource\", \"table_name\")\
)";

/// 表的所有权模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaBindingMode {
    /// 接管存量表：只允许附加变更，首次同步后保留该模式，直到框架实际演进该表。
    Adopted,
    /// 框架托管：声明式定义是唯一事实源。
    Managed,
}

impl SchemaBindingMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Adopted => "adopted",
            Self::Managed => "managed",
        }
    }

    fn parse(value: &str) -> Result<Self, BaseError> {
        match value {
            "adopted" => Ok(Self::Adopted),
            "managed" => Ok(Self::Managed),
            other => Err(BaseError::DatabaseInitFailed(format!(
                "未知 schema binding 模式: {other}"
            ))),
        }
    }
}

/// 已登记的表绑定。
#[derive(Debug, Clone)]
pub(crate) struct BindingRecord {
    pub(crate) mode: SchemaBindingMode,
    pub(crate) fingerprint: String,
}

/// 一次同步后需要写入的绑定。
#[derive(Debug, Clone)]
pub(crate) struct BindingUpdate {
    pub(crate) table: String,
    pub(crate) mode: SchemaBindingMode,
    pub(crate) fingerprint: String,
    pub(crate) adopted_at: i64,
    pub(crate) newly_bound: bool,
}

#[derive(FromRow)]
struct BindingRow {
    table_name: String,
    mode: String,
    fingerprint: String,
}

/// 幂等创建所有权登记表。
pub(crate) async fn ensure_binding_table(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
) -> Result<(), BaseError> {
    connection
        .execute(match driver {
            #[cfg(feature = "mysql")]
            SchemaDriver::MySql => BINDING_TABLE_DDL_MYSQL,
            #[cfg(feature = "postgres")]
            SchemaDriver::PostgreSql => BINDING_TABLE_DDL_POSTGRES,
        })
        .await
}

/// 读取某个逻辑数据源已登记的表绑定。
pub(crate) async fn load_bindings(
    connection: &mut SchemaConnection<'_>,
    datasource: &DatasourceId,
) -> Result<BTreeMap<String, BindingRecord>, BaseError> {
    let rows = match connection {
        #[cfg(feature = "mysql")]
        SchemaConnection::MySql(connection) => {
            sqlx::query_as::<_, BindingRow>(
                "SELECT CAST(table_name AS CHAR) AS table_name, \
                        CAST(mode AS CHAR) AS mode, \
                        CAST(fingerprint AS CHAR) AS fingerprint \
                 FROM `_yang_schema_binding` WHERE datasource = ?",
            )
            .bind(datasource.as_str())
            .fetch_all(&mut **connection)
            .await
        }
        #[cfg(feature = "postgres")]
        SchemaConnection::PostgreSql(connection) => {
            sqlx::query_as::<_, BindingRow>(
                "SELECT table_name, mode, fingerprint \
                 FROM \"_yang_schema_binding\" WHERE datasource = $1",
            )
            .bind(datasource.as_str())
            .fetch_all(&mut **connection)
            .await
        }
    }
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut bindings = BTreeMap::new();
    for row in rows {
        bindings.insert(
            row.table_name,
            BindingRecord {
                mode: SchemaBindingMode::parse(&row.mode)?,
                fingerprint: row.fingerprint,
            },
        );
    }
    Ok(bindings)
}

/// 写入/更新表绑定。
pub(crate) async fn upsert_bindings(
    connection: &mut SchemaConnection<'_>,
    datasource: &DatasourceId,
    updates: &[BindingUpdate],
) -> Result<(), BaseError> {
    for update in updates {
        let result = match connection {
            #[cfg(feature = "mysql")]
            SchemaConnection::MySql(connection) => sqlx::query(
                "INSERT INTO `_yang_schema_binding` \
                     (datasource, table_name, mode, fingerprint, adopted_at) \
                     VALUES (?, ?, ?, ?, ?) \
                     ON DUPLICATE KEY UPDATE mode = VALUES(mode), \
                     fingerprint = VALUES(fingerprint)",
            )
            .bind(datasource.as_str())
            .bind(&update.table)
            .bind(update.mode.as_str())
            .bind(&update.fingerprint)
            .bind(update.adopted_at)
            .execute(&mut **connection)
            .await
            .map(|_| ()),
            #[cfg(feature = "postgres")]
            SchemaConnection::PostgreSql(connection) => sqlx::query(
                "INSERT INTO \"_yang_schema_binding\" \
                     (datasource, table_name, mode, fingerprint, adopted_at) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT (datasource, table_name) DO UPDATE SET \
                     mode = EXCLUDED.mode, fingerprint = EXCLUDED.fingerprint",
            )
            .bind(datasource.as_str())
            .bind(&update.table)
            .bind(update.mode.as_str())
            .bind(&update.fingerprint)
            .bind(update.adopted_at)
            .execute(&mut **connection)
            .await
            .map(|_| ()),
        };
        result.map_err(|error| BaseError::DatabaseExecuteFailed(error.into()))?;
    }
    Ok(())
}

/// 计算表定义的结构指纹（FNV-1a 64 位十六进制）。
pub(crate) fn table_fingerprint(
    driver: SchemaDriver,
    definition: &TableDefinition,
) -> Result<String, BaseError> {
    let config = definition.shared_config();
    let signature = driver.dialect().table_signature(config.as_ref())?;
    Ok(format!("{:016x}", fnv1a_64(signature.as_bytes())))
}

pub(crate) fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
