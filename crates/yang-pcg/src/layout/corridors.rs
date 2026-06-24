// 走廊生成

use std::collections::HashMap;

use crate::config::ConnectionStrategy;
use crate::error::{PcgError, PcgResult};
use crate::model::geometry::GridPoint;
use crate::model::room::{Corridor, CorridorPath, DoorAnchor, RoomEdge};

/// 依据拓扑边和门锚点生成走廊路径。
///
/// # 错误
///
/// 当某条边缺少对应的门锚点时返回 `PcgError::Layout`。
pub fn generate_corridors(
    edges: &[RoomEdge],
    anchors: &[DoorAnchor],
    width_tiles: u16,
    strategy: ConnectionStrategy,
) -> PcgResult<Vec<Corridor>> {
    // 构建 (edge_id, room_id) -> &DoorAnchor 的 HashMap，将 O(n) 查找降为 O(1)
    let anchor_map: HashMap<(&str, &str), &DoorAnchor> = anchors
        .iter()
        .map(|a| ((a.edge_id.as_str(), a.room_id.as_str()), a))
        .collect();

    let mut corridors = Vec::with_capacity(edges.len());

    for (edge_index, edge) in edges.iter().enumerate() {
        let from_anchor = anchor_map
            .get(&(edge.id.as_str(), edge.from_room.as_str()))
            .ok_or_else(|| {
                PcgError::layout(format!(
                    "边 '{}' 缺少 from_room '{}' 的门锚点",
                    edge.id, edge.from_room
                ))
            })?;
        let to_anchor = anchor_map
            .get(&(edge.id.as_str(), edge.to_room.as_str()))
            .ok_or_else(|| {
                PcgError::layout(format!(
                    "边 '{}' 缺少 to_room '{}' 的门锚点",
                    edge.id, edge.to_room
                ))
            })?;

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

    Ok(corridors)
}
