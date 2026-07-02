// 缓存存储

use std::collections::HashMap;

use crate::model::result::GenerationResult;

use super::key::CacheKey;

/// 简单的内存结果缓存（调用方自管工具类）。
///
/// `ResultCache` 与 `MapGenerator` 完全解耦：`generate()` 不会自动读写缓存。
/// 调用方在需要"同 seed + config 跳过重复生成"的场景下自行管理缓存的
/// 插入（`insert`）与查询（`get` / `contains`）。
///
/// # 典型用法
///
/// ```rust,ignore
/// use yang_pcg::cache::{CacheKey, ResultCache};
///
/// let mut cache = ResultCache::new();
/// let key = CacheKey::for_full_floor(&result);
/// if let Some(cached) = cache.get(&key) {
///     // 命中缓存，直接使用
/// } else {
///     // 未命中，调用 generator.generate(request) 后插入缓存
///     cache.insert(key, result);
/// }
/// ```
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ResultCache {
    entries: HashMap<CacheKey, GenerationResult>,
}

impl ResultCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: CacheKey, result: GenerationResult) {
        self.entries.insert(key, result);
    }

    pub fn get(&self, key: &CacheKey) -> Option<&GenerationResult> {
        self.entries.get(key)
    }

    pub fn contains(&self, key: &CacheKey) -> bool {
        self.entries.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::key::CacheKey;
    use crate::debug::DebugBundle;
    use crate::model::result::ResultMetadata;
    use crate::model::room::RoomGraph;

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = ResultCache::new();
        let result = GenerationResult {
            metadata: ResultMetadata {
                seed: 7,
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
        cache.insert(key.clone(), result);
        assert!(cache.contains(&key));
        assert!(cache.get(&key).is_some());
    }
}
