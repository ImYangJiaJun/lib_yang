// 地形生成模块
// 负责为每个房间生成逻辑网格地形

pub mod carve;
pub mod connectivity;
pub mod default_strategy;
pub mod grid;
pub mod maze;
pub mod open_arena;
pub mod organic;
pub mod pillar;
pub mod selector;
pub mod strategy;

#[cfg(test)]
mod __tests__;

use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::Terrain;
use crate::rng::StableRng;

// 重新导出策略相关类型
pub use default_strategy::DefaultCarveStrategy;
pub use maze::MazeStrategy;
pub use open_arena::OpenArenaStrategy;
pub use organic::OrganicStrategy;
pub use pillar::PillarStrategy;
pub use selector::select_strategy;
pub use strategy::TerrainStrategy;

/// 批量生成房间地形。
///
/// 根据每个房间的类型和主题标签自动选择合适的地形策略，
/// 然后使用该策略生成地形数据。
pub fn generate_terrains(
    rooms: &[Room],
    door_anchors: &[DoorAnchor],
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> PcgResult<Vec<Terrain>> {
    let terrain_config = &config.config.terrain;
    let mut terrains = Vec::with_capacity(rooms.len());

    for room in rooms {
        // 跳过没有边界的房间
        if room.bounds.is_none() {
            continue;
        }

        // 根据房间属性选择策略
        let strategy = select_strategy(room);

        // 使用选定策略生成地形；策略失败回退默认策略，回退也失败则传播错误
        let terrain = match strategy.generate(room, door_anchors, terrain_config, rng) {
            Ok(terrain) => terrain,
            Err(primary_err) => DefaultCarveStrategy
                .generate(room, door_anchors, terrain_config, rng)
                .map_err(|_| primary_err)?,
        };
        terrains.push(terrain);
    }

    Ok(terrains)
}
