// 门锚点生成

use crate::model::geometry::{CardinalDir, GridPoint};
use crate::model::room::{DoorAnchor, Room, RoomEdge};

/// 根据房间边界和拓扑边生成门锚点。
pub fn generate_door_anchors(
    rooms: &[Room],
    edges: &[RoomEdge],
    width_tiles: u16,
) -> Vec<DoorAnchor> {
    let mut anchors = Vec::with_capacity(edges.len() * 2);

    for (edge_index, edge) in edges.iter().enumerate() {
        let from_room = rooms
            .iter()
            .find(|room| room.id == edge.from_room)
            .and_then(|room| room.bounds.map(|bounds| (room.id.clone(), bounds)));
        let to_room = rooms
            .iter()
            .find(|room| room.id == edge.to_room)
            .and_then(|room| room.bounds.map(|bounds| (room.id.clone(), bounds)));

        let Some((from_room_id, from_bounds)) = from_room else {
            continue;
        };
        let Some((to_room_id, to_bounds)) = to_room else {
            continue;
        };

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
