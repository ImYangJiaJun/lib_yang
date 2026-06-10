// §4.2 回退失败错误传播测试
// 验证当策略与默认回退策略都失败时，generate_terrains 返回 Err 而非静默丢房间。
// 需求映射：18.3（地形生成失败必须可观测，不得静默丢弃房间）

use crate::config::GenerationConfig;
use crate::model::geometry::{GridPoint, RoomBounds};
use crate::model::room::{Room, RoomType};
use crate::rng::StableRng;
use crate::terrain::generate_terrains;

/// 构造一个带边界但尺寸为零的房间。
///
/// 这种房间会越过 `generate_terrains` 开头的 `bounds.is_none()` 跳过判定，
/// 但底层 `carve_room_terrain_with_config` 因尺寸为零而失败——策略与回退都会失败。
fn zero_size_room(id: &str) -> Room {
    Room {
        id: id.to_string(),
        room_type: RoomType::Combat,
        depth_from_start: 1,
        branch_id: None,
        difficulty: 1,
        theme_tags: vec![],
        bounds: Some(RoomBounds {
            min: GridPoint { x: 5, y: 5 },
            max: GridPoint { x: 5, y: 5 }, // 零宽零高
        }),
        template_ref: None,
        grammar_token: None,
    }
}

#[test]
fn test_generate_terrains_propagates_error_when_fallback_also_fails() {
    // 验证需求: 18.3 - 策略与回退都失败时返回 Err，不静默丢房间
    let normalized = GenerationConfig::default()
        .normalize()
        .expect("默认配置应可归一化");
    let mut rng = StableRng::from_seed(7);

    let rooms = vec![zero_size_room("broken-room")];
    let result = generate_terrains(&rooms, &[], &normalized, &mut rng);

    assert!(
        result.is_err(),
        "策略与回退都失败时应返回 Err 而非静默丢弃房间"
    );
    let err = result.unwrap_err();
    assert_eq!(err.error_code(), "PCG-TERRAIN-001");
}

#[test]
fn test_generate_terrains_bounds_none_room_is_skipped_not_errored() {
    // 验证需求: 18.3 - bounds 为 None 的房间仍按现状静默跳过（不是失败场景）
    let normalized = GenerationConfig::default()
        .normalize()
        .expect("默认配置应可归一化");
    let mut rng = StableRng::from_seed(7);

    let mut room = zero_size_room("no-bounds-room");
    room.bounds = None;

    let result = generate_terrains(&[room], &[], &normalized, &mut rng);
    assert!(result.is_ok(), "bounds 为 None 的房间应被跳过而非报错");
    assert!(result.unwrap().is_empty(), "被跳过的房间不产生地形");
}
