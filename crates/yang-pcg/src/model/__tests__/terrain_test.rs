// 地形数据模型测试
// 验证需求: 5.1, 5.2, 5.3, 5.4

use crate::model::geometry::{GridPoint, GridSize};
use crate::model::terrain::*;

#[test]
fn test_tile_kind_variants() {
    // 测试瓦片类型枚举
    let kinds = [
        TileKind::Empty,
        TileKind::Floor,
        TileKind::Wall,
        TileKind::Obstacle,
        TileKind::Reserved,
        TileKind::Doorway,
    ];

    assert_eq!(kinds.len(), 6);
    assert_eq!(kinds[1], TileKind::Floor);
    assert_eq!(kinds[5], TileKind::Doorway);
}

#[test]
fn test_grid2d_creation() {
    // 测试 2D 网格创建
    let grid = Grid2D::new(10, 10, TileKind::Empty);

    assert_eq!(grid.width, 10);
    assert_eq!(grid.height, 10);
    assert_eq!(grid.data.len(), 100);
}

#[test]
fn test_grid2d_get_set() {
    // 测试网格读写操作
    let mut grid = Grid2D::new(5, 5, TileKind::Empty);

    // 设置瓦片
    assert!(grid.set(2, 3, TileKind::Floor));

    // 读取瓦片
    let tile = grid.get(2, 3);
    assert!(tile.is_some());
    assert_eq!(*tile.unwrap(), TileKind::Floor);
}

#[test]
fn test_grid2d_bounds_check() {
    // 测试网格边界检查
    let mut grid = Grid2D::new(5, 5, TileKind::Empty);

    // 越界访问应返回 None
    assert!(grid.get(-1, 0).is_none());
    assert!(grid.get(0, -1).is_none());
    assert!(grid.get(5, 0).is_none());
    assert!(grid.get(0, 5).is_none());

    // 越界设置应返回 false
    assert!(!grid.set(-1, 0, TileKind::Floor));
    assert!(!grid.set(5, 0, TileKind::Floor));
}

#[test]
fn test_terrain_creation() {
    // 测试地形创建
    let terrain = Terrain {
        room_id: "room-001".to_string(),
        grid_size: GridSize {
            width: 20,
            height: 20,
        },
        tiles: Grid2D::new(20, 20, TileKind::Empty),
        reserved_zones: vec![],
        connectivity_summary: ConnectivitySummary {
            all_doors_connected: true,
            walkable_tile_count: 300,
            total_tile_count: 400,
            connected_region_count: 1,
        },
    };

    assert_eq!(terrain.room_id, "room-001");
    assert_eq!(terrain.grid_size.width, 20);
    assert_eq!(terrain.grid_size.height, 20);
    assert!(terrain.connectivity_summary.all_doors_connected);
}

#[test]
fn test_reserved_zone_rect() {
    // 测试矩形保留区
    let zone = ReservedZone {
        id: "zone-001".to_string(),
        zone_type: "boss_center".to_string(),
        bounds: ReservedZoneBounds::Rect {
            min: GridPoint { x: 5, y: 5 },
            max: GridPoint { x: 15, y: 15 },
        },
        allow_items: false,
        allow_enemies: true,
    };

    assert_eq!(zone.zone_type, "boss_center");
    assert!(!zone.allow_items);
    assert!(zone.allow_enemies);
}

#[test]
fn test_reserved_zone_circle() {
    // 测试圆形保留区
    let zone = ReservedZone {
        id: "zone-002".to_string(),
        zone_type: "entrance".to_string(),
        bounds: ReservedZoneBounds::Circle {
            center: GridPoint { x: 10, y: 10 },
            radius: 5,
        },
        allow_items: true,
        allow_enemies: false,
    };

    match zone.bounds {
        ReservedZoneBounds::Circle { center, radius } => {
            assert_eq!(center.x, 10);
            assert_eq!(center.y, 10);
            assert_eq!(radius, 5);
        }
        _ => panic!("Expected Circle variant"),
    }
}

#[test]
fn test_connectivity_summary() {
    // 测试连通性摘要
    let summary = ConnectivitySummary {
        all_doors_connected: true,
        walkable_tile_count: 250,
        total_tile_count: 400,
        connected_region_count: 1,
    };

    assert!(summary.all_doors_connected);
    assert_eq!(summary.connected_region_count, 1);

    // 计算可通行比例
    let walkable_ratio = summary.walkable_tile_count as f32 / summary.total_tile_count as f32;
    assert!(walkable_ratio > 0.6);
}
