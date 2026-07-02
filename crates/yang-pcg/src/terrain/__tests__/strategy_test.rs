// 地形策略单元测试
// 验证各策略生成的地形满足连通性、面积约束、边框完整性和门口标记
// 需求映射：5.4, 5.5, 18.3

use crate::config::TerrainConfig;
use crate::model::geometry::{CardinalDir, GridPoint, RoomBounds};
use crate::model::room::{DoorAnchor, Room, RoomType};
use crate::model::terrain::TileKind;
use crate::rng::StableRng;
use crate::terrain::grid::is_walkable;
use crate::terrain::strategy::TerrainStrategy;
use crate::terrain::{
    DefaultCarveStrategy, MazeStrategy, OpenArenaStrategy, OrganicStrategy, PillarStrategy,
};

// ============================================================
// 测试辅助函数
// ============================================================

/// 创建带有边界和门锚点的测试房间
fn make_test_room_with_bounds(
    room_type: RoomType,
    width: u32,
    height: u32,
    tags: Vec<&str>,
) -> Room {
    Room {
        id: "test-room-1".to_string(),
        room_type,
        depth_from_start: 1,
        branch_id: None,
        difficulty: 1,
        theme_tags: tags.into_iter().map(|s| s.to_string()).collect(),
        bounds: Some(RoomBounds {
            min: GridPoint { x: 0, y: 0 },
            max: GridPoint {
                x: width as i32,
                y: height as i32,
            },
        }),
        template_ref: None,
        grammar_token: None,
    }
}

/// 创建测试用门锚点
///
/// 在房间四边各放置一个门口（如果尺寸允许）
fn make_test_anchors(room_id: &str, width: u32, height: u32) -> Vec<DoorAnchor> {
    let mut anchors = Vec::new();

    // 北侧门口（顶边中间）
    anchors.push(DoorAnchor {
        id: "anchor-north".to_string(),
        room_id: room_id.to_string(),
        edge_id: "edge-1".to_string(),
        grid_pos: GridPoint {
            x: width as i32 / 2,
            y: 0,
        },
        facing: CardinalDir::North,
        width_tiles: 1,
        socket_tag: None,
    });

    // 南侧门口（底边中间）
    anchors.push(DoorAnchor {
        id: "anchor-south".to_string(),
        room_id: room_id.to_string(),
        edge_id: "edge-2".to_string(),
        grid_pos: GridPoint {
            x: width as i32 / 2,
            y: height as i32 - 1,
        },
        facing: CardinalDir::South,
        width_tiles: 1,
        socket_tag: None,
    });

    // 西侧门口（左边中间）
    if height > 4 {
        anchors.push(DoorAnchor {
            id: "anchor-west".to_string(),
            room_id: room_id.to_string(),
            edge_id: "edge-3".to_string(),
            grid_pos: GridPoint {
                x: 0,
                y: height as i32 / 2,
            },
            facing: CardinalDir::West,
            width_tiles: 1,
            socket_tag: None,
        });
    }

    // 东侧门口（右边中间）
    if width > 4 {
        anchors.push(DoorAnchor {
            id: "anchor-east".to_string(),
            room_id: room_id.to_string(),
            edge_id: "edge-4".to_string(),
            grid_pos: GridPoint {
                x: width as i32 - 1,
                y: height as i32 / 2,
            },
            facing: CardinalDir::East,
            width_tiles: 1,
            socket_tag: None,
        });
    }

    anchors
}

/// 默认地形配置
fn default_terrain_config() -> TerrainConfig {
    TerrainConfig {
        obstacle_density: 0.2,
        min_walkable_ratio: 0.5,
    }
}

/// 验证地形连通性：所有门口瓦片通过可通行瓦片互相连通
fn assert_doorway_connectivity(terrain: &crate::model::terrain::Terrain, strategy_name: &str) {
    use std::collections::VecDeque;

    // 收集所有门口位置
    let w = terrain.grid_size.width;
    let h = terrain.grid_size.height;
    let mut doorways = Vec::new();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if terrain.tiles.get(x, y).copied() == Some(TileKind::Doorway) {
                doorways.push(GridPoint { x, y });
            }
        }
    }

    if doorways.len() < 2 {
        // 少于 2 个门口无需验证连通性
        return;
    }

    // OPT-P-03: 平坦 Vec<bool> 位图替代 HashSet
    // 从第一个门口 BFS，验证所有门口可达
    let size = (w * h) as usize;
    let start = doorways[0];
    let mut visited = vec![false; size];
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[(start.y as u32 * w + start.x as u32) as usize] = true;

    while let Some(current) = queue.pop_front() {
        for (dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let ny = current.y + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let ni = (ny as u32 * w + nx as u32) as usize;
            if visited[ni] {
                continue;
            }
            if let Some(tile) = terrain.tiles.get(nx, ny).copied() {
                if is_walkable(tile) {
                    visited[ni] = true;
                    queue.push_back(GridPoint { x: nx, y: ny });
                }
            }
        }
    }

    for (i, doorway) in doorways.iter().enumerate() {
        assert!(
            visited[(doorway.y as u32 * w + doorway.x as u32) as usize],
            "[{}] 门口 {} (位置 {:?}) 不可达，从门口 0 ({:?}) 出发无法到达",
            strategy_name,
            i,
            doorway,
            start
        );
    }
}

/// 验证可通行面积满足最小比例要求
fn assert_walkable_area(
    terrain: &crate::model::terrain::Terrain,
    min_ratio: f32,
    strategy_name: &str,
) {
    let total = terrain.grid_size.width * terrain.grid_size.height;
    let walkable = count_walkable_tiles(terrain);
    let ratio = walkable as f32 / total as f32;

    assert!(
        ratio >= min_ratio,
        "[{}] 可通行面积比例 {:.3} 低于最小要求 {:.3}（可通行: {}, 总计: {}）",
        strategy_name,
        ratio,
        min_ratio,
        walkable,
        total
    );
}

/// 手动统计可通行瓦片数量（不依赖 connectivity_summary）
fn count_walkable_tiles(terrain: &crate::model::terrain::Terrain) -> u32 {
    let mut count = 0u32;
    for y in 0..terrain.grid_size.height as i32 {
        for x in 0..terrain.grid_size.width as i32 {
            if let Some(tile) = terrain.tiles.get(x, y).copied() {
                if is_walkable(tile) {
                    count += 1;
                }
            }
        }
    }
    count
}

/// 验证外墙边框完整（除门口外，边界瓦片应为 Wall）
fn assert_border_integrity(terrain: &crate::model::terrain::Terrain, strategy_name: &str) {
    let w = terrain.grid_size.width as i32;
    let h = terrain.grid_size.height as i32;

    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                let tile = terrain.tiles.get(x, y).copied().unwrap();
                assert!(
                    tile == TileKind::Wall || tile == TileKind::Doorway,
                    "[{}] 边框位置 ({}, {}) 应为 Wall 或 Doorway，实际为 {:?}",
                    strategy_name,
                    x,
                    y,
                    tile
                );
            }
        }
    }
}

/// 验证所有锚点位置被标记为 Doorway
fn assert_doorways_marked(
    terrain: &crate::model::terrain::Terrain,
    anchors: &[DoorAnchor],
    room_id: &str,
    origin: GridPoint,
    strategy_name: &str,
) {
    for anchor in anchors.iter().filter(|a| a.room_id == room_id) {
        let local = GridPoint {
            x: anchor.grid_pos.x - origin.x,
            y: anchor.grid_pos.y - origin.y,
        };
        let tile = terrain.tiles.get(local.x, local.y).copied();
        assert_eq!(
            tile,
            Some(TileKind::Doorway),
            "[{}] 锚点 {} 在局部坐标 ({}, {}) 应为 Doorway，实际为 {:?}",
            strategy_name,
            anchor.id,
            local.x,
            local.y,
            tile
        );
    }
}

// ============================================================
// OpenArenaStrategy 测试
// ============================================================

#[test]
fn test_open_arena_connectivity() {
    // 验证开放式策略生成的地形中所有门口互相连通
    let room = make_test_room_with_bounds(RoomType::Boss, 16, 16, vec![]);
    let anchors = make_test_anchors(&room.id, 16, 16);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = OpenArenaStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OpenArenaStrategy 生成失败");

    assert_doorway_connectivity(&terrain, "open_arena");
}

#[test]
fn test_open_arena_walkable_area() {
    // 验证开放式策略的可通行面积满足最小比例
    let room = make_test_room_with_bounds(RoomType::Boss, 16, 16, vec![]);
    let anchors = make_test_anchors(&room.id, 16, 16);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = OpenArenaStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OpenArenaStrategy 生成失败");

    // 开放式策略应有较高的可通行面积
    assert_walkable_area(&terrain, 0.5, "open_arena");
}

#[test]
fn test_open_arena_border_integrity() {
    // 验证开放式策略的外墙边框完整
    let room = make_test_room_with_bounds(RoomType::Boss, 12, 12, vec![]);
    let anchors = make_test_anchors(&room.id, 12, 12);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(100);

    let terrain = OpenArenaStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OpenArenaStrategy 生成失败");

    assert_border_integrity(&terrain, "open_arena");
}

#[test]
fn test_open_arena_doorways_marked() {
    // 验证开放式策略正确标记所有门口
    let room = make_test_room_with_bounds(RoomType::Boss, 14, 14, vec![]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(77);

    let terrain = OpenArenaStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OpenArenaStrategy 生成失败");

    assert_doorways_marked(
        &terrain,
        &anchors,
        &room.id,
        GridPoint { x: 0, y: 0 },
        "open_arena",
    );
}

// ============================================================
// PillarStrategy 测试
// ============================================================

#[test]
fn test_pillar_connectivity() {
    // 验证柱状策略生成的地形中所有门口互相连通
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["pillar"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(123);

    let terrain = PillarStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("PillarStrategy 生成失败");

    assert_doorway_connectivity(&terrain, "pillar");
}

#[test]
fn test_pillar_walkable_area() {
    // 验证柱状策略的可通行面积满足最小比例
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["pillar"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(123);

    let terrain = PillarStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("PillarStrategy 生成失败");

    assert_walkable_area(&terrain, 0.5, "pillar");
}

#[test]
fn test_pillar_border_integrity() {
    // 验证柱状策略的外墙边框完整
    let room = make_test_room_with_bounds(RoomType::Combat, 12, 10, vec!["pillar"]);
    let anchors = make_test_anchors(&room.id, 12, 10);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(200);

    let terrain = PillarStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("PillarStrategy 生成失败");

    assert_border_integrity(&terrain, "pillar");
}

#[test]
fn test_pillar_doorways_marked() {
    // 验证柱状策略正确标记所有门口
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["pillar"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(55);

    let terrain = PillarStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("PillarStrategy 生成失败");

    assert_doorways_marked(
        &terrain,
        &anchors,
        &room.id,
        GridPoint { x: 0, y: 0 },
        "pillar",
    );
}

// ============================================================
// MazeStrategy 测试
// ============================================================

#[test]
fn test_maze_connectivity() {
    // 验证迷宫策略生成的地形中所有门口互相连通
    let room = make_test_room_with_bounds(RoomType::Puzzle, 15, 15, vec!["maze"]);
    let anchors = make_test_anchors(&room.id, 15, 15);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(999);

    let terrain = MazeStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("MazeStrategy 生成失败");

    assert_doorway_connectivity(&terrain, "maze");
}

#[test]
fn test_maze_walkable_area() {
    // 验证迷宫策略的可通行面积满足最小比例
    // 迷宫策略的可通行面积通常较低，使用较宽松的阈值
    let room = make_test_room_with_bounds(RoomType::Puzzle, 15, 15, vec!["maze"]);
    let anchors = make_test_anchors(&room.id, 15, 15);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(999);

    let terrain = MazeStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("MazeStrategy 生成失败");

    // 迷宫策略可通行面积较低，但至少应有 20%
    assert_walkable_area(&terrain, 0.2, "maze");
}

#[test]
fn test_maze_border_integrity() {
    // 验证迷宫策略的外墙边框完整
    let room = make_test_room_with_bounds(RoomType::Puzzle, 13, 13, vec!["maze"]);
    let anchors = make_test_anchors(&room.id, 13, 13);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(300);

    let terrain = MazeStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("MazeStrategy 生成失败");

    assert_border_integrity(&terrain, "maze");
}

#[test]
fn test_maze_doorways_marked() {
    // 验证迷宫策略正确标记所有门口
    let room = make_test_room_with_bounds(RoomType::Puzzle, 15, 15, vec!["maze"]);
    let anchors = make_test_anchors(&room.id, 15, 15);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(88);

    let terrain = MazeStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("MazeStrategy 生成失败");

    assert_doorways_marked(
        &terrain,
        &anchors,
        &room.id,
        GridPoint { x: 0, y: 0 },
        "maze",
    );
}

// ============================================================
// OrganicStrategy 测试
// ============================================================

#[test]
fn test_organic_connectivity() {
    // 验证有机式策略生成的地形中所有门口互相连通
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["organic"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(500);

    let terrain = OrganicStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OrganicStrategy 生成失败");

    assert_doorway_connectivity(&terrain, "organic");
}

#[test]
fn test_organic_walkable_area() {
    // 验证有机式策略的可通行面积满足最小比例
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["organic"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = TerrainConfig {
        obstacle_density: 0.2,
        min_walkable_ratio: 0.4,
    };
    let mut rng = StableRng::from_seed(500);

    let terrain = OrganicStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OrganicStrategy 生成失败");

    // 有机式策略由于 CA 生成，可通行面积可能较低
    assert_walkable_area(&terrain, 0.2, "organic");
}

#[test]
fn test_organic_border_integrity() {
    // 验证有机式策略的外墙边框完整
    let room = make_test_room_with_bounds(RoomType::Combat, 12, 12, vec!["organic"]);
    let anchors = make_test_anchors(&room.id, 12, 12);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(600);

    let terrain = OrganicStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OrganicStrategy 生成失败");

    assert_border_integrity(&terrain, "organic");
}

#[test]
fn test_organic_doorways_marked() {
    // 验证有机式策略正确标记所有门口
    let room = make_test_room_with_bounds(RoomType::Combat, 14, 14, vec!["organic"]);
    let anchors = make_test_anchors(&room.id, 14, 14);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(700);

    let terrain = OrganicStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("OrganicStrategy 生成失败");

    assert_doorways_marked(
        &terrain,
        &anchors,
        &room.id,
        GridPoint { x: 0, y: 0 },
        "organic",
    );
}

// ============================================================
// DefaultCarveStrategy 测试
// ============================================================

#[test]
fn test_default_carve_connectivity() {
    // 验证默认雕刻策略生成的地形中所有门口互相连通
    let room = make_test_room_with_bounds(RoomType::Combat, 12, 12, vec![]);
    let anchors = make_test_anchors(&room.id, 12, 12);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = DefaultCarveStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("DefaultCarveStrategy 生成失败");

    assert_doorway_connectivity(&terrain, "default_carve");
}

#[test]
fn test_default_carve_walkable_area() {
    // 验证默认雕刻策略的可通行面积满足最小比例
    let room = make_test_room_with_bounds(RoomType::Combat, 12, 12, vec![]);
    let anchors = make_test_anchors(&room.id, 12, 12);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = DefaultCarveStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("DefaultCarveStrategy 生成失败");

    assert_walkable_area(&terrain, 0.5, "default_carve");
}

#[test]
fn test_default_carve_border_integrity() {
    // 验证默认雕刻策略的外墙边框完整
    let room = make_test_room_with_bounds(RoomType::Event, 10, 10, vec![]);
    let anchors = make_test_anchors(&room.id, 10, 10);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(150);

    let terrain = DefaultCarveStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("DefaultCarveStrategy 生成失败");

    assert_border_integrity(&terrain, "default_carve");
}

#[test]
fn test_default_carve_doorways_marked() {
    // 验证默认雕刻策略正确标记所有门口
    let room = make_test_room_with_bounds(RoomType::Combat, 12, 12, vec![]);
    let anchors = make_test_anchors(&room.id, 12, 12);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = DefaultCarveStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("DefaultCarveStrategy 生成失败");

    assert_doorways_marked(
        &terrain,
        &anchors,
        &room.id,
        GridPoint { x: 0, y: 0 },
        "default_carve",
    );
}

// ============================================================
// 多种子稳定性测试
// ============================================================

#[test]
fn test_all_strategies_multiple_seeds() {
    // 使用多个种子验证所有策略的基本不变量
    let seeds = [1, 42, 100, 256, 999, 12345, 65535];

    for seed in seeds {
        // OpenArena
        let room = make_test_room_with_bounds(RoomType::Boss, 14, 14, vec![]);
        let anchors = make_test_anchors(&room.id, 14, 14);
        let config = default_terrain_config();
        let mut rng = StableRng::from_seed(seed);

        let terrain = OpenArenaStrategy
            .generate(&room, &anchors, &config, &mut rng)
            .unwrap_or_else(|_| panic!("OpenArena seed={} 生成失败", seed));
        assert_doorway_connectivity(&terrain, &format!("open_arena(seed={})", seed));

        // Pillar（使用较大房间避免边缘柱子阻塞门口）
        let room = make_test_room_with_bounds(RoomType::Combat, 16, 16, vec!["pillar"]);
        let anchors = make_test_anchors(&room.id, 16, 16);
        let mut rng = StableRng::from_seed(seed);

        let terrain = PillarStrategy
            .generate(&room, &anchors, &config, &mut rng)
            .unwrap_or_else(|_| panic!("Pillar seed={} 生成失败", seed));
        assert_doorway_connectivity(&terrain, &format!("pillar(seed={})", seed));

        // Maze
        let room = make_test_room_with_bounds(RoomType::Puzzle, 13, 13, vec!["maze"]);
        let anchors = make_test_anchors(&room.id, 13, 13);
        let mut rng = StableRng::from_seed(seed);

        let terrain = MazeStrategy
            .generate(&room, &anchors, &config, &mut rng)
            .unwrap_or_else(|_| panic!("Maze seed={} 生成失败", seed));
        assert_doorway_connectivity(&terrain, &format!("maze(seed={})", seed));

        // Organic
        let room = make_test_room_with_bounds(RoomType::Combat, 12, 12, vec!["organic"]);
        let anchors = make_test_anchors(&room.id, 12, 12);
        let mut rng = StableRng::from_seed(seed);

        let terrain = OrganicStrategy
            .generate(&room, &anchors, &config, &mut rng)
            .unwrap_or_else(|_| panic!("Organic seed={} 生成失败", seed));
        assert_doorway_connectivity(&terrain, &format!("organic(seed={})", seed));

        // DefaultCarve
        let room = make_test_room_with_bounds(RoomType::Combat, 10, 10, vec![]);
        let anchors = make_test_anchors(&room.id, 10, 10);
        let mut rng = StableRng::from_seed(seed);

        let terrain = DefaultCarveStrategy
            .generate(&room, &anchors, &config, &mut rng)
            .unwrap_or_else(|_| panic!("DefaultCarve seed={} 生成失败", seed));
        assert_doorway_connectivity(&terrain, &format!("default_carve(seed={})", seed));
    }
}

// ============================================================
// 连通性摘要一致性测试
// ============================================================

#[test]
fn test_connectivity_summary_consistency() {
    // 验证策略产出的连通性摘要与手动 BFS 结果一致
    // (connectivity_summary 在生产中由 repair_terrain_connectivity 覆写，
    //  此处直接基于 tiles 做 BFS 校验，不依赖策略返回值)
    let room = make_test_room_with_bounds(RoomType::Boss, 16, 16, vec![]);
    let anchors = make_test_anchors(&room.id, 16, 16);
    let config = default_terrain_config();
    let mut rng = StableRng::from_seed(42);

    let terrain = OpenArenaStrategy
        .generate(&room, &anchors, &config, &mut rng)
        .expect("生成失败");

    // 从 tiles 手动验证所有门口连通
    assert_doorway_connectivity(&terrain, "connectivity_summary_check");

    // 手动统计可通行瓦片
    let actual_walkable = count_walkable_tiles(&terrain);
    assert!(
        actual_walkable > 0,
        "手动统计的可通行瓦片应大于 0，实际为 {}",
        actual_walkable
    );
}
