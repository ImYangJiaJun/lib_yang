// 柱状地形策略
// 在地板上放置规则柱状障碍物，提供掩体

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::GridSize;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{ConnectivitySummary, Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

use super::carve::{extract_room_bounds, init_room_grid};
use super::strategy::TerrainStrategy;

/// 柱状地形策略
///
/// 在房间地板上按规则间距放置柱状障碍物，为战斗提供掩体。
/// 柱子按网格模式排列，间距由房间尺寸和配置决定。
///
/// # 生成规则
///
/// 1. 四周生成墙体边框
/// 2. 标记门口瓦片
/// 3. 按固定间距放置 1x1 或 2x2 的柱子
/// 4. 柱子不能覆盖门口位置
/// 5. 确保所有门口之间连通
#[non_exhaustive]
pub struct PillarStrategy;

impl TerrainStrategy for PillarStrategy {
    fn name(&self) -> &str {
        "pillar"
    }

    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain> {
        let (bounds, width, height) = extract_room_bounds(room)?;
        let mut tiles = init_room_grid(width, height)?;

        // 标记门口瓦片
        super::carve::mark_doorways(&mut tiles, anchors, &room.id, bounds.min);

        // 计算柱子间距（基于障碍物密度）
        // 密度越高，间距越小
        let spacing = calculate_pillar_spacing(width, height, config.obstacle_density);

        // 根据密度决定柱子大小（1x1 或 2x2）
        let pillar_size = if config.obstacle_density > 0.3 { 2 } else { 1 };

        // 按网格模式放置柱子
        let start_x = spacing;
        let start_y = spacing;

        let mut y = start_y as i32;
        while y < height as i32 - 1 {
            let mut x = start_x as i32;
            while x < width as i32 - 1 {
                // 添加轻微随机偏移
                let offset_x = if spacing > 2 {
                    rng.random_range(-1, 2)
                } else {
                    0
                };
                let offset_y = if spacing > 2 {
                    rng.random_range(-1, 2)
                } else {
                    0
                };

                let px = x + offset_x;
                let py = y + offset_y;

                // 放置柱子（检查是否可以放置）
                place_pillar(&mut tiles, px, py, pillar_size, width, height);

                x += spacing as i32;
            }
            y += spacing as i32;
        }

        Ok(Terrain {
            room_id: room.id.clone(),
            grid_size: GridSize { width, height },
            tiles,
            reserved_zones: Vec::new(),
            // 该字段在下游 repair_terrain_connectivity() 中会被覆写，此处只需占位
            connectivity_summary: ConnectivitySummary {
                all_doors_connected: false,
                walkable_tile_count: 0,
                total_tile_count: 0,
                connected_region_count: 0,
            },
        })
    }
}

/// 计算柱子间距
///
/// 根据房间尺寸和障碍物密度计算合适的柱子间距
fn calculate_pillar_spacing(width: u32, height: u32, density: f32) -> u32 {
    // 基础间距为房间较小维度的 1/4，但至少为 3
    let base_spacing = (width.min(height) / 4).max(3);

    // 根据密度调整：密度越高间距越小
    let adjusted = (base_spacing as f32 / (density * 2.0 + 0.5)).round() as u32;

    // 限制在合理范围内
    adjusted.clamp(3, 8)
}

/// 在指定位置放置柱子
///
/// 检查目标位置是否可以放置柱子（不覆盖门口、不超出边界）
fn place_pillar(tiles: &mut Grid2D<TileKind>, x: i32, y: i32, size: i32, width: u32, height: u32) {
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;

            // 检查边界
            if px < 1 || py < 1 || px >= width as i32 - 1 || py >= height as i32 - 1 {
                continue;
            }

            // 不覆盖门口
            if tiles.get(px, py).copied() == Some(TileKind::Doorway) {
                continue;
            }

            // 只在地板上放置
            if tiles.get(px, py).copied() == Some(TileKind::Floor) {
                tiles.set(px, py, TileKind::Obstacle);
            }
        }
    }
}
