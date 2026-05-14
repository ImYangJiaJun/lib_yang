// 几何数据结构测试
// 验证需求: 4.1, 4.2, 4.3

use crate::model::geometry::*;

#[test]
fn test_world_point_creation() {
    // 测试世界坐标点创建
    let point = WorldPoint {
        x: 10.0,
        y: 20.0,
        z: 30.0,
    };
    assert_eq!(point.x, 10.0);
    assert_eq!(point.y, 20.0);
    assert_eq!(point.z, 30.0);
}

#[test]
fn test_grid_point_creation() {
    // 测试网格坐标点创建
    let point = GridPoint { x: 5, y: 10 };
    assert_eq!(point.x, 5);
    assert_eq!(point.y, 10);
}

#[test]
fn test_room_bounds_dimensions() {
    // 测试房间边界尺寸计算
    let bounds = RoomBounds {
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: 10, y: 20 },
    };
    assert_eq!(bounds.width(), 10);
    assert_eq!(bounds.height(), 20);
}

#[test]
fn test_room_bounds_center() {
    // 测试房间边界中心点计算
    let bounds = RoomBounds {
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: 10, y: 20 },
    };
    let center = bounds.center();
    assert_eq!(center.x, 5);
    assert_eq!(center.y, 10);
}

#[test]
fn test_cardinal_directions() {
    // 测试基本方向枚举
    let north = CardinalDir::North;
    let south = CardinalDir::South;
    let east = CardinalDir::East;
    let west = CardinalDir::West;

    assert_ne!(north, south);
    assert_ne!(east, west);
}

#[test]
fn test_transform3_default() {
    // 测试 3D 变换默认值
    let transform = Transform3::default();
    assert_eq!(transform.position.x, 0.0);
    assert_eq!(transform.position.y, 0.0);
    assert_eq!(transform.position.z, 0.0);
    assert_eq!(transform.rotation, (0.0, 0.0, 0.0));
    assert_eq!(transform.scale, (1.0, 1.0, 1.0));
}

#[test]
fn test_bounds3_creation() {
    // 测试 3D 边界盒创建
    let bounds = Bounds3 {
        min: WorldPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        max: WorldPoint {
            x: 100.0,
            y: 100.0,
            z: 100.0,
        },
    };
    assert_eq!(bounds.min.x, 0.0);
    assert_eq!(bounds.max.x, 100.0);
}
