// 俯视角网格 backend
//
// 把现有俯视角的布局/地形/点位逻辑收拢到一个 `PipelineBackend` 实现。
// 本迭代各 stage 方法是现有自由函数（`layout::solve_layout` /
// `terrain::generate_terrains` / `spawn::generate_spawns(_with_debug)`）的薄转发，
// 行为与重构前完全一致；算法本体仍在 `src/layout`、`src/terrain`、`src/spawn`，
// 留待 platformer backend 落地时再做对称的物理迁移。

use crate::backend::PipelineBackend;
use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::layout::{self, LayoutOutput};
use crate::model::room::{DoorAnchor, Room, RoomGraph};
use crate::model::terrain::Terrain;
use crate::rng::StableRng;
use crate::spawn::{self, SpawnOutput, SpawnOutputWithDebug};
use crate::terrain;

/// 俯视角网格 backend（无状态）。
#[derive(Debug, Clone, Copy, Default)]
pub struct TopDownBackend;

impl PipelineBackend for TopDownBackend {
    fn solve_layout(
        &self,
        graph: &RoomGraph,
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<LayoutOutput> {
        layout::solve_layout(graph, config, rng)
    }

    fn generate_terrains(
        &self,
        rooms: &[Room],
        door_anchors: &[DoorAnchor],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Vec<Terrain>> {
        terrain::generate_terrains(rooms, door_anchors, config, rng)
    }

    fn generate_spawns(
        &self,
        rooms: &[Room],
        terrains: &[Terrain],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<SpawnOutput> {
        spawn::generate_spawns(rooms, terrains, config, rng)
    }

    fn generate_spawns_with_debug(
        &self,
        rooms: &[Room],
        terrains: &[Terrain],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<SpawnOutputWithDebug> {
        spawn::generate_spawns_with_debug(rooms, terrains, config, rng)
    }
}
