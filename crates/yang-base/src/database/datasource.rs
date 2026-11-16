//! 具名数据源注册表。
//!
//! [`DatasourceRegistry`] 是 `Tools` 的唯一数据库资源容器：一个应用可以持有多个
//! MySQL / PostgreSQL 连接池，模块通过逻辑数据源名解析自己的库。默认源保留既有
//! 单库语义，旧调用方无需改动。

use crate::database::AnyDatabase;
use crate::datasource::DatasourceId;
use crate::error::BaseError;
use crate::tools::ResourceHealth;
use std::collections::BTreeMap;
use yang_db::{BackendKind, DbError, PoolStatus};

/// 单个具名数据源。
#[derive(Debug)]
pub struct Datasource {
    id: DatasourceId,
    database: AnyDatabase,
    required: bool,
}

impl Datasource {
    /// 用逻辑名与数据库容器构造数据源；容器内部身份会被覆盖为同一逻辑名。
    pub fn new(id: DatasourceId, database: AnyDatabase, required: bool) -> Self {
        Self {
            database: database.with_id(id.clone()),
            id,
            required,
        }
    }

    /// 返回逻辑数据源名。
    pub fn id(&self) -> &DatasourceId {
        &self.id
    }

    /// 返回底层数据库容器。
    pub fn database(&self) -> &AnyDatabase {
        &self.database
    }

    /// readiness 是否要求该源健康。
    pub fn is_required(&self) -> bool {
        self.required
    }

    /// 返回连接池状态快照。
    pub fn pool_status(&self) -> PoolStatus {
        self.database.pool_status()
    }

    /// 执行健康检查。
    pub async fn health_check(&self) -> Result<bool, DbError> {
        self.database.health_check().await
    }

    /// 幂等关闭连接池。
    pub async fn close(&self) {
        self.database.close().await;
    }
}

/// 单个数据源的连接池状态快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasourcePoolStatus {
    /// 逻辑数据源名。
    pub datasource: DatasourceId,
    /// 物理后端。
    pub backend: BackendKind,
    /// 连接池状态。
    pub status: PoolStatus,
}

/// 单个数据源的健康快照（比 `ToolsHealth` 的扁平资源列表更易按源读取）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasourceHealth {
    /// 逻辑数据源名。
    pub datasource: DatasourceId,
    /// 物理后端。
    pub backend: BackendKind,
    /// 是否健康。
    pub healthy: bool,
    /// 失败原因；健康时为 `None`。
    pub detail: Option<String>,
    /// 是否参与 readiness。
    pub required: bool,
}

/// 应用持有的全部具名数据源。
#[derive(Debug)]
pub struct DatasourceRegistry {
    default: DatasourceId,
    order: Vec<DatasourceId>,
    by_id: BTreeMap<DatasourceId, Datasource>,
}

impl DatasourceRegistry {
    /// 构造注册表。
    ///
    /// - `default` 为 `None` 时：只有一个源则自动成为默认源，多个源则报错；
    /// - 重名、默认源不存在均报错。
    pub fn new(
        default: Option<DatasourceId>,
        datasources: Vec<Datasource>,
    ) -> Result<Self, BaseError> {
        if datasources.is_empty() {
            return Err(BaseError::ConfigError("数据源注册表不能为空".to_string()));
        }
        let mut order = Vec::with_capacity(datasources.len());
        let mut by_id = BTreeMap::new();
        for datasource in datasources {
            let id = datasource.id().clone();
            if by_id.insert(id.clone(), datasource).is_some() {
                return Err(BaseError::ConfigError(format!("数据源重复注册: {id}")));
            }
            order.push(id);
        }
        let default = match default {
            Some(default) => {
                if !by_id.contains_key(&default) {
                    return Err(BaseError::ConfigError(format!(
                        "默认数据源未注册: {default}"
                    )));
                }
                default
            }
            None if order.len() == 1 => order[0].clone(),
            None => {
                return Err(BaseError::ConfigError(
                    "注册了多个数据源时必须显式指定默认数据源".to_string(),
                ))
            }
        };
        Ok(Self {
            default,
            order,
            by_id,
        })
    }

    /// 返回默认数据源名。
    pub fn default_id(&self) -> &DatasourceId {
        &self.default
    }

    /// 返回默认数据源。
    pub fn default_datasource(&self) -> &Datasource {
        // 构造期已保证默认源存在。
        &self.by_id[&self.default]
    }

    /// 按逻辑名查找数据源。
    pub fn get(&self, id: &DatasourceId) -> Option<&Datasource> {
        self.by_id.get(id)
    }

    /// 解析可选数据源名：`None` 表示默认源。
    pub fn resolve(&self, id: Option<&DatasourceId>) -> Result<&Datasource, BaseError> {
        match id {
            Some(id) => self.resolve_id(id),
            None => Ok(self.default_datasource()),
        }
    }

    /// 解析数据源名：字面量 `default` 在未注册同名源时回退到注册表的默认源。
    pub fn resolve_id(&self, id: &DatasourceId) -> Result<&Datasource, BaseError> {
        if let Some(datasource) = self.get(id) {
            return Ok(datasource);
        }
        if id.is_default() {
            return Ok(self.default_datasource());
        }
        Err(BaseError::ConfigError(format!("数据源未注册: {id}")))
    }

    /// 是否包含指定数据源。
    pub fn contains(&self, id: &DatasourceId) -> bool {
        self.by_id.contains_key(id)
    }

    /// 数据源数量。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空（构造期已禁止，保留 API 一致性）。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// 按注册顺序迭代。
    pub fn iter(&self) -> impl Iterator<Item = &Datasource> {
        self.order.iter().filter_map(|id| self.by_id.get(id))
    }

    /// 逐源健康检查；只包含参与 readiness 的 required 源。
    pub async fn health_check(&self) -> Vec<ResourceHealth> {
        let mut resources = Vec::with_capacity(self.order.len());
        for datasource in self.iter().filter(|datasource| datasource.is_required()) {
            resources.push(resource_health(datasource).await);
        }
        resources
    }

    /// 逐源健康快照（带逻辑名与 required 标记）。
    pub async fn health(&self) -> Vec<DatasourceHealth> {
        let mut resources = Vec::with_capacity(self.order.len());
        for datasource in self.iter() {
            let result = datasource.health_check().await;
            resources.push(DatasourceHealth {
                datasource: datasource.id().clone(),
                backend: datasource.database().backend(),
                healthy: matches!(result, Ok(true)),
                detail: match result {
                    Ok(true) => None,
                    Ok(false) => Some("健康检查返回 false".to_string()),
                    Err(error) => Some(error.to_string()),
                },
                required: datasource.is_required(),
            });
        }
        resources
    }

    /// 逐源连接池状态。
    pub fn pool_statuses(&self) -> Vec<DatasourcePoolStatus> {
        self.iter()
            .map(|datasource| DatasourcePoolStatus {
                datasource: datasource.id().clone(),
                backend: datasource.database().backend(),
                status: datasource.pool_status(),
            })
            .collect()
    }

    /// 按注册逆序关闭所有连接池。
    pub async fn close(&self) {
        for id in self.order.iter().rev() {
            if let Some(datasource) = self.by_id.get(id) {
                datasource.close().await;
            }
        }
    }
}

async fn resource_health(datasource: &Datasource) -> ResourceHealth {
    let resource = match datasource.database().backend() {
        yang_db::BackendKind::MySql => "mysql",
        yang_db::BackendKind::PostgreSql => "postgres",
        _ => "database",
    };
    match datasource.health_check().await {
        Ok(healthy) => ResourceHealth {
            resource,
            healthy,
            detail: if healthy {
                None
            } else {
                Some(format!("数据源 {} 健康检查返回 false", datasource.id()))
            },
        },
        Err(error) => ResourceHealth {
            resource,
            healthy: false,
            detail: Some(format!("数据源 {}: {error}", datasource.id())),
        },
    }
}

#[cfg(all(test, feature = "mysql"))]
mod tests {
    use super::*;

    fn lazy_mysql() -> AnyDatabase {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .connect_lazy("mysql://root:test@127.0.0.1:3306/test")
            .expect("惰性 MySQL 池应创建成功");
        AnyDatabase::mysql(
            yang_db::mysql::Database::from_pool(pool, yang_db::mysql::DatabaseConfig::default())
                .expect("默认配置应有效"),
        )
    }

    fn datasource(name: &str) -> Datasource {
        Datasource::new(
            DatasourceId::new(name).expect("测试数据源名应有效"),
            lazy_mysql(),
            true,
        )
    }

    #[tokio::test]
    async fn single_source_becomes_default() {
        let registry = DatasourceRegistry::new(None, vec![datasource("default")])
            .expect("单源应自动成为默认源");
        assert!(registry.default_id().is_default());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn multiple_sources_require_explicit_default() {
        let error = DatasourceRegistry::new(None, vec![datasource("a"), datasource("b")])
            .expect_err("多源必须显式默认");
        assert!(error.to_string().contains("默认数据源"));
    }

    #[tokio::test]
    async fn duplicate_names_are_rejected() {
        let error = DatasourceRegistry::new(
            Some(DatasourceId::default_source()),
            vec![datasource("default"), datasource("default")],
        )
        .expect_err("重复数据源应被拒绝");
        assert!(error.to_string().contains("重复注册"));
    }

    #[tokio::test]
    async fn resolve_falls_back_to_default_and_reports_unknown() {
        let registry = DatasourceRegistry::new(
            Some(DatasourceId::default_source()),
            vec![datasource("default"), datasource("pg_main")],
        )
        .expect("两源应构建成功");
        assert!(registry.resolve(None).is_ok());
        let unknown = DatasourceId::new("missing").expect("名称应有效");
        assert!(registry.resolve(Some(&unknown)).is_err());
    }
}
