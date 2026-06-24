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
use crate::model::geometry::GridPoint;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

// 重新导出策略相关类型
pub use default_strategy::DefaultCarveStrategy;
pub use maze::MazeStrategy;
pub use open_arena::OpenArenaStrategy;
pub use organic::OrganicStrategy;
pub use pillar::PillarStrategy;
pub use selector::select_strategy;
pub use strategy::{TerrainStrategy, TerrainStrategyKind};

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

        // 根据房间属性选择策略并生成地形；策略失败回退默认策略，回退也失败则传播错误
        let mut terrain = match select_strategy(room).generate(room, door_anchors, terrain_config, rng) {
            Ok(terrain) => terrain,
            Err(primary_err) => {
                let mut fallback_rng = rng.derive(&format!("fallback:{}", room.id));
                TerrainStrategyKind::DefaultCarve
                    .generate(room, door_anchors, terrain_config, &mut fallback_rng)
                    .map_err(|_| primary_err)?
            }
        };

        // 连通性兜底：保证所有门口互相可达（生成时保证，而非生成后补救）
        repair_terrain_connectivity(&mut terrain);
        terrains.push(terrain);
    }

    Ok(terrains)
}

/// 对单个地形运行连通性兜底修复，并重算连通性摘要。
///
/// 在策略产出之后统一调用（整层与分块两路共用），保证「所有门口互相可达」硬不变量。
/// 纯确定性、不消耗 RNG。
pub(crate) fn repair_terrain_connectivity(terrain: &mut Terrain) {
    let doorways = collect_doorways(&terrain.tiles);
    connectivity::connect_all_doorways(&mut terrain.tiles, &doorways);
    terrain.connectivity_summary = connectivity::summarize_connectivity(&terrain.tiles);
}

/// 扫描网格收集所有门口瓦片的局部坐标（行优先顺序，确定性）。
fn collect_doorways(grid: &Grid2D<TileKind>) -> Vec<GridPoint> {
    let mut out = Vec::new();
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            if grid.get(x, y).copied() == Some(TileKind::Doorway) {
                out.push(GridPoint { x, y });
            }
        }
    }
    out
}
