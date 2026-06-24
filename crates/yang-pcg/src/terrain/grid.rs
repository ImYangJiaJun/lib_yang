// 网格数据结构

use crate::model::geometry::GridPoint;
use crate::model::terrain::{Grid2D, TileKind};

/// 将世界网格坐标转换为房间局部网格坐标。
pub fn to_local(origin: GridPoint, point: GridPoint) -> GridPoint {
    GridPoint {
        x: point.x - origin.x,
        y: point.y - origin.y,
    }
}

/// 判断某个瓦片是否可通行。
pub fn is_walkable(tile: TileKind) -> bool {
    matches!(
        tile,
        TileKind::Floor | TileKind::Doorway | TileKind::Reserved
    )
}

/// 收集网格中的所有可通行候选点。
pub fn collect_walkable_points(grid: &Grid2D<TileKind>) -> Vec<GridPoint> {
    let mut points = Vec::with_capacity((grid.width * grid.height) as usize);
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            if grid.get(x, y).copied().is_some_and(is_walkable) {
                points.push(GridPoint { x, y });
            }
        }
    }
    points
}
