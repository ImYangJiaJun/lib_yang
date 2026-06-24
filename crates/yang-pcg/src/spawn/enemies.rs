// 敌人点位生成

use crate::config::{NormalizedConfig, RangeU16};
use crate::debug::RejectionReason;
use crate::model::geometry::{GridPoint, Transform3, WorldPoint};
use crate::model::room::{Room, RoomType};
use crate::model::spawn::{SpawnKind, SpawnMetadata, SpawnPoint};
use crate::model::terrain::{Terrain, TileKind};
use crate::rng::StableRng;
use crate::topology::graph::sample_range_u16;

use super::budget::enemy_budget;
use super::sampling::{select_spaced_points_excluding, select_spaced_points_tracked_excluding};

/// 敌人点位生成的跟踪结果。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EnemySpawnTracked {
    /// 生成的点位列表
    pub spawns: Vec<SpawnPoint>,
    /// 候选点位总数
    pub candidate_count: usize,
    /// 被拒绝的点位及原因
    pub rejections: Vec<RejectionReason>,
}

/// 生成单个房间的敌人点位。
pub fn generate_enemy_spawns_for_room(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> Vec<SpawnPoint> {
    generate_enemy_spawns_for_room_excluding(room, terrain, config, &[], 0, rng)
}

/// 生成单个房间的敌人点位，并与「已占用点」（如先放置的交互物，局部坐标）保持跨类型间距。
///
/// `occupied` 为局部坐标的已占用点集合，`occupied_spacing` 为跨类型间距阈值。
/// 与 `generate_enemy_spawns_for_room` 行为一致（传空 occupied 时字节等价），
/// RNG 消耗不受 occupied 影响（仅取决于候选洗牌）。
pub fn generate_enemy_spawns_for_room_excluding(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    occupied: &[GridPoint],
    occupied_spacing: u16,
    rng: &mut StableRng,
) -> Vec<SpawnPoint> {
    match room.room_type {
        RoomType::Start
        | RoomType::Treasure
        | RoomType::Shop
        | RoomType::Safe
        | RoomType::Secret => {
            return Vec::new();
        }
        RoomType::Boss => return vec![boss_spawn(room, terrain, config, rng)],
        _ => {}
    }

    let desired_count = usize::from(sample_range_u16(
        rng,
        adjusted_enemy_count_range(room.room_type, config.config.enemy_spawns.count_per_room),
    ));
    let candidates = candidate_points(
        terrain,
        config.config.enemy_spawns.min_distance_from_entrance,
    );
    let points = select_spaced_points_excluding(
        &candidates,
        desired_count,
        config.config.enemy_spawns.min_spacing,
        occupied,
        occupied_spacing,
        rng,
    );
    let budget = enemy_budget(room, config);

    build_enemy_spawn_points(room, &points, budget, rng)
}

/// 生成单个房间的敌人点位，同时记录拒绝信息。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
pub fn generate_enemy_spawns_for_room_tracked(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> EnemySpawnTracked {
    generate_enemy_spawns_for_room_tracked_excluding(room, terrain, config, &[], 0, rng)
}

/// `generate_enemy_spawns_for_room_tracked` 的跨类型间距版本。
///
/// 与「已占用点」（局部坐标）保持 `occupied_spacing` 间距，其余行为一致。
pub fn generate_enemy_spawns_for_room_tracked_excluding(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    occupied: &[GridPoint],
    occupied_spacing: u16,
    rng: &mut StableRng,
) -> EnemySpawnTracked {
    match room.room_type {
        RoomType::Start
        | RoomType::Treasure
        | RoomType::Shop
        | RoomType::Safe
        | RoomType::Secret => {
            return EnemySpawnTracked {
                spawns: Vec::new(),
                candidate_count: 0,
                rejections: Vec::new(),
            };
        }
        RoomType::Boss => {
            return EnemySpawnTracked {
                spawns: vec![boss_spawn(room, terrain, config, rng)],
                candidate_count: 1,
                rejections: Vec::new(),
            };
        }
        _ => {}
    }

    let desired_count = usize::from(sample_range_u16(
        rng,
        adjusted_enemy_count_range(room.room_type, config.config.enemy_spawns.count_per_room),
    ));
    let candidates = candidate_points(
        terrain,
        config.config.enemy_spawns.min_distance_from_entrance,
    );
    let candidate_count = candidates.len();
    let result = select_spaced_points_tracked_excluding(
        &candidates,
        desired_count,
        config.config.enemy_spawns.min_spacing,
        occupied,
        occupied_spacing,
        rng,
    );
    let budget = enemy_budget(room, config);

    let spawns = build_enemy_spawn_points(room, &result.selected, budget, rng);

    EnemySpawnTracked {
        spawns,
        candidate_count,
        rejections: result.rejections,
    }
}

fn build_enemy_spawn_points(
    room: &Room,
    points: &[GridPoint],
    budget: u16,
    rng: &mut StableRng,
) -> Vec<SpawnPoint> {
    points
        .iter()
        .enumerate()
        .map(|(index, &local_point)| {
            let world_point = world_grid_point(room, local_point);
            SpawnPoint {
                id: format!("enemy-{}-{index:03}", room.id),
                room_id: room.id.clone(),
                kind: SpawnKind::Enemy,
                grid_pos: world_point,
                world_transform: Some(grid_point_to_transform(world_point)),
                metadata: SpawnMetadata {
                    spawn_tag: "enemy_spawn".to_string(),
                    rarity_tier: None,
                    enemy_pool_tag: Some(enemy_pool_tag(room.room_type).to_string()),
                    encounter_id: Some(format!("encounter-{}", room.id)),
                    wave_id: Some(format!("wave-{index:02}")),
                    difficulty: Some(budget.saturating_add(index as u16)),
                    seed: rng.seed(),
                },
            }
        })
        .collect()
}

fn boss_spawn(
    room: &Room,
    terrain: &Terrain,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> SpawnPoint {
    let local_point = GridPoint {
        x: (terrain.grid_size.width / 2) as i32,
        y: (terrain.grid_size.height / 2) as i32,
    };
    let world_point = world_grid_point(room, local_point);
    SpawnPoint {
        id: format!("boss-{}", room.id),
        room_id: room.id.clone(),
        kind: SpawnKind::Boss,
        grid_pos: world_point,
        world_transform: Some(grid_point_to_transform(world_point)),
        metadata: SpawnMetadata {
            spawn_tag: "boss_spawn".to_string(),
            rarity_tier: None,
            enemy_pool_tag: Some("boss".to_string()),
            encounter_id: Some(format!("boss-encounter-{}", room.id)),
            wave_id: Some("boss".to_string()),
            difficulty: Some(enemy_budget(room, config)),
            seed: rng.seed(),
        },
    }
}

fn adjusted_enemy_count_range(room_type: RoomType, base: RangeU16) -> RangeU16 {
    match room_type {
        RoomType::Elite => RangeU16::new(base.min.saturating_add(1), base.max.saturating_add(2)),
        RoomType::Event | RoomType::Puzzle => RangeU16::new(0, base.max.min(2)),
        _ => base,
    }
}

fn candidate_points(terrain: &Terrain, min_distance_from_entrance: u16) -> Vec<GridPoint> {
    let mut points = Vec::with_capacity((terrain.grid_size.width * terrain.grid_size.height) as usize);
    let min_safe_distance = i32::from(min_distance_from_entrance);
    let doorway_positions: Vec<GridPoint> = (0..terrain.grid_size.height as i32)
        .flat_map(|y| {
            (0..terrain.grid_size.width as i32).filter_map(move |x| {
                (terrain.tiles.get(x, y) == Some(&TileKind::Doorway)).then_some(GridPoint { x, y })
            })
        })
        .collect();

    for y in 1..terrain.grid_size.height.saturating_sub(1) as i32 {
        for x in 1..terrain.grid_size.width.saturating_sub(1) as i32 {
            let tile = terrain.tiles.get(x, y).copied();
            if !matches!(tile, Some(TileKind::Floor | TileKind::Reserved)) {
                continue;
            }
            let point = GridPoint { x, y };
            if doorway_positions.iter().all(|doorway| {
                (doorway.x - point.x).abs() + (doorway.y - point.y).abs() >= min_safe_distance
            }) {
                points.push(point);
            }
        }
    }
    points
}

fn enemy_pool_tag(room_type: RoomType) -> &'static str {
    match room_type {
        RoomType::Elite => "elite",
        RoomType::Puzzle => "puzzle_guard",
        RoomType::Event => "event_guard",
        _ => "combat",
    }
}

fn world_grid_point(room: &Room, local_point: GridPoint) -> GridPoint {
    // 调用方已保证 bounds 存在；用 unwrap_or 兜底为局部坐标，
    // 避免在内部不变量被违反时 panic（与 items.rs::world_grid_point 一致）。
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
