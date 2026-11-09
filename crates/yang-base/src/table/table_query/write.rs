//! 写入执行：写路径分 insert / update / delete / 批量 upsert 四类
//! （各自带事务内变体），含插入数据准备（默认值/时间戳/租户注入）与更新数据校验。
//!
//! 公共入口按连接池/事务后端运行时分派：MySQL 分支保持原有行为，PostgreSQL 分支
//! 复用同一套校验与守卫，仅替换底层 `yang_db::postgres::QueryBuilder`。

#![cfg(any(feature = "mysql", feature = "postgres"))]

use super::TableQuery;
use crate::action::builtin::add::ReturningTarget;
use crate::database::AnyTransaction;
use crate::error::BaseError;
use crate::table::field_config::Audience;
use serde_json::Value;

/// 将记录转为可写入的列映射。
///
/// MySQL 构建复用 `Record::into_columns`（既有内部契约）；仅 PostgreSQL 时直接从
/// `into_map` 收敛，避免依赖 MySQL 专属方法。
fn record_into_columns(record: crate::table::Record) -> std::collections::HashMap<String, Value> {
    #[cfg(feature = "mysql")]
    {
        record.into_columns()
    }
    #[cfg(not(feature = "mysql"))]
    {
        record.into_map().into_iter().collect()
    }
}

/// 把 RETURNING / `last_insert_id` 得到的主键值收敛为 `u64`（仅整型兼容入口使用）。
fn json_to_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .or_else(|| number.as_i64().and_then(|value| u64::try_from(value).ok())),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

/// 批量 upsert 的计划产物：一次性校验通过的待写行集与 ODKU 赋值列。
///
/// 只由 [`TableQuery::plan_upsert_batch`] 构造，是「校验已完成、只剩执行」的中间态；
/// 空批不会产出它（计划返回 `None`）。仅 MySQL 的 ODKU 路径消费该产物。
#[cfg(feature = "mysql")]
struct PreparedUpsertBatch {
    /// 逐行同构的 INSERT 行（已补默认值/时间戳、已剔自增列）。
    rows: Vec<std::collections::HashMap<String, Value>>,
    /// ODKU 赋值列（已含 `updated_at`）；列序由生成器按 INSERT 列序收敛。
    tail: Vec<String>,
}

/// PostgreSQL 批量 upsert 的计划产物：待写行 + 调用方显式提供的列集。
///
/// `provided_columns` 用来校验显式 `update_columns`：只能点名调用方真正提供的列，
/// 框架补出来的默认值 / `created_at` 不算「调用方想更新」。
#[cfg(feature = "postgres")]
#[derive(Debug)]
struct PreparedPgUpsert {
    rows: Vec<std::collections::HashMap<String, Value>>,
    provided_columns: std::collections::BTreeSet<String>,
}

impl TableQuery {
    /// 执行 INSERT 操作
    ///
    /// 插入数据到表中，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 INSERT SQL 语句
    /// 4. 执行插入操作
    ///
    /// # 返回值
    ///
    /// - `Ok(1)`：插入成功
    /// - `Err(BaseError)`：插入失败
    pub async fn insert(self, data: crate::table::Record) -> Result<u64, BaseError> {
        // 填充默认值/时间戳并校验（顺序：写权限→填充默认值→必填/类型校验）
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => {
                self.compile_db_write_query()?
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
            }
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let database = self.pg_database()?;
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
                    data.keys(),
                )?;
                // 单行插入用 `insert_batch`（无 RETURNING）：主键可以是任意类型，
                // 不要求整型自增列。
                query
                    .insert_batch(std::slice::from_ref(&data))
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
            }
            other => {
                return Err(BaseError::ConfigError(format!(
                    "TableQuery 不支持当前数据库后端: {other:?}"
                )));
            }
        }
        Ok(1)
    }

    /// 在事务中执行 INSERT 操作
    ///
    /// 与 [`TableQuery::insert`] 完全一致的写权限校验/默认值填充/时间戳/必填校验
    /// 流程，但在调用方提供的事务内执行，可与其它写操作原子提交。
    pub async fn insert_in_tx(
        self,
        tx: &mut AnyTransaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                self.apply_write_plan(query)?
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                let query =
                    self.register_pg_fields(self.apply_pg_write_plan(query)?, data.keys())?;
                query
                    .insert_batch(std::slice::from_ref(&data))
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
            }
        }
        Ok(1)
    }

    /// 执行 INSERT 并返回 `RETURNING` 结果。
    ///
    /// - [`ReturningTarget::Pk`]：只返回主键列，UUID / text / numeric 等非整型主键
    ///   也能正确回填；
    /// - [`ReturningTarget::Columns`]：直接 `RETURNING <列...>`（单列/多列均可）；
    /// - [`ReturningTarget::All`]：返回整行。
    ///
    /// # MySQL 限制
    ///
    /// MySQL 没有 `RETURNING`：只有「目标就是整型主键」的请求能由 `last_insert_id`
    /// 满足；`All`、多列或非主键列的 `Columns` 返回明确的
    /// [`BaseError::ConfigError`]，绝不用提交的输入伪造返回值。
    pub async fn insert_returning(
        self,
        data: crate::table::Record,
        target: ReturningTarget<'_>,
    ) -> Result<crate::table::Record, BaseError> {
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => {
                let primary_key = self.table_config.primary_key.clone();
                let allowed = match target {
                    ReturningTarget::Pk => true,
                    ReturningTarget::Columns(columns) => {
                        columns.len() == 1 && columns[0] == primary_key.as_str()
                    }
                    ReturningTarget::All => false,
                };
                if !allowed {
                    return Err(Self::mysql_returning_unsupported("非整型主键列或整行"));
                }
                let value = self.mysql_insert_returning_pk(&data).await?;
                Ok(crate::table::Record::new().set(primary_key, value))
            }
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => self.pg_insert_returning(&data, target).await,
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 批量 INSERT 并返回 `RETURNING` 行，保持输入顺序。
    ///
    /// PostgreSQL 走驱动的 `insert_batch_returning`：所有分片在同一事务内执行，
    /// 任一分片失败或解码失败都会整体回滚。MySQL 没有 `RETURNING`，返回明确的
    /// [`BaseError::ConfigError`]，绝不按输入伪造返回值。
    ///
    /// 空批返回空向量，不生成语句。
    #[cfg_attr(not(feature = "postgres"), allow(unused_variables))]
    pub async fn insert_batch_returning(
        self,
        data: &[crate::table::Record],
        target: ReturningTarget<'_>,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => Err(Self::mysql_batch_returning_unsupported(
                "insert_batch_returning",
            )),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let Some(rows) = self.prepare_pg_insert_rows(data)? else {
                    return Ok(Vec::new());
                };
                let database = self.pg_database()?;
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
                    rows[0].keys(),
                )?;
                self.apply_pg_returning_target(query, target)?
                    .insert_batch_returning(&rows)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 在调用方事务内批量 INSERT 并返回 `RETURNING` 行。
    ///
    /// 校验/默认值/类型规则与 [`TableQuery::insert_batch_returning`] 完全一致，
    /// 只是执行在调用方事务内。MySQL 同样明确返回 [`BaseError::ConfigError`]。
    #[cfg_attr(not(feature = "postgres"), allow(unused_variables))]
    pub async fn insert_batch_returning_in_tx(
        self,
        tx: &mut AnyTransaction,
        data: &[crate::table::Record],
        target: ReturningTarget<'_>,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(_) => Err(Self::mysql_batch_returning_unsupported(
                "insert_batch_returning_in_tx",
            )),
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let Some(rows) = self.prepare_pg_insert_rows(data)? else {
                    return Ok(Vec::new());
                };
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(tx.table(&self.table_config.table_ref))?,
                    rows[0].keys(),
                )?;
                self.apply_pg_returning_target(query, target)?
                    .insert_batch_returning(&rows)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
        }
    }

    /// 批量插入准备：逐行复用插入校验（默认值/时间戳/权限），空批返回 `Ok(None)`。
    ///
    /// 与驱动的批量 INSERT 一致，要求所有行最终列集相同；异构在触库前明确拒绝。
    #[cfg(feature = "postgres")]
    fn prepare_pg_insert_rows(
        &self,
        records: &[crate::table::Record],
    ) -> Result<Option<Vec<std::collections::HashMap<String, Value>>>, BaseError> {
        let mut rows = Vec::with_capacity(records.len());
        let mut base_keys: Option<std::collections::BTreeSet<String>> = None;
        for (index, record) in records.iter().enumerate() {
            let prepared = self.prepare_and_validate_insert(record_into_columns(record.clone()))?;
            let keys: std::collections::BTreeSet<String> = prepared.keys().cloned().collect();
            match &base_keys {
                Some(base) if *base != keys => {
                    return Err(BaseError::ParamInvalid(
                        "data".to_string(),
                        format!(
                            "批量 insert 第 {} 条记录提供的列集与第 1 条不一致",
                            index + 1
                        ),
                    ));
                }
                None => base_keys = Some(keys),
                _ => {}
            }
            rows.push(prepared);
        }
        Ok((!rows.is_empty()).then_some(rows))
    }

    /// 执行 INSERT 并返回真实类型的主键值。
    ///
    /// PostgreSQL 走 `INSERT ... RETURNING <主键>`；MySQL 走 `last_insert_id`
    /// （仅整型主键，非整型返回明确错误）。返回 `(影响行数, 主键 JSON 值)`。
    pub async fn insert_returning_pk_value(
        self,
        data: crate::table::Record,
    ) -> Result<(u64, Value), BaseError> {
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => {
                let value = self.mysql_insert_returning_pk(&data).await?;
                Ok((1, value))
            }
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let primary_key = self.table_config.primary_key.clone();
                let record = self.pg_insert_returning(&data, ReturningTarget::Pk).await?;
                let value = record.get(&primary_key).cloned().ok_or_else(|| {
                    BaseError::ConfigError(
                        "INSERT ... RETURNING 未返回主键列，无法回填主键".to_string(),
                    )
                })?;
                Ok((1, value))
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 执行 INSERT 操作并返回自增主键（整型兼容入口）
    ///
    /// 内部经 [`TableQuery::insert_returning_pk_value`] 取回真实类型主键后再转成
    /// `u64`；非整型主键在触库前返回明确的 [`BaseError::ParamInvalid`]。需要
    /// UUID / text / numeric 主键时改用 `insert_returning_pk_value()` 或
    /// [`TableQuery::insert_returning`]。
    ///
    /// # 返回值
    ///
    /// - `Ok((1, id))`：插入成功，返回 (影响行数, 主键值)。表无自增列时 `id` 为 0。
    /// - `Err(BaseError)`：插入失败或主键非整型
    pub async fn insert_returning_id(
        self,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        self.ensure_integer_primary_key()?;
        let (affected, value) = self.insert_returning_pk_value(data).await?;
        let id = json_to_u64(&value).ok_or_else(|| {
            BaseError::ParamInvalid(
                "primary_key".to_string(),
                format!(
                    "insert_returning_id 仅支持整型主键，当前主键值为 {value}；\
                     请改用 insert_returning_pk_value() 或 insert_returning(ReturningTarget::Pk)"
                ),
            )
        })?;
        Ok((affected, id))
    }

    /// 在事务中执行 INSERT 并返回自增主键
    ///
    /// 与 [`TableQuery::insert_returning_id`] 一致，但在事务内执行；整型主键限制
    /// 同样适用。
    pub async fn insert_returning_id_in_tx(
        self,
        tx: &mut AnyTransaction,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        self.ensure_integer_primary_key()?;
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                let id = self
                    .apply_write_plan(query)?
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
                Ok((1, id))
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let primary_key = self.field_ref(&self.table_config.primary_key)?;
                let query = tx.table(&self.table_config.table_ref);
                let query =
                    self.register_pg_fields(self.apply_pg_write_plan(query)?, data.keys())?;
                let id = query
                    .returning(primary_key)
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
                Ok((1, id))
            }
        }
    }

    /// 主键类型守卫：`insert_returning_id` 先把主键转成 `u64`，非整型在触库前拒绝。
    fn ensure_integer_primary_key(&self) -> Result<(), BaseError> {
        let primary_key = &self.table_config.primary_key;
        let config = self.table_config.get_field(primary_key).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), primary_key.clone())
        })?;
        if matches!(
            config.field_type,
            crate::table::FieldType::Integer | crate::table::FieldType::BigInt
        ) {
            return Ok(());
        }
        Err(BaseError::ParamInvalid(
            "primary_key".to_string(),
            format!(
                "insert_returning_id 仅支持整型主键，当前主键 {}.{primary_key} 为 {:?}；\
                 请改用 insert_returning_pk_value() / insert_returning(ReturningTarget::Pk) \
                 取回真实类型主键，或 insert() 并在插入后按业务键查询",
                self.table_config.table_name, config.field_type
            ),
        ))
    }

    /// MySQL 无 `RETURNING` 时的明确错误（不伪造返回值）。
    #[cfg(feature = "mysql")]
    fn mysql_returning_unsupported(what: &str) -> BaseError {
        BaseError::ConfigError(format!(
            "MySQL 不支持 INSERT ... RETURNING（请求 {what}）：只能取回整型主键；\
             请改用 insert_returning(ReturningTarget::Pk) 或插入后按业务键查询"
        ))
    }

    /// MySQL 无 `RETURNING` 时批量写入的明确错误（不伪造返回值）。
    #[cfg(feature = "mysql")]
    fn mysql_batch_returning_unsupported(entry: &str) -> BaseError {
        BaseError::ConfigError(format!(
            "{entry} 仅支持 PostgreSQL：MySQL 没有 RETURNING，批量写入无法按真实行回填；\
             请改用 PostgreSQL，或逐条写入后按业务键查询"
        ))
    }

    /// MySQL 整型主键回填：调用方显式提供主键时用该值，否则用 `last_insert_id`。
    #[cfg(feature = "mysql")]
    async fn mysql_insert_returning_pk(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<Value, BaseError> {
        self.ensure_integer_primary_key()?;
        let primary_key = &self.table_config.primary_key;
        if let Some(value) = data.get(primary_key).filter(|value| !value.is_null()) {
            if json_to_u64(value).is_none() {
                return Err(Self::mysql_returning_unsupported("非整型主键值"));
            }
            return Ok(value.clone());
        }
        let id = self
            .compile_db_write_query()?
            .insert(data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(Value::Number(id.into()))
    }

    /// 把 `ReturningTarget` 应用到 PostgreSQL 查询：主键 / 指定列 / 整行。
    #[cfg(feature = "postgres")]
    fn apply_pg_returning_target<'a>(
        &self,
        query: yang_db::postgres::QueryBuilder<'a>,
        target: ReturningTarget<'_>,
    ) -> Result<yang_db::postgres::QueryBuilder<'a>, BaseError> {
        match target {
            ReturningTarget::Pk => {
                Ok(query.returning(self.field_ref(&self.table_config.primary_key)?))
            }
            ReturningTarget::All => Ok(query.returning_all()),
            ReturningTarget::Columns(columns) => {
                if columns.is_empty() {
                    return Err(BaseError::ParamInvalid(
                        "columns".to_string(),
                        "ReturningTarget::Columns 至少需要一列".to_string(),
                    ));
                }
                for name in columns {
                    if !self.table_config.fields.contains_key(*name) {
                        return Err(BaseError::FieldNotFound(
                            self.table_config.table_name.clone(),
                            (*name).to_string(),
                        ));
                    }
                }
                let refs = self.pg_field_refs(columns)?;
                Ok(query.returning_columns(&refs))
            }
        }
    }

    /// PostgreSQL `INSERT ... RETURNING`：按目标列取回。
    #[cfg(feature = "postgres")]
    async fn pg_insert_returning(
        &self,
        data: &std::collections::HashMap<String, Value>,
        target: ReturningTarget<'_>,
    ) -> Result<crate::table::Record, BaseError> {
        let database = self.pg_database()?;
        let query = self.register_pg_fields(
            self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
            data.keys(),
        )?;
        self.apply_pg_returning_target(query, target)?
            .insert_returning(data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// PostgreSQL 批量 UPSERT（非事务）：多行 `INSERT ... ON CONFLICT (...) DO UPDATE`。
    ///
    /// `conflict_columns` 必须显式给出，且必须命中表上的唯一索引/约束——PostgreSQL
    /// 的 `ON CONFLICT` 不做隐式推断，空切片会在触库前返回
    /// [`BaseError::ParamInvalid`]。
    ///
    /// 冲突时更新的列使用**默认白名单**：插入列 − 冲突列 − 框架保护列（主键、租户
    /// 字段、`created_at`/`deleted_at`、`source_file`/`company` 等审计归属列）。
    /// 需要精确控制时用 [`TableQuery::upsert_batch_with_update_columns`]。
    /// 返回受影响行数；空批返回 `Ok(0)`，不生成语句。
    ///
    /// 仅 `postgres` feature 提供。MySQL 继续使用
    /// [`TableQuery::upsert_batch_in_tx`] 的 `ON DUPLICATE KEY UPDATE` 语义，
    /// 本方法不改变 MySQL 的既有行为。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch(
        self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
    ) -> Result<u64, BaseError> {
        let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
            return Ok(0);
        };
        self.execute_pg_upsert_batch(prepared, conflict_columns, None, None, false)
            .await
    }

    /// 批量 UPSERT 并返回 `RETURNING` 行（保持输入顺序）。
    ///
    /// `conflict_columns` 语义与 [`TableQuery::upsert_batch`] 完全相同：PostgreSQL
    /// 必须显式给出且命中唯一索引/约束；冲突时按默认白名单更新，返回写入后的行
    /// （新插入行与冲突更新行都会出现在结果里）。
    ///
    /// MySQL 没有 `RETURNING`，明确返回 [`BaseError::ConfigError`]。空批返回空向量。
    #[cfg_attr(not(feature = "postgres"), allow(unused_variables))]
    pub async fn upsert_batch_returning(
        self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
        target: ReturningTarget<'_>,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => Err(Self::mysql_batch_returning_unsupported(
                "upsert_batch_returning",
            )),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
                    return Ok(Vec::new());
                };
                let database = self.pg_database()?;
                let conflicts = self.pg_field_refs(conflict_columns)?;
                let query = self.register_pg_fields(
                    database.table(&self.table_config.table_ref),
                    prepared.rows[0].keys(),
                )?;
                let columns = self.resolve_pg_update_columns(&prepared, conflict_columns, None)?;
                let query = if columns.is_empty() {
                    query.do_nothing()
                } else {
                    let names: Vec<&str> = columns.iter().map(String::as_str).collect();
                    query.update_columns(&self.pg_field_refs(&names)?)
                };
                self.apply_pg_returning_target(query, target)?
                    .upsert_batch_returning(&prepared.rows, &conflicts)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// PostgreSQL 批量 UPSERT，并显式指定 `DO UPDATE SET` 的更新列白名单。
    ///
    /// 更新列必须：存在于表定义、由调用方在待写行里提供、且不包含任何冲突列；
    /// 另外主键/租户/创建时间/自动列永远不允许出现在赋值列里。配置了 `updated_at`
    /// 时会被自动追加（语义同逐行 update）。
    ///
    /// 仅 `postgres` feature 提供。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch_with_update_columns(
        self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
        update_columns: &[&str],
    ) -> Result<u64, BaseError> {
        let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
            return Ok(0);
        };
        self.execute_pg_upsert_batch(
            prepared,
            conflict_columns,
            None,
            Some(update_columns),
            false,
        )
        .await
    }

    /// PostgreSQL 批量 UPSERT，冲突行保持原值（`ON CONFLICT (...) DO NOTHING`）。
    ///
    /// 与 [`TableQuery::upsert_batch`] 相同的前置处理；返回真正插入的行数。
    /// 仅 `postgres` feature 提供。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch_do_nothing(
        self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
    ) -> Result<u64, BaseError> {
        let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
            return Ok(0);
        };
        self.execute_pg_upsert_batch(prepared, conflict_columns, None, None, true)
            .await
    }

    /// PostgreSQL 批量 UPSERT，冲突目标带部分唯一索引谓词
    /// （`ON CONFLICT (cols) WHERE (<predicate>) DO UPDATE ...`）。
    ///
    /// 前置校验与默认更新列白名单同 [`TableQuery::upsert_batch`]；`predicate` 为
    /// `yang-db` 的受信 SQL 表达式（拒绝分号 / 注释标记 / NUL）。表声明租户字段时，
    /// 冲突列仍必须包含该字段。
    ///
    /// 仅 `postgres` feature 提供。需要在调用方事务内执行时用
    /// [`TableQuery::upsert_batch_in_tx_with_conflict_predicate`]。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch_with_conflict_predicate(
        self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
        predicate: Option<&str>,
    ) -> Result<u64, BaseError> {
        let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
            return Ok(0);
        };
        self.execute_pg_upsert_batch(prepared, conflict_columns, predicate, None, false)
            .await
    }

    /// 在调用方事务内批量 upsert，返回受影响行数。
    ///
    /// 第三参数按运行时后端解释：
    ///
    /// - **MySQL**：`INSERT ... ON DUPLICATE KEY UPDATE` 的赋值列白名单（既有语义，
    ///   未改变）；
    /// - **PostgreSQL**：`ON CONFLICT` 的冲突列，必填且必须命中唯一索引/约束。
    ///   冲突时更新全部非冲突插入列（`yang-db` 默认）；调用方不得依赖隐式冲突推断。
    ///
    /// 空批返回 `Ok(0)`，不生成语句。MySQL 的校验/默认值/时间戳/ODKU 行为与既有
    /// 实现完全一致。
    ///
    /// # MySQL 调用方义务（框架不兜、也兜不了）
    ///
    /// - 冲突键由**表上的唯一索引**决定，本方法无法指定；
    /// - 本方法**没有 WHERE**。身份列（如 `source_key`）必须由调用方自己从
    ///   `update_columns` 里排除：把它放进赋值列，跨源命中就会把归属**静默改写**——
    ///   而逐行路径的 `where_eq(id)+where_eq(owner)` 至少还会撞唯一索引响亮失败。
    /// - 调用方应在事务前做归属预检，并在同一事务内做写后归属再断言。
    ///
    /// # MySQL 返回值
    ///
    /// `rows_affected` 之和 ∈ `[N, 2N]`（1=插入，2=更新且值有变；sqlx 带
    /// `CLIENT_FOUND_ROWS` ⇒「命中但值未变」也计 1）。**不是行数**（要行数用
    /// `rows.len()`），也**分不出** inserted/updated。
    ///
    /// # PostgreSQL 返回值
    ///
    /// 插入与更新各计 1 行；`DO NOTHING` 跳过的冲突行不计入。
    pub async fn upsert_batch_in_tx(
        self,
        tx: &mut AnyTransaction,
        rows: Vec<crate::table::Record>,
        columns: &[&str],
    ) -> Result<u64, BaseError> {
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                // 空批 = 写 0 行：没写就没写，不发明语句（生成器也会拒空数据）。
                let Some(batch) = self.plan_upsert_batch(rows, columns)? else {
                    return Ok(0);
                };
                // 不走 apply_write_plan：写路径计划只挂 WHERE 与软删守卫，而本方法两者都不支持
                // （plan_upsert_batch 已把带 where / 软删表的调用拒掉），挂上去只会给人
                // 「守卫已应用」的错觉。
                tx.table(&self.table_config.table_ref)
                    .upsert_batch(&batch.rows, &batch.tail)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let Some(prepared) = self.prepare_pg_upsert_rows(rows, columns)? else {
                    return Ok(0);
                };
                self.execute_pg_upsert_batch_in_tx(tx, &prepared, columns, None, None, false)
                    .await
            }
        }
    }

    /// 在调用方事务内批量 UPSERT，并显式指定 PostgreSQL 的更新列白名单。
    ///
    /// 与 [`TableQuery::upsert_batch_with_update_columns`] 的校验/默认值/时间戳
    /// 规则完全一致，只是执行在调用方事务内。仅 `postgres` feature 提供。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch_in_tx_with_update_columns(
        self,
        tx: &mut AnyTransaction,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
        update_columns: &[&str],
    ) -> Result<u64, BaseError> {
        match tx {
            AnyTransaction::Postgres(tx) => {
                let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
                    return Ok(0);
                };
                self.execute_pg_upsert_batch_in_tx(
                    tx,
                    &prepared,
                    conflict_columns,
                    None,
                    Some(update_columns),
                    false,
                )
                .await
            }
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(_) => Err(BaseError::ConfigError(
                "upsert_batch_in_tx_with_update_columns 仅支持 PostgreSQL；MySQL 请使用 upsert_batch_in_tx(rows, update_columns) 的 ON DUPLICATE KEY UPDATE 语义"
                    .to_string(),
            )),
        }
    }

    /// 在调用方事务内执行 PostgreSQL 批量 UPSERT，冲突行保持原值
    /// （`ON CONFLICT (...) DO NOTHING`）。
    ///
    /// 仅 `postgres` feature 提供；MySQL 事务返回明确的
    /// [`BaseError::ConfigError`]（MySQL 的 `DO NOTHING` 等价入口是 ODKU 的空赋值
    /// 白名单，语义不同，因此不在这里伪装）。
    #[cfg(feature = "postgres")]
    pub async fn upsert_batch_do_nothing_in_tx(
        self,
        tx: &mut AnyTransaction,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
    ) -> Result<u64, BaseError> {
        match tx {
            AnyTransaction::Postgres(tx) => {
                let Some(prepared) = self.prepare_pg_upsert_rows(rows, conflict_columns)? else {
                    return Ok(0);
                };
                self.execute_pg_upsert_batch_in_tx(tx, &prepared, conflict_columns, None, None, true)
                    .await
            }
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(_) => Err(BaseError::ConfigError(
                "upsert_batch_do_nothing_in_tx 仅支持 PostgreSQL；MySQL 请使用 upsert_batch_in_tx 的 ON DUPLICATE KEY UPDATE 语义"
                    .to_string(),
            )),
        }
    }

    /// 在调用方事务内批量 upsert，并为冲突目标指定部分唯一索引谓词
    /// （`ON CONFLICT (<columns>) WHERE (<predicate>) DO ...`）。
    ///
    /// `predicate` 为 `None` 时与原 [`TableQuery::upsert_batch_in_tx`] 完全一致。
    ///
    /// - **PostgreSQL**：把谓词透传给 `yang-db` 的
    ///   `postgres::QueryBuilder::on_conflict_with_predicate`。
    /// - **MySQL**：`ON DUPLICATE KEY UPDATE` 没有部分索引冲突谓词。传入
    ///   `Some(_)` 时明确返回 [`BaseError::ConfigError`]，绝不静默丢弃谓词。
    ///
    /// 表声明租户字段时，冲突列必须包含该字段（PostgreSQL 校验）。
    pub async fn upsert_batch_in_tx_with_conflict_predicate(
        self,
        tx: &mut AnyTransaction,
        rows: Vec<crate::table::Record>,
        columns: &[&str],
        predicate: Option<&str>,
    ) -> Result<u64, BaseError> {
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                if let Some(predicate) = predicate {
                    return Err(BaseError::ConfigError(format!(
                        "MySQL 不支持 ON DUPLICATE KEY UPDATE 的冲突谓词（收到 `{predicate}`）；\
                         请改用 PostgreSQL，或去掉谓词后使用 upsert_batch_in_tx"
                    )));
                }
                let Some(batch) = self.plan_upsert_batch(rows, columns)? else {
                    return Ok(0);
                };
                tx.table(&self.table_config.table_ref)
                    .upsert_batch(&batch.rows, &batch.tail)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let Some(prepared) = self.prepare_pg_upsert_rows(rows, columns)? else {
                    return Ok(0);
                };
                self.execute_pg_upsert_batch_in_tx(tx, &prepared, columns, predicate, None, false)
                    .await
            }
        }
    }

    /// PostgreSQL 批量 upsert 的准备：逐行复用插入校验，返回 `Ok(None)` 表示空批。
    ///
    /// 与 MySQL 的 [`TableQuery::plan_upsert_batch`] 保持同一失败面（where / 软删
    /// 守卫无法表达），差别只在冲突列：`ON CONFLICT` 的冲突列必须由调用方显式
    /// 给出，且每行待写数据都必须包含这些列。
    #[cfg(feature = "postgres")]
    fn prepare_pg_upsert_rows(
        &self,
        rows: Vec<crate::table::Record>,
        conflict_columns: &[&str],
    ) -> Result<Option<PreparedPgUpsert>, BaseError> {
        let total = rows.len();
        if total == 0 {
            return Ok(None);
        }

        // ON CONFLICT 没有 WHERE：带条件的调用会被静默忽略（写错行远比报错危险）。
        if !self.query_params.where_conditions.is_empty() {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                "批量 upsert 不支持 where 条件：ON CONFLICT 没有 WHERE，条件会被静默忽略"
                    .to_string(),
            ));
        }
        if self.table_config.soft_delete_field.is_some() && !self.include_trashed {
            return Err(BaseError::ParamInvalid(
                "soft_delete_field".to_string(),
                "该表配置了软删字段，而批量 upsert 无法表达软删守卫；要么改用 update_in_tx，要么先 with_trashed()"
                    .to_string(),
            ));
        }
        if conflict_columns.is_empty() {
            return Err(BaseError::ParamInvalid(
                "conflict_columns".to_string(),
                "PostgreSQL 批量 upsert 必须显式指定冲突列（ON CONFLICT 目标）".to_string(),
            ));
        }
        for name in conflict_columns {
            if !self.table_config.fields.contains_key(*name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    (*name).to_string(),
                ));
            }
        }
        // 表声明了租户字段时，冲突键必须包含它：否则同一业务唯一键可能命中他租户行，
        // 默认/显式 update 列就会跨租户改写。
        if let Some(tenant_field) = self.tenant_key_field() {
            if !conflict_columns.contains(&tenant_field) {
                return Err(BaseError::ParamInvalid(
                    "conflict_columns".to_string(),
                    format!(
                        "表 {} 声明了租户字段 `{tenant_field}`，批量 upsert 的冲突列必须包含该字段，否则会跨租户改写",
                        self.table_config.table_name
                    ),
                ));
            }
        }

        let mut prepared_rows = Vec::with_capacity(total);
        let mut base_keys: Option<std::collections::BTreeSet<String>> = None;
        let mut provided_columns = std::collections::BTreeSet::new();
        for (index, record) in rows.into_iter().enumerate() {
            // prepare 之前先抓调用方提供的列：之后补出来的默认值 / created_at
            // 不能算「调用方想更新」。
            let provided: std::collections::BTreeSet<String> =
                record.as_map().keys().cloned().collect();
            if index == 0 {
                provided_columns = provided.clone();
            } else if provided != provided_columns {
                return Err(BaseError::ParamInvalid(
                    "rows".to_string(),
                    format!(
                        "批量 upsert 第 {} 条记录提供的列集与第 1 条不一致",
                        index + 1
                    ),
                ));
            }

            let prepared = self.prepare_and_validate_insert(record_into_columns(record))?;
            let keys: std::collections::BTreeSet<String> = prepared.keys().cloned().collect();
            match &base_keys {
                Some(base) if *base != keys => {
                    return Err(BaseError::ParamInvalid(
                        "rows".to_string(),
                        format!(
                            "批量 upsert 第 {} 条记录提供的列集与第 1 条不一致",
                            index + 1
                        ),
                    ));
                }
                None => base_keys = Some(keys),
                _ => {}
            }
            for name in conflict_columns {
                if !prepared.contains_key(*name) {
                    return Err(BaseError::ParamInvalid(
                        "conflict_columns".to_string(),
                        format!("冲突列 `{name}` 不在第 {} 条待写记录里", index + 1),
                    ));
                }
            }
            prepared_rows.push(prepared);
        }

        Ok(Some(PreparedPgUpsert {
            rows: prepared_rows,
            provided_columns,
        }))
    }

    /// 把列名解析为 `yang-db` 的字段引用。
    #[cfg(feature = "postgres")]
    fn pg_field_refs(&self, columns: &[&str]) -> Result<Vec<&yang_db::FieldRef>, BaseError> {
        columns.iter().map(|name| self.field_ref(name)).collect()
    }

    /// 返回表定义中声明为 `tenant_key` 的租户隔离字段。
    #[cfg(feature = "postgres")]
    fn tenant_key_field(&self) -> Option<&str> {
        self.table_config
            .fields
            .iter()
            .find_map(|(name, field)| field.tenant_key.then_some(name.as_str()))
    }

    /// 框架保护的列：主键 / 租户 / 自动列 / 创建时间 / 软删时间，upsert 一律不得覆盖。
    fn is_framework_owned_column(&self, name: &str) -> bool {
        if name == self.table_config.primary_key {
            return true;
        }
        if let Some((tenant_field, _)) = &self.tenant_scope {
            if tenant_field == name {
                return true;
            }
        }
        if self
            .table_config
            .get_field(name)
            .is_some_and(|field| field.tenant_key || field.auto_increment)
        {
            return true;
        }
        if let Some(ts) = &self.table_config.timestamp_fields {
            if ts.created_at.as_deref() == Some(name) || ts.deleted_at.as_deref() == Some(name) {
                return true;
            }
        }
        matches!(name, "created_at" | "tenant_id")
    }

    /// 默认 upsert 白名单额外排除的审计/归属列（显式白名单仍可由调用方点名）。
    #[cfg(feature = "postgres")]
    fn is_default_upsert_excluded_column(&self, name: &str) -> bool {
        self.is_framework_owned_column(name) || matches!(name, "source_file" | "company")
    }

    /// PostgreSQL 默认 upsert 更新列白名单：插入列 − 冲突列 − 框架保护列。
    ///
    /// 审计/归属列（`created_at`、`source_file`、`company`、租户、主键）默认不覆盖，
    /// 避免一次 upsert 把创建时间或归属重置掉。
    #[cfg(feature = "postgres")]
    fn default_pg_update_columns(
        &self,
        insert_columns: &[String],
        conflict_columns: &[&str],
    ) -> Vec<String> {
        insert_columns
            .iter()
            .filter(|name| !conflict_columns.contains(&name.as_str()))
            .filter(|name| !self.is_default_upsert_excluded_column(name))
            .cloned()
            .collect()
    }

    /// 校验显式更新列并去重：必须存在于表、由调用方提供、且不是冲突列。
    #[cfg(feature = "postgres")]
    fn validate_pg_update_columns(
        &self,
        provided_columns: &std::collections::BTreeSet<String>,
        conflict_columns: &[&str],
        update_columns: &[&str],
    ) -> Result<Vec<String>, BaseError> {
        if update_columns.is_empty() {
            return Err(BaseError::ParamInvalid(
                "update_columns".to_string(),
                "批量 upsert 至少需要一个可更新列".to_string(),
            ));
        }
        let mut columns = Vec::with_capacity(update_columns.len());
        for name in update_columns {
            if !self.table_config.fields.contains_key(*name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    (*name).to_string(),
                ));
            }
            if conflict_columns.contains(name) {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!("更新列 `{name}` 是冲突列，不能出现在 DO UPDATE SET 里"),
                ));
            }
            if !provided_columns.contains(*name) {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!("更新列 `{name}` 不在调用方提供的待写列里"),
                ));
            }
            if self.is_framework_owned_column(name) {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!("更新列 `{name}` 是主键/租户/创建时间等框架保护列，不能被 upsert 覆盖"),
                ));
            }
            if !columns.iter().any(|existing| existing == name) {
                columns.push((*name).to_string());
            }
        }
        // 与逐行 update 一致：配置了 updated_at 时自动追加。
        if let Some(updated_at) = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|fields| fields.updated_at.as_ref())
            .filter(|name| self.table_config.fields.contains_key(*name))
        {
            if !columns.iter().any(|existing| existing == updated_at) {
                columns.push(updated_at.clone());
            }
        }
        Ok(columns)
    }

    /// 解析本次 upsert 的 `DO UPDATE SET` 列：显式白名单或默认白名单。
    #[cfg(feature = "postgres")]
    fn resolve_pg_update_columns(
        &self,
        prepared: &PreparedPgUpsert,
        conflict_columns: &[&str],
        update_columns: Option<&[&str]>,
    ) -> Result<Vec<String>, BaseError> {
        match update_columns {
            Some(explicit) => self.validate_pg_update_columns(
                &prepared.provided_columns,
                conflict_columns,
                explicit,
            ),
            None => {
                let insert_columns: Vec<String> = prepared.rows[0].keys().cloned().collect();
                Ok(self.default_pg_update_columns(&insert_columns, conflict_columns))
            }
        }
    }

    /// 在连接池上执行已准备的 PostgreSQL 批量 upsert。
    #[cfg(feature = "postgres")]
    async fn execute_pg_upsert_batch(
        self,
        prepared: PreparedPgUpsert,
        conflict_columns: &[&str],
        conflict_predicate: Option<&str>,
        update_columns: Option<&[&str]>,
        do_nothing: bool,
    ) -> Result<u64, BaseError> {
        let database = self.pg_database()?;
        let conflicts = self.pg_field_refs(conflict_columns)?;
        let query = self.register_pg_fields(
            database.table(&self.table_config.table_ref),
            prepared.rows[0].keys(),
        )?;
        let query = match conflict_predicate {
            Some(predicate) => query.on_conflict_with_predicate(&conflicts, predicate),
            None => query,
        };
        let query = if do_nothing {
            query.do_nothing()
        } else {
            let columns =
                self.resolve_pg_update_columns(&prepared, conflict_columns, update_columns)?;
            if columns.is_empty() {
                // 全部插入列都是冲突列/受保护列：冲突行没有可写列，等价 DO NOTHING。
                query.do_nothing()
            } else {
                let names: Vec<&str> = columns.iter().map(String::as_str).collect();
                let refs = self.pg_field_refs(&names)?;
                query.update_columns(&refs)
            }
        };
        query
            .upsert_batch(&prepared.rows, &conflicts)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在调用方事务内执行已准备的 PostgreSQL 批量 upsert。
    #[cfg(feature = "postgres")]
    async fn execute_pg_upsert_batch_in_tx(
        &self,
        tx: &mut yang_db::postgres::Transaction,
        prepared: &PreparedPgUpsert,
        conflict_columns: &[&str],
        conflict_predicate: Option<&str>,
        update_columns: Option<&[&str]>,
        do_nothing: bool,
    ) -> Result<u64, BaseError> {
        let conflicts = self.pg_field_refs(conflict_columns)?;
        let query = self.register_pg_fields(
            tx.table(&self.table_config.table_ref),
            prepared.rows[0].keys(),
        )?;
        let query = match conflict_predicate {
            Some(predicate) => query.on_conflict_with_predicate(&conflicts, predicate),
            None => query,
        };
        let query = if do_nothing {
            query.do_nothing()
        } else {
            let columns =
                self.resolve_pg_update_columns(prepared, conflict_columns, update_columns)?;
            if columns.is_empty() {
                query.do_nothing()
            } else {
                let names: Vec<&str> = columns.iter().map(String::as_str).collect();
                let refs = self.pg_field_refs(&names)?;
                query.update_columns(&refs)
            }
        };
        query
            .upsert_batch(&prepared.rows, &conflicts)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 校验并准备一批 MySQL ODKU upsert：`Ok(None)` = 空批（写 0 行），
    /// `Ok(Some(_))` = 可执行计划。
    ///
    /// 失败面顺序即调用方踩坑的顺序：空批 → where → 软删 → 逐行列集 → 赋值列。
    #[cfg(feature = "mysql")]
    fn plan_upsert_batch(
        &self,
        rows: Vec<crate::table::Record>,
        update_columns: &[&str],
    ) -> Result<Option<PreparedUpsertBatch>, BaseError> {
        let total = rows.len();
        let mut records = rows.into_iter();
        // 空批 = 写 0 行：没有第一行可推赋值列，也不发明语句。
        let Some(first) = records.next() else {
            return Ok(None);
        };

        // ODKU 没有 WHERE：带条件的调用会被静默忽略（写错行远比报错危险），直接拒绝。
        if !self.query_params.where_conditions.is_empty() {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                "批量 upsert 不支持 where 条件：ON DUPLICATE KEY UPDATE 没有 WHERE，条件会被静默忽略"
                    .to_string(),
            ));
        }
        // 软删守卫是 `deleted_at IS NULL` 这类 WHERE，批量 upsert 表达不了：命中已软删行时
        // 会把它们改活，而逐行 update 路径上这会被守卫挡住。
        if self.table_config.soft_delete_field.is_some() && !self.include_trashed {
            return Err(BaseError::ParamInvalid(
                "soft_delete_field".to_string(),
                "该表配置了软删字段，而批量 upsert 无法表达软删守卫；要么改用 update_in_tx，要么先 with_trashed()"
                    .to_string(),
            ));
        }

        // 第 1 条记录的两套列集就是全批基准：provided 管「省略即重置」，prepared 管列集
        // 异构。先取 provided 再 prepare——prepare 会把默认值与时间戳补进来，之后就分不出
        // 「调用方写过这一列」与「框架补的」了。
        let first_provided: std::collections::BTreeSet<String> =
            first.as_map().keys().cloned().collect();
        let first_prepared = self.prepare_and_validate_insert(record_into_columns(first))?;
        let first_prepared_keys: std::collections::BTreeSet<String> =
            first_prepared.keys().cloned().collect();

        let mut prepared_rows = Vec::with_capacity(total);
        prepared_rows.push(first_prepared);
        for (offset, record) in records.enumerate() {
            let provided: std::collections::BTreeSet<String> =
                record.as_map().keys().cloned().collect();
            let prepared = self.prepare_and_validate_insert(record_into_columns(record))?;
            let prepared_keys: std::collections::BTreeSet<String> =
                prepared.keys().cloned().collect();
            // 第 1 条在循环外已处理，这里从第 2 条起数。
            let row_number = offset + 2;
            // 某行省略了带默认值的列 ⇒ 批量路径会拿第 1 行的值替它做 ODKU 赋值，等价于
            // 「省略即重置」（prepared 两侧都被默认值补齐，只看 prepared 拦不住）。
            if provided != first_provided {
                return Err(BaseError::ParamInvalid(
                    "rows".to_string(),
                    format!("批量 upsert 第 {row_number} 条记录提供的列集与第 1 条不一致"),
                ));
            }
            // 列集异构交给生成器只会得到一条更晚、更难归因的 SerializationError。
            if prepared_keys != first_prepared_keys {
                return Err(BaseError::ParamInvalid(
                    "rows".to_string(),
                    format!("批量 upsert 第 {row_number} 条记录提供的列集与第 1 条不一致"),
                ));
            }
            prepared_rows.push(prepared);
        }

        if update_columns.is_empty() {
            return Err(BaseError::ParamInvalid(
                "update_columns".to_string(),
                "批量 upsert 至少需要一个可更新列（ON DUPLICATE KEY UPDATE 的赋值列)".to_string(),
            ));
        }
        for name in update_columns {
            // 要求 ⊆ provided 而不是 ⊆ prepared：prepared 里有框架补出来的默认值与
            // created_at，让它们进赋值列就等于「省略即重置」「创建时间被刷新」。
            // created_at 不必单独排除：它是 Audience::Nobody，调用方提交会先吃
            // FieldPermissionDenied，永远进不了 provided，上一条已把它挡住。
            if !first_provided.contains(*name) {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!(
                        "赋值列 `{name}` 不在调用方提供的列里（框架补出来的默认值与 created_at 不能当赋值列）"
                    ),
                ));
            }
            if self
                .table_config
                .get_field(name)
                .is_some_and(|field| field.auto_increment)
            {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!("赋值列 `{name}` 是自增列，不能出现在 ON DUPLICATE KEY UPDATE 里"),
                ));
            }
        }
        // 与 PostgreSQL 默认白名单同一套守卫：主键/租户/创建时间/审计归属列
        // 一律不许进 ODKU 赋值列。
        for name in update_columns {
            if self.is_framework_owned_column(name) {
                return Err(BaseError::ParamInvalid(
                    "update_columns".to_string(),
                    format!("赋值列 `{name}` 是主键/租户/创建时间等框架保护列，不能被 upsert 覆盖"),
                ));
            }
        }

        // 赋值列 = 调用方白名单（去重）+ updated_at（语义同 with_updated_timestamp：列存在
        // 且不在集合里时追加）。列序不在这里排：生成器按 INSERT 列序收敛。
        let mut tail: Vec<String> = Vec::with_capacity(update_columns.len() + 1);
        for name in update_columns {
            if !tail.iter().any(|existing| existing.as_str() == *name) {
                tail.push((*name).to_string());
            }
        }
        if let Some(updated_at) = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|fields| fields.updated_at.as_ref())
            .filter(|name| self.table_config.fields.contains_key(*name))
        {
            if !tail.iter().any(|existing| existing == updated_at) {
                tail.push(updated_at.clone());
            }
        }

        Ok(Some(PreparedUpsertBatch {
            rows: prepared_rows,
            tail,
        }))
    }

    /// 填充默认值/时间戳并验证插入数据
    ///
    /// 处理顺序（修复 required+default 字段被误报 FieldRequired 的问题）：
    /// 1. 字段与写权限校验：显式提交只读字段时拒绝；null 自增主键视为未提供
    /// 2. 规范化数据库生成字段：未提供或为 null 的自增字段交给数据库生成
    /// 3. 填充默认值：data 中缺失且配置了 `default_value` 的字段补默认值
    /// 4. 填充时间戳：`timestamp_fields` 配置且列存在、调用方未提供时，写入当前时间
    /// 5. 必填/类型/验证器校验：在补齐后的数据上执行
    pub(crate) fn prepare_and_validate_insert(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        let mut prepared = data;

        // tenant key 只能由请求上下文注入，业务输入不得覆盖。
        if let Some((field, value)) = &self.tenant_scope {
            if prepared.contains_key(field) {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止显式写入租户字段: {field}"
                )));
            }
            prepared.insert(field.clone(), value.clone());
        }

        // 1. 校验调用方显式提交的字段和写权限。只有 null 自增主键可视为“未提供”，
        // 其余只读字段即使提交 null 也必须拒绝，避免绕过字段边界并覆盖数据库默认值。
        for (field_name, value) in &prepared {
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;
            let omitted_auto_increment = field_config.auto_increment && value.is_null();
            let injected_tenant = self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name);
            if !injected_tenant
                && !omitted_auto_increment
                && !field_config.permissions.can_write(&self.user_roles_set)
            {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }
        }

        // 2. 数据库生成的自增字段未提供或为 null 时，不进入 INSERT 字段列表。
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment
                && prepared
                    .get(field_name)
                    .map(serde_json::Value::is_null)
                    .unwrap_or(true)
            {
                prepared.remove(field_name);
            }
        }

        // 3. 填充默认值（仅缺失时；显式 null 仍受 nullable/required 约束）
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment {
                continue;
            }
            if let Some(default) = &field_config.default_value {
                if !prepared.contains_key(field_name) {
                    prepared.insert(field_name.clone(), default.clone());
                }
            }
        }

        // 4. 填充创建/更新时间戳（列存在且调用方未提供时）
        if let Some(ts) = &self.table_config.timestamp_fields {
            let now = chrono::Utc::now().timestamp();
            for ts_field in [&ts.created_at, &ts.updated_at].into_iter().flatten() {
                if self.table_config.fields.contains_key(ts_field) {
                    let missing = prepared.get(ts_field).map(|v| v.is_null()).unwrap_or(true);
                    if missing {
                        prepared.insert(ts_field.clone(), Value::Number(now.into()));
                    }
                }
            }
        }

        // 5. 在补齐后的数据上执行必填/类型/验证器校验
        for (field_name, field_config) in &self.table_config.fields {
            if !field_config.permissions.can_write(&self.user_roles_set) {
                continue;
            }
            if field_config.auto_increment && !prepared.contains_key(field_name) {
                continue;
            }
            let value = prepared.get(field_name).unwrap_or(&Value::Null);
            field_config.validate(value, &self.regex_cache)?;
        }

        Ok(prepared)
    }

    /// 执行 UPDATE 操作
    ///
    /// 更新表中的数据，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 UPDATE SQL 语句
    /// 4. 应用已配置的 WHERE 条件
    /// 5. 执行更新操作
    pub async fn update(self, data: crate::table::Record) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(record_into_columns(data))?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self
                .compile_db_write_query()?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let database = self.pg_database()?;
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
                    data.keys(),
                )?;
                query
                    .update(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 在事务中执行 UPDATE 操作
    ///
    /// 与 [`TableQuery::update`] 一致的字段校验/权限/WHERE 守卫/自动 `updated_at`
    /// 逻辑，但在事务内执行。
    pub async fn update_in_tx(
        self,
        tx: &mut AnyTransaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(record_into_columns(data))?;
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                self.apply_write_plan(query)?
                    .update(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                let query =
                    self.register_pg_fields(self.apply_pg_write_plan(query)?, data.keys())?;
                query
                    .update(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
        }
    }

    fn prepare_update_data(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        self.validate_update_data(&data)?;
        self.with_updated_timestamp(data)
    }

    fn with_updated_timestamp(
        &self,
        mut data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "无可更新字段".to_string(),
            ));
        }
        if let Some(updated_at) = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|fields| fields.updated_at.as_ref())
            .filter(|name| self.table_config.fields.contains_key(*name))
        {
            data.insert(
                updated_at.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            );
        }
        Ok(data)
    }

    /// 验证更新数据
    #[cfg(test)]
    pub fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据（内部实现）
    #[cfg(not(test))]
    fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据的实际实现
    fn validate_update_data_impl(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "至少需要一个更新字段".to_string(),
            ));
        }
        self.validate_update_fields(data, None)
    }

    /// 校验待写字段的存在性/写权限/类型；`skip` 命中的列由框架自行写入，不参与校验。
    fn validate_update_fields(
        &self,
        data: &std::collections::HashMap<String, Value>,
        skip: Option<&str>,
    ) -> Result<(), BaseError> {
        // 只验证提供的字段（与 INSERT 不同，UPDATE 不需要验证所有字段）
        for (field_name, value) in data {
            if skip.is_some_and(|name| name == field_name) {
                continue;
            }
            if self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name)
            {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止修改租户字段: {field_name}"
                )));
            }
            // 1. 检查字段是否存在于表配置中
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;

            // 2. 检查用户是否有写入权限
            if !field_config.permissions.can_write(&self.user_roles_set) {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }

            // 3. 验证显式提供的字段值。部分更新不要求提交其它必填字段，但若本字段
            // 被显式设为 null，仍执行 required 约束。
            field_config.validate(value, &self.regex_cache)?;
        }

        Ok(())
    }

    /// 准备软删除写入的标记数据。
    ///
    /// 软删标记本身由框架写入，因此与逐行 update 共用同一字段校验路径，但写权限
    /// 仍保持默认 `Nobody`（纯框架托管）的软删列不参与 can_write 判定；开发者
    /// 显式把软删列声明为角色可写/全员可写时，仍按声明校验。
    fn prepare_soft_delete_data(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        let framework_managed = self.table_config.soft_delete_field.as_ref().filter(|name| {
            self.table_config
                .get_field(name)
                .is_some_and(|field| matches!(field.permissions.writable, Audience::Nobody))
        });
        self.validate_update_fields(&data, framework_managed.map(String::as_str))?;
        self.with_updated_timestamp(data)
    }

    /// 批量 upsert 的**计划**（不连库、不执行）——测试专用。
    ///
    /// 只把 [`TableQuery::plan_upsert_batch`] 的产物摊平成 `(待写行集, 赋值列)`，让无库
    /// 用例能断言「哪些列进了 INSERT、哪些列进了 ODKU 尾巴」；生产路径不经过这里。
    // 摊平元组比再发明一个测试专用结构体更省；clippy 只嫌它长。
    #[allow(clippy::type_complexity)]
    #[cfg(all(test, feature = "mysql"))]
    pub fn upsert_batch_plan_for_test(
        &self,
        rows: Vec<crate::table::Record>,
        update_columns: &[&str],
    ) -> Result<Option<(Vec<std::collections::HashMap<String, Value>>, Vec<String>)>, BaseError>
    {
        Ok(self
            .plan_upsert_batch(rows, update_columns)?
            .map(|batch| (batch.rows, batch.tail)))
    }

    /// 执行 DELETE 操作
    ///
    /// 删除表中的数据，支持软删除和物理删除两种模式：
    /// 1. 如果配置了软删除字段（soft_delete_field），执行 UPDATE 设置删除标记
    /// 2. 如果未配置软删除字段，执行物理删除
    /// 3. 应用已配置的 WHERE 条件
    pub async fn delete(self) -> Result<u64, BaseError> {
        if let Some(soft_delete_field) = self.table_config.soft_delete_field.clone() {
            let data = self.prepare_soft_delete_data(std::collections::HashMap::from([(
                soft_delete_field,
                Value::Number(chrono::Utc::now().timestamp().into()),
            )]))?;
            return self.execute_soft_delete(data).await;
        }
        self.execute_physical_delete().await
    }

    /// 软删除执行：两种后端都走 UPDATE 标记。
    async fn execute_soft_delete(
        self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<u64, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self
                .compile_db_write_query()?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let database = self.pg_database()?;
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
                    data.keys(),
                )?;
                query
                    .update(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 物理删除执行。
    async fn execute_physical_delete(self) -> Result<u64, BaseError> {
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => self
                .compile_db_write_query()?
                .delete()
                .await
                .map_err(BaseError::DatabaseExecuteFailed),
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                let database = self.pg_database()?;
                self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?
                    .delete()
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 在事务中执行 DELETE 操作
    ///
    /// 与 [`TableQuery::delete`] 一致：配置了软删除字段时走 UPDATE 标记（同样在
    /// 事务内），否则物理删除；WHERE 守卫与软删语义完全复用。
    pub async fn delete_in_tx(self, tx: &mut AnyTransaction) -> Result<u64, BaseError> {
        // 软删标记与逐行 update 走同一字段校验路径（含 can_write），再进入事务分支。
        let soft_delete_data = self
            .table_config
            .soft_delete_field
            .clone()
            .map(|field| {
                self.prepare_soft_delete_data(std::collections::HashMap::from([(
                    field,
                    Value::Number(chrono::Utc::now().timestamp().into()),
                )]))
            })
            .transpose()?;
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                if let Some(data) = soft_delete_data {
                    return self
                        .apply_write_plan(query)?
                        .update(&data)
                        .await
                        .map_err(BaseError::DatabaseExecuteFailed);
                }
                self.apply_write_plan(query)?
                    .delete()
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
            #[cfg(feature = "postgres")]
            AnyTransaction::Postgres(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                if let Some(data) = soft_delete_data {
                    let query =
                        self.register_pg_fields(self.apply_pg_write_plan(query)?, data.keys())?;
                    return query
                        .update(&data)
                        .await
                        .map_err(BaseError::DatabaseExecuteFailed);
                }
                self.apply_pg_write_plan(query)?
                    .delete()
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)
            }
        }
    }
}

#[cfg(all(test, feature = "postgres"))]
mod pg_tests {
    use super::*;
    use crate::table::{Field, Record, Table};
    use std::sync::Arc;

    fn lazy_query(table: Table) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(Vec::<String>::new()),
            None,
        )
    }

    /// PostgreSQL 的 insert_returning_id 依赖 RETURNING 整型主键；字符串主键必须
    /// 明确报错并给出可执行指引，不能静默委托给底层返回含糊错误。
    #[test]
    fn insert_returning_id_rejects_non_integer_primary_key() {
        let query = lazy_query(Table::new("pg_string_pk").fields([
            Field::string("code", 32).primary_key().required(),
            Field::string("name", 32).required(),
        ]));

        let error = query
            .ensure_integer_primary_key()
            .expect_err("字符串主键必须被拒绝");
        match error {
            BaseError::ParamInvalid(field, message) => {
                assert_eq!(field, "primary_key");
                assert!(
                    message.contains("insert_returning_id"),
                    "错误信息应指引到 insert_returning_id: {message}"
                );
                assert!(
                    message.contains("insert()"),
                    "错误信息应给出替代入口: {message}"
                );
            }
            other => panic!("应返回 ParamInvalid，实际: {other:?}"),
        }
    }

    /// 整型主键（Integer / BigInt）必须放行。
    #[test]
    fn insert_returning_id_accepts_integer_primary_key() {
        let query = lazy_query(
            Table::new("pg_int_pk").fields([Field::id("id"), Field::string("name", 32).required()]),
        );

        query.ensure_integer_primary_key().expect("整型主键应放行");
    }

    /// 表声明了租户字段时，`ON CONFLICT` 的显式冲突列必须包含它，否则可能命中
    /// 他租户行并被 DO UPDATE 改写。
    #[test]
    fn pg_upsert_requires_tenant_key_in_conflict_columns() {
        let query = lazy_query(Table::new("pg_upsert_tenant_conflict").fields([
            Field::id("id"),
            Field::bigint("tenant_id").tenant_key(),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
        ]));
        let rows = vec![Record::new()
            .set("tenant_id", 1_i64)
            .set("option_key", "opt_a")
            .set("label", "x")];

        let error = query
            .prepare_pg_upsert_rows(rows, &["option_key"])
            .expect_err("缺少租户冲突列必须拒绝");
        match error {
            BaseError::ParamInvalid(field, message) => {
                assert_eq!(field, "conflict_columns");
                assert!(
                    message.contains("tenant_id"),
                    "错误信息应点名租户字段: {message}"
                );
            }
            other => panic!("应返回 ParamInvalid，实际: {other:?}"),
        }
    }

    /// 冲突列包含租户字段时放行，且默认更新列白名单必须排除租户列。
    #[test]
    fn pg_default_update_columns_exclude_tenant_key() {
        let query = lazy_query(Table::new("pg_upsert_tenant_whitelist").fields([
            Field::id("id"),
            Field::bigint("tenant_id").tenant_key(),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
        ]));
        let rows = vec![Record::new()
            .set("tenant_id", 1_i64)
            .set("option_key", "opt_a")
            .set("label", "x")];
        let prepared = query
            .prepare_pg_upsert_rows(rows, &["tenant_id", "option_key"])
            .expect("含租户冲突列应通过校验")
            .expect("非空批应产出计划");
        let insert_columns: Vec<String> = prepared.rows[0].keys().cloned().collect();
        let update_columns =
            query.default_pg_update_columns(&insert_columns, &["tenant_id", "option_key"]);

        assert!(
            !update_columns.contains(&"tenant_id".to_string()),
            "默认白名单不得包含租户列: {update_columns:?}"
        );
        assert!(
            update_columns.contains(&"label".to_string()),
            "非保护业务列应在默认白名单内: {update_columns:?}"
        );
    }
}

#[cfg(all(test, feature = "mysql"))]
mod batch_returning_mysql_tests {
    use super::*;
    use crate::database::AnyPool;
    use crate::table::{Field, Record, Table};
    use std::sync::Arc;

    fn lazy_mysql_query(table: Table) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .connect_lazy("mysql://root@127.0.0.1:3306/test")
            .expect("懒连接池构造不应触发网络连接");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(Vec::<String>::new()),
            Some(AnyPool::MySql(Arc::new(pool))),
        )
    }

    /// MySQL 没有 `RETURNING`：批量 insert 必须明确返回 ConfigError，绝不降级为
    /// 「按输入伪造返回值」。
    #[tokio::test]
    async fn insert_batch_returning_rejects_mysql() {
        let query = lazy_mysql_query(
            Table::new("tq_batch_returning_mysql")
                .fields([Field::id("id"), Field::string("label", 32).required()]),
        );

        let error = query
            .insert_batch_returning(&[Record::new().set("label", "x")], ReturningTarget::Pk)
            .await
            .expect_err("MySQL 批量 RETURNING 必须被拒绝");
        assert!(
            matches!(
                error,
                BaseError::ConfigError(ref message) if message.contains("insert_batch_returning")
            ),
            "应返回 ConfigError 并点名入口，实际: {error:?}"
        );
    }

    /// 批量 upsert 的 RETURNING 同样在 MySQL 上明确拒绝。
    #[tokio::test]
    async fn upsert_batch_returning_rejects_mysql() {
        let query = lazy_mysql_query(Table::new("tq_batch_upsert_returning_mysql").fields([
            Field::id("id"),
            Field::string("option_key", 32).required(),
            Field::string("label", 32).required(),
        ]));

        let error = query
            .upsert_batch_returning(
                vec![Record::new().set("option_key", "k").set("label", "x")],
                &["option_key"],
                ReturningTarget::Pk,
            )
            .await
            .expect_err("MySQL 批量 upsert RETURNING 必须被拒绝");
        assert!(
            matches!(
                error,
                BaseError::ConfigError(ref message) if message.contains("upsert_batch_returning")
            ),
            "应返回 ConfigError 并点名入口，实际: {error:?}"
        );
    }
}

#[cfg(test)]
mod delete_permission_tests {
    use super::*;
    use crate::table::{Field, Table};
    use std::sync::Arc;

    fn query_for(table: Table, roles: &[&str]) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(
                roles
                    .iter()
                    .map(|role| (*role).to_string())
                    .collect::<Vec<String>>(),
            ),
            None,
        )
    }

    /// 软删写入必须走与逐行 update 相同的字段写权限校验：显式声明了写角色的
    /// 软删列，对无该角色的用户必须返回权限错误。
    #[tokio::test]
    async fn soft_delete_respects_explicit_soft_delete_write_audience() {
        let query = query_for(
            Table::new("tq_soft_delete_guard").fields([
                Field::id("id"),
                Field::soft_delete("deleted_at").writable_by(["admin"]),
            ]),
            &["user"],
        );

        let error = query
            .delete()
            .await
            .expect_err("无写权限的软删列必须被拒绝");
        assert!(
            matches!(
                error,
                BaseError::FieldPermissionDenied(_, ref field, _) if field == "deleted_at"
            ),
            "应返回 deleted_at 的 FieldPermissionDenied，实际: {error:?}"
        );
    }

    /// 默认 `Field::soft_delete`（写权限 Nobody，仅框架可写）是框架托管列：
    /// 守卫应放行并继续执行，而不是被自己的删除标记挡住（无连接池时停在初始化错误）。
    #[tokio::test]
    async fn framework_managed_soft_delete_passes_field_permission_guard() {
        let query = query_for(
            Table::new("tq_soft_delete_framework")
                .fields([Field::id("id"), Field::soft_delete("deleted_at")]),
            &["user"],
        );

        let error = query
            .delete()
            .await
            .expect_err("无连接池应在守卫通过后才报错");
        assert!(
            matches!(error, BaseError::DatabaseNotInitialized),
            "默认软删列不应触发字段权限错误，实际: {error:?}"
        );
    }
}
