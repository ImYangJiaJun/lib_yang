// 连通性验证

use std::collections::{HashSet, VecDeque};

use crate::model::geometry::GridPoint;
use crate::model::terrain::{ConnectivitySummary, Grid2D, TileKind};

use super::grid::{collect_walkable_points, is_walkable};

/// 计算地形连通性摘要。
pub fn summarize_connectivity(grid: &Grid2D<TileKind>) -> ConnectivitySummary {
    let walkable_points = collect_walkable_points(grid);
    let total_tile_count = grid.width * grid.height;
    let walkable_tile_count = walkable_points.len() as u32;

    let mut visited = HashSet::new();
    let mut connected_region_count = 0u32;

    for start in &walkable_points {
        if visited.contains(start) {
            continue;
        }
        connected_region_count += 1;
        flood_fill(*start, grid, &mut visited);
    }

    ConnectivitySummary {
        all_doors_connected: connected_region_count <= 1,
        walkable_tile_count,
        total_tile_count,
        connected_region_count,
    }
}

fn flood_fill(start: GridPoint, grid: &Grid2D<TileKind>, visited: &mut HashSet<GridPoint>) {
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) {
            continue;
        }

        for neighbor in neighbors(current) {
            if visited.contains(&neighbor) {
                continue;
            }
            if grid
                .get(neighbor.x, neighbor.y)
                .copied()
                .is_some_and(is_walkable)
            {
                queue.push_back(neighbor);
            }
        }
    }
}

fn neighbors(point: GridPoint) -> [GridPoint; 4] {
    [
        GridPoint {
            x: point.x + 1,
            y: point.y,
        },
        GridPoint {
            x: point.x - 1,
            y: point.y,
        },
        GridPoint {
            x: point.x,
            y: point.y + 1,
        },
        GridPoint {
            x: point.x,
            y: point.y - 1,
        },
    ]
}

/// 强制连通所有门口的最终兜底 pass（确定性、不消耗 RNG）。
///
/// 各地形策略各有 best-effort 连通修复，但都不保证任意门口布局下全连通；
/// 本 pass 在策略产出之后统一运行，保证「所有 Doorway 瓦片互相可达」这一硬不变量
/// （即 `validate_terrain_connectivity` 必过）。
///
/// 做法：从首个门口 flood，对每个不可达门口，沿其内侧入口到首门口内侧入口的
/// L 形正交路径把途经的**内部**非门口瓦片凿为 `Floor`，每凿一条重新 flood。
/// **严格只改内部瓦片**（保护外圈边框，仅门口本格除外），故不破坏边框完整性。
pub(crate) fn connect_all_doorways(grid: &mut Grid2D<TileKind>, doorways: &[GridPoint]) {
    if doorways.len() <= 1 {
        return;
    }
    let w = grid.width as i32;
    let h = grid.height as i32;
    if w < 3 || h < 3 {
        // 没有可雕刻的内部空间，无能为力（合法配置下不会发生）
        return;
    }

    let first = doorways[0];
    let target = inward_neighbor(first, w, h);
    carve_floor(grid, target, w, h);

    for &door in &doorways[1..] {
        // 当前是否已能从首门口到达 door？已连通则跳过，避免无谓雕刻。
        if reachable_from(grid, first).contains(&door) {
            continue;
        }
        let entry = inward_neighbor(door, w, h);
        carve_floor(grid, entry, w, h);
        carve_orthogonal_interior(grid, entry, target, w, h);
    }
}

/// 从 `start` 出发 flood 所有可通行瓦片，返回可达点集合。
fn reachable_from(grid: &Grid2D<TileKind>, start: GridPoint) -> HashSet<GridPoint> {
    let mut visited = HashSet::new();
    flood_fill(start, grid, &mut visited);
    visited
}

/// 计算门口的内侧入口（与门口正交相邻的内部格）。
fn inward_neighbor(door: GridPoint, w: i32, h: i32) -> GridPoint {
    if door.x == 0 {
        GridPoint { x: 1, y: door.y }
    } else if door.x == w - 1 {
        GridPoint {
            x: w - 2,
            y: door.y,
        }
    } else if door.y == 0 {
        GridPoint { x: door.x, y: 1 }
    } else if door.y == h - 1 {
        GridPoint {
            x: door.x,
            y: h - 2,
        }
    } else {
        door
    }
}

/// 若 `p` 是内部格且非门口，则凿为 `Floor`。
fn carve_floor(grid: &mut Grid2D<TileKind>, p: GridPoint, w: i32, h: i32) {
    if p.x <= 0 || p.y <= 0 || p.x >= w - 1 || p.y >= h - 1 {
        return; // 保护外圈
    }
    if grid.get(p.x, p.y).copied() == Some(TileKind::Doorway) {
        return; // 不覆盖门口
    }
    grid.set(p.x, p.y, TileKind::Floor);
}

/// 沿 L 形正交路径（先 x 后 y）把内部途经格凿为 `Floor`。
fn carve_orthogonal_interior(
    grid: &mut Grid2D<TileKind>,
    from: GridPoint,
    to: GridPoint,
    w: i32,
    h: i32,
) {
    let mut x = from.x;
    while x != to.x {
        carve_floor(grid, GridPoint { x, y: from.y }, w, h);
        x += if to.x > from.x { 1 } else { -1 };
    }
    let mut y = from.y;
    while y != to.y {
        carve_floor(grid, GridPoint { x: to.x, y }, w, h);
        y += if to.y > from.y { 1 } else { -1 };
    }
    carve_floor(grid, to, w, h);
}
