// 有机式地形策略
// 使用 cellular automata 生成自然洞穴形态

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::{GridPoint, GridSize};
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{ConnectivitySummary, Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

use super::carve::{extract_room_bounds, init_room_grid};
use super::grid::to_local;
use super::strategy::TerrainStrategy;

/// 有机式地形策略
///
/// 使用 cellular automata（元胞自动机）生成自然洞穴形态。
/// 适用于带有 "organic" 或 "cave" 主题标签的房间。
///
/// # 生成规则
///
/// 1. 随机初始化网格（根据密度参数）
/// 2. 迭代应用 cellular automata 规则（4-5 规则）
/// 3. 保留外墙边框
/// 4. 标记门口瓦片
/// 5. 确保所有门口之间连通
#[non_exhaustive]
pub struct OrganicStrategy;

/// Cellular automata 迭代次数
const CA_ITERATIONS: usize = 4;

/// 邻居墙体阈值（超过此数量则变为墙）
const WALL_THRESHOLD: usize = 4;

impl TerrainStrategy for OrganicStrategy {
    fn name(&self) -> &str {
        "organic"
    }

    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain> {
        let (bounds, width, height) = extract_room_bounds(room)?;

        // 标记门口位置
        let room_anchors: Vec<&DoorAnchor> =
            anchors.iter().filter(|a| a.room_id == room.id).collect();
        let doorway_locals: Vec<GridPoint> = room_anchors
            .iter()
            .map(|a| to_local(bounds.min, a.grid_pos))
            .collect();

        // 步骤 1：随机初始化网格
        let mut grid_a = initialize_random_grid(width, height, config.obstacle_density, rng)?;
        // grid_b 仅用于双缓冲写入目标，首次 apply_ca_step_into 会全覆盖，无需 clone
        let mut grid_b = Grid2D::new(width, height, TileKind::Wall)?;

        // 步骤 2：应用 cellular automata 规则（双缓冲，避免每次迭代重新分配）
        for _ in 0..CA_ITERATIONS {
            apply_ca_step_into(&grid_a, &mut grid_b, width, height);
            std::mem::swap(&mut grid_a, &mut grid_b);
        }
        let mut tiles = grid_a;

        // 步骤 3：确保外墙边框
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                if x == 0 || y == 0 || x == width as i32 - 1 || y == height as i32 - 1 {
                    tiles.set(x, y, TileKind::Wall);
                }
            }
        }

        // 步骤 4：标记门口瓦片并确保门口周围可通行
        for pos in &doorway_locals {
            tiles.set(pos.x, pos.y, TileKind::Doorway);
            // 确保门口周围至少有一格地板
            for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nx = pos.x + dx;
                let ny = pos.y + dy;
                if nx > 0
                    && nx < width as i32 - 1
                    && ny > 0
                    && ny < height as i32 - 1
                    && tiles.get(nx, ny).copied() == Some(TileKind::Wall)
                {
                    tiles.set(nx, ny, TileKind::Floor);
                }
            }
        }

        // 步骤 5：确保连通性
        ensure_organic_connectivity(&mut tiles, &doorway_locals, width, height);

        // 将内部的 Wall 转换为 Obstacle（保持语义一致：内部障碍物用 Obstacle）
        convert_internal_walls_to_obstacles(&mut tiles, width, height);

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

/// 随机初始化网格
///
/// 根据障碍物密度随机填充墙体和地板
fn initialize_random_grid(
    width: u32,
    height: u32,
    density: f32,
    rng: &mut StableRng,
) -> PcgResult<Grid2D<TileKind>> {
    // OPT-P-13: 使用 init_room_grid 单遍初始化（Wall 边框 + Floor 内部）
    let mut tiles = init_room_grid(width, height)?;

    // 初始填充概率（密度越高，初始墙体越多）
    let fill_probability = (density * 0.8 + 0.2) as f64;

    for y in 1..height as i32 - 1 {
        for x in 1..width as i32 - 1 {
            if rng.gen_bool_with_probability(fill_probability) {
                tiles.set(x, y, TileKind::Wall);
            }
        }
    }

    Ok(tiles)
}

/// 应用一步 cellular automata 规则（双缓冲版本，写入目标网格以避免分配）
///
/// 使用 B5678/S45678 变体规则（适合生成洞穴）：
/// - 如果一个格子周围有 >= WALL_THRESHOLD 个墙体邻居，则变为墙
/// - 否则变为地板
///
/// `src` 为当前代网格（只读），`dst` 为下一代网格（写入目标）。
/// 调用方通过 swap 交替双缓冲来复用在两个网格之间。
fn apply_ca_step_into(src: &Grid2D<TileKind>, dst: &mut Grid2D<TileKind>, width: u32, height: u32) {
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            // 边框始终为墙
            if x == 0 || y == 0 || x == width as i32 - 1 || y == height as i32 - 1 {
                dst.set(x, y, TileKind::Wall);
                continue;
            }

            let wall_count = count_wall_neighbors(src, x, y);

            if wall_count >= WALL_THRESHOLD {
                dst.set(x, y, TileKind::Wall);
            } else {
                dst.set(x, y, TileKind::Floor);
            }
        }
    }
}

/// 计算 8 邻域中的墙体数量
fn count_wall_neighbors(tiles: &Grid2D<TileKind>, x: i32, y: i32) -> usize {
    let mut count = 0;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            match tiles.get(nx, ny).copied() {
                Some(TileKind::Wall) | Some(TileKind::Obstacle) | None => {
                    count += 1;
                }
                _ => {}
            }
        }
    }
    count
}

/// 确保有机地形中所有门口连通
fn ensure_organic_connectivity(
    tiles: &mut Grid2D<TileKind>,
    doorways: &[GridPoint],
    width: u32,
    height: u32,
) {
    use std::collections::VecDeque;

    if doorways.len() < 2 {
        return;
    }

    let size = (width * height) as usize;

    // 从第一个门口 BFS 找到所有可达区域
    // OPT-P-03: HashSet 替换为平坦 Vec<bool> 位图
    let start = doorways[0];
    let mut visited = vec![false; size];
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[(start.y as u32 * width + start.x as u32) as usize] = true;

    while let Some(current) = queue.pop_front() {
        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;

            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let ni = (ny as u32 * width + nx as u32) as usize;
            if visited[ni] {
                continue;
            }
            if let Some(tile) = tiles.get(nx, ny).copied() {
                if super::grid::is_walkable(tile) {
                    visited[ni] = true;
                    queue.push_back(GridPoint { x: nx, y: ny });
                }
            }
        }
    }

    // 对不可达的门口，打通路径
    // OPT-P-09: 预分配 BFS 缓冲区，跨门口复用
    // OPT-P-03: HashSet/HashMap 替换为平坦 Vec 位图/扁平 parent 数组
    let mut bfs_queue = VecDeque::new();
    let mut bfs_visited = vec![false; size];
    let mut bfs_parent: Vec<Option<GridPoint>> = vec![None; size];

    for doorway in doorways.iter().skip(1) {
        let di = (doorway.y as u32 * width + doorway.x as u32) as usize;
        if visited[di] {
            continue;
        }

        // 使用 A* 风格的路径打通（优先选择已有地板的方向）
        carve_path_to_reachable(
            tiles,
            *doorway,
            &visited,
            width,
            height,
            &mut bfs_queue,
            &mut bfs_visited,
            &mut bfs_parent,
        );

        // 更新可达集合
        let mut new_queue = VecDeque::new();
        new_queue.push_back(*doorway);
        if !visited[di] {
            visited[di] = true;
        }

        while let Some(current) = new_queue.pop_front() {
            for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nx = current.x + dx;
                let ny = current.y + dy;

                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                let ni = (ny as u32 * width + nx as u32) as usize;
                if visited[ni] {
                    continue;
                }
                if let Some(tile) = tiles.get(nx, ny).copied() {
                    if super::grid::is_walkable(tile) {
                        visited[ni] = true;
                        new_queue.push_back(GridPoint { x: nx, y: ny });
                    }
                }
            }
        }
    }
}

/// 从不可达门口向可达区域打通路径
///
/// BFS 缓冲区由调用方预分配并跨门口复用（OPT-P-09）。
/// OPT-P-03: HashSet/HashMap 替换为平坦 Vec 位图/扁平 parent 数组。
#[allow(clippy::too_many_arguments)]
fn carve_path_to_reachable(
    tiles: &mut Grid2D<TileKind>,
    start: GridPoint,
    reachable: &[bool],
    width: u32,
    height: u32,
    queue: &mut std::collections::VecDeque<GridPoint>,
    visited: &mut [bool],
    parent: &mut [Option<GridPoint>],
) {
    // BFS 搜索最近的可达瓦片
    queue.clear();
    visited.fill(false);
    parent.fill(None);

    queue.push_back(start);
    visited[(start.y as u32 * width + start.x as u32) as usize] = true;

    let mut target = None;
    while let Some(current) = queue.pop_front() {
        let ci = (current.y as u32 * width + current.x as u32) as usize;
        if reachable[ci] && current != start {
            target = Some(current);
            break;
        }

        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;

            if nx <= 0 || ny <= 0 || nx >= width as i32 - 1 || ny >= height as i32 - 1 {
                continue;
            }
            let ni = (ny as u32 * width + nx as u32) as usize;
            if visited[ni] {
                continue;
            }

            visited[ni] = true;
            parent[ni] = Some(current);
            queue.push_back(GridPoint { x: nx, y: ny });
        }
    }

    // 回溯路径，打通墙体
    if let Some(end) = target {
        let mut current = end;
        while current != start {
            if let Some(tile) = tiles.get(current.x, current.y).copied() {
                if tile == TileKind::Wall {
                    tiles.set(current.x, current.y, TileKind::Floor);
                }
            }
            let ci = (current.y as u32 * width + current.x as u32) as usize;
            if let Some(prev) = parent[ci] {
                current = prev;
            } else {
                break;
            }
        }
    }
}

/// 将内部墙体转换为障碍物
///
/// 外墙保持为 Wall，内部的墙体（由 CA 生成）转换为 Obstacle
fn convert_internal_walls_to_obstacles(tiles: &mut Grid2D<TileKind>, width: u32, height: u32) {
    for y in 1..height as i32 - 1 {
        for x in 1..width as i32 - 1 {
            if tiles.get(x, y).copied() == Some(TileKind::Wall) {
                tiles.set(x, y, TileKind::Obstacle);
            }
        }
    }
}
