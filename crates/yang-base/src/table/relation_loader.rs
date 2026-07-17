//! 关系字段批量加载：查询次数只与关系种类有关，不随结果行数增长。

use super::Record;
use crate::definition::FieldRef;
use crate::error::BaseError;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// 启动期校验完成的关系加载定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSpec {
    source: FieldRef,
    target: FieldRef,
    display: Vec<FieldRef>,
    #[cfg(feature = "mysql")]
    target_table_ref: yang_db::TableRef,
    #[cfg(feature = "mysql")]
    target_field_ref: yang_db::FieldRef,
    #[cfg(feature = "mysql")]
    display_field_refs: Vec<yang_db::FieldRef>,
}

impl RelationSpec {
    /// 创建关系加载定义。
    pub fn new(source: FieldRef, target: FieldRef) -> Self {
        #[cfg(feature = "mysql")]
        let target_table_ref =
            yang_db::TableRef::__from_validated_owned(target.table().as_str().to_string());
        #[cfg(feature = "mysql")]
        let target_field_ref =
            yang_db::FieldRef::__from_validated_owned(target.field().as_str().to_string());
        Self {
            source,
            target,
            display: Vec::new(),
            #[cfg(feature = "mysql")]
            target_table_ref,
            #[cfg(feature = "mysql")]
            target_field_ref,
            #[cfg(feature = "mysql")]
            display_field_refs: Vec::new(),
        }
    }

    /// 设置目标展示字段。
    pub fn display<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = FieldRef>,
    {
        self.display = fields.into_iter().collect();
        #[cfg(feature = "mysql")]
        {
            self.display_field_refs = self
                .display
                .iter()
                .map(|field| {
                    yang_db::FieldRef::__from_validated_owned(field.field().as_str().to_string())
                })
                .collect();
        }
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

/// 基于当前 App Database 的默认 MySQL 批量关系执行器。
#[cfg(feature = "mysql")]
pub struct DatabaseRelationExecutor<'a> {
    database: &'a yang_db::Database,
}

#[cfg(feature = "mysql")]
impl<'a> DatabaseRelationExecutor<'a> {
    /// 创建执行器。
    pub const fn new(database: &'a yang_db::Database) -> Self {
        Self { database }
    }
}

#[cfg(feature = "mysql")]
#[async_trait]
impl RelationBatchExecutor for DatabaseRelationExecutor<'_> {
    async fn load_batch(&self, batch: &RelationBatch) -> Result<Vec<Record>, BaseError> {
        let spec = batch.spec();
        let mut query = self
            .database
            .table(&spec.target_table_ref)
            .field(&spec.target_field_ref);
        for field in &spec.display_field_refs {
            query = query.field(field);
        }
        query
            .where_in(&spec.target_field_ref, batch.keys().to_vec())
            .select::<Record>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
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
