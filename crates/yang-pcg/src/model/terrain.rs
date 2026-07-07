// 地形数据模型
// 定义房间内部地形的数据结构

use crate::error::{PcgError, PcgResult};
use crate::model::geometry::{GridPoint, GridSize};
use crate::model::room::RoomId;
use serde::{Deserialize, Serialize};

/// 单个 `Grid2D` 允许的最大格子数。
///
/// PCG 房间地形是内存常驻基础结构；超过该上限通常表示配置错误或异常输入。
/// 该限制避免病态尺寸触发整数溢出或巨量分配。
const MAX_GRID_CELLS: usize = 1_048_576;

/// 地形
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Terrain {
    /// 所属房间 ID
    pub room_id: RoomId,
    /// 网格尺寸
    pub grid_size: GridSize,
    /// 瓦片网格
    pub tiles: Grid2D<TileKind>,
    /// 保留区列表
    pub reserved_zones: Vec<ReservedZone>,
    /// 连通性摘要
    pub connectivity_summary: ConnectivitySummary,
}

/// 瓦片类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TileKind {
    /// 空白(未定义)
    Empty,
    /// 地板(可通行)
    Floor,
    /// 墙体(不可通行)
    Wall,
    /// 障碍物(不可通行)
    Obstacle,
    /// 保留区(特殊用途)
    Reserved,
    /// 门口(连通区)
    Doorway,
}

/// 2D 网格
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Grid2D<T> {
    /// 网格宽度
    pub width: u32,
    /// 网格高度
    pub height: u32,
    /// 瓦片数据(行优先存储)
    pub data: Vec<T>,
}

impl<T: Clone> Grid2D<T> {
    /// 创建新网格
    pub fn new(width: u32, height: u32, default_value: T) -> PcgResult<Self> {
        if width == 0 || height == 0 {
            return Err(PcgError::terrain("Grid2D 宽度和高度必须大于 0"));
        }

        let size = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PcgError::terrain("Grid2D 尺寸乘法溢出"))?;
        if size > MAX_GRID_CELLS {
            return Err(PcgError::terrain(format!(
                "Grid2D 尺寸过大: {}x{} = {}，最大允许 {}",
                width, height, size, MAX_GRID_CELLS
            )));
        }

        Ok(Self {
            width,
            height,
            data: vec![default_value; size],
        })
    }

    /// 获取指定位置的瓦片
    pub fn get(&self, x: i32, y: i32) -> Option<&T> {
        if x < 0 || y < 0 {
            return None;
        }
        let x = x as u32;
        let y = y as u32;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?;
        self.data.get(index)
    }

    /// 设置指定位置的瓦片
    pub fn set(&mut self, x: i32, y: i32, value: T) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let x = x as u32;
        let y = y as u32;
        if x >= self.width || y >= self.height {
            return false;
        }
        let Some(index) = (y as usize)
            .checked_mul(self.width as usize)
            .and_then(|base| base.checked_add(x as usize))
        else {
            return false;
        };
        if index < self.data.len() {
            self.data[index] = value;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_get_returns_none_when_index_calculation_overflows() {
        let grid = Grid2D {
            width: 65_536,
            height: 65_537,
            data: vec![TileKind::Floor],
        };

        assert_eq!(grid.get(0, 65_536), None);
    }

    #[test]
    fn grid_set_returns_false_when_index_calculation_overflows() {
        let mut grid = Grid2D {
            width: 65_536,
            height: 65_537,
            data: vec![TileKind::Floor],
        };

        assert!(!grid.set(0, 65_536, TileKind::Wall));
        assert_eq!(grid.data[0], TileKind::Floor);
    }

    #[test]
    fn grid_new_rejects_excessive_size() {
        let result = Grid2D::new(65_536, 65_536, TileKind::Wall);

        assert!(result.is_err(), "异常尺寸不应 panic 或尝试巨量分配");
    }

    #[test]
    fn grid_new_rejects_zero_dimensions() {
        assert!(Grid2D::new(0, 1, TileKind::Wall).is_err());
        assert!(Grid2D::new(1, 0, TileKind::Wall).is_err());
    }
}

/// 保留区
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReservedZone {
    /// 区域 ID
    pub id: String,
    /// 区域类型(如 "boss_center", "entrance", "treasure_spot")
    pub zone_type: String,
    /// 区域边界
    pub bounds: ReservedZoneBounds,
    /// 是否允许放置交互物
    pub allow_items: bool,
    /// 是否允许放置敌人
    pub allow_enemies: bool,
}

/// 保留区边界
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ReservedZoneBounds {
    /// 矩形区域
    Rect { min: GridPoint, max: GridPoint },
    /// 圆形区域
    Circle { center: GridPoint, radius: u32 },
    /// 多边形区域
    Polygon { points: Vec<GridPoint> },
}

/// 连通性摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ConnectivitySummary {
    /// 是否所有门口之间可达
    pub all_doors_connected: bool,
    /// 可通行瓦片数量
    pub walkable_tile_count: u32,
    /// 总瓦片数量
    pub total_tile_count: u32,
    /// 连通区域数量
    pub connected_region_count: u32,
}
