// 走廊生成

use crate::config::ConnectionStrategy;
use crate::model::geometry::GridPoint;
use crate::model::room::{Corridor, CorridorPath, DoorAnchor, RoomEdge};

/// 依据拓扑边和门锚点生成走廊路径。
pub fn generate_corridors(
    edges: &[RoomEdge],
    anchors: &[DoorAnchor],
    width_tiles: u16,
    strategy: ConnectionStrategy,
) -> Vec<Corridor> {
    let mut corridors = Vec::with_capacity(edges.len());

    for (edge_index, edge) in edges.iter().enumerate() {
        let from_anchor = anchors
            .iter()
            .find(|anchor| anchor.edge_id == edge.id && anchor.room_id == edge.from_room);
        let to_anchor = anchors
            .iter()
            .find(|anchor| anchor.edge_id == edge.id && anchor.room_id == edge.to_room);

        let (Some(from_anchor), Some(to_anchor)) = (from_anchor, to_anchor) else {
            continue;
        };

        let path = match strategy {
            ConnectionStrategy::Straight | ConnectionStrategy::SharedEdge
                if from_anchor.grid_pos.x == to_anchor.grid_pos.x
                    || from_anchor.grid_pos.y == to_anchor.grid_pos.y =>
            {
                CorridorPath::Straight(vec![from_anchor.grid_pos, to_anchor.grid_pos])
            }
            ConnectionStrategy::Straight => CorridorPath::Polyline(vec![
                from_anchor.grid_pos,
                GridPoint {
                    x: to_anchor.grid_pos.x,
                    y: from_anchor.grid_pos.y,
                },
                to_anchor.grid_pos,
            ]),
            ConnectionStrategy::SharedEdge | ConnectionStrategy::Orthogonal => {
                let midpoint = GridPoint {
                    x: to_anchor.grid_pos.x,
                    y: from_anchor.grid_pos.y,
                };
                CorridorPath::Orthogonal(vec![from_anchor.grid_pos, midpoint, to_anchor.grid_pos])
            }
        };

        corridors.push(Corridor {
            id: format!("corridor-{edge_index:03}"),
            from_room: edge.from_room.clone(),
            to_room: edge.to_room.clone(),
            from_anchor: from_anchor.id.clone(),
            to_anchor: to_anchor.id.clone(),
            width_tiles,
            path,
            segment_tags: Vec::new(),
        });
    }

    corridors
}
