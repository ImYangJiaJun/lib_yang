// 地形雕刻算法

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::geometry::{GridPoint, GridSize, RoomBounds};
use crate::model::room::{DoorAnchor, Room, RoomType};
use crate::model::terrain::{ConnectivitySummary, Grid2D, ReservedZone, ReservedZoneBounds, Terrain, TileKind};
use crate::rng::StableRng;

use super::grid::to_local;

/// 过滤属于指定房间的门锚点，转换为局部坐标，标记为 `Doorway`。
///
/// 返回标记后的局部坐标列表，供后续连通性修复使用。
pub(crate) fn mark_doorways(
    tiles: &mut Grid2D<TileKind>,
    anchors: &[DoorAnchor],
    room_id: &str,
    origin: GridPoint,
) -> Vec<GridPoint> {
    let doorway_locals: Vec<GridPoint> = anchors
        .iter()
        .filter(|a| a.room_id == room_id)
        .map(|a| to_local(origin, a.grid_pos))
        .collect();
    for pos in &doorway_locals {
        tiles.set(pos.x, pos.y, TileKind::Doorway);
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
pub(crate) fn init_room_grid(width: u32, height: u32) -> Grid2D<TileKind> {
    let mut tiles = Grid2D::new(width, height, TileKind::Wall);
    for y in 1..height as i32 - 1 {
        for x in 1..width as i32 - 1 {
            tiles.set(x, y, TileKind::Floor);
        }
    }
    tiles
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
    let mut tiles = init_room_grid(width, height);

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
