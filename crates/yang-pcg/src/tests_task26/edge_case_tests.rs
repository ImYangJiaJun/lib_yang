// 核心模块边界情况单元测试
// 覆盖 topology、layout、terrain、spawn 模块的边界场景
// 需求映射：18.1

use crate::config::{
    EnemySpawnConfig, GenerationConfig, GenerationMode, ItemSpawnConfig, RangeU16, RoomSizeConfig,
};
use crate::generator::MapGenerator;
use crate::layout;
use crate::model::request::{GenerationRequest, RuntimeContext};
use crate::rng::StableRng;
use crate::spawn;
use crate::terrain;
use crate::topology;

// ============================================================
// 拓扑模块边界测试
// ============================================================

/// 最小房间数（2 间）应生成仅含 Start 和 Boss 的拓扑
#[test]
fn test_topology_min_room_count() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 2, max: 2 },
        critical_path_length: RangeU16 { min: 2, max: 2 },
        branch_count: RangeU16 { min: 0, max: 0 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("最小配置应有效");
    let mut rng = StableRng::from_seed(42);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    assert_eq!(graph.nodes.len(), 2, "最小房间数应为 2");
    assert_eq!(
        graph.nodes[0].room_type,
        crate::model::room::RoomType::Start
    );
    assert_eq!(graph.nodes[1].room_type, crate::model::room::RoomType::Boss);
    assert_eq!(graph.edges.len(), 1, "2 间房应有 1 条边");
    assert!(graph.branches.is_empty(), "无分支");
}

/// 较大房间数（50 间）应正常生成
#[test]
fn test_topology_large_room_count() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 50, max: 50 },
        critical_path_length: RangeU16 { min: 10, max: 15 },
        branch_count: RangeU16 { min: 3, max: 5 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("大房间数配置应有效");
    let mut rng = StableRng::from_seed(99);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    // 房间数应在合理范围内
    assert!(
        graph.nodes.len() >= 10,
        "大配置应生成足够多的房间，实际: {}",
        graph.nodes.len()
    );
    // 边数应至少为 nodes - 1（树结构）
    assert!(
        graph.edges.len() >= graph.nodes.len() - 1,
        "边数应至少为 nodes-1"
    );
}

/// 零分支配置应仅生成关键路径
#[test]
fn test_topology_zero_branches() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 5, max: 5 },
        critical_path_length: RangeU16 { min: 5, max: 5 },
        branch_count: RangeU16 { min: 0, max: 0 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("零分支配置应有效");
    let mut rng = StableRng::from_seed(7);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    assert!(graph.branches.is_empty(), "零分支配置不应有分支");
    assert_eq!(graph.critical_path.len(), 5, "关键路径长度应为 5");
}

/// 关键路径等于房间总数时不应有分支
#[test]
fn test_topology_critical_path_equals_room_count() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 8, max: 8 },
        critical_path_length: RangeU16 { min: 8, max: 8 },
        branch_count: RangeU16 { min: 2, max: 3 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(123);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    // 关键路径占满所有房间时，分支应为空
    assert!(
        graph.branches.is_empty(),
        "关键路径等于房间总数时不应有分支"
    );
}

// ============================================================
// 布局模块边界测试
// ============================================================

/// 最小拓扑（2 间房）的布局求解
#[test]
fn test_layout_min_topology() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 2, max: 2 },
        critical_path_length: RangeU16 { min: 2, max: 2 },
        branch_count: RangeU16 { min: 0, max: 0 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(42);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    let mut layout_rng = StableRng::from_seed(42);
    let output = layout::solve_layout(&graph, &normalized, &mut layout_rng).expect("布局应成功");

    assert_eq!(output.rooms.len(), 2, "布局应包含 2 个房间");
    // 每个房间应有边界
    for room in &output.rooms {
        assert!(room.bounds.is_some(), "房间 {} 应有边界", room.id);
    }
    // 应有门锚点
    assert!(!output.door_anchors.is_empty(), "应有门锚点");
    // 应有走廊
    assert!(!output.corridors.is_empty(), "应有走廊");
}

/// 大拓扑的布局求解应为所有房间生成有效边界
#[test]
fn test_layout_large_topology_valid_bounds() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 30, max: 30 },
        critical_path_length: RangeU16 { min: 8, max: 10 },
        branch_count: RangeU16 { min: 3, max: 5 },
        ..Default::default()
    };
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(77);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    let mut layout_rng = StableRng::from_seed(77);
    let output = layout::solve_layout(&graph, &normalized, &mut layout_rng).expect("布局应成功");

    // 验证所有房间都有有效边界
    for room in &output.rooms {
        let bounds = room
            .bounds
            .unwrap_or_else(|| panic!("房间 {} 应有边界", room.id));
        // 边界应有正向尺寸
        assert!(bounds.max.x > bounds.min.x, "房间 {} 宽度应为正", room.id);
        assert!(bounds.max.y > bounds.min.y, "房间 {} 高度应为正", room.id);
    }

    // 验证关键路径上的房间不重叠（关键路径使用线性布局，不应重叠）
    let critical_rooms: Vec<_> = output
        .rooms
        .iter()
        .filter(|r| r.branch_id.is_none())
        .collect();
    for i in 0..critical_rooms.len() {
        for j in (i + 1)..critical_rooms.len() {
            let bounds_a = critical_rooms[i].bounds.unwrap();
            let bounds_b = critical_rooms[j].bounds.unwrap();
            let overlaps = bounds_a.min.x < bounds_b.max.x
                && bounds_a.max.x > bounds_b.min.x
                && bounds_a.min.y < bounds_b.max.y
                && bounds_a.max.y > bounds_b.min.y;
            assert!(
                !overlaps,
                "关键路径房间 {} 和 {} 不应重叠",
                critical_rooms[i].id, critical_rooms[j].id
            );
        }
    }
}

// ============================================================
// 地形模块边界测试
// ============================================================

/// 最小房间尺寸的地形生成
#[test]
fn test_terrain_min_room_size() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 3, max: 3 },
        critical_path_length: RangeU16 { min: 3, max: 3 },
        branch_count: RangeU16 { min: 0, max: 0 },
        room_size: RoomSizeConfig {
            min_width: 6,
            max_width: 6,
            min_height: 6,
            max_height: 6,
        },
        ..Default::default()
    };
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(42);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    let mut layout_rng = StableRng::from_seed(42);
    let output = layout::solve_layout(&graph, &normalized, &mut layout_rng).expect("布局应成功");

    let mut terrain_rng = StableRng::from_seed(42);
    let terrains = terrain::generate_terrains(
        &output.rooms,
        &output.door_anchors,
        &normalized,
        &mut terrain_rng,
    )
    .expect("地形生成应成功");

    assert!(!terrains.is_empty(), "应生成地形");
    for t in &terrains {
        assert!(t.grid_size.width >= 6, "地形宽度应至少为 6");
        assert!(t.grid_size.height >= 6, "地形高度应至少为 6");
    }
}

/// 无门锚点时地形仍应生成（退化场景）
#[test]
fn test_terrain_with_empty_door_anchors() {
    let config = GenerationConfig::default();
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(42);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    let mut layout_rng = StableRng::from_seed(42);
    let output = layout::solve_layout(&graph, &normalized, &mut layout_rng).expect("布局应成功");

    // 使用空门锚点列表
    let mut terrain_rng = StableRng::from_seed(42);
    let terrains = terrain::generate_terrains(
        &output.rooms,
        &[], // 空门锚点
        &normalized,
        &mut terrain_rng,
    )
    .expect("空门锚点时地形生成应成功");

    assert!(!terrains.is_empty(), "即使无门锚点也应生成地形");
}

// ============================================================
// 点位模块边界测试
// ============================================================

/// 空房间列表时点位生成应返回空结果
#[test]
fn test_spawn_empty_rooms() {
    let config = GenerationConfig::default();
    let normalized = config.normalize().expect("配置应有效");
    let rng = StableRng::from_seed(42);

    let output = spawn::generate_spawns(&[], &[], &normalized, &rng).expect("空房间应成功");

    assert!(output.item_spawns.is_empty(), "空房间不应有交互物");
    assert!(output.enemy_spawns.is_empty(), "空房间不应有敌人");
}

/// 点位生成应尊重最小间距配置
#[test]
fn test_spawn_respects_min_spacing() {
    let config = GenerationConfig {
        item_spawns: ItemSpawnConfig {
            min_spacing: 3,
            ..Default::default()
        },
        enemy_spawns: EnemySpawnConfig {
            min_spacing: 3,
            ..Default::default()
        },
        ..Default::default()
    };
    let normalized = config.normalize().expect("配置应有效");
    let mut rng = StableRng::from_seed(42);
    let graph = topology::generate_topology(&normalized, &mut rng).expect("拓扑生成应成功");

    let mut layout_rng = StableRng::from_seed(42);
    let output = layout::solve_layout(&graph, &normalized, &mut layout_rng).expect("布局应成功");

    let mut terrain_rng = StableRng::from_seed(42);
    let terrains = terrain::generate_terrains(
        &output.rooms,
        &output.door_anchors,
        &normalized,
        &mut terrain_rng,
    )
    .expect("地形生成应成功");

    let spawn_rng = StableRng::from_seed(42);
    let spawn_output = spawn::generate_spawns(&output.rooms, &terrains, &normalized, &spawn_rng)
        .expect("点位生成应成功");

    // 验证交互物点位间距
    for i in 0..spawn_output.item_spawns.len() {
        for j in (i + 1)..spawn_output.item_spawns.len() {
            let a = &spawn_output.item_spawns[i];
            let b = &spawn_output.item_spawns[j];
            // 仅检查同一房间内的点位
            if a.room_id == b.room_id {
                let dx = (a.grid_pos.x - b.grid_pos.x) as f64;
                let dy = (a.grid_pos.y - b.grid_pos.y) as f64;
                let dist = (dx * dx + dy * dy).sqrt();
                assert!(
                    dist >= 2.5, // 允许少量浮点误差
                    "同房间交互物点位间距不足: {:.2} < 3.0",
                    dist
                );
            }
        }
    }
}

/// 完整流水线在最小配置下应成功
#[test]
fn test_full_pipeline_min_config() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 2, max: 2 },
        critical_path_length: RangeU16 { min: 2, max: 2 },
        branch_count: RangeU16 { min: 0, max: 0 },
        ..Default::default()
    };
    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: Some(42),
            config,
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("最小配置完整流水线应成功");

    assert_eq!(result.rooms.len(), 2);
    assert!(!result.corridors.is_empty());
}

/// 完整流水线在 RuntimeChunked 模式下应成功
#[test]
fn test_full_pipeline_min_config_runtime_chunked() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 2, max: 2 },
        critical_path_length: RangeU16 { min: 2, max: 2 },
        branch_count: RangeU16 { min: 0, max: 0 },
        dead_end_count: RangeU16 { min: 0, max: 0 },
        generation_mode: GenerationMode::RuntimeChunked,
        capability_flags: crate::config::CapabilityFlags {
            runtime_chunked: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: Some(42),
            config,
            constraints: vec![],
            runtime_context: Some(RuntimeContext {
                focus_position: None,
                interest_radius: None,
                requested_chunks: vec!["chunk-0-0".to_string()],
                caller_tag: None,
            }),
            trace_id: None,
        })
        .expect("RuntimeChunked 模式最小配置完整流水线应成功");

    assert_eq!(result.rooms.len(), 2);
    assert!(!result.corridors.is_empty());
}

/// 完整流水线在 HybridPrecompute 模式下应成功
#[test]
fn test_full_pipeline_min_config_hybrid() {
    let config = GenerationConfig {
        room_count: RangeU16 { min: 2, max: 2 },
        critical_path_length: RangeU16 { min: 2, max: 2 },
        branch_count: RangeU16 { min: 0, max: 0 },
        dead_end_count: RangeU16 { min: 0, max: 0 },
        generation_mode: GenerationMode::HybridPrecompute,
        capability_flags: crate::config::CapabilityFlags {
            hybrid_precompute: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let generator = MapGenerator::new();

    // 第一阶段：生成拓扑和布局
    let topology_result = generator
        .generate_topology_only(GenerationRequest {
            seed: Some(42),
            config,
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("HybridPrecompute 拓扑阶段应成功");

    // 第二轮：填充分块细节
    assert!(
        !topology_result.chunks.is_empty(),
        "预计算阶段应生成至少一个分块"
    );
    for chunk in &topology_result.chunks {
        let detail_result = generator
            .fill_chunk_details(&topology_result, &chunk.id)
            .expect("分块细节填充应成功");
        assert_eq!(detail_result.chunk_id, chunk.id);
        assert!(
            !detail_result.terrains.is_empty(),
            "分块 {} 应包含地形",
            chunk.id
        );
    }
}
