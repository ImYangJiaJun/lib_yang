// 点位数据模型测试
// 验证需求: 7.1, 7.2, 7.7, 8.1, 8.2, 8.7

use crate::model::geometry::{GridPoint, Transform3, WorldPoint};
use crate::model::spawn::*;

#[test]
fn test_spawn_kind_variants() {
    // 测试点位类型枚举
    let kinds = [SpawnKind::Item,
        SpawnKind::Enemy,
        SpawnKind::Boss,
        SpawnKind::Reward,
        SpawnKind::Interaction];

    assert_eq!(kinds.len(), 5);
    assert_eq!(kinds[0], SpawnKind::Item);
    assert_eq!(kinds[2], SpawnKind::Boss);
}

#[test]
fn test_spawn_metadata_creation() {
    // 测试点位元数据创建
    let metadata = SpawnMetadata {
        spawn_tag: "chest_gold".to_string(),
        rarity_tier: Some(3),
        enemy_pool_tag: None,
        encounter_id: None,
        wave_id: None,
        difficulty: None,
        seed: 12345,
    };

    assert_eq!(metadata.spawn_tag, "chest_gold");
    assert_eq!(metadata.rarity_tier, Some(3));
    assert_eq!(metadata.seed, 12345);
}

#[test]
fn test_item_spawn_point() {
    // 测试交互物点位创建
    let spawn = SpawnPoint {
        id: "spawn-item-001".to_string(),
        room_id: "room-001".to_string(),
        kind: SpawnKind::Item,
        grid_pos: GridPoint { x: 10, y: 15 },
        world_transform: Some(Transform3 {
            position: WorldPoint {
                x: 100.0,
                y: 150.0,
                z: 0.0,
            },
            rotation: (0.0, 0.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        }),
        metadata: SpawnMetadata {
            spawn_tag: "treasure_chest".to_string(),
            rarity_tier: Some(2),
            enemy_pool_tag: None,
            encounter_id: None,
            wave_id: None,
            difficulty: None,
            seed: 54321,
        },
    };

    assert_eq!(spawn.kind, SpawnKind::Item);
    assert_eq!(spawn.metadata.spawn_tag, "treasure_chest");
    assert!(spawn.world_transform.is_some());
}

#[test]
fn test_enemy_spawn_point() {
    // 测试敌人点位创建
    let spawn = SpawnPoint {
        id: "spawn-enemy-001".to_string(),
        room_id: "room-002".to_string(),
        kind: SpawnKind::Enemy,
        grid_pos: GridPoint { x: 5, y: 8 },
        world_transform: None,
        metadata: SpawnMetadata {
            spawn_tag: "goblin".to_string(),
            rarity_tier: None,
            enemy_pool_tag: Some("common_enemies".to_string()),
            encounter_id: Some("encounter-001".to_string()),
            wave_id: Some("wave-1".to_string()),
            difficulty: Some(3),
            seed: 99999,
        },
    };

    assert_eq!(spawn.kind, SpawnKind::Enemy);
    assert_eq!(
        spawn.metadata.enemy_pool_tag,
        Some("common_enemies".to_string())
    );
    assert_eq!(
        spawn.metadata.encounter_id,
        Some("encounter-001".to_string())
    );
    assert_eq!(spawn.metadata.difficulty, Some(3));
}

#[test]
fn test_boss_spawn_point() {
    // 测试 Boss 点位创建
    let spawn = SpawnPoint {
        id: "spawn-boss-001".to_string(),
        room_id: "room-boss".to_string(),
        kind: SpawnKind::Boss,
        grid_pos: GridPoint { x: 20, y: 20 },
        world_transform: Some(Transform3::default()),
        metadata: SpawnMetadata {
            spawn_tag: "dragon_boss".to_string(),
            rarity_tier: Some(5),
            enemy_pool_tag: Some("boss_pool".to_string()),
            encounter_id: Some("boss-encounter".to_string()),
            wave_id: None,
            difficulty: Some(10),
            seed: 77777,
        },
    };

    assert_eq!(spawn.kind, SpawnKind::Boss);
    assert_eq!(spawn.metadata.spawn_tag, "dragon_boss");
    assert_eq!(spawn.metadata.difficulty, Some(10));
}

#[test]
fn test_spawn_point_without_world_transform() {
    // 测试不带世界变换的点位
    let spawn = SpawnPoint {
        id: "spawn-003".to_string(),
        room_id: "room-003".to_string(),
        kind: SpawnKind::Interaction,
        grid_pos: GridPoint { x: 12, y: 8 },
        world_transform: None,
        metadata: SpawnMetadata {
            spawn_tag: "lever".to_string(),
            rarity_tier: None,
            enemy_pool_tag: None,
            encounter_id: None,
            wave_id: None,
            difficulty: None,
            seed: 11111,
        },
    };

    assert!(spawn.world_transform.is_none());
    assert_eq!(spawn.kind, SpawnKind::Interaction);
}
