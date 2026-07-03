// 运行时分块生成模块
// 提供 RuntimeChunked 和 HybridPrecompute 模式的增量生成逻辑

use std::collections::HashSet;
use std::time::Instant;

use crate::backend::{select_backend, PipelineBackend, ValidationScope};
use crate::config::NormalizedConfig;
use crate::constraint;
use crate::digest::ConfigDigest;
use crate::error::{PcgError, PcgResult};
use crate::export::CURRENT_SCHEMA_VERSION;
use crate::layout::LayoutOutput;
use crate::model::chunk::Chunk;
use crate::model::request::{ChunkId, GenerationRequest};
use crate::model::result::{GenerationResult, ResultMetadata};
use crate::model::room::{Corridor, DoorAnchor, Room, RoomGraph};
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use crate::rng::StableRng;
use crate::terrain::strategy::TerrainStrategyKind;
use crate::ue;
use crate::validation::{
    validate_no_overlap, validate_request, validate_spawn_spacing, validate_terrain_connectivity,
};
use crate::{terrain, topology};

/// 拓扑预计算结果
///
/// 包含楼层拓扑和布局信息，可复用于后续分块细节填充。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TopologyResult {
    /// 随机种子
    pub seed: u64,
    /// 配置摘要
    pub config_digest: String,
    /// 归一化配置（crate 内部访问；外部通过 [`config()`](Self::config) 获取）
    pub(crate) normalized: NormalizedConfig,
    /// 拓扑图
    pub topology: RoomGraph,
    /// 布局输出（房间边界、门锚点、走廊）
    pub layout: LayoutOutput,
    /// 分块列表
    pub chunks: Vec<Chunk>,
    /// 约束列表
    pub constraints: Vec<crate::model::request::Constraint>,
    /// 追踪标识
    pub trace_id: Option<String>,
}

/// 分块细节结果
///
/// 包含单个分块内房间的地形和点位数据。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChunkDetailResult {
    /// 分块 ID
    pub chunk_id: ChunkId,
    /// 分块内房间的地形
    pub terrains: Vec<Terrain>,
    /// 分块内的交互物点位
    pub item_spawns: Vec<SpawnPoint>,
    /// 分块内的敌人点位
    pub enemy_spawns: Vec<SpawnPoint>,
    /// 是否因预算限制而提前返回（部分结果）
    pub partial: bool,
}

/// 仅生成楼层拓扑和布局（HybridPrecompute 模式第一阶段）
///
/// 生成完整的楼层拓扑图和空间布局，但不填充房间内部细节（地形、点位）。
/// 返回的 `TopologyResult` 可用于后续按需调用 `fill_chunk_details`。
///
/// # 参数
/// - `request`: 生成请求
///
/// # 返回
/// - `Ok(TopologyResult)`: 拓扑和布局预计算结果
/// - `Err(PcgError)`: 配置或拓扑生成错误
pub fn generate_topology_only(request: GenerationRequest) -> PcgResult<TopologyResult> {
    let normalized = validate_request(&request)?;
    // seed_and_digest_from_config 一次序列化同时产出 seed 和 digest，避免双重序列化。
    let (seed, config_digest) = match request.seed {
        Some(s) => (
            s,
            ConfigDigest::from_config(&normalized.config)?.into_string(),
        ),
        None => ConfigDigest::seed_and_digest_from_config(&normalized.config)?,
    };
    let root_rng = StableRng::from_seed(seed);

    constraint::validate_constraints(&request.constraints)?;

    let backend = select_backend(&normalized);

    // 拓扑阶段
    let mut topology_rng = root_rng.derive("topology");
    let mut graph = topology::generate_topology(&normalized, &mut topology_rng)?;
    constraint::apply_room_constraints(&mut graph.nodes, &request.constraints);

    // 布局阶段
    let mut layout_rng = root_rng.derive("layout");
    let layout_output = backend.solve_layout(&graph, &normalized, &mut layout_rng)?;

    // 布局阶段硬校验：房间边界不重叠（拓扑/布局已就绪，地形/点位尚未生成）
    validate_no_overlap(&layout_output.rooms)?;

    // 生成分块元数据
    let chunks = ue::streaming::build_chunks(&layout_output.rooms, &normalized);

    Ok(TopologyResult {
        seed,
        config_digest,
        normalized,
        topology: graph,
        layout: layout_output,
        chunks,
        constraints: request.constraints,
        trace_id: request.trace_id,
    })
}

/// 按需填充分块内部细节（HybridPrecompute 模式第二阶段）
///
/// 根据已有的拓扑结果，仅为指定分块内的房间生成地形和点位。
/// 支持时间预算和迭代预算限制，超出预算时返回部分结果。
///
/// # 参数
/// - `topology_result`: 拓扑预计算结果
/// - `chunk_id`: 要填充的分块 ID
///
/// # 返回
/// - `Ok(ChunkDetailResult)`: 分块细节
/// - `Err(PcgError)`: 分块不存在或生成错误
pub fn fill_chunk_details(
    topology_result: &TopologyResult,
    chunk_id: &str,
) -> PcgResult<ChunkDetailResult> {
    // 查找目标分块
    let chunk = topology_result
        .chunks
        .iter()
        .find(|c| c.id == chunk_id)
        .ok_or_else(|| {
            PcgError::config_with_field(format!("分块 '{}' 不存在", chunk_id), "chunk_id")
        })?;

    // 预建 HashSet，O(1) 查找替代 Vec::contains 的 O(N)
    let room_id_set: HashSet<&str> = chunk.room_ids.iter().map(|s| s.as_str()).collect();

    // 筛选分块内的房间
    let chunk_rooms: Vec<&Room> = topology_result
        .layout
        .rooms
        .iter()
        .filter(|room| room_id_set.contains(room.id.as_str()))
        .collect();

    // 筛选分块内房间相关的门锚点（owned，避免在循环内每轮重复 clone）
    let chunk_anchors: Vec<DoorAnchor> = topology_result
        .layout
        .door_anchors
        .iter()
        .filter(|anchor| room_id_set.contains(anchor.room_id.as_str()))
        .cloned()
        .collect();

    let normalized = &topology_result.normalized;
    let seed = topology_result.seed;
    let root_rng = StableRng::from_seed(seed);
    let start_time = Instant::now();
    let time_budget_ms = normalized.time_budget_ms;
    let iteration_budget = normalized.iteration_budget;

    let mut terrains = Vec::new();
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();
    let mut partial = false;

    // 为分块内每个房间生成地形和点位
    for (idx, room) in chunk_rooms.iter().enumerate() {
        // 检查时间预算
        if let Some(budget_ms) = time_budget_ms {
            if start_time.elapsed().as_millis() as u64 >= budget_ms {
                partial = true;
                break;
            }
        }

        // 检查迭代预算
        if let Some(budget) = iteration_budget {
            if idx as u32 >= budget {
                partial = true;
                break;
            }
        }

        // 使用确定性 RNG 派生（基于 chunk_id + room_id 保证一致性）
        let mut terrain_rng = root_rng.derive(&format!("terrain:chunk:{}:{}", chunk_id, room.id));

        // 生成地形
        let terrain_config = &normalized.config.terrain;
        let terrain_result = terrain::select_strategy(room).generate(
            room,
            &chunk_anchors,
            terrain_config,
            &mut terrain_rng,
        );

        match terrain_result {
            Ok(mut t) => {
                // 连通性兜底（与整层路径一致，须在点位采样之前）
                terrain::repair_terrain_connectivity(&mut t);

                // 生成点位
                let mut item_rng =
                    root_rng.derive(&format!("items:chunk:{}:{}", chunk_id, room.id));
                let mut enemy_rng =
                    root_rng.derive(&format!("enemies:chunk:{}:{}", chunk_id, room.id));

                let room_items = crate::spawn::items::generate_item_spawns_for_room(
                    room,
                    &t,
                    normalized,
                    &mut item_rng,
                );
                // 敌人采样避开已放置交互物，保证跨类型间距（与整层路径一致）
                let occupied = crate::spawn::occupied_local_points(room, &room_items);
                let cross_spacing = crate::spawn::min_cross_type_spacing(&normalized.config);
                let room_enemies = crate::spawn::enemies::generate_enemy_spawns_for_room_excluding(
                    room,
                    &t,
                    normalized,
                    &occupied,
                    cross_spacing,
                    &mut enemy_rng,
                );

                // 应用约束过滤
                let (filtered_items, filtered_enemies) = constraint::apply_spawn_constraints(
                    room_items,
                    room_enemies,
                    &topology_result.constraints,
                );

                item_spawns.extend(filtered_items);
                enemy_spawns.extend(filtered_enemies);
                terrains.push(t);
            }
            Err(primary_err) => {
                // 策略失败时回退到默认策略；若回退也失败，则传播错误而非静默丢房间
                let mut fallback_rng =
                    root_rng.derive(&format!("terrain:fallback:{}:{}", chunk_id, room.id));
                let mut t = TerrainStrategyKind::DefaultCarve
                    .generate(room, &chunk_anchors, terrain_config, &mut fallback_rng)
                    .map_err(|_| primary_err)?;
                terrain::repair_terrain_connectivity(&mut t);
                terrains.push(t);
            }
        }
    }

    // 分块硬校验（局部不变量，对任意子集成立）：地形连通 + 点位间距。
    // 不做整图结构/可达性校验（部分结果天然不满足）。
    validate_terrain_connectivity(&terrains)?;
    let all_spawns: Vec<SpawnPoint> = item_spawns
        .iter()
        .chain(enemy_spawns.iter())
        .cloned()
        .collect();
    let min_spacing = i32::from(crate::spawn::min_cross_type_spacing(&normalized.config));
    validate_spawn_spacing(&all_spawns, &topology_result.constraints, Some(min_spacing))?;

    Ok(ChunkDetailResult {
        chunk_id: chunk_id.to_string(),
        terrains,
        item_spawns,
        enemy_spawns,
        partial,
    })
}

/// RuntimeChunked 模式的增量生成
///
/// 根据请求中的 `runtime_context.requested_chunks` 列表，
/// 仅生成指定分块内的房间细节和点位。复用已有拓扑结果。
///
/// # 参数
/// - `request`: 生成请求（必须包含 runtime_context）
///
/// # 返回
/// - `Ok(GenerationResult)`: 仅包含请求分块内容的生成结果
/// - `Err(PcgError)`: 配置或生成错误
pub fn generate_chunk(request: GenerationRequest) -> PcgResult<GenerationResult> {
    let normalized = validate_request(&request)?;
    // seed_and_digest_from_config 一次序列化同时产出 seed 和 digest，避免双重序列化。
    let (seed, config_digest) = match request.seed {
        Some(s) => (
            s,
            ConfigDigest::from_config(&normalized.config)?.into_string(),
        ),
        None => ConfigDigest::seed_and_digest_from_config(&normalized.config)?,
    };
    let root_rng = StableRng::from_seed(seed);

    // 获取请求的分块 ID 列表
    let requested_chunks: Vec<ChunkId> = request
        .runtime_context
        .as_ref()
        .map(|ctx| ctx.requested_chunks.clone())
        .unwrap_or_default();

    constraint::validate_constraints(&request.constraints)?;

    let backend = select_backend(&normalized);

    // 拓扑阶段（整层，可复用）
    let mut topology_rng = root_rng.derive("topology");
    let mut graph = topology::generate_topology(&normalized, &mut topology_rng)?;
    constraint::apply_room_constraints(&mut graph.nodes, &request.constraints);

    // 布局阶段（整层，可复用）
    let mut layout_rng = root_rng.derive("layout");
    let layout_output = backend.solve_layout(&graph, &normalized, &mut layout_rng)?;

    // 生成分块元数据
    let all_chunks = ue::streaming::build_chunks(&layout_output.rooms, &normalized);

    // 确定需要生成细节的分块
    let target_chunks: Vec<&Chunk> = if requested_chunks.is_empty() {
        // 如果没有指定分块，生成所有分块
        all_chunks.iter().collect()
    } else {
        all_chunks
            .iter()
            .filter(|c| requested_chunks.contains(&c.id))
            .collect()
    };

    // 收集目标分块内的房间 ID
    let target_room_ids: std::collections::HashSet<String> = target_chunks
        .iter()
        .flat_map(|c| c.room_ids.iter().cloned())
        .collect();

    // 筛选目标房间
    let target_rooms: Vec<&Room> = layout_output
        .rooms
        .iter()
        .filter(|room| target_room_ids.contains(&room.id))
        .collect();

    // 筛选目标门锚点
    let target_anchors: Vec<DoorAnchor> = layout_output
        .door_anchors
        .iter()
        .filter(|anchor| target_room_ids.contains(&anchor.room_id))
        .cloned()
        .collect();

    // 筛选目标走廊
    let target_corridors: Vec<Corridor> = layout_output
        .corridors
        .iter()
        .filter(|corridor| {
            target_room_ids.contains(&corridor.from_room)
                || target_room_ids.contains(&corridor.to_room)
        })
        .cloned()
        .collect();

    let start_time = Instant::now();
    let time_budget_ms = normalized.time_budget_ms;
    let iteration_budget = normalized.iteration_budget;

    // 地形阶段（仅目标房间）
    let mut terrains = Vec::new();
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();

    for (idx, room) in target_rooms.iter().enumerate() {
        // 检查时间预算
        if let Some(budget_ms) = time_budget_ms {
            if start_time.elapsed().as_millis() as u64 >= budget_ms {
                break;
            }
        }

        // 检查迭代预算
        if let Some(budget) = iteration_budget {
            if idx as u32 >= budget {
                break;
            }
        }

        // 使用与整层生成一致的 RNG 派生路径，保证确定性
        let mut terrain_rng = root_rng.derive(&format!("terrain:{}", room.id));

        let terrain_config = &normalized.config.terrain;
        let terrain_result = terrain::select_strategy(room).generate(
            room,
            &target_anchors,
            terrain_config,
            &mut terrain_rng,
        );

        match terrain_result {
            Ok(mut t) => {
                // 连通性兜底（与整层路径一致，须在点位采样之前）
                terrain::repair_terrain_connectivity(&mut t);

                // 生成点位
                let mut item_rng = root_rng.derive(&format!("items:{}", room.id));
                let mut enemy_rng = root_rng.derive(&format!("enemies:{}", room.id));

                let room_items = crate::spawn::items::generate_item_spawns_for_room(
                    room,
                    &t,
                    &normalized,
                    &mut item_rng,
                );
                // 敌人采样避开已放置交互物，保证跨类型间距（与整层路径一致）
                let occupied = crate::spawn::occupied_local_points(room, &room_items);
                let cross_spacing = crate::spawn::min_cross_type_spacing(&normalized.config);
                let room_enemies = crate::spawn::enemies::generate_enemy_spawns_for_room_excluding(
                    room,
                    &t,
                    &normalized,
                    &occupied,
                    cross_spacing,
                    &mut enemy_rng,
                );

                let (filtered_items, filtered_enemies) = constraint::apply_spawn_constraints(
                    room_items,
                    room_enemies,
                    &request.constraints,
                );

                item_spawns.extend(filtered_items);
                enemy_spawns.extend(filtered_enemies);
                terrains.push(t);
            }
            Err(primary_err) => {
                // 策略失败时回退到默认策略；若回退也失败，则传播错误而非静默丢房间
                let mut fallback_rng = root_rng.derive(&format!("terrain:fallback:{}", room.id));
                let mut t = TerrainStrategyKind::DefaultCarve
                    .generate(room, &target_anchors, terrain_config, &mut fallback_rng)
                    .map_err(|_| primary_err)?;
                terrain::repair_terrain_connectivity(&mut t);
                terrains.push(t);
            }
        }
    }

    // 组装结果
    let result = GenerationResult {
        metadata: ResultMetadata {
            seed,
            config_digest,
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
            algorithm_version: env!("CARGO_PKG_VERSION").to_string(),
            target_engine_version: None,
            trace_id: request.trace_id,
        },
        topology: graph,
        rooms: target_rooms.into_iter().cloned().collect(),
        door_anchors: target_anchors,
        corridors: target_corridors,
        terrains,
        item_spawns,
        enemy_spawns,
        chunks: all_chunks,
        debug: None,
    };

    // 分块部分结果的硬校验（跳过整图结构/可达性，保留局部不变量）
    backend.validate(
        &result,
        &normalized,
        &request.constraints,
        ValidationScope::Chunk,
    )?;

    Ok(result)
}

#[cfg(test)]
#[path = "chunked_tests.rs"]
mod tests;
