// 有机式地形策略
// 使用 cellular automata 生成自然洞穴形态

use crate::config::TerrainConfig;
use crate::error::{PcgError, PcgResult};
use crate::model::geometry::{GridPoint, GridSize};
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::{Grid2D, Terrain, TileKind};
use crate::rng::StableRng;

use super::connectivity::summarize_connectivity;
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
        let bounds = room.bounds.ok_or_else(|| {
            PcgError::terrain(format!("房间 {} 没有边界信息", room.id))
        })?;
        let width = bounds.width();
        let height = bounds.height();
        if width == 0 || height == 0 {
            return Err(PcgError::terrain(format!(
                "房间 {} 边界尺寸为零: {}x{}",
                room.id, width, height
            )));
        }

        // 标记门口位置
        let room_anchors: Vec<&DoorAnchor> = anchors
            .iter()
            .filter(|a| a.room_id == room.id)
            .collect();
        let doorway_locals: Vec<GridPoint> = room_anchors
            .iter()
            .map(|a| to_local(bounds.min, a.grid_pos))
            .collect();

        // 步骤 1：随机初始化网格
        let mut tiles = initialize_random_grid(width, height, config.obstacle_density, rng);

        // 步骤 2：应用 cellular automata 规则
        for _ in 0..CA_ITERATIONS {
            tiles = apply_ca_step(&tiles, width, height);
        }

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
                if nx > 0 && nx < width as i32 - 1 && ny > 0 && ny < height as i32 - 1
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

/// 随机初始化网格
///
/// 根据障碍物密度随机填充墙体和地板
fn initialize_random_grid(
    width: u32,
    height: u32,
    density: f32,
    rng: &mut StableRng,
) -> Grid2D<TileKind> {
    let mut tiles = Grid2D::new(width, height, TileKind::Floor);

    // 初始填充概率（密度越高，初始墙体越多）
    let fill_probability = (density * 0.8 + 0.2) as f64;

    for y in 1..height as i32 - 1 {
        for x in 1..width as i32 - 1 {
            if rng.gen_bool_with_probability(fill_probability) {
                tiles.set(x, y, TileKind::Wall);
            }
        }
    }

    // 边框始终为墙
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            if x == 0 || y == 0 || x == width as i32 - 1 || y == height as i32 - 1 {
                tiles.set(x, y, TileKind::Wall);
            }
        }
    }

    tiles
}

/// 应用一步 cellular automata 规则
///
/// 使用 B5678/S45678 变体规则（适合生成洞穴）：
/// - 如果一个格子周围有 >= WALL_THRESHOLD 个墙体邻居，则变为墙
/// - 否则变为地板
fn apply_ca_step(tiles: &Grid2D<TileKind>, width: u32, height: u32) -> Grid2D<TileKind> {
    let mut new_tiles = Grid2D::new(width, height, TileKind::Floor);

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            // 边框始终为墙
            if x == 0 || y == 0 || x == width as i32 - 1 || y == height as i32 - 1 {
                new_tiles.set(x, y, TileKind::Wall);
                continue;
            }

            let wall_count = count_wall_neighbors(tiles, x, y);

            if wall_count >= WALL_THRESHOLD {
                new_tiles.set(x, y, TileKind::Wall);
            } else {
                new_tiles.set(x, y, TileKind::Floor);
            }
        }
    }

    new_tiles
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
    use std::collections::{HashSet, VecDeque};

    if doorways.len() < 2 {
        return;
    }

    // 从第一个门口 BFS 找到所有可达区域
    let start = doorways[0];
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
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

    // 对不可达的门口，打通路径
    for doorway in doorways.iter().skip(1) {
        if visited.contains(doorway) {
            continue;
        }

        // 使用 A* 风格的路径打通（优先选择已有地板的方向）
        carve_path_to_reachable(tiles, *doorway, &visited, width, height);

        // 更新可达集合
        let mut new_queue = VecDeque::new();
        new_queue.push_back(*doorway);
        if !visited.contains(doorway) {
            visited.insert(*doorway);
        }

        while let Some(current) = new_queue.pop_front() {
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
                        new_queue.push_back(neighbor);
                    }
                }
            }
        }
    }
}

/// 从不可达门口向可达区域打通路径
fn carve_path_to_reachable(
    tiles: &mut Grid2D<TileKind>,
    start: GridPoint,
    reachable: &std::collections::HashSet<GridPoint>,
    width: u32,
    height: u32,
) {
    use std::collections::{HashMap, HashSet, VecDeque};

    // BFS 搜索最近的可达瓦片
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent: HashMap<GridPoint, GridPoint> = HashMap::new();

    queue.push_back(start);
    visited.insert(start);

    let mut target = None;
    while let Some(current) = queue.pop_front() {
        if reachable.contains(&current) && current != start {
            target = Some(current);
            break;
        }

        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;
            let neighbor = GridPoint { x: nx, y: ny };

            if visited.contains(&neighbor) {
                continue;
            }
            // 不穿越外墙
            if nx <= 0 || ny <= 0 || nx >= width as i32 - 1 || ny >= height as i32 - 1 {
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
        while current != start {
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
