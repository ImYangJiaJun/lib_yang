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
fn test_room_bounds_zero_area() {
    // 零面积房间：min == max，宽高均为 0
    let bounds = RoomBounds {
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: 0, y: 0 },
    };
    assert_eq!(bounds.width(), 0);
    assert_eq!(bounds.height(), 0);
}

#[test]
fn test_room_bounds_center_odd_size() {
    // 奇数尺寸：center() 整除截断，不四舍五入
    let bounds = RoomBounds {
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: 3, y: 5 },
    };
    let center = bounds.center();
    assert_eq!(center.x, 1); // (0+3)/2 = 1（截断）
    assert_eq!(center.y, 2); // (0+5)/2 = 2（截断）
}

#[test]
fn test_room_bounds_large_coords() {
    // 大坐标：使用 i32::MAX 作为边界，验证不溢出
    let bounds = RoomBounds {
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: i32::MAX, y: i32::MAX },
    };
    // width/height 用 unsigned_abs，不会溢出
    assert_eq!(bounds.width(), i32::MAX as u32);
    assert_eq!(bounds.height(), i32::MAX as u32);
    // center: (0 + i32::MAX) / 2 = 1073741823
    let center = bounds.center();
    assert_eq!(center.x, i32::MAX / 2);
    assert_eq!(center.y, i32::MAX / 2);
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
