// 缓存键生成

use crate::digest::ConstraintDigest;
use crate::model::request::Constraint;
use crate::model::result::GenerationResult;

/// 缓存作用域。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CacheScope {
    FullFloor,
    Chunk(String),
    Export(String),
}

/// 结果缓存键。
///
/// 缓存键由 `schema_version`、`algorithm_version`、`seed`、`config_digest`、
/// `constraints_digest` 和 `scope` 组成。
///
/// 注意：`trace_id` 故意不包含在缓存键中，因为相同的 seed + config 应该命中缓存，
/// 不论调用方使用什么追踪标识。`trace_id` 仅用于日志串联和导出元数据关联。
/// `constraints_digest` 必须参与键：constraints 会实质改变输出，缺失会导致键相同而地图不同。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CacheKey {
    pub schema_version: String,
    pub algorithm_version: String,
    pub seed: u64,
    pub config_digest: String,
    pub constraints_digest: String,
    pub scope: CacheScope,
}

impl CacheKey {
    /// 为整层（`FullFloor`）生成结果构造缓存键。
    ///
    /// `scope` 固定为 [`CacheScope::FullFloor`]，签名与输出（含 [`Self::as_string`]
    /// 的 `full-floor` 键格式）保持不变，已有缓存键仍可命中。
    ///
    /// 对 `RuntimeChunked` 分块结果请使用 [`Self::for_chunk`]：分块身份
    /// （`chunk_id`）必须进入缓存键，否则不同 `requested_chunks` 的分块结果
    /// 会生成完全相同的键并相互串用。
    pub fn for_full_floor(
        result: &GenerationResult,
        constraints: &[Constraint],
    ) -> crate::error::PcgResult<Self> {
        Ok(Self {
            schema_version: result.metadata.schema_version.clone(),
            algorithm_version: result.metadata.algorithm_version.clone(),
            seed: result.metadata.seed,
            config_digest: result.metadata.config_digest.clone(),
            constraints_digest: ConstraintDigest::from_constraints(constraints)?.into_string(),
            scope: CacheScope::FullFloor,
        })
    }

    /// 为 `RuntimeChunked` 分块结果构造缓存键。
    ///
    /// 与 [`Self::for_full_floor`] 的唯一区别是 `scope` 使用
    /// [`CacheScope::Chunk`] 携带 `chunk_id`：不同分块（不同 `requested_chunks`）
    /// 因此得到不同缓存键，避免命中后返回另一个分块的地图。
    ///
    /// 这是新增的显式 opt-in 构造器；`for_full_floor` 的既有键不重算、不变。
    pub fn for_chunk(
        result: &GenerationResult,
        constraints: &[Constraint],
        chunk_id: impl Into<String>,
    ) -> crate::error::PcgResult<Self> {
        Ok(Self {
            schema_version: result.metadata.schema_version.clone(),
            algorithm_version: result.metadata.algorithm_version.clone(),
            seed: result.metadata.seed,
            config_digest: result.metadata.config_digest.clone(),
            constraints_digest: ConstraintDigest::from_constraints(constraints)?.into_string(),
            scope: CacheScope::Chunk(chunk_id.into()),
        })
    }

    pub fn as_string(&self) -> String {
        let scope = match &self.scope {
            CacheScope::FullFloor => "full-floor".to_string(),
            CacheScope::Chunk(chunk_id) => format!("chunk:{chunk_id}"),
            CacheScope::Export(format_name) => format!("export:{format_name}"),
        };
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.schema_version,
            self.algorithm_version,
            self.seed,
            self.config_digest,
            self.constraints_digest,
            scope
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::DebugBundle;
    use crate::model::result::ResultMetadata;
    use crate::model::room::RoomGraph;

    fn sample_result() -> GenerationResult {
        GenerationResult {
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
        }
    }

    #[test]
    fn test_cache_key_string_contains_scope() {
        let result = sample_result();

        let key = CacheKey::for_full_floor(&result, &[]).expect("空约束摘要不应失败");
        assert!(key.as_string().contains("full-floor"));
    }

    #[test]
    fn test_cache_key_for_chunk_distinguishes_requested_chunks() {
        // RED 场景：修复前唯一公开构造器把 scope 固定为 FullFloor，不同
        // requested_chunks 的分块结果会得到完全相同的缓存键，命中后串用地图。
        let result = sample_result();

        let chunk_a = CacheKey::for_chunk(&result, &[], "chunk-0-0").expect("空约束摘要不应失败");
        let chunk_a_again =
            CacheKey::for_chunk(&result, &[], "chunk-0-0").expect("空约束摘要不应失败");
        let chunk_b = CacheKey::for_chunk(&result, &[], "chunk-1-0").expect("空约束摘要不应失败");

        // 同一分块 id 的键稳定且相等
        assert_eq!(chunk_a, chunk_a_again);
        assert_eq!(chunk_a.as_string(), chunk_a_again.as_string());

        // 不同分块 id 的键必须不同（Hash/Eq 与字符串形式均不同）
        assert_ne!(chunk_a, chunk_b);
        assert_ne!(chunk_a.as_string(), chunk_b.as_string());
        assert_eq!(chunk_a.scope, CacheScope::Chunk("chunk-0-0".to_string()));
        assert!(chunk_a.as_string().contains("chunk:chunk-0-0"));
        assert!(chunk_b.as_string().contains("chunk:chunk-1-0"));
    }

    #[test]
    fn test_cache_key_full_floor_format_unchanged_and_distinct_from_chunk() {
        let result = sample_result();

        // for_full_floor 语义保持：scope 恒为 FullFloor，键仍以 :full-floor 结尾
        // （旧缓存键继续命中，不得因新增 for_chunk 而重算）。
        let full = CacheKey::for_full_floor(&result, &[]).expect("空约束摘要不应失败");
        assert_eq!(full.scope, CacheScope::FullFloor);
        assert!(full.as_string().ends_with(":full-floor"));

        let chunk = CacheKey::for_chunk(&result, &[], "chunk-0-0").expect("空约束摘要不应失败");
        assert_ne!(full, chunk);
        assert_ne!(full.as_string(), chunk.as_string());
    }
}
