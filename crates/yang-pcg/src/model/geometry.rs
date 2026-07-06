// 几何数据结构
// 定义基础几何类型和空间相关数据结构

use serde::{Deserialize, Serialize};

/// 世界坐标点
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl WorldPoint {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// 逻辑网格坐标点
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GridPoint {
    pub x: i32,
    pub y: i32,
}

impl GridPoint {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 网格尺寸
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GridSize {
    pub width: u32,
    pub height: u32,
}

/// 房间边界(逻辑网格空间)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoomBounds {
    /// 左下角坐标
    pub min: GridPoint,
    /// 右上角坐标
    pub max: GridPoint,
}

impl RoomBounds {
    /// 获取房间宽度
    pub fn width(&self) -> u32 {
        (i64::from(self.max.x) - i64::from(self.min.x))
            .unsigned_abs()
            .min(u64::from(u32::MAX)) as u32
    }

    /// 获取房间高度
    pub fn height(&self) -> u32 {
        (i64::from(self.max.y) - i64::from(self.min.y))
            .unsigned_abs()
            .min(u64::from(u32::MAX)) as u32
    }

    /// 获取房间中心点
    pub fn center(&self) -> GridPoint {
        GridPoint {
            x: ((i64::from(self.min.x) + i64::from(self.max.x)) / 2) as i32,
            y: ((i64::from(self.min.y) + i64::from(self.max.y)) / 2) as i32,
        }
    }

    /// 判断是否包含指定网格点。
    pub fn contains(&self, point: GridPoint) -> bool {
        point.x >= self.min.x
            && point.x < self.max.x
            && point.y >= self.min.y
            && point.y < self.max.y
    }

    /// 判断是否与另一个边界相交。
    pub fn intersects(&self, other: &RoomBounds) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }
}

/// 基本方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CardinalDir {
    /// 北(上)
    North,
    /// 南(下)
    South,
    /// 东(右)
    East,
    /// 西(左)
    West,
}

/// 3D 变换
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Transform3 {
    /// 位置
    pub position: WorldPoint,
    /// 旋转(欧拉角,度数)
    pub rotation: (f32, f32, f32),
    /// 缩放
    pub scale: (f32, f32, f32),
}

impl Transform3 {
    pub fn new(position: WorldPoint, rotation: (f32, f32, f32), scale: (f32, f32, f32)) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }
}

impl Default for Transform3 {
    fn default() -> Self {
        Self {
            position: WorldPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: (0.0, 0.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        }
    }
}

/// 3D 边界盒
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bounds3 {
    /// 最小点
    pub min: WorldPoint,
    /// 最大点
    pub max: WorldPoint,
}

impl Bounds3 {
    pub fn new(min: WorldPoint, max: WorldPoint) -> Self {
        Self { min, max }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_bounds_width_height_do_not_overflow_extreme_coordinates() {
        let bounds = RoomBounds {
            min: GridPoint {
                x: i32::MIN,
                y: i32::MIN,
            },
            max: GridPoint {
                x: i32::MAX,
                y: i32::MAX,
            },
        };

        assert_eq!(bounds.width(), u32::MAX);
        assert_eq!(bounds.height(), u32::MAX);
    }

    #[test]
    fn test_room_bounds_center_uses_wide_arithmetic() {
        let bounds = RoomBounds {
            min: GridPoint {
                x: i32::MAX - 1,
                y: i32::MAX - 1,
            },
            max: GridPoint {
                x: i32::MAX,
                y: i32::MAX,
            },
        };

        assert_eq!(
            bounds.center(),
            GridPoint {
                x: i32::MAX - 1,
                y: i32::MAX - 1,
            }
        );
    }
}
