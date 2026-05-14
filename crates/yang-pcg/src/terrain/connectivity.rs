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
