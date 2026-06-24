// 空间布局模块
// 负责将拓扑图映射到平面空间，生成房间边界、门锚点和走廊

pub mod corridors;
pub mod doors;
pub mod solver;

use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::model::room::{Corridor, DoorAnchor, Room, RoomGraph};
use crate::rng::StableRng;

/// 布局结果。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LayoutOutput {
    pub rooms: Vec<Room>,
    pub door_anchors: Vec<DoorAnchor>,
    pub corridors: Vec<Corridor>,
}

/// 求解房间布局、门锚点和走廊。
pub fn solve_layout(
    graph: &RoomGraph,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> PcgResult<LayoutOutput> {
    let bounds_map = solver::solve_room_bounds(graph, config, rng);
    let rooms = solver::apply_room_bounds(&graph.nodes, &bounds_map);
    let door_anchors =
        doors::generate_door_anchors(&rooms, &graph.edges, config.config.corridor.width);
    let corridors = corridors::generate_corridors(
        &graph.edges,
        &door_anchors,
        config.config.corridor.width,
        config.config.corridor.connection_strategy,
    );

    Ok(LayoutOutput {
        rooms,
        door_anchors,
        corridors,
    })
}
