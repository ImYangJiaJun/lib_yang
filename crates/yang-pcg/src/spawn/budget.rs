// 预算管理

use crate::config::NormalizedConfig;
use crate::model::room::{Room, RoomType};

/// 计算房间的敌人预算。
pub fn enemy_budget(room: &Room, config: &NormalizedConfig) -> u16 {
    let base = config.config.enemy_spawns.base_difficulty_budget;
    match room.room_type {
        RoomType::Boss => base.saturating_mul(3),
        RoomType::Elite => base.saturating_mul(2),
        RoomType::Combat => base + room.difficulty,
        RoomType::Event | RoomType::Puzzle => base / 2,
        RoomType::Start
        | RoomType::Treasure
        | RoomType::Shop
        | RoomType::Safe
        | RoomType::Secret => 0,
    }
}
