// 属性测试 - 验证地图生成器的核心不变量
// 使用 proptest 框架随机生成配置参数，验证生成结果满足不变量
// OPT-T-03: 覆盖三模式（OfflineFullFloor / RuntimeChunked / HybridPrecompute）

use proptest::prelude::*;

use crate::config::{
    CapabilityFlags, ChunkingConfig, ConnectionStrategy, CorridorConfig, EnemySpawnConfig,
    GenerationConfig, GenerationMode, ItemSpawnConfig, RangeU16, RoomSizeConfig, TerrainConfig,
};
use crate::generator::MapGenerator;
use crate::model::request::{
    AnchorConstraint, Constraint, ExclusionZoneConstraint, GenerationRequest, RuntimeContext,
};
use crate::model::room::RoomType;
use crate::model::spawn::SpawnPoint;
use crate::validation::{
    validate_no_overlap, validate_reachability, validate_spawn_spacing,
    validate_terrain_connectivity,
};

/// 生成合法的 GenerationConfig 策略（仅 OfflineFullFloor，用于无模式分发场景）
///
/// 约束范围：
/// - room_count: 2..=50（OPT-T-13: 扩大上限以提升种子空间覆盖）
/// - critical_path_length: 2..=room_count
/// - branch_count: 0..=3
/// - dead_end_count: 0..=2
fn arb_generation_config() -> impl Strategy<Value = GenerationConfig> {
    // 先生成房间数量范围
    (2u16..=50u16)
        .prop_flat_map(|room_count| {
            // 关键路径长度不超过房间数
            let max_path = room_count.min(8);
            let path_len = 2u16..=max_path;
            // 分支数量
            let branch = 0u16..=3u16;
            // 死路数量
            let dead_end = 0u16..=2u16;

            (Just(room_count), path_len, branch, dead_end)
        })
        .prop_map(
            |(room_count, path_len, branch_max, dead_end_max)| GenerationConfig {
                room_count: RangeU16 {
                    min: room_count,
                    max: room_count,
                },
                critical_path_length: RangeU16 {
                    min: path_len,
                    max: path_len,
                },
                branch_count: RangeU16 {
                    min: 0,
                    max: branch_max,
                },
                dead_end_count: RangeU16 {
                    min: 0,
                    max: dead_end_max,
                },
                room_size: RoomSizeConfig {
                    min_width: 8,
                    max_width: 12,
                    min_height: 8,
                    max_height: 12,
                },
                corridor: CorridorConfig {
                    width: 2,
                    max_turns: 3,
                    connection_strategy: ConnectionStrategy::Orthogonal,
                },
                terrain: TerrainConfig {
                    obstacle_density: 0.15,
                    min_walkable_ratio: 0.6,
                },
                item_spawns: ItemSpawnConfig {
                    count_per_room: RangeU16 { min: 1, max: 2 },
                    min_spacing: 2,
                    rarity_weights: vec![0.6, 0.3, 0.1],
                },
                enemy_spawns: EnemySpawnConfig {
                    count_per_room: RangeU16 { min: 1, max: 3 },
                    min_spacing: 3,
                    min_distance_from_entrance: 4,
                    base_difficulty_budget: 100,
                },
                chunking: ChunkingConfig {
                    chunk_size: 32,
                    enabled: false,
                },
                theme_tags: vec!["default".to_string()],
                generation_mode: GenerationMode::OfflineFullFloor,
                capability_flags: CapabilityFlags::default(),
            },
        )
}

/// 生成三模式分布策略
///
/// 权重分布：OfflineFullFloor 60% / RuntimeChunked 20% / HybridPrecompute 20%
fn arb_generation_mode() -> impl Strategy<Value = GenerationMode> {
    prop_oneof![
        6 => Just(GenerationMode::OfflineFullFloor),
        2 => Just(GenerationMode::RuntimeChunked),
        2 => Just(GenerationMode::HybridPrecompute),
    ]
}

/// 生成模式感知的测试输入
///
/// 返回 `(GenerationConfig, Option<RuntimeContext>)`，其中：
/// - `capability_flags` 根据模式自动设置
/// - `chunking.enabled` 在分块模式下为 true
/// - `RuntimeContext` 仅在 RuntimeChunked 模式下提供
fn arb_test_input() -> impl Strategy<Value = (GenerationConfig, Option<RuntimeContext>)> {
    (arb_generation_config(), arb_generation_mode()).prop_map(|(mut config, mode)| {
        config.generation_mode = mode.clone();
        config.capability_flags = match mode {
            GenerationMode::OfflineFullFloor => CapabilityFlags::default(),
            GenerationMode::RuntimeChunked => CapabilityFlags {
                runtime_chunked: true,
                ..CapabilityFlags::default()
            },
            GenerationMode::HybridPrecompute => CapabilityFlags {
                hybrid_precompute: true,
                ..CapabilityFlags::default()
            },
        };
        // 分块模式启用 chunking
        if matches!(
            mode,
            GenerationMode::RuntimeChunked | GenerationMode::HybridPrecompute
        ) {
            config.chunking.enabled = true;
        }

        let runtime_ctx = match mode {
            GenerationMode::RuntimeChunked => Some(RuntimeContext {
                focus_position: None,
                interest_radius: None,
                requested_chunks: vec![],
                caller_tag: Some("proptest".to_string()),
            }),
            _ => None,
        };

        (config, runtime_ctx)
    })
}

/// 生成任意 u64 种子的策略
fn arb_seed() -> impl Strategy<Value = u64> {
    any::<u64>()
}

/// 通过 HybridPrecompute 两阶段调用生成完整结果
///
/// 第一阶段：`generate_topology_only` 生成拓扑+布局
/// 第二阶段：`fill_chunk_details` 为每个分块填充地形和点位
/// 合并后返回等价的 `GenerationResult`
fn generate_hybrid_full(
    seed: u64,
    config: GenerationConfig,
    constraints: Vec<crate::model::request::Constraint>,
    trace_id: Option<String>,
) -> crate::error::PcgResult<crate::model::result::GenerationResult> {
    let generator = MapGenerator::new();

    // 第一阶段：拓扑 + 布局
    let topo_req = GenerationRequest {
        seed: Some(seed),
        config,
        constraints,
        runtime_context: None,
        trace_id,
    };
    let topo_result = generator.generate_topology_only(topo_req)?;

    // 第二阶段：逐分块填充细节
    let mut all_terrains = Vec::new();
    let mut all_item_spawns = Vec::new();
    let mut all_enemy_spawns = Vec::new();
    for chunk in &topo_result.chunks {
        let detail = generator.fill_chunk_details(&topo_result, &chunk.id)?;
        all_terrains.extend(detail.terrains);
        all_item_spawns.extend(detail.item_spawns);
        all_enemy_spawns.extend(detail.enemy_spawns);
    }

    // 组装等价的 GenerationResult（供校验函数使用）
    use crate::export::CURRENT_SCHEMA_VERSION;
    Ok(crate::model::result::GenerationResult {
        metadata: crate::model::result::ResultMetadata {
            seed: topo_result.seed,
            config_digest: topo_result.config_digest,
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
            algorithm_version: env!("CARGO_PKG_VERSION").to_string(),
            target_engine_version: None,
            trace_id: topo_result.trace_id,
        },
        topology: topo_result.topology,
        rooms: topo_result.layout.rooms,
        door_anchors: topo_result.layout.door_anchors,
        corridors: topo_result.layout.corridors,
        terrains: all_terrains,
        item_spawns: all_item_spawns,
        enemy_spawns: all_enemy_spawns,
        chunks: topo_result.chunks,
        debug: None,
    })
}

/// 根据模式生成地图，返回统一的 GenerationResult
///
/// - OfflineFullFloor: `generator.generate()`
/// - RuntimeChunked: `generator.generate()`（内部自动委托 generate_chunk）
/// - HybridPrecompute: `generate_topology_only()` + `fill_chunk_details()`
fn generate_by_mode(
    seed: u64,
    config: GenerationConfig,
    runtime_ctx: Option<RuntimeContext>,
    constraints: Vec<crate::model::request::Constraint>,
    trace_id: Option<String>,
) -> crate::error::PcgResult<crate::model::result::GenerationResult> {
    let mode = config.generation_mode.clone();
    match mode {
        GenerationMode::HybridPrecompute => {
            generate_hybrid_full(seed, config, constraints, trace_id)
        }
        _ => {
            // OfflineFullFloor 和 RuntimeChunked 都走 generator.generate()
            let generator = MapGenerator::new();
            generator.generate(GenerationRequest {
                seed: Some(seed),
                config,
                constraints,
                runtime_context: runtime_ctx,
                trace_id,
            })
        }
    }
}

// ============================================================
// 27.1 确定性属性测试
// 相同 seed + config 生成相同结果哈希
// **Validates: Requirements 2.2, 18.2**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 确定性属性：相同 seed + config 必须生成相同结果摘要（三模式覆盖）
    #[test]
    fn prop_deterministic_generation(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
    ) {
        // 第一次生成
        let result1 = generate_by_mode(
            seed, config.clone(), runtime_ctx.clone(), vec![], None,
        );

        // 第二次生成（相同 seed + config + mode）
        let result2 = generate_by_mode(
            seed, config.clone(), runtime_ctx.clone(), vec![], None,
        );

        // 两次生成都应成功
        let r1 = result1.expect("第一次生成应成功");
        let r2 = result2.expect("第二次生成应成功");

        // 验证元数据一致
        prop_assert_eq!(&r1.metadata.seed, &r2.metadata.seed);
        prop_assert_eq!(&r1.metadata.config_digest, &r2.metadata.config_digest);

        // 验证拓扑一致（房间数、边数、关键路径）
        prop_assert_eq!(r1.topology.nodes.len(), r2.topology.nodes.len());
        prop_assert_eq!(r1.topology.edges.len(), r2.topology.edges.len());
        prop_assert_eq!(&r1.topology.critical_path, &r2.topology.critical_path);

        // 验证房间 ID 列表一致
        let ids1: Vec<&str> = r1.rooms.iter().map(|r| r.id.as_str()).collect();
        let ids2: Vec<&str> = r2.rooms.iter().map(|r| r.id.as_str()).collect();
        prop_assert_eq!(&ids1, &ids2);

        // 验证走廊数量一致
        prop_assert_eq!(r1.corridors.len(), r2.corridors.len());

        // 验证点位数量一致
        prop_assert_eq!(r1.item_spawns.len(), r2.item_spawns.len());
        prop_assert_eq!(r1.enemy_spawns.len(), r2.enemy_spawns.len());

        // 验证序列化后的 JSON 完全一致（最强确定性验证）
        let json1 = serde_json::to_string(&r1).expect("序列化应成功");
        let json2 = serde_json::to_string(&r2).expect("序列化应成功");
        prop_assert_eq!(&json1, &json2, "相同 seed+config 的两次生成结果 JSON 应完全一致");
    }
}

// ============================================================
// 27.2 拓扑连通性属性测试
// 任意合法配置下所有房间从 Start 可达
// **Validates: Requirements 3.2, 18.3**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 拓扑连通性属性：任意合法配置下所有房间从 Start 可达（三模式覆盖）
    #[test]
    fn prop_topology_reachability(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
    ) {
        let result = generate_by_mode(
            seed, config, runtime_ctx, vec![], None,
        ).expect("生成应成功");

        // 验证所有房间从 Start 可达
        let reachability_result = validate_reachability(&result.topology);
        prop_assert!(
            reachability_result.is_ok(),
            "所有房间应从 Start 可达: {:?}",
            reachability_result.err()
        );
    }
}

// ============================================================
// 27.3 房间边界不重叠属性测试
// 任意合法配置下房间 AABB 不重叠
// **Validates: Requirements 4.7, 18.3**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 房间边界不重叠属性：任意合法配置下房间 AABB 不重叠（三模式覆盖）
    /// 由 `solve_room_bounds` 的确定性防重叠（分支竖直外推）保证（验证需求 4.7）。
    #[test]
    fn prop_no_room_overlap(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
    ) {
        let result = generate_by_mode(
            seed, config, runtime_ctx, vec![], None,
        ).expect("生成应成功");

        // 验证房间边界不重叠
        let overlap_result = validate_no_overlap(&result.rooms);
        prop_assert!(
            overlap_result.is_ok(),
            "房间边界不应重叠: {:?}",
            overlap_result.err()
        );
    }
}

// ============================================================
// 27.4 地形连通性属性测试
// 任意房间从入口到出口存在通路
// **Validates: Requirements 5.4, 18.3**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 地形连通性属性：任意房间从入口到出口存在通路（三模式覆盖）
    /// 由 `repair_terrain_connectivity` 的强制连通兜底 pass 保证（验证需求 5.4）。
    #[test]
    fn prop_terrain_connectivity(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
    ) {
        let result = generate_by_mode(
            seed, config, runtime_ctx, vec![], None,
        ).expect("生成应成功");

        // 验证地形连通性
        let connectivity_result = validate_terrain_connectivity(&result.terrains);
        prop_assert!(
            connectivity_result.is_ok(),
            "所有房间地形应从入口到出口连通: {:?}",
            connectivity_result.err()
        );
    }
}

// ============================================================
// 27.5 点位最小间距属性测试
// 任意配置下点位满足最小间距
// **Validates: Requirements 7.4, 8.3, 18.4**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 点位最小间距属性：任意配置下点位满足最小间距（三模式覆盖）
    /// 由 spawn 阶段「敌人采样避开已放置交互物」的跨类型间距保证（验证需求 7.4/8.3）。
    #[test]
    fn prop_spawn_spacing(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
    ) {
        let result = generate_by_mode(
            seed, config.clone(), runtime_ctx, vec![], None,
        ).expect("生成应成功");

        // 合并所有点位
        let all_spawns: Vec<SpawnPoint> = result
            .item_spawns
            .iter()
            .chain(result.enemy_spawns.iter())
            .cloned()
            .collect();

        // 使用配置中的最小间距进行验证
        // 交互物和敌人各自有最小间距，取较小值作为全局验证阈值
        let min_spacing = config.item_spawns.min_spacing.min(config.enemy_spawns.min_spacing) as i32;

        let spacing_result = validate_spawn_spacing(&all_spawns, &[], Some(min_spacing));
        prop_assert!(
            spacing_result.is_ok(),
            "所有点位应满足最小间距 {}: {:?}",
            min_spacing,
            spacing_result.err()
        );
    }
}

// ============================================================
// 27.6 约束满足属性测试
// 锚点和排除区约束在结果中被满足
// **Validates: Requirements 6.5, 18.6**
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 约束满足属性：排除区约束在结果中被满足（点位不在排除区内）（三模式覆盖）
    #[test]
    fn prop_constraints_satisfied(
        seed in arb_seed(),
        (config, runtime_ctx) in arb_test_input(),
        // 生成一个排除区（坐标范围覆盖可能的生成区域）
        zone_x in 0i32..50i32,
        zone_y in 0i32..50i32,
    ) {
        // 构建排除区约束
        let exclusion_zone = Constraint::ExclusionZone(ExclusionZoneConstraint {
            label: "test-exclusion".to_string(),
            min: crate::model::geometry::GridPoint { x: zone_x, y: zone_y },
            max: crate::model::geometry::GridPoint { x: zone_x + 10, y: zone_y + 10 },
            exclude_rooms: false,
            exclude_spawns: true,
        });

        // 构建锚点约束（指定某个房间类型）
        let anchor = Constraint::Anchor(AnchorConstraint {
            label: "test-anchor".to_string(),
            room_id: None,
            room_type: Some(RoomType::Treasure),
            target_grid_pos: None,
        });

        let constraints = vec![exclusion_zone.clone(), anchor];

        let result = generate_by_mode(
            seed, config, runtime_ctx, constraints.clone(), None,
        ).expect("带约束的生成应成功");

        // 验证排除区约束：所有点位不在排除区内
        let all_spawns: Vec<SpawnPoint> = result
            .item_spawns
            .iter()
            .chain(result.enemy_spawns.iter())
            .cloned()
            .collect();

        // 提取排除区
        if let Constraint::ExclusionZone(ref zone) = constraints[0] {
            for spawn in &all_spawns {
                let in_zone = spawn.grid_pos.x >= zone.min.x
                    && spawn.grid_pos.x < zone.max.x
                    && spawn.grid_pos.y >= zone.min.y
                    && spawn.grid_pos.y < zone.max.y;
                prop_assert!(
                    !in_zone,
                    "点位 {} 在坐标 ({},{}) 不应在排除区 [{},{})x[{},{}) 内",
                    spawn.id,
                    spawn.grid_pos.x,
                    spawn.grid_pos.y,
                    zone.min.x,
                    zone.max.x,
                    zone.min.y,
                    zone.max.y,
                );
            }
        }

        // 验证锚点约束：如果指定了 room_type=Treasure，且存在分支房间，
        // 则应有至少一个房间被分配为 Treasure 类型。
        // 注意：锚点约束是"尽力满足"的，当所有房间都在关键路径上时
        //（Start/Combat/Boss 已固定），可能无法分配 Treasure。
        // 因此仅在存在分支房间时验证此约束。
        let branch_rooms: Vec<&crate::model::room::Room> = result.rooms.iter()
            .filter(|r| r.branch_id.is_some())
            .collect();
        if !branch_rooms.is_empty() {
            let has_treasure = result.rooms.iter().any(|r| r.room_type == RoomType::Treasure);
            prop_assert!(
                has_treasure,
                "存在分支房间时，锚点约束指定 Treasure 类型应被满足"
            );
        }
    }
}
