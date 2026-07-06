// 预算管理

use crate::config::NormalizedConfig;
use crate::model::room::{Room, RoomType};

/// 计算房间的敌人预算。
pub(crate) fn enemy_budget(room: &Room, config: &NormalizedConfig) -> u16 {
    let base = config.config.enemy_spawns.base_difficulty_budget;
    match room.room_type {
        RoomType::Boss => base.saturating_mul(3),
        RoomType::Elite => base.saturating_mul(2),
        RoomType::Combat => base.saturating_add(room.difficulty),
        RoomType::Event | RoomType::Puzzle => base / 2,
        RoomType::Start
        | RoomType::Treasure
        | RoomType::Shop
        | RoomType::Safe
        | RoomType::Secret => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EnemySpawnConfig, GenerationConfig};

    fn room(room_type: RoomType, difficulty: u16) -> Room {
        Room {
            id: "room-1".to_string(),
            room_type,
            depth_from_start: 0,
            branch_id: None,
            difficulty,
            theme_tags: Vec::new(),
            bounds: None,
            template_ref: None,
            grammar_token: None,
        }
    }

    fn normalized_with_base_budget(base_difficulty_budget: u16) -> NormalizedConfig {
        GenerationConfig {
            enemy_spawns: EnemySpawnConfig {
                base_difficulty_budget,
                ..EnemySpawnConfig::default()
            },
            ..GenerationConfig::default()
        }
        .normalize()
        .expect("测试配置应可归一化")
    }

    #[test]
    fn test_combat_enemy_budget_saturates_on_overflow() {
        let config = normalized_with_base_budget(u16::MAX);
        let room = room(RoomType::Combat, 1);

        assert_eq!(enemy_budget(&room, &config), u16::MAX);
    }
}
