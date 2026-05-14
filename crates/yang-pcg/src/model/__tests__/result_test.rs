// 生成结果数据模型测试
// 验证需求: 1.3, 2.5, 14.2

use crate::debug::DebugBundle;
use crate::model::result::*;
use crate::model::room::RoomGraph;

#[test]
fn test_result_metadata_creation() {
    // 测试结果元数据创建
    let metadata = ResultMetadata {
        seed: 12345,
        config_digest: "abc123def456".to_string(),
        schema_version: "1.0.0".to_string(),
        algorithm_version: "0.1.0".to_string(),
        target_engine_version: Some("UE5.5".to_string()),
        trace_id: Some("trace-001".to_string()),
    };

    assert_eq!(metadata.seed, 12345);
    assert_eq!(metadata.config_digest, "abc123def456");
    assert_eq!(metadata.schema_version, "1.0.0");
    assert_eq!(metadata.algorithm_version, "0.1.0");
    assert_eq!(metadata.target_engine_version, Some("UE5.5".to_string()));
    assert_eq!(metadata.trace_id, Some("trace-001".to_string()));
}

#[test]
fn test_generation_result_creation() {
    // 测试生成结果创建
    let result = GenerationResult {
        metadata: ResultMetadata {
            seed: 99999,
            config_digest: "digest123".to_string(),
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
        debug: None,
    };

    assert_eq!(result.metadata.seed, 99999);
    assert!(result.rooms.is_empty());
    assert!(result.corridors.is_empty());
    assert!(result.debug.is_none());
}

#[test]
fn test_generation_result_with_debug() {
    // 测试带调试信息的生成结果
    let result = GenerationResult {
        metadata: ResultMetadata {
            seed: 54321,
            config_digest: "test_digest".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.1.0".to_string(),
            target_engine_version: Some("UE5.6".to_string()),
            trace_id: Some("debug-trace".to_string()),
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

    assert!(result.debug.is_some());
    assert_eq!(result.metadata.trace_id, Some("debug-trace".to_string()));
}

#[test]
fn test_result_metadata_minimal() {
    // 测试最小结果元数据
    let metadata = ResultMetadata {
        seed: 1,
        config_digest: "minimal".to_string(),
        schema_version: "1.0.0".to_string(),
        algorithm_version: "0.1.0".to_string(),
        target_engine_version: None,
        trace_id: None,
    };

    assert!(metadata.target_engine_version.is_none());
    assert!(metadata.trace_id.is_none());
}

#[test]
fn test_generation_result_json_roundtrip() {
    let result = GenerationResult {
        metadata: ResultMetadata {
            seed: 11,
            config_digest: "digest".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.1.0".to_string(),
            target_engine_version: None,
            trace_id: Some("trace".to_string()),
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

    let json = serde_json::to_string(&result).expect("结果应可序列化为 JSON");
    let restored: GenerationResult = serde_json::from_str(&json).expect("结果应可从 JSON 反序列化");

    assert_eq!(restored.metadata.seed, 11);
    assert_eq!(restored.metadata.trace_id.as_deref(), Some("trace"));
}

/// 验证需求: 14.1 - GenerationResult 及所有子结构可完整序列化/反序列化
/// 使用包含所有子结构的完整数据进行 JSON 往返测试
#[test]
fn test_generation_result_full_json_roundtrip() {
    use crate::debug::{DebugChannels, RejectionReason, SpawnDebugInfo, StageStat};
    use crate::model::chunk::{Chunk, StreamingMetadata};
    use crate::model::geometry::{
        CardinalDir, GridPoint, GridSize, RoomBounds, Transform3, WorldPoint,
    };
    use crate::model::room::{
        Branch, Corridor, CorridorPath, DoorAnchor, Room, RoomEdge, RoomType,
    };
    use crate::model::spawn::{SpawnKind, SpawnMetadata, SpawnPoint};
    use crate::model::terrain::{
        ConnectivitySummary, Grid2D, ReservedZone, ReservedZoneBounds, Terrain, TileKind,
    };
    use crate::validation::{ValidationItem, ValidationReport};

    // 构建包含所有子结构的完整 GenerationResult
    let rooms = vec![
        Room {
            id: "room-0".to_string(),
            room_type: RoomType::Start,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec!["dungeon".to_string()],
            bounds: Some(RoomBounds {
                min: GridPoint { x: 0, y: 0 },
                max: GridPoint { x: 10, y: 10 },
            }),
            template_ref: None,
            grammar_token: None,
        },
        Room {
            id: "room-1".to_string(),
            room_type: RoomType::Boss,
            depth_from_start: 3,
            branch_id: Some("branch-0".to_string()),
            difficulty: 5,
            theme_tags: vec!["fire".to_string(), "boss".to_string()],
            bounds: Some(RoomBounds {
                min: GridPoint { x: 20, y: 0 },
                max: GridPoint { x: 35, y: 15 },
            }),
            template_ref: Some("boss_arena_01".to_string()),
            grammar_token: None,
        },
    ];

    let topology = RoomGraph {
        nodes: rooms.clone(),
        edges: vec![RoomEdge {
            id: "edge-0".to_string(),
            from_room: "room-0".to_string(),
            to_room: "room-1".to_string(),
            is_critical: true,
        }],
        critical_path: vec!["room-0".to_string(), "room-1".to_string()],
        branches: vec![Branch {
            id: "branch-0".to_string(),
            start_room: "room-0".to_string(),
            end_room: "room-1".to_string(),
            room_ids: vec!["room-1".to_string()],
            purpose: "reward".to_string(),
        }],
    };

    let door_anchors = vec![DoorAnchor {
        id: "door-0".to_string(),
        room_id: "room-0".to_string(),
        edge_id: "edge-0".to_string(),
        grid_pos: GridPoint { x: 10, y: 5 },
        facing: CardinalDir::East,
        width_tiles: 2,
        socket_tag: None,
    }];

    let corridors = vec![Corridor {
        id: "corridor-0".to_string(),
        from_room: "room-0".to_string(),
        to_room: "room-1".to_string(),
        from_anchor: "door-0".to_string(),
        to_anchor: "door-1".to_string(),
        width_tiles: 2,
        path: CorridorPath::Orthogonal(vec![
            GridPoint { x: 10, y: 5 },
            GridPoint { x: 15, y: 5 },
            GridPoint { x: 15, y: 3 },
            GridPoint { x: 20, y: 3 },
        ]),
        segment_tags: Vec::new(),
    }];

    // 构建包含 Grid2D 的地形数据
    let terrain_tiles = Grid2D::new(5, 3, TileKind::Floor);
    let terrains = vec![Terrain {
        room_id: "room-0".to_string(),
        grid_size: GridSize {
            width: 5,
            height: 3,
        },
        tiles: terrain_tiles,
        reserved_zones: vec![
            ReservedZone {
                id: "zone-0".to_string(),
                zone_type: "boss_center".to_string(),
                bounds: ReservedZoneBounds::Rect {
                    min: GridPoint { x: 1, y: 1 },
                    max: GridPoint { x: 4, y: 2 },
                },
                allow_items: false,
                allow_enemies: true,
            },
            ReservedZone {
                id: "zone-1".to_string(),
                zone_type: "entrance".to_string(),
                bounds: ReservedZoneBounds::Circle {
                    center: GridPoint { x: 2, y: 2 },
                    radius: 3,
                },
                allow_items: true,
                allow_enemies: false,
            },
            ReservedZone {
                id: "zone-2".to_string(),
                zone_type: "treasure_spot".to_string(),
                bounds: ReservedZoneBounds::Polygon {
                    points: vec![
                        GridPoint { x: 0, y: 0 },
                        GridPoint { x: 3, y: 0 },
                        GridPoint { x: 3, y: 3 },
                    ],
                },
                allow_items: true,
                allow_enemies: false,
            },
        ],
        connectivity_summary: ConnectivitySummary {
            all_doors_connected: true,
            walkable_tile_count: 12,
            total_tile_count: 15,
            connected_region_count: 1,
        },
    }];

    let item_spawns = vec![SpawnPoint {
        id: "item-0".to_string(),
        room_id: "room-0".to_string(),
        kind: SpawnKind::Item,
        grid_pos: GridPoint { x: 3, y: 3 },
        world_transform: Some(Transform3 {
            position: WorldPoint {
                x: 300.0,
                y: 300.0,
                z: 0.0,
            },
            rotation: (0.0, 90.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        }),
        metadata: SpawnMetadata {
            spawn_tag: "chest_gold".to_string(),
            rarity_tier: Some(2),
            enemy_pool_tag: None,
            encounter_id: None,
            wave_id: None,
            difficulty: Some(3),
            seed: 42,
        },
    }];

    let enemy_spawns = vec![SpawnPoint {
        id: "enemy-0".to_string(),
        room_id: "room-1".to_string(),
        kind: SpawnKind::Boss,
        grid_pos: GridPoint { x: 7, y: 7 },
        world_transform: None,
        metadata: SpawnMetadata {
            spawn_tag: "boss_dragon".to_string(),
            rarity_tier: None,
            enemy_pool_tag: Some("elite_pool".to_string()),
            encounter_id: Some("enc-001".to_string()),
            wave_id: Some("wave-1".to_string()),
            difficulty: Some(10),
            seed: 99,
        },
    }];

    let chunks = vec![Chunk {
        id: "chunk-0".to_string(),
        bounds: RoomBounds {
            min: GridPoint { x: 0, y: 0 },
            max: GridPoint { x: 32, y: 32 },
        },
        room_ids: vec!["room-0".to_string(), "room-1".to_string()],
        dependencies: vec!["chunk-1".to_string()],
        streaming_metadata: StreamingMetadata {
            data_layer: Some("DL_Dungeon_01".to_string()),
            external_data_layer: Some("EDL_Dungeon_01".to_string()),
            hlod_layer: Some("HLOD_0".to_string()),
            streaming_priority: Some(100),
        },
    }];

    // 构建完整的 DebugBundle
    let debug = DebugBundle {
        trace_id: Some("trace-full-test".to_string()),
        stage_stats: vec![
            StageStat {
                stage_name: "topology".to_string(),
                duration_ms: 5,
                iterations: 1,
                produced_count: 2,
            },
            StageStat {
                stage_name: "layout".to_string(),
                duration_ms: 12,
                iterations: 3,
                produced_count: 2,
            },
        ],
        notes: vec!["测试备注".to_string()],
        validation_report: Some(ValidationReport {
            reachability: ValidationItem::passed("reachability"),
            no_overlap: ValidationItem::passed("no_overlap"),
            terrain_connectivity: ValidationItem::passed("terrain_connectivity"),
            spawn_spacing: ValidationItem::failed("spawn_spacing", "间距不足"),
            all_passed: false,
            passed_count: 3,
            failed_count: 1,
        }),
        debug_channels: Some(DebugChannels {
            critical_path_nodes: vec!["room-0".to_string(), "room-1".to_string()],
            door_anchor_positions: vec![GridPoint { x: 10, y: 5 }],
            corridor_centerlines: vec![vec![
                GridPoint { x: 10, y: 5 },
                GridPoint { x: 20, y: 5 },
            ]],
            rejected_rooms: vec!["room-rejected".to_string()],
            spawn_debug: Some(SpawnDebugInfo {
                candidate_count: 20,
                rejected_count: 5,
                rejection_reasons: vec![RejectionReason {
                    position: GridPoint { x: 1, y: 1 },
                    reason: "间距不足".to_string(),
                }],
                accepted_count: 15,
            }),
        }),
    };

    let result = GenerationResult {
        metadata: ResultMetadata {
            seed: 42,
            config_digest: "full-test-digest-abc123".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.2.0".to_string(),
            target_engine_version: Some("UE5.5".to_string()),
            trace_id: Some("trace-full-test".to_string()),
        },
        topology,
        rooms,
        door_anchors,
        corridors,
        terrains,
        item_spawns,
        enemy_spawns,
        chunks,
        debug: Some(debug),
    };

    // 序列化为 JSON
    let json = serde_json::to_string_pretty(&result)
        .expect("完整 GenerationResult 应可序列化为 JSON");

    // 验证 JSON 包含关键字段
    assert!(json.contains("room-0"));
    assert!(json.contains("room-1"));
    assert!(json.contains("boss_arena_01"));
    assert!(json.contains("Orthogonal"));
    assert!(json.contains("boss_center"));
    assert!(json.contains("chest_gold"));
    assert!(json.contains("chunk-0"));
    assert!(json.contains("DL_Dungeon_01"));
    assert!(json.contains("trace-full-test"));
    assert!(json.contains("topology"));
    assert!(json.contains("terrain_connectivity"));

    // 反序列化回 GenerationResult
    let restored: GenerationResult =
        serde_json::from_str(&json).expect("完整 JSON 应可反序列化为 GenerationResult");

    // 验证元数据
    assert_eq!(restored.metadata.seed, 42);
    assert_eq!(
        restored.metadata.config_digest,
        "full-test-digest-abc123"
    );
    assert_eq!(restored.metadata.schema_version, "1.0.0");
    assert_eq!(restored.metadata.algorithm_version, "0.2.0");
    assert_eq!(
        restored.metadata.target_engine_version.as_deref(),
        Some("UE5.5")
    );

    // 验证拓扑
    assert_eq!(restored.topology.nodes.len(), 2);
    assert_eq!(restored.topology.edges.len(), 1);
    assert_eq!(restored.topology.critical_path.len(), 2);
    assert_eq!(restored.topology.branches.len(), 1);
    assert_eq!(restored.topology.branches[0].purpose, "reward");

    // 验证房间
    assert_eq!(restored.rooms.len(), 2);
    assert_eq!(restored.rooms[0].room_type, RoomType::Start);
    assert_eq!(restored.rooms[1].room_type, RoomType::Boss);
    assert_eq!(
        restored.rooms[1].template_ref.as_deref(),
        Some("boss_arena_01")
    );

    // 验证门锚点
    assert_eq!(restored.door_anchors.len(), 1);
    assert_eq!(restored.door_anchors[0].facing, CardinalDir::East);
    assert_eq!(restored.door_anchors[0].width_tiles, 2);

    // 验证走廊（含 CorridorPath 枚举）
    assert_eq!(restored.corridors.len(), 1);
    match &restored.corridors[0].path {
        CorridorPath::Orthogonal(points) => assert_eq!(points.len(), 4),
        _ => panic!("走廊路径类型应为 Orthogonal"),
    }

    // 验证地形（含 Grid2D 自定义容器）
    assert_eq!(restored.terrains.len(), 1);
    assert_eq!(restored.terrains[0].tiles.width, 5);
    assert_eq!(restored.terrains[0].tiles.height, 3);
    assert_eq!(restored.terrains[0].tiles.data.len(), 15);
    assert_eq!(
        restored.terrains[0].tiles.get(0, 0),
        Some(&TileKind::Floor)
    );

    // 验证保留区（含 ReservedZoneBounds 枚举的三种变体）
    assert_eq!(restored.terrains[0].reserved_zones.len(), 3);
    match &restored.terrains[0].reserved_zones[0].bounds {
        ReservedZoneBounds::Rect { min, max } => {
            assert_eq!(min.x, 1);
            assert_eq!(max.x, 4);
        }
        _ => panic!("第一个保留区应为 Rect 类型"),
    }
    match &restored.terrains[0].reserved_zones[1].bounds {
        ReservedZoneBounds::Circle { center, radius } => {
            assert_eq!(center.x, 2);
            assert_eq!(*radius, 3);
        }
        _ => panic!("第二个保留区应为 Circle 类型"),
    }
    match &restored.terrains[0].reserved_zones[2].bounds {
        ReservedZoneBounds::Polygon { points } => {
            assert_eq!(points.len(), 3);
        }
        _ => panic!("第三个保留区应为 Polygon 类型"),
    }

    // 验证连通性摘要
    assert!(restored.terrains[0].connectivity_summary.all_doors_connected);
    assert_eq!(
        restored.terrains[0].connectivity_summary.walkable_tile_count,
        12
    );

    // 验证交互物点位（含 Transform3）
    assert_eq!(restored.item_spawns.len(), 1);
    assert_eq!(restored.item_spawns[0].kind, SpawnKind::Item);
    assert!(restored.item_spawns[0].world_transform.is_some());
    let transform = restored.item_spawns[0].world_transform.as_ref().unwrap();
    assert_eq!(transform.position.x, 300.0);
    assert_eq!(transform.rotation.1, 90.0);

    // 验证敌人点位
    assert_eq!(restored.enemy_spawns.len(), 1);
    assert_eq!(restored.enemy_spawns[0].kind, SpawnKind::Boss);
    assert_eq!(
        restored.enemy_spawns[0].metadata.enemy_pool_tag.as_deref(),
        Some("elite_pool")
    );

    // 验证分块（含 StreamingMetadata）
    assert_eq!(restored.chunks.len(), 1);
    assert_eq!(restored.chunks[0].id, "chunk-0");
    assert_eq!(restored.chunks[0].room_ids.len(), 2);
    assert_eq!(
        restored.chunks[0]
            .streaming_metadata
            .data_layer
            .as_deref(),
        Some("DL_Dungeon_01")
    );
    assert_eq!(
        restored.chunks[0].streaming_metadata.streaming_priority,
        Some(100)
    );

    // 验证调试信息（含 ValidationReport 和 DebugChannels）
    let debug = restored.debug.as_ref().expect("调试信息应存在");
    assert_eq!(debug.trace_id.as_deref(), Some("trace-full-test"));
    assert_eq!(debug.stage_stats.len(), 2);
    assert_eq!(debug.stage_stats[0].stage_name, "topology");
    assert_eq!(debug.stage_stats[0].duration_ms, 5);
    assert_eq!(debug.notes, vec!["测试备注"]);

    // 验证 ValidationReport
    let report = debug.validation_report.as_ref().expect("验证报告应存在");
    assert!(report.reachability.passed);
    assert!(!report.spawn_spacing.passed);
    assert_eq!(
        report.spawn_spacing.error_message.as_deref(),
        Some("间距不足")
    );
    assert!(!report.all_passed);
    assert_eq!(report.passed_count, 3);
    assert_eq!(report.failed_count, 1);

    // 验证 DebugChannels
    let channels = debug.debug_channels.as_ref().expect("调试通道应存在");
    assert_eq!(channels.critical_path_nodes.len(), 2);
    assert_eq!(channels.door_anchor_positions.len(), 1);
    assert_eq!(channels.corridor_centerlines.len(), 1);
    assert_eq!(channels.rejected_rooms, vec!["room-rejected"]);

    // 验证 SpawnDebugInfo
    let spawn_debug = channels.spawn_debug.as_ref().expect("点位调试信息应存在");
    assert_eq!(spawn_debug.candidate_count, 20);
    assert_eq!(spawn_debug.rejected_count, 5);
    assert_eq!(spawn_debug.rejection_reasons.len(), 1);
    assert_eq!(spawn_debug.accepted_count, 15);
}
