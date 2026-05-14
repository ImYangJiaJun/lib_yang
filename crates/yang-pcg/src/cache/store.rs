// 缓存存储

use std::collections::HashMap;

use crate::model::result::GenerationResult;

use super::key::CacheKey;

/// 简单的内存结果缓存。
#[derive(Debug, Default)]
pub struct ResultCache {
    entries: HashMap<String, GenerationResult>,
}

impl ResultCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: CacheKey, result: GenerationResult) {
        self.entries.insert(key.as_string(), result);
    }

    pub fn get(&self, key: &CacheKey) -> Option<&GenerationResult> {
        self.entries.get(&key.as_string())
    }

    pub fn contains(&self, key: &CacheKey) -> bool {
        self.entries.contains_key(&key.as_string())
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
