// 门锚点生成

use crate::model::geometry::{CardinalDir, GridPoint, RoomBounds};
use crate::model::room::{DoorAnchor, Room, RoomEdge};
use std::collections::HashMap;

/// 根据房间边界和拓扑边生成门锚点。
pub fn generate_door_anchors(
    rooms: &[Room],
    edges: &[RoomEdge],
    width_tiles: u16,
) -> Vec<DoorAnchor> {
    // 构建 room_id -> RoomBounds 索引，将每条边的 O(R) 线性查找降为 O(1)
    // （与 corridors.rs 的 anchor_map 同一模式）
    let bounds_map: HashMap<&str, RoomBounds> = rooms
        .iter()
        .filter_map(|room| room.bounds.map(|bounds| (room.id.as_str(), bounds)))
        .collect();

    let mut anchors = Vec::with_capacity(edges.len() * 2);

    for (edge_index, edge) in edges.iter().enumerate() {
        // 原语义：房间不存在或缺 bounds 即跳过该边，此处等价
        let Some(&from_bounds) = bounds_map.get(edge.from_room.as_str()) else {
            continue;
        };
        let Some(&to_bounds) = bounds_map.get(edge.to_room.as_str()) else {
            continue;
        };
        let from_room_id = edge.from_room.clone();
        let to_room_id = edge.to_room.clone();

        let from_center = from_bounds.center();
        let to_center = to_bounds.center();
        let dx = to_center.x - from_center.x;
        let dy = to_center.y - from_center.y;

        let (from_pos, from_facing, to_pos, to_facing) = if dx.abs() >= dy.abs() {
            if dx >= 0 {
                (
                    GridPoint {
                        x: from_bounds.max.x - 1,
                        y: from_center.y,
                    },
                    CardinalDir::East,
                    GridPoint {
                        x: to_bounds.min.x,
                        y: to_center.y,
                    },
                    CardinalDir::West,
                )
            } else {
                (
                    GridPoint {
                        x: from_bounds.min.x,
                        y: from_center.y,
                    },
                    CardinalDir::West,
                    GridPoint {
                        x: to_bounds.max.x - 1,
                        y: to_center.y,
                    },
                    CardinalDir::East,
                )
            }
        } else if dy >= 0 {
            (
                GridPoint {
                    x: from_center.x,
                    y: from_bounds.max.y - 1,
                },
                CardinalDir::North,
                GridPoint {
                    x: to_center.x,
                    y: to_bounds.min.y,
                },
                CardinalDir::South,
            )
        } else {
            (
                GridPoint {
                    x: from_center.x,
                    y: from_bounds.min.y,
                },
                CardinalDir::South,
                GridPoint {
                    x: to_center.x,
                    y: to_bounds.max.y - 1,
                },
                CardinalDir::North,
            )
        };

        anchors.push(DoorAnchor {
            id: format!("anchor-{edge_index:03}-from"),
            room_id: from_room_id,
            edge_id: edge.id.clone(),
            grid_pos: from_pos,
            facing: from_facing,
            width_tiles,
            socket_tag: None,
        });
        anchors.push(DoorAnchor {
            id: format!("anchor-{edge_index:03}-to"),
            room_id: to_room_id,
            edge_id: edge.id.clone(),
            grid_pos: to_pos,
            facing: to_facing,
            width_tiles,
            socket_tag: None,
        });
    }

    anchors
}
