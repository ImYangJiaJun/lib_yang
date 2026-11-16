// 地形雕刻算法

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::{GridPoint, GridSize, RoomBounds};
use crate::model::room::{DoorAnchor, Room, RoomType};
use crate::model::terrain::{
    ConnectivitySummary, Grid2D, ReservedZone, ReservedZoneBounds, Terrain, TileKind,
};
use crate::rng::StableRng;

use super::grid::to_local;

/// 过滤属于指定房间的门锚点，转换为局部坐标，标记为 `Doorway`。
///
/// 返回已成功标记的局部坐标列表，供后续连通性修复使用。
///
/// 只有 `tiles.set` 成功（即锚点落在房间网格内）的坐标才会进入返回值：
/// 与房间边界不自洽的越界锚点会被过滤，避免下游策略（Maze/OpenArena/Organic）
/// 用越界局部坐标无边界检查地索引平坦位图而 panic。界内锚点（含边界环上的
/// 局部坐标 `0` / `width-1` / `height-1`）的结果、顺序与重复均保持不变。
pub(crate) fn mark_doorways(
    tiles: &mut Grid2D<TileKind>,
    anchors: &[DoorAnchor],
    room_id: &str,
    origin: GridPoint,
) -> Vec<GridPoint> {
    let mut doorway_locals = Vec::new();
    for anchor in anchors.iter().filter(|a| a.room_id == room_id) {
        let pos = to_local(origin, anchor.grid_pos);
        // `set` 返回 false 等价于局部坐标不在 [0, width) x [0, height) 内，
        // 即 world 坐标不在房间 RoomBounds 的 [min, max) 内。
        if tiles.set(pos.x, pos.y, TileKind::Doorway) {
            doorway_locals.push(pos);
        }
    }
    doorway_locals
}

/// 从房间中提取边界信息并校验尺寸。
///
/// 返回 `(bounds, width, height)`，房间无边界或尺寸为零时返回错误。
pub(crate) fn extract_room_bounds(room: &Room) -> PcgResult<(RoomBounds, u32, u32)> {
    use crate::error::PcgError;

    let bounds = room
        .bounds
        .ok_or_else(|| PcgError::terrain(format!("房间 {} 没有边界信息", room.id)))?;
    let width = bounds.width();
    let height = bounds.height();
    if width == 0 || height == 0 {
        return Err(PcgError::terrain(format!(
            "房间 {} 边界尺寸为零: {}x{}",
            room.id, width, height
        )));
    }
    Ok((bounds, width, height))
}

/// 初始化房间网格：外墙为 Wall，内部为 Floor。
///
/// 一次性完成全 Wall 填充 + 内部 Floor 覆写（与 OPT-P-13 单遍初始化合并），
/// 避免各策略重复内联边框绘制循环。
pub(crate) fn init_room_grid(width: u32, height: u32) -> PcgResult<Grid2D<TileKind>> {
    let mut tiles = Grid2D::new(width, height, TileKind::Wall)?;
    for y in 1..height as i32 - 1 {
        for x in 1..width as i32 - 1 {
            tiles.set(x, y, TileKind::Floor);
        }
    }
    Ok(tiles)
}

/// 使用 TerrainConfig 为单个房间生成地形。
///
/// 接受 `TerrainConfig`，便于作为 `TerrainStrategy` trait 的底层实现使用。
///
/// # 参数
///
/// * `room` - 目标房间
/// * `door_anchors` - 属于该房间的门锚点
/// * `config` - 地形配置
/// * `rng` - 确定性随机数生成器
///
/// # 错误
///
/// 当房间没有边界或边界尺寸为零时返回错误。
pub fn carve_room_terrain_with_config(
    room: &Room,
    door_anchors: &[DoorAnchor],
    config: &TerrainConfig,
    rng: &mut StableRng,
) -> PcgResult<Terrain> {
    let (bounds, width, height) = extract_room_bounds(room)?;
    let mut tiles = init_room_grid(width, height)?;

    mark_doorways(&mut tiles, door_anchors, &room.id, bounds.min);

    let reserved_zones = build_reserved_zones(room, width, height);
    mark_reserved_zones(&mut tiles, &reserved_zones);
    place_obstacles_with_config(&mut tiles, room, config, rng);

    Ok(Terrain {
        room_id: room.id.clone(),
        grid_size: GridSize { width, height },
        tiles,
        reserved_zones,
        // 该字段在下游 repair_terrain_connectivity() 中会被覆写，此处只需占位
        connectivity_summary: ConnectivitySummary {
            all_doors_connected: false,
            walkable_tile_count: 0,
            total_tile_count: 0,
            connected_region_count: 0,
        },
    })
}

fn build_reserved_zones(room: &Room, width: u32, height: u32) -> Vec<ReservedZone> {
    let mut zones = Vec::new();
    if matches!(room.room_type, RoomType::Boss) {
        let center = GridPoint {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
        };
        zones.push(ReservedZone {
            id: format!("{}-boss-center", room.id),
            zone_type: "boss_center".to_string(),
            bounds: ReservedZoneBounds::Circle {
                center,
                radius: (width.min(height) / 4).max(2),
            },
            allow_items: false,
            allow_enemies: true,
        });
    }
    zones
}

fn mark_reserved_zones(grid: &mut Grid2D<TileKind>, zones: &[ReservedZone]) {
    for zone in zones {
        match &zone.bounds {
            ReservedZoneBounds::Rect { min, max } => {
                for y in min.y..max.y {
                    for x in min.x..max.x {
                        grid.set(x, y, TileKind::Reserved);
                    }
                }
            }
            ReservedZoneBounds::Circle { center, radius } => {
                let radius_sq = (*radius as i32).pow(2);
                for y in 0..grid.height as i32 {
                    for x in 0..grid.width as i32 {
                        let dx = x - center.x;
                        let dy = y - center.y;
                        if dx * dx + dy * dy <= radius_sq {
                            grid.set(x, y, TileKind::Reserved);
                        }
                    }
                }
            }
            ReservedZoneBounds::Polygon { .. } => {}
        }
    }
}

/// 使用 TerrainConfig 放置障碍物
fn place_obstacles_with_config(
    grid: &mut Grid2D<TileKind>,
    room: &Room,
    config: &TerrainConfig,
    rng: &mut StableRng,
) {
    if matches!(
        room.room_type,
        RoomType::Boss | RoomType::Start | RoomType::Shop | RoomType::Safe
    ) {
        return;
    }

    let candidate_budget =
        ((grid.width * grid.height) as f32 * config.obstacle_density).round() as usize;
    let target_obstacle_count = candidate_budget.min(4);
    let max_attempts = target_obstacle_count.saturating_mul(10).max(40);
    let mut placed = 0usize;
    let mut attempts = 0usize;
    let max_x = grid.width as i32 - 2;
    let max_y = grid.height as i32 - 2;

    while placed < target_obstacle_count && attempts < max_attempts && max_x > 1 && max_y > 1 {
        let x = rng.random_range(1, max_x);
        let y = rng.random_range(1, max_y);
        attempts += 1;
        if grid.get(x, y).copied() == Some(TileKind::Floor) {
            grid.set(x, y, TileKind::Obstacle);
            placed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::geometry::CardinalDir;
    use crate::terrain::strategy::TerrainStrategy;
    use crate::terrain::{MazeStrategy, OpenArenaStrategy, OrganicStrategy};

    fn room_with_bounds() -> Room {
        Room {
            id: "room-1".to_string(),
            room_type: RoomType::Combat,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 1,
            theme_tags: vec![],
            bounds: Some(RoomBounds {
                min: GridPoint { x: 0, y: 0 },
                max: GridPoint { x: 8, y: 8 },
            }),
            template_ref: None,
            grammar_token: None,
        }
    }

    fn anchor(room_id: &str, x: i32, y: i32) -> DoorAnchor {
        DoorAnchor {
            id: format!("anchor-{x}-{y}"),
            room_id: room_id.to_string(),
            edge_id: "edge-1".to_string(),
            grid_pos: GridPoint { x, y },
            facing: CardinalDir::South,
            width_tiles: 1,
            socket_tag: None,
        }
    }

    /// 越界/负坐标锚点必须被过滤；界内锚点（含边界环）的顺序与重复保持不变。
    #[test]
    fn mark_doorways_filters_out_of_bounds_anchors_and_keeps_valid_order() {
        // RED 场景：修复前越界锚点会原样进入 doorway_locals，下游策略用它
        // （负坐标 `as u32` 回绕或直接越界）索引平坦位图时 panic。
        let mut tiles = init_room_grid(8, 8).expect("8x8 网格应创建成功");
        let anchors = vec![
            anchor("room-1", 0, 4),   // 合法：局部 (0,4)，边界环
            anchor("room-1", 100, 4), // 非法：局部 x 越界
            anchor("room-1", 4, 4),   // 合法：内部点位
            anchor("room-1", 4, 4),   // 合法重复点位，顺序与重复都应保留
            anchor("room-1", -1, 4),  // 非法：局部 x 为负
            anchor("room-2", 5, 5),   // 其他房间，忽略
        ];

        let locals = mark_doorways(&mut tiles, &anchors, "room-1", GridPoint { x: 0, y: 0 });

        assert_eq!(
            locals,
            vec![
                GridPoint { x: 0, y: 4 },
                GridPoint { x: 4, y: 4 },
                GridPoint { x: 4, y: 4 },
            ],
            "仅保留界内锚点，且顺序与重复保持不变"
        );
        assert_eq!(tiles.get(0, 4).copied(), Some(TileKind::Doorway));
        assert_eq!(tiles.get(4, 4).copied(), Some(TileKind::Doorway));

        // 边界环上的 width-1 仍是合法锚点，不得被判为越界。
        let ring = vec![anchor("room-1", 7, 3)];
        let ring_locals = mark_doorways(&mut tiles, &ring, "room-1", GridPoint { x: 0, y: 0 });
        assert_eq!(ring_locals, vec![GridPoint { x: 7, y: 3 }]);
        assert_eq!(tiles.get(7, 3).copied(), Some(TileKind::Doorway));
    }

    /// 三套会索引平坦位图的策略在收到越界锚点时不得 panic。
    ///
    /// RED 场景：修复前 Maze/OpenArena/Organic 用 `doorway.y as u32 * width + doorway.x as u32`
    /// 索引 `visited`，负坐标会回绕成巨大索引、正越界坐标会直接越界，均触发 panic。
    #[test]
    fn strategies_ignore_out_of_bounds_door_anchors_without_panicking() {
        let room = room_with_bounds();
        let anchors = vec![
            anchor("room-1", 0, 4),
            anchor("room-1", 100, 4),
            anchor("room-1", -1, 4),
        ];
        let config = TerrainConfig::default();

        let mut maze_rng = StableRng::from_seed(42);
        let maze = MazeStrategy
            .generate(&room, &anchors, &config, &mut maze_rng)
            .expect("越界锚点不应让 Maze 失败");
        assert_eq!(maze.tiles.get(0, 4).copied(), Some(TileKind::Doorway));

        let mut arena_rng = StableRng::from_seed(42);
        let arena = OpenArenaStrategy
            .generate(&room, &anchors, &config, &mut arena_rng)
            .expect("越界锚点不应让 OpenArena 失败");
        assert_eq!(arena.tiles.get(0, 4).copied(), Some(TileKind::Doorway));

        let mut organic_rng = StableRng::from_seed(42);
        let organic = OrganicStrategy
            .generate(&room, &anchors, &config, &mut organic_rng)
            .expect("越界锚点不应让 Organic 失败");
        assert_eq!(organic.tiles.get(0, 4).copied(), Some(TileKind::Doorway));
    }
}
