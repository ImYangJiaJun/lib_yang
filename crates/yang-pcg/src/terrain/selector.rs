// 地形策略选择器
// 根据房间类型、主题标签和模板引用选择合适的地形生成策略

use crate::model::room::{Room, RoomType};

use super::default_strategy::DefaultCarveStrategy;
use super::maze::MazeStrategy;
use super::open_arena::OpenArenaStrategy;
use super::organic::OrganicStrategy;
use super::pillar::PillarStrategy;
use super::strategy::TerrainStrategy;

/// 地形策略选择器
///
/// 根据房间属性（类型、主题标签、模板引用）选择最合适的地形生成策略。
///
/// # 选择规则（优先级从高到低）
///
/// 1. Boss 房间 → `OpenArenaStrategy`（开放式战斗区）
/// 2. Combat/Elite 且带有 "pillar" 主题标签 → `PillarStrategy`（柱状掩体）
/// 3. Puzzle 且带有 "maze" 主题标签 → `MazeStrategy`（迷宫式通道）
/// 4. 任何房间带有 "organic" 或 "cave" 主题标签 → `OrganicStrategy`（自然洞穴）
/// 5. 其他情况 → `DefaultCarveStrategy`（默认雕刻）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::terrain::selector::select_strategy;
/// use yang_pcg::model::room::{Room, RoomType};
///
/// let room = Room { room_type: RoomType::Boss, .. };
/// let strategy = select_strategy(&room);
/// assert_eq!(strategy.name(), "open_arena");
/// ```
pub fn select_strategy(room: &Room) -> Box<dyn TerrainStrategy> {
    // 规则 1：Boss 房间使用开放式策略
    if room.room_type == RoomType::Boss {
        return Box::new(OpenArenaStrategy);
    }

    // 规则 2：Combat/Elite 且带有 "pillar" 标签使用柱状策略
    if matches!(room.room_type, RoomType::Combat | RoomType::Elite)
        && has_theme_tag(room, "pillar")
    {
        return Box::new(PillarStrategy);
    }

    // 规则 3：Puzzle 且带有 "maze" 标签使用迷宫策略
    if room.room_type == RoomType::Puzzle && has_theme_tag(room, "maze") {
        return Box::new(MazeStrategy);
    }

    // 规则 4：任何房间带有 "organic" 或 "cave" 标签使用有机策略
    if has_theme_tag(room, "organic") || has_theme_tag(room, "cave") {
        return Box::new(OrganicStrategy);
    }

    // 规则 5：默认策略
    Box::new(DefaultCarveStrategy)
}

/// 检查房间是否包含指定的主题标签
fn has_theme_tag(room: &Room, tag: &str) -> bool {
    room.theme_tags.iter().any(|t| t == tag)
}

#[cfg(test)]
mod __tests__ {
    use super::*;
    use crate::model::room::Room;

    /// 创建测试用房间
    fn make_test_room(room_type: RoomType, tags: Vec<&str>) -> Room {
        Room {
            id: "test-room".to_string(),
            room_type,
            depth_from_start: 1,
            branch_id: None,
            difficulty: 1,
            theme_tags: tags.into_iter().map(|s| s.to_string()).collect(),
            bounds: None,
            template_ref: None,
            grammar_token: None,
        }
    }

    #[test]
    fn test_boss_room_selects_open_arena() {
        let room = make_test_room(RoomType::Boss, vec![]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "open_arena");
    }

    #[test]
    fn test_combat_with_pillar_tag_selects_pillar() {
        let room = make_test_room(RoomType::Combat, vec!["pillar"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "pillar");
    }

    #[test]
    fn test_elite_with_pillar_tag_selects_pillar() {
        let room = make_test_room(RoomType::Elite, vec!["pillar"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "pillar");
    }

    #[test]
    fn test_puzzle_with_maze_tag_selects_maze() {
        let room = make_test_room(RoomType::Puzzle, vec!["maze"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "maze");
    }

    #[test]
    fn test_room_with_organic_tag_selects_organic() {
        let room = make_test_room(RoomType::Combat, vec!["organic"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "organic");
    }

    #[test]
    fn test_room_with_cave_tag_selects_organic() {
        let room = make_test_room(RoomType::Treasure, vec!["cave"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "organic");
    }

    #[test]
    fn test_default_strategy_for_plain_combat() {
        let room = make_test_room(RoomType::Combat, vec![]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "default_carve");
    }

    #[test]
    fn test_default_strategy_for_shop() {
        let room = make_test_room(RoomType::Shop, vec![]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "default_carve");
    }

    #[test]
    fn test_boss_overrides_organic_tag() {
        // Boss 优先级高于 organic 标签
        let room = make_test_room(RoomType::Boss, vec!["organic"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "open_arena");
    }

    #[test]
    fn test_pillar_overrides_organic_tag() {
        // Combat + pillar 优先级高于 organic 标签
        let room = make_test_room(RoomType::Combat, vec!["pillar", "organic"]);
        let strategy = select_strategy(&room);
        assert_eq!(strategy.name(), "pillar");
    }
}
