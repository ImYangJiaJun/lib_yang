// 拓扑生成模块
// 负责生成房间图、关键路径和分支

pub mod graph;
pub mod planner;
pub mod room_types;

use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::model::room::RoomGraph;
use crate::rng::StableRng;

pub use planner::TopologyGenerator;

/// 生成房间拓扑图。
pub fn generate_topology(config: &NormalizedConfig, rng: &mut StableRng) -> PcgResult<RoomGraph> {
    TopologyGenerator::new(config).generate(rng)
}
