// 开放式地形策略
// Boss 房优先生成开放中心战斗区，障碍物稀疏分布在边缘

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::GridSize;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{ConnectivitySummary, Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

use super::carve::{extract_room_bounds, init_room_grid};
use super::strategy::TerrainStrategy;

/// 开放式地形策略
///
/// 适用于 Boss 房间，生成宽敞的中心战斗区域，障碍物稀疏分布在边缘。
/// 中心区域保持完全开放，为大型战斗提供充足空间。
///
/// # 生成规则
///
/// 1. 四周生成墙体边框
/// 2. 标记门口瓦片
/// 3. 中心区域保持完全开放（地板）
/// 4. 障碍物仅放置在靠近墙体的边缘区域
/// 5. 确保所有门口之间连通
#[non_exhaustive]
pub struct OpenArenaStrategy;

impl TerrainStrategy for OpenArenaStrategy {
    fn name(&self) -> &str {
        "open_arena"
    }

    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain> {
        let (bounds, width, height) = extract_room_bounds(room)?;
        let mut tiles = init_room_grid(width, height);

        // 标记门口瓦片
        let doorway_locals = super::carve::mark_doorways(&mut tiles, anchors, &room.id, bounds.min);

        // 在边缘区域稀疏放置障碍物
        // 边缘区域定义为距离墙体 1-2 格的区域
        let edge_margin = 2i32;
        let obstacle_budget =
            ((width * height) as f32 * config.obstacle_density * 0.3).round() as usize;
        let mut placed = 0usize;
        let max_attempts = obstacle_budget * 10;
        let mut attempts = 0usize;

        while placed < obstacle_budget && attempts < max_attempts {
            attempts += 1;
            let x = rng.random_range(1, width as i32 - 1);
            let y = rng.random_range(1, height as i32 - 1);

            // 仅在边缘区域放置（靠近墙体但不在中心）
            let dist_to_left = x;
            let dist_to_right = width as i32 - 1 - x;
            let dist_to_top = y;
            let dist_to_bottom = height as i32 - 1 - y;
            let min_dist = dist_to_left
                .min(dist_to_right)
                .min(dist_to_top)
                .min(dist_to_bottom);

            // 只在距离墙体 1~edge_margin 格的位置放置
            if min_dist >= 1
                && min_dist <= edge_margin
                && tiles.get(x, y).copied() == Some(TileKind::Floor)
            {
                tiles.set(x, y, TileKind::Obstacle);
                placed += 1;
            }
        }

        // 确保门口之间连通（通过 BFS 验证，如果不连通则移除阻塞障碍物）
        ensure_doorway_connectivity(&mut tiles, &doorway_locals);

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

/// 确保所有门口之间连通
///
/// 如果门口之间不连通，逐步移除障碍物直到连通
fn ensure_doorway_connectivity(
    tiles: &mut Grid2D<TileKind>,
    doorways: &[crate::model::geometry::GridPoint],
) {
    use std::collections::VecDeque;

    if doorways.len() < 2 {
        return;
    }

    let w = tiles.width;
    let h = tiles.height;
    let size = (w * h) as usize;

    // OPT-P-03: 平坦 Vec<bool> 位图替代 HashSet
    // 从第一个门口开始 BFS，检查是否能到达所有其他门口
    let start = doorways[0];
    let mut visited = vec![false; size];
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[(start.y as u32 * w + start.x as u32) as usize] = true;

    while let Some(current) = queue.pop_front() {
        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let ni = (ny as u32 * w + nx as u32) as usize;
            if visited[ni] {
                continue;
            }
            if let Some(tile) = tiles.get(nx, ny).copied() {
                if super::grid::is_walkable(tile) {
                    visited[ni] = true;
                    queue.push_back(crate::model::geometry::GridPoint { x: nx, y: ny });
                }
            }
        }
    }

    // 检查是否所有门口都可达
    let all_reachable = doorways
        .iter()
        .all(|d| visited[(d.y as u32 * w + d.x as u32) as usize]);
    if all_reachable {
        return;
    }

    // 如果不连通，移除障碍物来建立通路
    // 策略：沿着中心十字线清除障碍物
    let cx = tiles.width as i32 / 2;
    let cy = tiles.height as i32 / 2;

    // 清除水平中心线附近的障碍物
    for x in 1..tiles.width as i32 - 1 {
        for dy in -1..=1i32 {
            let y = cy + dy;
            if y > 0
                && y < tiles.height as i32 - 1
                && tiles.get(x, y).copied() == Some(TileKind::Obstacle)
            {
                tiles.set(x, y, TileKind::Floor);
            }
        }
    }

    // 清除垂直中心线附近的障碍物
    for y in 1..tiles.height as i32 - 1 {
        for dx in -1..=1i32 {
            let x = cx + dx;
            if x > 0
                && x < tiles.width as i32 - 1
                && tiles.get(x, y).copied() == Some(TileKind::Obstacle)
            {
                tiles.set(x, y, TileKind::Floor);
            }
        }
    }
}
