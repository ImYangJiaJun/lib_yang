// 交互物点位生成

use crate::config::{NormalizedConfig, RangeU16};
use crate::debug::RejectionReason;
use crate::model::geometry::{GridPoint, Transform3, WorldPoint};
use crate::model::room::{Room, RoomType};
use crate::model::spawn::{SpawnKind, SpawnMetadata, SpawnPoint};
use crate::model::terrain::{Terrain, TileKind};
use crate::rng::StableRng;
use crate::topology::graph::sample_range_u16;

use super::sampling::{select_spaced_points, select_spaced_points_tracked};

/// 交互物点位生成的跟踪结果。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ItemSpawnTracked {
    /// 生成的点位列表
    pub spawns: Vec<SpawnPoint>,
    /// 候选点位总数
    pub candidate_count: usize,
    /// 被拒绝的点位及原因
    pub rejections: Vec<RejectionReason>,
}

/// 生成单个房间的交互物点位。
pub fn generate_item_spawns_for_room(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> Vec<SpawnPoint> {
    if matches!(room.room_type, RoomType::Boss) {
        return Vec::new();
    }
    // 房间无边界时无法定位点位，返回空（点位生成阶段不会发生，与 occupied_local_points 约定一致）
    if room.bounds.is_none() {
        return Vec::new();
    }

    let count_range =
        adjusted_item_count_range(room.room_type, config.config.item_spawns.count_per_room);
    let desired_count = usize::from(sample_range_u16(rng, count_range));
    let candidates = candidate_points(terrain);
    let points = select_spaced_points(
        &candidates,
        desired_count,
        config.config.item_spawns.min_spacing,
        rng,
    );

    build_spawn_points(room, &points, config, rng)
}

/// 生成单个房间的交互物点位，同时记录拒绝信息。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
pub fn generate_item_spawns_for_room_tracked(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> ItemSpawnTracked {
    if matches!(room.room_type, RoomType::Boss) {
        return ItemSpawnTracked {
            spawns: Vec::new(),
            candidate_count: 0,
            rejections: Vec::new(),
        };
    }
    // 房间无边界时无法定位点位，返回空（与 occupied_local_points 约定一致）
    if room.bounds.is_none() {
        return ItemSpawnTracked {
            spawns: Vec::new(),
            candidate_count: 0,
            rejections: Vec::new(),
        };
    }

    let count_range =
        adjusted_item_count_range(room.room_type, config.config.item_spawns.count_per_room);
    let desired_count = usize::from(sample_range_u16(rng, count_range));
    let candidates = candidate_points(terrain);
    let candidate_count = candidates.len();
    let result = select_spaced_points_tracked(
        &candidates,
        desired_count,
        config.config.item_spawns.min_spacing,
        rng,
    );

    let spawns = build_spawn_points(room, &result.selected, config, rng);

    ItemSpawnTracked {
        spawns,
        candidate_count,
        rejections: result.rejections,
    }
}

fn build_spawn_points(
    room: &Room,
    points: &[GridPoint],
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> Vec<SpawnPoint> {
    points
        .iter()
        .enumerate()
        .map(|(index, &local_point)| SpawnPoint {
            id: format!("item-{}-{index:03}", room.id),
            room_id: room.id.clone(),
            kind: SpawnKind::Item,
            grid_pos: world_grid_point(room, local_point),
            world_transform: Some(grid_point_to_transform(world_grid_point(room, local_point))),
            metadata: SpawnMetadata {
                spawn_tag: item_spawn_tag(room.room_type).to_string(),
                rarity_tier: Some(sample_rarity_tier(
                    &config.config.item_spawns.rarity_weights,
                    rng,
                )),
                enemy_pool_tag: None,
                encounter_id: None,
                wave_id: None,
                difficulty: Some(room.difficulty),
                seed: rng.seed(),
            },
        })
        .collect()
}

fn adjusted_item_count_range(room_type: RoomType, base: RangeU16) -> RangeU16 {
    match room_type {
        RoomType::Treasure => RangeU16::new(base.min.saturating_add(1), base.max.saturating_add(1)),
        RoomType::Shop => RangeU16::new(base.min.max(1), base.max.saturating_add(1)),
        RoomType::Start | RoomType::Safe => RangeU16::new(0, base.max.min(1)),
        _ => base,
    }
}

fn candidate_points(terrain: &Terrain) -> Vec<GridPoint> {
    let mut points =
        Vec::with_capacity((terrain.grid_size.width * terrain.grid_size.height) as usize);
    for y in 1..terrain.grid_size.height.saturating_sub(1) as i32 {
        for x in 1..terrain.grid_size.width.saturating_sub(1) as i32 {
            if matches!(
                terrain.tiles.get(x, y),
                Some(TileKind::Floor | TileKind::Reserved)
            ) {
                points.push(GridPoint { x, y });
            }
        }
    }
    points
}

fn sample_rarity_tier(weights: &[f32], rng: &mut StableRng) -> u8 {
    // len==3 已由 OPT-L-04 config 校验保证
    let w = [f64::from(weights[0]), f64::from(weights[1]), f64::from(weights[2])];
    let tiers = [0u8, 1, 2];
    rng.choose_weighted(&tiers, &w)
        .copied()
        .unwrap_or(0)
}

fn item_spawn_tag(room_type: RoomType) -> &'static str {
    match room_type {
        RoomType::Treasure => "treasure_item",
        RoomType::Shop => "shop_item",
        RoomType::Start => "starter_item",
        _ => "room_item",
    }
}

fn world_grid_point(room: &Room, local_point: GridPoint) -> GridPoint {
    // 调用方（build_spawn_points 的入口函数）已保证 bounds 存在；
    // 这里用 unwrap_or 兜底为局部坐标，避免在内部不变量被违反时 panic。
    let origin = room
        .bounds
        .map(|b| b.min)
        .unwrap_or(GridPoint { x: 0, y: 0 });
    GridPoint {
        x: origin.x + local_point.x,
        y: origin.y + local_point.y,
    }
}

fn grid_point_to_transform(point: GridPoint) -> Transform3 {
    Transform3 {
        position: WorldPoint {
            x: point.x as f32,
            y: point.y as f32,
            z: 0.0,
        },
        ..Transform3::default()
    }
}
