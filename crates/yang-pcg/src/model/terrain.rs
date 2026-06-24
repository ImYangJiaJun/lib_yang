// 地形数据模型
// 定义房间内部地形的数据结构

use crate::model::geometry::{GridPoint, GridSize};
use crate::model::room::RoomId;
use serde::{Deserialize, Serialize};

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
    pub fn new(width: u32, height: u32, default_value: T) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            data: vec![default_value; size],
        }
    }

    /// 获取指定位置的瓦片
    pub fn get(&self, x: i32, y: i32) -> Option<&T> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        let index = (y as u32 * self.width + x as u32) as usize;
        self.data.get(index)
    }

    /// 设置指定位置的瓦片
    pub fn set(&mut self, x: i32, y: i32, value: T) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        let index = (y as u32 * self.width + x as u32) as usize;
        if index < self.data.len() {
            self.data[index] = value;
            true
        } else {
            false
        }
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
