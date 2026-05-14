// 点位生成模块
// 负责生成交互物和敌人点位

pub mod budget;
pub mod enemies;
pub mod items;
pub mod sampling;

use crate::config::NormalizedConfig;
use crate::debug::{RejectionReason, SpawnDebugInfo};
use crate::error::PcgResult;
use crate::model::room::Room;
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use crate::rng::StableRng;

/// 点位生成结果。
#[derive(Debug, Clone)]
pub struct SpawnOutput {
    pub item_spawns: Vec<SpawnPoint>,
    pub enemy_spawns: Vec<SpawnPoint>,
}

/// 带调试信息的点位生成结果。
#[derive(Debug, Clone)]
pub struct SpawnOutputWithDebug {
    /// 点位生成结果
    pub output: SpawnOutput,
    /// 点位生成调试信息
    pub debug_info: SpawnDebugInfo,
}

/// 为所有房间生成交互物和敌人点位。
pub fn generate_spawns(
    rooms: &[Room],
    terrains: &[Terrain],
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> PcgResult<SpawnOutput> {
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();

    for room in rooms {
        let Some(terrain) = terrains.iter().find(|terrain| terrain.room_id == room.id) else {
            continue;
        };
        let mut item_rng = rng.derive(&format!("items:{}", room.id));
        let mut enemy_rng = rng.derive(&format!("enemies:{}", room.id));
        item_spawns.extend(items::generate_item_spawns_for_room(
            room,
            terrain,
            config,
            &mut item_rng,
        ));
        enemy_spawns.extend(enemies::generate_enemy_spawns_for_room(
            room,
            terrain,
            config,
            &mut enemy_rng,
        ));
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
pub fn generate_spawns_with_debug(
    rooms: &[Room],
    terrains: &[Terrain],
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> PcgResult<SpawnOutputWithDebug> {
    let mut item_spawns = Vec::new();
    let mut enemy_spawns = Vec::new();
    let mut total_candidates = 0usize;
    let mut total_rejections = Vec::<RejectionReason>::new();
    let mut total_accepted = 0usize;

    for room in rooms {
        let Some(terrain) = terrains.iter().find(|terrain| terrain.room_id == room.id) else {
            continue;
        };
        let mut item_rng = rng.derive(&format!("items:{}", room.id));
        let mut enemy_rng = rng.derive(&format!("enemies:{}", room.id));

        let item_result = items::generate_item_spawns_for_room_tracked(
            room,
            terrain,
            config,
            &mut item_rng,
        );
        total_candidates += item_result.candidate_count;
        total_rejections.extend(item_result.rejections);
        total_accepted += item_result.spawns.len();
        item_spawns.extend(item_result.spawns);

        let enemy_result = enemies::generate_enemy_spawns_for_room_tracked(
            room,
            terrain,
            config,
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
