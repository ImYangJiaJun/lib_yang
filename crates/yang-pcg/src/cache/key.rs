// 缓存键生成

use crate::model::result::GenerationResult;

/// 缓存作用域。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheScope {
    FullFloor,
    Chunk(String),
    Export(String),
}

/// 结果缓存键。
///
/// 缓存键由 `schema_version`、`algorithm_version`、`seed`、`config_digest` 和 `scope` 组成。
///
/// 注意：`trace_id` 故意不包含在缓存键中，因为相同的 seed + config 应该命中缓存，
/// 不论调用方使用什么追踪标识。`trace_id` 仅用于日志串联和导出元数据关联。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub schema_version: String,
    pub algorithm_version: String,
    pub seed: u64,
    pub config_digest: String,
    pub scope: CacheScope,
}

impl CacheKey {
    pub fn for_full_floor(result: &GenerationResult) -> Self {
        Self {
            schema_version: result.metadata.schema_version.clone(),
            algorithm_version: result.metadata.algorithm_version.clone(),
            seed: result.metadata.seed,
            config_digest: result.metadata.config_digest.clone(),
            scope: CacheScope::FullFloor,
        }
    }

    pub fn as_string(&self) -> String {
        let scope = match &self.scope {
            CacheScope::FullFloor => "full-floor".to_string(),
            CacheScope::Chunk(chunk_id) => format!("chunk:{chunk_id}"),
            CacheScope::Export(format_name) => format!("export:{format_name}"),
        };
        format!(
            "{}:{}:{}:{}:{}",
            self.schema_version, self.algorithm_version, self.seed, self.config_digest, scope
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::DebugBundle;
    use crate::model::result::ResultMetadata;
    use crate::model::room::RoomGraph;

    #[test]
    fn test_cache_key_string_contains_scope() {
        let result = GenerationResult {
            metadata: ResultMetadata {
                seed: 42,
                config_digest: "digest".to_string(),
                schema_version: "1.0.0".to_string(),
                algorithm_version: "0.1.0".to_string(),
                target_engine_version: None,
                trace_id: None,
            },
            topology: RoomGraph {
                nodes: vec![],
                edges: vec![],
                critical_path: vec![],
                branches: vec![],
            },
            rooms: vec![],
            door_anchors: vec![],
            corridors: vec![],
            terrains: vec![],
            item_spawns: vec![],
            enemy_spawns: vec![],
            chunks: vec![],
            debug: Some(DebugBundle::default()),
        };

        let key = CacheKey::for_full_floor(&result);
        assert!(key.as_string().contains("full-floor"));
    }
}
