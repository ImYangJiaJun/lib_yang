// 迷宫式地形策略
// 使用递归回溯算法生成迷宫式通道布局

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::{GridPoint, GridSize};
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

use super::carve::extract_room_bounds;
use super::connectivity::summarize_connectivity;
use super::grid::to_local;
use super::strategy::TerrainStrategy;

/// 迷宫式地形策略
///
/// 使用递归回溯算法生成迷宫式通道布局，适用于谜题房间。
/// 生成的迷宫保证所有门口之间连通，通道宽度为 1 格。
///
/// # 生成规则
///
/// 1. 初始化整个网格为墙体
/// 2. 使用递归回溯算法雕刻通道
/// 3. 确保门口位置被连接到迷宫网络
/// 4. 根据配置的可通行比例调整迷宫密度
#[non_exhaustive]
pub struct MazeStrategy;

impl TerrainStrategy for MazeStrategy {
    fn name(&self) -> &str {
        "maze"
    }

    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        _config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain> {
        let (bounds, width, height) = extract_room_bounds(room)?;

        // 初始化网格为墙体（迷宫从全墙开始雕刻）
        let mut tiles = Grid2D::new(width, height, TileKind::Wall);

        // 标记门口瓦片
        let room_anchors: Vec<&DoorAnchor> =
            anchors.iter().filter(|a| a.room_id == room.id).collect();
        let doorway_locals: Vec<GridPoint> = room_anchors
            .iter()
            .map(|a| to_local(bounds.min, a.grid_pos))
            .collect();

        for pos in &doorway_locals {
            tiles.set(pos.x, pos.y, TileKind::Doorway);
        }

        // 使用递归回溯算法生成迷宫
        // 迷宫在奇数坐标上雕刻通道（确保墙体间隔）
        generate_maze_recursive_backtrack(&mut tiles, width, height, rng);

        // 确保门口连接到迷宫网络
        connect_doorways_to_maze(&mut tiles, &doorway_locals, width, height);

        // 如果门口数量 >= 2，额外确保连通
        // (force_connect_doorways 在已连通时是空操作)
        if doorway_locals.len() >= 2 {
            force_connect_doorways(&mut tiles, &doorway_locals, width, height);
        }

        let connectivity_summary = summarize_connectivity(&tiles);

        Ok(Terrain {
            room_id: room.id.clone(),
            grid_size: GridSize { width, height },
            tiles,
            reserved_zones: Vec::new(),
            connectivity_summary,
        })
    }
}

/// 使用递归回溯算法生成迷宫
///
/// 从内部起点开始，随机选择方向雕刻通道
fn generate_maze_recursive_backtrack(
    tiles: &mut Grid2D<TileKind>,
    width: u32,
    height: u32,
    rng: &mut StableRng,
) {
    // 迷宫起点（选择内部奇数坐标）
    let start_x = if width > 4 { 2 } else { 1 };
    let start_y = if height > 4 { 2 } else { 1 };

    // 使用栈模拟递归（避免栈溢出）
    let mut stack: Vec<GridPoint> = Vec::new();
    let mut visited = Grid2D::new(width, height, false);

    let start = GridPoint {
        x: start_x,
        y: start_y,
    };
    tiles.set(start.x, start.y, TileKind::Floor);
    visited.set(start.x, start.y, true);
    stack.push(start);

    while let Some(current) = stack.last().copied() {
        // 获取未访问的邻居（步长为 2，确保墙体间隔）
        let (neighbors, count) = get_unvisited_neighbors(current, &visited, width, height);

        if count == 0 {
            stack.pop();
            continue;
        }

        // 随机选择一个邻居
        let idx = rng.random_range(0, count as i32) as usize;
        let next = neighbors[idx];

        // 雕刻当前到邻居之间的墙
        let wall_x = (current.x + next.x) / 2;
        let wall_y = (current.y + next.y) / 2;
        tiles.set(wall_x, wall_y, TileKind::Floor);
        tiles.set(next.x, next.y, TileKind::Floor);
        visited.set(next.x, next.y, true);

        stack.push(next);
    }
}

/// 获取未访问的邻居节点（步长为 2）
/// 返回栈数组 + 有效元素数量，避免每格分配 Vec
fn get_unvisited_neighbors(
    point: GridPoint,
    visited: &Grid2D<bool>,
    width: u32,
    height: u32,
) -> ([GridPoint; 4], usize) {
    let directions = [(0, 2), (0, -2), (2, 0), (-2, 0)];
    let default = GridPoint { x: 0, y: 0 };
    let mut neighbors = [default; 4];
    let mut count = 0usize;

    for (dx, dy) in &directions {
        let nx = point.x + dx;
        let ny = point.y + dy;

        // 检查边界（保留外墙）
        if nx < 1 || ny < 1 || nx >= width as i32 - 1 || ny >= height as i32 - 1 {
            continue;
        }

        if visited.get(nx, ny).copied() == Some(false) {
            neighbors[count] = GridPoint { x: nx, y: ny };
            count += 1;
        }
    }

    (neighbors, count)
}

/// 将门口连接到最近的迷宫通道
fn connect_doorways_to_maze(
    tiles: &mut Grid2D<TileKind>,
    doorways: &[GridPoint],
    width: u32,
    height: u32,
) {
    use std::collections::{HashSet, VecDeque};

    for doorway in doorways {
        // 检查门口是否已经与迷宫连通
        let mut adjacent_floor = false;
        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = doorway.x + dx;
            let ny = doorway.y + dy;
            if let Some(tile) = tiles.get(nx, ny).copied() {
                if tile == TileKind::Floor || tile == TileKind::Doorway {
                    adjacent_floor = true;
                    break;
                }
            }
        }

        if adjacent_floor {
            continue;
        }

        // BFS 找到最近的地板瓦片，沿途打通
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent: std::collections::HashMap<GridPoint, GridPoint> =
            std::collections::HashMap::new();

        queue.push_back(*doorway);
        visited.insert(*doorway);

        let mut target = None;
        while let Some(current) = queue.pop_front() {
            if current != *doorway {
                if let Some(tile) = tiles.get(current.x, current.y).copied() {
                    if tile == TileKind::Floor {
                        target = Some(current);
                        break;
                    }
                }
            }

            for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nx = current.x + dx;
                let ny = current.y + dy;
                let neighbor = GridPoint { x: nx, y: ny };

                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                if visited.contains(&neighbor) {
                    continue;
                }

                visited.insert(neighbor);
                parent.insert(neighbor, current);
                queue.push_back(neighbor);
            }
        }

        // 回溯路径，打通墙体
        if let Some(end) = target {
            let mut current = end;
            while current != *doorway {
                if let Some(tile) = tiles.get(current.x, current.y).copied() {
                    if tile == TileKind::Wall {
                        tiles.set(current.x, current.y, TileKind::Floor);
                    }
                }
                if let Some(&prev) = parent.get(&current) {
                    current = prev;
                } else {
                    break;
                }
            }
        }
    }
}

/// 强制连通所有门口
///
/// 当迷宫生成后门口之间仍不连通时，直接打通路径
fn force_connect_doorways(
    tiles: &mut Grid2D<TileKind>,
    doorways: &[GridPoint],
    width: u32,
    height: u32,
) {
    use std::collections::{HashSet, VecDeque};

    if doorways.len() < 2 {
        return;
    }

    // 从第一个门口 BFS 找到所有可达的门口
    let start = doorways[0];
    let mut reachable = HashSet::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        if doorways.contains(&current) {
            reachable.insert(current);
        }

        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;
            let neighbor = GridPoint { x: nx, y: ny };

            if visited.contains(&neighbor) {
                continue;
            }
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            if let Some(tile) = tiles.get(nx, ny).copied() {
                if super::grid::is_walkable(tile) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // 对不可达的门口，打通直线路径到第一个门口
    for doorway in doorways.iter().skip(1) {
        if reachable.contains(doorway) {
            continue;
        }

        // 打通从 doorway 到 start 的正交路径
        carve_orthogonal_path(tiles, *doorway, start, width, height);
    }
}

/// 雕刻正交路径（先水平后垂直）
fn carve_orthogonal_path(
    tiles: &mut Grid2D<TileKind>,
    from: GridPoint,
    to: GridPoint,
    width: u32,
    height: u32,
) {
    // 水平段
    let x_step = if to.x > from.x { 1 } else { -1 };
    let mut x = from.x;
    while x != to.x {
        if x > 0
            && x < width as i32 - 1
            && from.y > 0
            && from.y < height as i32 - 1
            && tiles.get(x, from.y).copied() == Some(TileKind::Wall)
        {
            tiles.set(x, from.y, TileKind::Floor);
        }
        x += x_step;
    }

    // 垂直段
    let y_step = if to.y > from.y { 1 } else { -1 };
    let mut y = from.y;
    while y != to.y {
        if to.x > 0
            && to.x < width as i32 - 1
            && y > 0
            && y < height as i32 - 1
            && tiles.get(to.x, y).copied() == Some(TileKind::Wall)
        {
            tiles.set(to.x, y, TileKind::Floor);
        }
        y += y_step;
    }
}
