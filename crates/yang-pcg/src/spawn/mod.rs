// 点位生成模块
// 负责生成交互物和敌人点位

pub(crate) mod budget;
pub(crate) mod enemies;
pub(crate) mod items;
pub(crate) mod sampling;

use std::collections::HashMap;

use crate::config::NormalizedConfig;
use crate::debug::{RejectionReason, SpawnDebugInfo};
use crate::error::PcgResult;
use crate::model::geometry::GridPoint;
use crate::model::room::Room;
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use crate::rng::StableRng;

/// 点位生成结果。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(crate) struct SpawnOutput {
    pub(crate) item_spawns: Vec<SpawnPoint>,
    pub(crate) enemy_spawns: Vec<SpawnPoint>,
}

/// 跨类型点位间距阈值：交互物与敌人最小间距的较小者。
///
/// 用作交互物↔敌人之间的生成期间距下限，以及校验期的统一阈值，
/// 保证「生成时满足 ⟹ 校验通过」。
pub(crate) fn min_cross_type_spacing(config: &crate::config::GenerationConfig) -> u16 {
    config
        .item_spawns
        .min_spacing
        .min(config.enemy_spawns.min_spacing)
}

/// 把一组点位（世界坐标）转换为相对房间边界的局部坐标，作为敌人采样的占用集合。
///
/// 敌人候选点是局部坐标，而点位 `grid_pos` 存的是世界坐标（= bounds.min + local），
/// 故跨类型间距比较前需换回局部坐标。房间无边界时返回空（点位生成阶段不会发生）。
pub(crate) fn occupied_local_points(room: &Room, spawns: &[SpawnPoint]) -> Vec<GridPoint> {
    let Some(bounds) = room.bounds else {
        return Vec::new();
    };
    spawns
        .iter()
        .map(|s| GridPoint {
            x: s.grid_pos.x - bounds.min.x,
            y: s.grid_pos.y - bounds.min.y,
        })
        .collect()
}

/// 带调试信息的点位生成结果。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(crate) struct SpawnOutputWithDebug {
    /// 点位生成结果
    pub(crate) output: SpawnOutput,
    /// 点位生成调试信息
    pub(crate) debug_info: SpawnDebugInfo,
}

/// 为所有房间生成交互物和敌人点位。
///
/// `rng` 只需 `&StableRng`——每房间通过 `derive` 派生独立子 RNG，父 RNG 无需可变借用。
pub(crate) fn generate_spawns(
    rooms: &[Room],
    terrains: &[Terrain],
    config: &NormalizedConfig,
    rng: &StableRng,
) -> PcgResult<SpawnOutput> {
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();

    let cross_spacing = min_cross_type_spacing(&config.config);

    // 构建 room_id -> terrain 映射，避免 O(R²) 线性扫描
    let terrain_map: HashMap<&str, &Terrain> = terrains
        .iter()
        .map(|t| (t.room_id.as_str(), t))
        .collect();

    for room in rooms {
        let Some(terrain) = terrain_map.get(room.id.as_str()) else {
            continue;
        };
        // 先派生两个子 RNG（顺序与重构前一致，保持父 RNG 消耗不变），再依次生成
        let mut item_rng = rng.derive(&format!("items:{}", room.id));
        let mut enemy_rng = rng.derive(&format!("enemies:{}", room.id));

        let room_items = items::generate_item_spawns_for_room(room, terrain, config, &mut item_rng);
        // 敌人采样避开已放置交互物，保证跨类型间距（生成时保证）
        let occupied = occupied_local_points(room, &room_items);
        let room_enemies = enemies::generate_enemy_spawns_for_room_excluding(
            room,
            terrain,
            config,
            &occupied,
            cross_spacing,
            &mut enemy_rng,
        );

        item_spawns.extend(room_items);
        enemy_spawns.extend(room_enemies);
    }

    Ok(SpawnOutput {
        item_spawns,
        enemy_spawns,
    })
}

/// 为所有房间生成交互物和敌人点位，同时收集调试信息。
///
/// 与 `generate_spawns` 功能相同，但额外记录候选点位数、拒绝数和拒绝原因，
/// 用于调试模式下的约束报告输出。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
/// - 需求 15.5: 失败阶段与失败约束输出
pub(crate) fn generate_spawns_with_debug(
    rooms: &[Room],
    terrains: &[Terrain],
    config: &NormalizedConfig,
    rng: &StableRng,
) -> PcgResult<SpawnOutputWithDebug> {
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();
    let mut total_candidates = 0usize;
    let mut total_rejections = Vec::<RejectionReason>::new();
    let mut total_accepted = 0usize;

    let cross_spacing = min_cross_type_spacing(&config.config);

    // 构建 room_id -> terrain 映射，避免 O(R²) 线性扫描
    let terrain_map: HashMap<&str, &Terrain> = terrains
        .iter()
        .map(|t| (t.room_id.as_str(), t))
        .collect();

    for room in rooms {
        let Some(terrain) = terrain_map.get(room.id.as_str()) else {
            continue;
        };
        let mut item_rng = rng.derive(&format!("items:{}", room.id));
        let mut enemy_rng = rng.derive(&format!("enemies:{}", room.id));

        let item_result =
            items::generate_item_spawns_for_room_tracked(room, terrain, config, &mut item_rng);
        total_candidates += item_result.candidate_count;
        total_rejections.extend(item_result.rejections);
        total_accepted += item_result.spawns.len();

        // 敌人采样避开已放置交互物，保证跨类型间距
        let occupied = occupied_local_points(room, &item_result.spawns);
        item_spawns.extend(item_result.spawns);

        let enemy_result = enemies::generate_enemy_spawns_for_room_tracked_excluding(
            room,
            terrain,
            config,
            &occupied,
            cross_spacing,
            &mut enemy_rng,
        );
        total_candidates += enemy_result.candidate_count;
        total_rejections.extend(enemy_result.rejections);
        total_accepted += enemy_result.spawns.len();
        enemy_spawns.extend(enemy_result.spawns);
    }

    let rejected_count = total_rejections.len();

    Ok(SpawnOutputWithDebug {
        output: SpawnOutput {
            item_spawns,
            enemy_spawns,
        },
        debug_info: SpawnDebugInfo {
            candidate_count: total_candidates,
            rejected_count,
            rejection_reasons: total_rejections,
            accepted_count: total_accepted,
        },
    })
}
