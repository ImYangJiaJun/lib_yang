// 布局求解器

use std::collections::HashMap;

use crate::config::NormalizedConfig;
use crate::model::geometry::{GridPoint, RoomBounds};
use crate::model::room::{Room, RoomGraph};
use crate::rng::StableRng;
use crate::topology::graph::sample_range_u16;

/// 求解房间边界布局。
pub fn solve_room_bounds(
    graph: &RoomGraph,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> HashMap<String, RoomBounds> {
    let mut bounds_map = HashMap::with_capacity(graph.nodes.len());
    let mut critical_cursor_x = 0i32;
    let row_spacing =
        i32::from(config.config.room_size.max_height + config.config.corridor.width + 8);

    for room_id in &graph.critical_path {
        let width = i32::from(sample_range_u16(
            rng,
            crate::config::RangeU16::new(
                config.config.room_size.min_width,
                config.config.room_size.max_width,
            ),
        ));
        let height = i32::from(sample_range_u16(
            rng,
            crate::config::RangeU16::new(
                config.config.room_size.min_height,
                config.config.room_size.max_height,
            ),
        ));
        let bounds = RoomBounds {
            min: GridPoint {
                x: critical_cursor_x,
                y: -height / 2,
            },
            max: GridPoint {
                x: critical_cursor_x + width,
                y: -height / 2 + height,
            },
        };
        critical_cursor_x = bounds.max.x + i32::from(config.config.corridor.width) + 6;
        bounds_map.insert(room_id.clone(), bounds);
    }

    for (branch_index, branch) in graph.branches.iter().enumerate() {
        let parent_bounds = bounds_map
            .get(&branch.start_room)
            .copied()
            .unwrap_or(RoomBounds {
                min: GridPoint { x: 0, y: 0 },
                max: GridPoint { x: 12, y: 12 },
            });
        let vertical_direction = if branch_index % 2 == 0 { 1 } else { -1 };
        let base_y = if vertical_direction > 0 {
            parent_bounds.max.y + row_spacing
        } else {
            parent_bounds.min.y - row_spacing
        };
        let mut branch_cursor_x = parent_bounds.center().x;

        for room_id in &branch.room_ids {
            let width = i32::from(sample_range_u16(
                rng,
                crate::config::RangeU16::new(
                    config.config.room_size.min_width,
                    config.config.room_size.max_width,
                ),
            ));
            let height = i32::from(sample_range_u16(
                rng,
                crate::config::RangeU16::new(
                    config.config.room_size.min_height,
                    config.config.room_size.max_height,
                ),
            ));

            let min_y = if vertical_direction > 0 {
                base_y
            } else {
                base_y - height
            };
            let bounds = RoomBounds {
                min: GridPoint {
                    x: branch_cursor_x,
                    y: min_y,
                },
                max: GridPoint {
                    x: branch_cursor_x + width,
                    y: min_y + height,
                },
            };

            branch_cursor_x = bounds.max.x + i32::from(config.config.corridor.width) + 4;
            bounds_map.insert(room_id.clone(), bounds);
        }
    }

    bounds_map
}

/// 将边界写回房间列表。
pub fn apply_room_bounds(rooms: &[Room], bounds_map: &HashMap<String, RoomBounds>) -> Vec<Room> {
    rooms
        .iter()
        .cloned()
        .map(|mut room| {
            room.bounds = bounds_map.get(&room.id).copied();
            room
        })
        .collect()
}
