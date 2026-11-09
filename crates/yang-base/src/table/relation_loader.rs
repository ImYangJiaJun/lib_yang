//! 关系字段批量加载：查询次数只与关系种类有关，不随结果行数增长。

use super::{Record, TableDefinition, TableQuery};
#[cfg(any(feature = "mysql", feature = "postgres"))]
use crate::database::AnyDatabase;
use crate::definition::FieldRef;
use crate::error::BaseError;
use async_trait::async_trait;
use serde_json::Value;
#[cfg(any(feature = "mysql", feature = "postgres"))]
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
#[cfg(any(feature = "mysql", feature = "postgres"))]
use std::sync::Arc;

/// 启动期校验完成的关系加载定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSpec {
    source: FieldRef,
    target: FieldRef,
    display: Vec<FieldRef>,
}

impl RelationSpec {
    /// 创建关系加载定义。
    pub fn new(source: FieldRef, target: FieldRef) -> Self {
        Self {
            source,
            target,
            display: Vec::new(),
        }
    }

    /// 设置目标展示字段。
    pub fn display<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = FieldRef>,
    {
        self.display = fields.into_iter().collect();
        self
    }

    /// 返回源字段。
    pub fn source(&self) -> &FieldRef {
        &self.source
    }

    /// 返回目标关联键。
    pub fn target(&self) -> &FieldRef {
        &self.target
    }

    /// 返回目标展示字段。
    pub fn display_fields(&self) -> &[FieldRef] {
        &self.display
    }
}

/// 基于当前 App Database 的跨后端批量关系执行器。
///
/// 执行器按目标表自身的 [`TableDefinition`] 复用受保护查询层，让关联加载与主查询
/// 使用同一套守卫：
/// - 目标表声明了 tenant_key 且执行器绑定了租户值时，追加该字段的等值条件；
/// - 目标表声明了 tenant_key 但执行器未绑定租户值时，fail-closed 返回
///   [`BaseError::ConfigError`]；如确需系统级跨租户查询，必须显式调用
///   [`DatabaseRelationExecutor::without_tenant_scope`] 关闭该守卫；
/// - 目标表声明了软删字段时，自动追加软删字段 `IS NULL`；
/// - 投影只保留目标关联键与当前角色可读、非隐藏的展示字段，其余展示列被裁剪。
///
/// 未登记定义的目标表会 fail-closed 返回 [`BaseError::ConfigError`]，绝不退化为
/// 绕过守卫的裸查询。
#[cfg(any(feature = "mysql", feature = "postgres"))]
pub struct DatabaseRelationExecutor<'a> {
    database: &'a AnyDatabase,
    roles: Arc<[String]>,
    roles_set: HashSet<String>,
    tenant_value: Option<Value>,
    tenant_scope_disabled: bool,
    targets: HashMap<String, TableDefinition>,
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
impl<'a> DatabaseRelationExecutor<'a> {
    /// 创建执行器；`roles` 必须与主查询使用的角色集合一致，字段裁剪依赖它。
    pub fn new<I, S>(database: &'a AnyDatabase, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let roles: Arc<[String]> =
            Arc::from(roles.into_iter().map(Into::into).collect::<Vec<String>>());
        let roles_set = roles.iter().cloned().collect();
        Self {
            database,
            roles,
            roles_set,
            tenant_value: None,
            tenant_scope_disabled: false,
            targets: HashMap::new(),
        }
    }

    /// 登记关联目标表定义；加载前必须登记，否则拒绝查询。
    pub fn target(mut self, definition: TableDefinition) -> Self {
        self.targets
            .insert(definition.name().to_string(), definition);
        self
    }

    /// 绑定主查询的租户值；目标表声明 tenant_key 时用于追加租户过滤。
    ///
    /// 未调用本方法且目标表声明了 tenant_key 时，查询默认 fail-closed；确需系统级
    /// 跨租户访问时必须显式调用 [`Self::without_tenant_scope`]。
    pub fn with_tenant(mut self, value: Value) -> Self {
        self.tenant_value = Some(value);
        self
    }

    /// 显式关闭目标表的租户守卫，允许跨租户加载关联行。
    ///
    /// 仅用于系统级/运维路径。默认行为是 fail-closed：目标表声明了 tenant_key
    /// 但未绑定租户值时直接返回 [`BaseError::ConfigError`]，避免泄露他租户数据。
    /// 调用本方法即表示调用方已审阅并接受跨租户读取范围。
    pub fn without_tenant_scope(mut self) -> Self {
        self.tenant_scope_disabled = true;
        self
    }

    /// 为单个关系批次构造受保护的目标表查询。
    fn target_query(&self, batch: &RelationBatch) -> Result<TableQuery, BaseError> {
        let spec = batch.spec();
        let table = spec.target().table().as_str();
        let definition = self.targets.get(table).ok_or_else(|| {
            BaseError::ConfigError(format!(
                "关系加载缺少目标表定义: {table}（拒绝执行无权限/租户/软删守卫的关联查询）"
            ))
        })?;
        let table_config = definition.shared_config();
        let key = spec.target().field().as_str();
        let key_config = table_config
            .get_field(key)
            .ok_or_else(|| BaseError::FieldNotFound(table.to_string(), key.to_string()))?;
        // 关联键既要参与匹配又要回传给调用方，不可读时 fail-closed。
        if !key_config.permissions.can_read(&self.roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                table.to_string(),
                key.to_string(),
                "用户无关联键读取权限".to_string(),
            ));
        }

        // 关联键恒在投影内；展示字段按 can_read + 非隐藏裁剪。
        let mut projection: Vec<&str> = vec![key];
        for field in spec.display_fields() {
            let name = field.field().as_str();
            let config = table_config
                .get_field(name)
                .ok_or_else(|| BaseError::FieldNotFound(table.to_string(), name.to_string()))?;
            if config.permissions.can_read(&self.roles_set)
                && !config.hidden
                && !projection.contains(&name)
            {
                projection.push(name);
            }
        }

        let mut query =
            TableQuery::new(table_config, self.roles.clone(), Some(self.database.pool()));
        query = query.select_fields(&projection)?;
        if let Some(tenant_field) = definition.tenant_key_field() {
            match &self.tenant_value {
                Some(value) => {
                    query = query.scope_tenant(tenant_field, value.clone())?;
                }
                None if !self.tenant_scope_disabled => {
                    return Err(BaseError::ConfigError(format!(
                        "关系加载目标表 {table} 声明了租户字段 {tenant_field}，但执行器未绑定租户值；\
                         请调用 with_tenant(...) 绑定租户，或显式 without_tenant_scope() 允许跨租户查询"
                    )));
                }
                None => {}
            }
        }
        // 关系加载是框架自有寻址：校验字段存在性与值类型，但不要求目标关联键可筛选。
        query.where_in_trusted(key, batch.keys().to_vec())
    }
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
#[async_trait]
impl RelationBatchExecutor for DatabaseRelationExecutor<'_> {
    async fn load_batch(&self, batch: &RelationBatch) -> Result<Vec<Record>, BaseError> {
        self.target_query(batch)?.all().await
    }
}

/// 单种关系的一次批量查询请求。
#[derive(Debug, Clone)]
pub struct RelationBatch {
    spec: RelationSpec,
    keys: Vec<Value>,
}

impl RelationBatch {
    /// 返回关系定义。
    pub fn spec(&self) -> &RelationSpec {
        &self.spec
    }

    /// 返回去重后的关联键。
    pub fn keys(&self) -> &[Value] {
        &self.keys
    }
}

/// 关系批次执行边界；数据库 Adapter 必须对一个 batch 只发起一次 IN 查询。
#[async_trait]
pub trait RelationBatchExecutor: Send + Sync {
    /// 加载当前关系的全部目标记录。
    async fn load_batch(&self, batch: &RelationBatch) -> Result<Vec<Record>, BaseError>;
}

/// 每个源字段对应的已批量加载记录。
pub type RelationData = HashMap<String, Vec<Record>>;

/// 预编译关系集合的批量加载器。
#[derive(Debug, Clone, Default)]
pub struct RelationLoader {
    relations: Vec<RelationSpec>,
}

impl RelationLoader {
    /// 创建批量加载器。
    pub fn new(relations: Vec<RelationSpec>) -> Self {
        Self { relations }
    }

    /// 从结果行收集并去重外键，每种关系最多形成一个 batch。
    pub fn plan(&self, rows: &[Record]) -> Vec<RelationBatch> {
        self.relations
            .iter()
            .filter_map(|relation| {
                let mut values = BTreeMap::new();
                for row in rows {
                    let Some(value) = row.get(relation.source.field().as_str()) else {
                        continue;
                    };
                    if value.is_null() {
                        continue;
                    }
                    if let Ok(key) = serde_json::to_string(value) {
                        values.entry(key).or_insert_with(|| value.clone());
                    }
                }
                (!values.is_empty()).then(|| RelationBatch {
                    spec: relation.clone(),
                    keys: values.into_values().collect(),
                })
            })
            .collect()
    }

    /// 执行批量加载；调用次数等于非空关系批次数，与输入行数无关。
    pub async fn load<E>(&self, rows: &[Record], executor: &E) -> Result<RelationData, BaseError>
    where
        E: RelationBatchExecutor,
    {
        let mut data = HashMap::new();
        for batch in self.plan(rows) {
            let source = batch.spec.source().to_string();
            data.insert(source, executor.load_batch(&batch).await?);
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{FieldName, TableName};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn field(table: &str, field: &str) -> FieldRef {
        FieldRef::new(
            TableName::new(table).expect("合法表名"),
            FieldName::new(field).expect("合法字段名"),
        )
    }

    struct CountingExecutor(AtomicUsize);

    #[async_trait]
    impl RelationBatchExecutor for CountingExecutor {
        async fn load_batch(&self, _batch: &RelationBatch) -> Result<Vec<Record>, BaseError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn query_count_depends_on_relation_kinds_not_row_count() {
        let loader = RelationLoader::new(vec![RelationSpec::new(
            field("orders", "user_id"),
            field("users", "id"),
        )]);
        let rows = (0..1_000)
            .map(|index| Record::new().set("user_id", index % 10))
            .collect::<Vec<_>>();
        let executor = CountingExecutor(AtomicUsize::new(0));
        let batches = loader.plan(&rows);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].keys().len(), 10);
        loader
            .load(&rows, &executor)
            .await
            .expect("批量关系加载应成功");
        assert_eq!(executor.0.load(Ordering::Relaxed), 1);
    }
}

#[cfg(all(test, feature = "postgres"))]
mod guard_tests {
    use super::*;
    use crate::database::AnyDatabase;
    use crate::definition::{FieldName, TableName};
    use crate::table::{Field, Table};
    use serde_json::json;
    use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

    fn field(table: &str, field: &str) -> FieldRef {
        FieldRef::new(
            TableName::new(table).expect("合法表名"),
            FieldName::new(field).expect("合法字段名"),
        )
    }

    fn lazy_database() -> AnyDatabase {
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres:postgres@127.0.0.1:55432/test")
            .expect("惰性 PG 连接池应创建成功");
        AnyDatabase::postgres(
            PgDatabase::from_pool(pool, PgDatabaseConfig::default())
                .expect("默认 PG 配置应通过校验"),
        )
    }

    fn child_definition() -> TableDefinition {
        Table::new("w3_rel_child")
            .fields([
                Field::id("id"),
                Field::string("label", 64).required(),
                Field::string("secret", 64).readable_by(["admin"]),
                Field::bigint("tenant_id").tenant_key(),
                Field::soft_delete("deleted_at"),
            ])
            .build()
            .expect("关系目标表定义应有效")
    }

    fn batch() -> RelationBatch {
        RelationBatch {
            spec: RelationSpec::new(
                field("w3_rel_parent", "child_id"),
                field("w3_rel_child", "id"),
            )
            .display([
                field("w3_rel_child", "label"),
                field("w3_rel_child", "secret"),
            ]),
            keys: vec![json!(1), json!(2)],
        }
    }

    /// 目标查询必须带上租户条件，并按当前角色裁剪不可读的展示字段。
    #[tokio::test]
    async fn target_query_applies_tenant_guard_and_trims_unreadable_columns() {
        let database = lazy_database();
        let executor = DatabaseRelationExecutor::new(&database, ["user"])
            .target(child_definition())
            .with_tenant(json!(1));

        let query = executor.target_query(&batch()).expect("目标查询应构造成功");
        let params = query.get_query_params();

        assert_eq!(
            params.fields.as_deref(),
            Some(["id".to_string(), "label".to_string()].as_slice()),
            "不可读的 secret 必须从投影中裁剪"
        );
        assert!(
            params.where_conditions.iter().any(|condition| matches!(
                condition,
                crate::table::WhereCondition::Eq { field, value }
                    if field == "tenant_id" && value == &json!(1)
            )),
            "必须追加租户等值条件: {:?}",
            params.where_conditions
        );
        assert!(
            params.where_conditions.iter().any(|condition| matches!(
                condition,
                crate::table::WhereCondition::In { field, values }
                    if field == "id" && values == &vec![json!(1), json!(2)]
            )),
            "必须追加关联键 IN 条件: {:?}",
            params.where_conditions
        );
    }

    /// 目标表声明 tenant_key 但未绑定租户值时必须 fail-closed，不能退化为全域查询。
    #[tokio::test]
    async fn target_query_fails_closed_when_tenant_scope_unbound() {
        let database = lazy_database();
        let executor =
            DatabaseRelationExecutor::new(&database, ["user"]).target(child_definition());

        let error = executor
            .target_query(&batch())
            .expect_err("未绑定租户值必须失败");
        assert!(
            matches!(error, BaseError::ConfigError(ref message)
                if message.contains("w3_rel_child") && message.contains("with_tenant")),
            "应返回提示绑定租户的 ConfigError，实际: {error:?}"
        );
    }

    /// 显式 opt-out 后才允许不带租户条件的目标查询。
    #[tokio::test]
    async fn without_tenant_scope_explicitly_allows_cross_tenant_query() {
        let database = lazy_database();
        let executor = DatabaseRelationExecutor::new(&database, ["user"])
            .target(child_definition())
            .without_tenant_scope();

        let query = executor
            .target_query(&batch())
            .expect("显式 opt-out 后应允许构造目标查询");
        let params = query.get_query_params();
        assert!(
            !params.where_conditions.iter().any(|condition| matches!(
                condition,
                crate::table::WhereCondition::Eq { field, .. } if field == "tenant_id"
            )),
            "显式 opt-out 后不应追加租户条件: {:?}",
            params.where_conditions
        );
    }

    /// 未登记目标表定义时必须 fail-closed，不能退化为无守卫裸查询。
    #[tokio::test]
    async fn missing_target_definition_fails_closed() {
        let database = lazy_database();
        let executor = DatabaseRelationExecutor::new(&database, ["user"]).with_tenant(json!(1));

        let error = executor
            .target_query(&batch())
            .expect_err("缺少目标表定义必须失败");
        assert!(
            matches!(error, BaseError::ConfigError(ref message) if message.contains("w3_rel_child")),
            "应返回目标表缺失的 ConfigError，实际: {error:?}"
        );
    }
}
