// 运行时分块生成测试
// 验证 RuntimeChunked 和 HybridPrecompute 模式的正确性

use crate::config::{CapabilityFlags, ChunkingConfig, GenerationConfig, GenerationMode};
use crate::generator::MapGenerator;
use crate::model::request::{GenerationRequest, RuntimeContext};

/// 创建 RuntimeChunked 模式的测试配置
fn make_runtime_chunked_config() -> GenerationConfig {
    GenerationConfig {
        generation_mode: GenerationMode::RuntimeChunked,
        capability_flags: CapabilityFlags {
            runtime_chunked: true,
            hybrid_precompute: false,
            grammar_support: false,
            debug_output: false,
        },
        chunking: ChunkingConfig {
            chunk_size: 32,
            enabled: true,
        },
        ..Default::default()
    }
}

/// 创建 HybridPrecompute 模式的测试配置
fn make_hybrid_config() -> GenerationConfig {
    GenerationConfig {
        generation_mode: GenerationMode::HybridPrecompute,
        capability_flags: CapabilityFlags {
            runtime_chunked: false,
            hybrid_precompute: true,
            grammar_support: false,
            debug_output: false,
        },
        chunking: ChunkingConfig {
            chunk_size: 32,
            enabled: true,
        },
        ..Default::default()
    }
}

// ========== Task 23.1: RuntimeChunked 模式增量生成 ==========

#[test]
fn test_generate_chunk_basic() {
    // 验证 RuntimeChunked 模式可以正常生成
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![], // 空列表表示生成所有分块
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result = generator.generate_chunk(request).expect("分块生成应成功");

    // 验证结果非空
    assert!(!result.rooms.is_empty(), "应有房间生成");
    assert!(!result.terrains.is_empty(), "应有地形生成");
    assert!(!result.chunks.is_empty(), "应有分块元数据");
    assert_eq!(result.metadata.seed, 42);
}

#[test]
fn test_generate_chunk_specific_chunks() {
    // 验证可以请求特定分块
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    // 先生成一次获取分块 ID
    let request_all = GenerationRequest {
        seed: Some(42),
        config: config.clone(),
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result_all = generator
        .generate_chunk(request_all)
        .expect("全量分块生成应成功");
    assert!(!result_all.chunks.is_empty());

    // 请求第一个分块
    let first_chunk_id = result_all.chunks[0].id.clone();
    let request_one = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![first_chunk_id.clone()],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result_one = generator
        .generate_chunk(request_one)
        .expect("单分块生成应成功");

    // 单分块结果的房间数应不超过全量结果
    assert!(result_one.rooms.len() <= result_all.rooms.len());
    // 单分块结果的房间应都属于请求的分块
    let chunk = result_all
        .chunks
        .iter()
        .find(|c| c.id == first_chunk_id)
        .unwrap();
    for room in &result_one.rooms {
        assert!(
            chunk.room_ids.contains(&room.id),
            "房间 {} 应属于分块 {}",
            room.id,
            first_chunk_id
        );
    }
}

#[test]
fn test_generate_chunk_reuses_topology() {
    // 验证分块生成复用拓扑（相同 seed 下拓扑一致）
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    let request1 = GenerationRequest {
        seed: Some(42),
        config: config.clone(),
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let request2 = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result1 = generator.generate_chunk(request1).expect("生成应成功");
    let result2 = generator.generate_chunk(request2).expect("生成应成功");

    // 拓扑应完全一致
    assert_eq!(result1.topology.nodes.len(), result2.topology.nodes.len());
    assert_eq!(result1.topology.edges.len(), result2.topology.edges.len());
    assert_eq!(
        result1.topology.critical_path,
        result2.topology.critical_path
    );
}

// ========== Task 23.2: HybridPrecompute 模式 ==========

#[test]
fn test_generate_topology_only() {
    // 验证 generate_topology_only 仅生成拓扑和布局
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo_result = generator
        .generate_topology_only(request)
        .expect("拓扑预计算应成功");

    // 验证拓扑和布局已生成
    assert!(!topo_result.topology.nodes.is_empty(), "应有拓扑节点");
    assert!(!topo_result.layout.rooms.is_empty(), "应有布局房间");
    assert!(
        !topo_result.layout.door_anchors.is_empty(),
        "应有门锚点"
    );
    assert!(!topo_result.layout.corridors.is_empty(), "应有走廊");
    assert!(!topo_result.chunks.is_empty(), "应有分块元数据");
}

#[test]
fn test_fill_chunk_details() {
    // 验证 fill_chunk_details 可以为分块填充细节
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo_result = generator
        .generate_topology_only(request)
        .expect("拓扑预计算应成功");

    // 为第一个分块填充细节
    let first_chunk_id = &topo_result.chunks[0].id;
    let detail = generator
        .fill_chunk_details(&topo_result, first_chunk_id)
        .expect("分块细节填充应成功");

    assert_eq!(detail.chunk_id, *first_chunk_id);
    assert!(!detail.terrains.is_empty(), "应有地形生成");
    assert!(!detail.partial, "无预算限制时不应为部分结果");
}

#[test]
fn test_fill_chunk_details_invalid_chunk_id() {
    // 验证请求不存在的分块 ID 时返回错误
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo_result = generator
        .generate_topology_only(request)
        .expect("拓扑预计算应成功");

    let result = generator.fill_chunk_details(&topo_result, "nonexistent-chunk");
    assert!(result.is_err(), "不存在的分块 ID 应返回错误");
}

// ========== Task 23.3: 时间预算/迭代预算限制 ==========

#[test]
fn test_iteration_budget_limits_generation() {
    // 验证迭代预算限制生效
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let mut topo_result = generator
        .generate_topology_only(request)
        .expect("拓扑预计算应成功");

    // 设置迭代预算为 1（只处理 1 个房间）
    topo_result.normalized = topo_result.normalized.with_iteration_budget(1);

    let first_chunk_id = &topo_result.chunks[0].id.clone();
    let chunk = topo_result
        .chunks
        .iter()
        .find(|c| c.id == *first_chunk_id)
        .unwrap();

    // 只有当分块内有多个房间时，预算限制才会产生 partial 效果
    if chunk.room_ids.len() > 1 {
        let detail = generator
            .fill_chunk_details(&topo_result, first_chunk_id)
            .expect("带预算的分块细节填充应成功");

        assert!(
            detail.partial,
            "迭代预算为 1 且分块有多个房间时应为部分结果"
        );
        assert_eq!(
            detail.terrains.len(),
            1,
            "迭代预算为 1 时应只生成 1 个房间的地形"
        );
    }
}

#[test]
fn test_time_budget_field_exists() {
    // 验证时间预算字段可以设置
    let config = GenerationConfig::default();
    let normalized = config.normalize().expect("默认配置应有效");

    assert!(normalized.time_budget_ms.is_none(), "默认时间预算应为 None");
    assert!(
        normalized.iteration_budget.is_none(),
        "默认迭代预算应为 None"
    );

    let with_budget = normalized.with_time_budget(100).with_iteration_budget(10);
    assert_eq!(with_budget.time_budget_ms, Some(100));
    assert_eq!(with_budget.iteration_budget, Some(10));
}

// ========== Task 23.4: 相同 Chunk 重复请求返回一致结果 ==========

#[test]
fn test_chunk_determinism_same_seed_config() {
    // 验证相同 seed + config + chunk_id 下结果确定性
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request1 = GenerationRequest {
        seed: Some(12345),
        config: config.clone(),
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let request2 = GenerationRequest {
        seed: Some(12345),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo1 = generator
        .generate_topology_only(request1)
        .expect("生成应成功");
    let topo2 = generator
        .generate_topology_only(request2)
        .expect("生成应成功");

    // 拓扑应一致
    assert_eq!(topo1.topology.nodes.len(), topo2.topology.nodes.len());
    assert_eq!(topo1.topology.critical_path, topo2.topology.critical_path);
    assert_eq!(topo1.chunks.len(), topo2.chunks.len());

    // 对每个分块填充细节，验证结果一致
    for chunk in &topo1.chunks {
        let detail1 = generator
            .fill_chunk_details(&topo1, &chunk.id)
            .expect("填充应成功");
        let detail2 = generator
            .fill_chunk_details(&topo2, &chunk.id)
            .expect("填充应成功");

        assert_eq!(
            detail1.terrains.len(),
            detail2.terrains.len(),
            "分块 {} 的地形数量应一致",
            chunk.id
        );
        assert_eq!(
            detail1.item_spawns.len(),
            detail2.item_spawns.len(),
            "分块 {} 的交互物点位数量应一致",
            chunk.id
        );
        assert_eq!(
            detail1.enemy_spawns.len(),
            detail2.enemy_spawns.len(),
            "分块 {} 的敌人点位数量应一致",
            chunk.id
        );

        // 验证点位坐标一致
        for (s1, s2) in detail1.item_spawns.iter().zip(detail2.item_spawns.iter()) {
            assert_eq!(s1.grid_pos, s2.grid_pos, "交互物点位坐标应一致");
            assert_eq!(s1.room_id, s2.room_id, "交互物点位房间 ID 应一致");
        }
        for (s1, s2) in detail1.enemy_spawns.iter().zip(detail2.enemy_spawns.iter()) {
            assert_eq!(s1.grid_pos, s2.grid_pos, "敌人点位坐标应一致");
            assert_eq!(s1.room_id, s2.room_id, "敌人点位房间 ID 应一致");
        }
    }
}

#[test]
fn test_runtime_chunked_determinism() {
    // 验证 RuntimeChunked 模式下相同请求返回一致结果
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    let make_request = || GenerationRequest {
        seed: Some(99),
        config: config.clone(),
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result1 = generator.generate_chunk(make_request()).expect("生成应成功");
    let result2 = generator.generate_chunk(make_request()).expect("生成应成功");

    // 验证结果完全一致
    assert_eq!(result1.rooms.len(), result2.rooms.len());
    assert_eq!(result1.terrains.len(), result2.terrains.len());
    assert_eq!(result1.item_spawns.len(), result2.item_spawns.len());
    assert_eq!(result1.enemy_spawns.len(), result2.enemy_spawns.len());
    assert_eq!(
        result1.metadata.config_digest,
        result2.metadata.config_digest
    );

    // 验证房间 ID 顺序一致
    let room_ids1: Vec<&str> = result1.rooms.iter().map(|r| r.id.as_str()).collect();
    let room_ids2: Vec<&str> = result2.rooms.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(room_ids1, room_ids2);
}

// ========== Task 23.5: 分块模式与整层模式一致性测试 ==========

#[test]
fn test_chunked_vs_full_floor_consistency() {
    // 验证分块结果合并后与整层结果语义一致
    // 注意：由于 RNG 派生路径不同，我们验证的是结构一致性而非逐字节一致
    let generator = MapGenerator::new();
    let seed = 42u64;

    // 整层生成（OfflineFullFloor 模式）
    let full_config = GenerationConfig {
        chunking: ChunkingConfig {
            chunk_size: 32,
            enabled: true,
        },
        ..Default::default()
    };

    let full_request = GenerationRequest {
        seed: Some(seed),
        config: full_config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let full_result = generator.generate(full_request).expect("整层生成应成功");

    // HybridPrecompute 模式：先拓扑，再逐块填充
    let hybrid_config = make_hybrid_config();
    let hybrid_request = GenerationRequest {
        seed: Some(seed),
        config: hybrid_config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo_result = generator
        .generate_topology_only(hybrid_request)
        .expect("拓扑预计算应成功");

    // 验证拓扑一致性
    assert_eq!(
        full_result.topology.nodes.len(),
        topo_result.topology.nodes.len(),
        "拓扑节点数应一致"
    );
    assert_eq!(
        full_result.topology.edges.len(),
        topo_result.topology.edges.len(),
        "拓扑边数应一致"
    );
    assert_eq!(
        full_result.topology.critical_path,
        topo_result.topology.critical_path,
        "关键路径应一致"
    );

    // 验证布局一致性
    assert_eq!(
        full_result.rooms.len(),
        topo_result.layout.rooms.len(),
        "房间数应一致"
    );
    assert_eq!(
        full_result.door_anchors.len(),
        topo_result.layout.door_anchors.len(),
        "门锚点数应一致"
    );
    assert_eq!(
        full_result.corridors.len(),
        topo_result.layout.corridors.len(),
        "走廊数应一致"
    );

    // 验证分块元数据一致性
    assert_eq!(
        full_result.chunks.len(),
        topo_result.chunks.len(),
        "分块数应一致"
    );

    // 合并所有分块的细节
    let mut total_terrains = 0;

    for chunk in &topo_result.chunks {
        let detail = generator
            .fill_chunk_details(&topo_result, &chunk.id)
            .expect("分块细节填充应成功");
        total_terrains += detail.terrains.len();
    }

    // 验证合并后的数量与整层结果一致
    assert_eq!(
        total_terrains,
        full_result.terrains.len(),
        "分块合并后地形总数应与整层一致"
    );

    // 关键不变量：每个房间都应该有地形生成
    assert_eq!(
        total_terrains,
        topo_result
            .layout
            .rooms
            .iter()
            .filter(|r| r.bounds.is_some())
            .count(),
        "每个有边界的房间都应有地形"
    );
}

#[test]
fn test_chunked_covers_all_rooms() {
    // 验证所有分块合并后覆盖所有房间
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    let topo_result = generator
        .generate_topology_only(request)
        .expect("拓扑预计算应成功");

    // 收集所有分块覆盖的房间 ID
    let mut covered_room_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for chunk in &topo_result.chunks {
        covered_room_ids.extend(chunk.room_ids.iter().cloned());
    }

    // 验证所有有边界的房间都被分块覆盖
    for room in &topo_result.layout.rooms {
        if room.bounds.is_some() {
            assert!(
                covered_room_ids.contains(&room.id),
                "房间 {} 应被某个分块覆盖",
                room.id
            );
        }
    }
}

#[test]
fn test_generate_dispatches_to_chunked_for_runtime_mode() {
    // 验证 generate() 方法在 RuntimeChunked 模式下自动委托给 generate_chunk
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![],
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result = generator
        .generate(request)
        .expect("RuntimeChunked 模式生成应成功");
    assert!(!result.rooms.is_empty());
    assert!(!result.chunks.is_empty());
}
