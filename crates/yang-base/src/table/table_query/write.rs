//! 写入执行：写路径分 insert / update / delete / 批量 upsert 四类
//! （各自带事务内变体），含插入数据准备（默认值/时间戳/租户注入）与更新数据校验。
//!
//! 公共入口按连接池/事务后端运行时分派：MySQL 分支保持原有行为，PostgreSQL 分支
//! 复用同一套校验与守卫，仅替换底层 `yang_db::postgres::QueryBuilder`。

#![cfg(any(feature = "mysql", feature = "postgres"))]

use super::TableQuery;
use crate::database::AnyTransaction;
use crate::error::BaseError;
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

    /// 执行 INSERT 操作并返回自增主键
    ///
    /// 与 [`TableQuery::insert`] 完全一致的校验与拼 SQL 流程，但额外返回本次
    /// INSERT 产生的自增主键值，便于调用方拿到新建记录 ID。
    ///
    /// # PostgreSQL 限制
    ///
    /// PostgreSQL 没有 `last_insert_id()`，本方法通过 `INSERT ... RETURNING <主键>`
    /// 取回主键，因此要求主键是整型（`Integer` / `BigInt`）。非整型主键会返回明确的
    /// [`BaseError::ParamInvalid`] 指引，请改用 [`TableQuery::insert`] 并在插入后按
    /// 业务键查询，或直接使用 `yang_db` 的按真实类型返回接口。
    ///
    /// # 返回值
    ///
    /// - `Ok((1, id))`：插入成功，返回 (影响行数, 主键值)。表无自增列时 `id` 为 0。
    /// - `Err(BaseError)`：插入失败
    pub async fn insert_returning_id(
        self,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(record_into_columns(data))?;
        match self.backend()? {
            #[cfg(feature = "mysql")]
            yang_db::BackendKind::MySql => {
                let id = self
                    .compile_db_write_query()?
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
                Ok((1, id))
            }
            #[cfg(feature = "postgres")]
            yang_db::BackendKind::PostgreSql => {
                self.ensure_pg_integer_primary_key()?;
                let database = self.pg_database()?;
                let primary_key = self.field_ref(&self.table_config.primary_key)?;
                let query = self.register_pg_fields(
                    self.apply_pg_write_plan(database.table(&self.table_config.table_ref))?,
                    data.keys(),
                )?;
                let id = query
                    .returning(primary_key)
                    .insert(&data)
                    .await
                    .map_err(BaseError::DatabaseExecuteFailed)?;
                Ok((1, id))
            }
            other => Err(BaseError::ConfigError(format!(
                "TableQuery 不支持当前数据库后端: {other:?}"
            ))),
        }
    }

    /// 在事务中执行 INSERT 并返回自增主键
    ///
    /// 与 [`TableQuery::insert_returning_id`] 一致，但在事务内执行；PostgreSQL 的
    /// 整型主键限制同样适用。
    pub async fn insert_returning_id_in_tx(
        self,
        tx: &mut AnyTransaction,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
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
                self.ensure_pg_integer_primary_key()?;
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

    /// PostgreSQL 主键类型守卫：`insert_returning_id` 依赖 `RETURNING <整型主键>`。
    #[cfg(feature = "postgres")]
    fn ensure_pg_integer_primary_key(&self) -> Result<(), BaseError> {
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
                "PostgreSQL 的 insert_returning_id 仅支持整型主键，当前主键 {}.{primary_key} 为 {:?}；\
                 请改用 insert() 并在插入后按业务键查询，或使用 yang_db 的按真实类型返回接口",
                self.table_config.table_name, config.field_type
            ),
        ))
    }

    /// PostgreSQL 批量 UPSERT（非事务）：多行 `INSERT ... ON CONFLICT (...) DO UPDATE`。
    ///
    /// `conflict_columns` 必须显式给出，且必须命中表上的唯一索引/约束——PostgreSQL
    /// 的 `ON CONFLICT` 不做隐式推断，空切片会在触库前返回
    /// [`BaseError::ParamInvalid`]。冲突时更新全部非冲突插入列（`yang-db` 的默认
    /// 行为），返回受影响行数；空批返回 `Ok(0)`，不生成语句。
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
        self.execute_pg_upsert_batch(prepared, conflict_columns, false)
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
        self.execute_pg_upsert_batch(prepared, conflict_columns, true)
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
                self.execute_pg_upsert_batch_in_tx(tx, &prepared, columns, false)
                    .await
            }
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
                self.execute_pg_upsert_batch_in_tx(tx, &prepared, conflict_columns, true)
                    .await
            }
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(_) => Err(BaseError::ConfigError(
                "upsert_batch_do_nothing_in_tx 仅支持 PostgreSQL；MySQL 请使用 upsert_batch_in_tx 的 ON DUPLICATE KEY UPDATE 语义"
                    .to_string(),
            )),
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
    ) -> Result<Option<Vec<std::collections::HashMap<String, Value>>>, BaseError> {
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

        let mut prepared_rows = Vec::with_capacity(total);
        let mut base_keys: Option<std::collections::BTreeSet<String>> = None;
        for (index, record) in rows.into_iter().enumerate() {
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

        Ok(Some(prepared_rows))
    }

    /// 把冲突列名解析为 `yang-db` 的字段引用。
    #[cfg(feature = "postgres")]
    fn pg_conflict_refs(
        &self,
        conflict_columns: &[&str],
    ) -> Result<Vec<&yang_db::FieldRef>, BaseError> {
        conflict_columns
            .iter()
            .map(|name| self.field_ref(name))
            .collect()
    }

    /// 在连接池上执行已准备的 PostgreSQL 批量 upsert。
    #[cfg(feature = "postgres")]
    async fn execute_pg_upsert_batch(
        self,
        prepared: Vec<std::collections::HashMap<String, Value>>,
        conflict_columns: &[&str],
        do_nothing: bool,
    ) -> Result<u64, BaseError> {
        let database = self.pg_database()?;
        let conflicts = self.pg_conflict_refs(conflict_columns)?;
        let query = self.register_pg_fields(
            database.table(&self.table_config.table_ref),
            prepared[0].keys(),
        )?;
        let query = if do_nothing {
            query.do_nothing()
        } else {
            query
        };
        query
            .upsert_batch(&prepared, &conflicts)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在调用方事务内执行已准备的 PostgreSQL 批量 upsert。
    #[cfg(feature = "postgres")]
    async fn execute_pg_upsert_batch_in_tx(
        &self,
        tx: &mut yang_db::postgres::Transaction,
        prepared: &[std::collections::HashMap<String, Value>],
        conflict_columns: &[&str],
        do_nothing: bool,
    ) -> Result<u64, BaseError> {
        let conflicts = self.pg_conflict_refs(conflict_columns)?;
        let query =
            self.register_pg_fields(tx.table(&self.table_config.table_ref), prepared[0].keys())?;
        let query = if do_nothing {
            query.do_nothing()
        } else {
            query
        };
        query
            .upsert_batch(prepared, &conflicts)
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
        // 只验证提供的字段（与 INSERT 不同，UPDATE 不需要验证所有字段）
        for (field_name, value) in data {
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
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                soft_delete_field.clone(),
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
        let soft_delete = self.table_config.soft_delete_field.clone();
        match tx {
            #[cfg(feature = "mysql")]
            AnyTransaction::MySql(tx) => {
                let query = tx.table(&self.table_config.table_ref);
                if let Some(soft_delete_field) = soft_delete {
                    let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                        soft_delete_field,
                        Value::Number(chrono::Utc::now().timestamp().into()),
                    )]))?;
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
                if let Some(soft_delete_field) = soft_delete {
                    let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                        soft_delete_field,
                        Value::Number(chrono::Utc::now().timestamp().into()),
                    )]))?;
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
    use crate::table::{Field, Table};
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
            .ensure_pg_integer_primary_key()
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

        query
            .ensure_pg_integer_primary_key()
            .expect("整型主键应放行");
    }
}
