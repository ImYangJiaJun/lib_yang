// 俯视角网格 backend
//
// 把现有俯视角的布局/地形/点位/校验逻辑收拢到一个 `PipelineBackend` 实现。
// 本迭代各 stage 方法是现有自由函数（`layout::solve_layout` /
// `terrain::generate_terrains` / `spawn::generate_spawns(_with_debug)`）的薄转发，
// 行为与重构前完全一致；算法本体仍在 `src/layout`、`src/terrain`、`src/spawn`，
// 留待 platformer backend 落地时再做对称的物理迁移。

use crate::backend::{PipelineBackend, ValidationScope};
use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::layout::{self, LayoutOutput};
use crate::model::request::Constraint;
use crate::model::result::GenerationResult;
use crate::model::room::{DoorAnchor, Room, RoomGraph};
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use crate::rng::StableRng;
use crate::spawn::{self, SpawnOutput, SpawnOutputWithDebug};
use crate::validation::{
    validate_no_overlap, validate_reachability, validate_result, validate_spawn_spacing,
    validate_terrain_connectivity,
};
use crate::{spawn::min_cross_type_spacing, terrain};

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

    fn validate(
        &self,
        result: &GenerationResult,
        config: &NormalizedConfig,
        constraints: &[Constraint],
        scope: ValidationScope,
    ) -> PcgResult<()> {
        // 点位间距阈值：与生成侧（spawn 跨类型间距保证）及 property 测试一致，
        // 取交互物/敌人最小间距的较小者。
        let min_spacing = i32::from(min_cross_type_spacing(&config.config));

        let all_spawns: Vec<SpawnPoint> = result
            .item_spawns
            .iter()
            .chain(result.enemy_spawns.iter())
            .cloned()
            .collect();

        match scope {
            ValidationScope::FullFloor => {
                // 整层：结构一致性 + 整图可达 + 重叠 + 连通 + 间距。
                validate_result(result)?;
                validate_reachability(&result.topology)?;
                validate_no_overlap(&result.rooms)?;
                validate_terrain_connectivity(&result.terrains)?;
                validate_spawn_spacing(&all_spawns, constraints, Some(min_spacing))?;
            }
            ValidationScope::Chunk => {
                // 分块部分结果：跳过整图结构计数与整图可达性，
                // 仅保留对任意子集成立的局部不变量。
                validate_no_overlap(&result.rooms)?;
                validate_terrain_connectivity(&result.terrains)?;
                validate_spawn_spacing(&all_spawns, constraints, Some(min_spacing))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;
    use crate::generator::MapGenerator;
    use crate::model::request::GenerationRequest;

    /// 用真实生成器产出一个合法结果（seed 42 经各算法修复后满足全部不变量），
    /// 连同其归一化配置一起返回，供校验测试在其上做最小破坏。
    fn valid_result_and_config() -> (GenerationResult, NormalizedConfig) {
        let config = GenerationConfig::default();
        let normalized = config.normalize().expect("默认配置应归一化");
        let result = MapGenerator::new()
            .generate(GenerationRequest {
                seed: Some(42),
                config,
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("默认配置生成应成功并通过硬校验");
        (result, normalized)
    }

    #[test]
    fn test_validate_fullfloor_passes_for_valid_result() {
        let (result, normalized) = valid_result_and_config();
        assert!(TopDownBackend
            .validate(&result, &normalized, &[], ValidationScope::FullFloor)
            .is_ok());
    }

    #[test]
    fn test_validate_detects_overlap() {
        let (mut result, normalized) = valid_result_and_config();
        // 把第二个房间边界改成与第一个相同 → 重叠（结构计数不变，先过结构检查再触发重叠）
        let first_bounds = result.rooms[0].bounds;
        result.rooms[1].bounds = first_bounds;
        let err = TopDownBackend
            .validate(&result, &normalized, &[], ValidationScope::FullFloor)
            .unwrap_err();
        assert_eq!(err.error_code(), "PCG-LAYOUT-001");
    }

    #[test]
    fn test_chunk_scope_skips_whole_graph_structural_checks() {
        let (mut result, normalized) = valid_result_and_config();
        // 删掉一个房间制造「rooms 数 != topology.nodes 数」的整图结构不一致
        result.rooms.pop();
        // FullFloor 应因结构计数不符而失败
        assert!(TopDownBackend
            .validate(&result, &normalized, &[], ValidationScope::FullFloor)
            .is_err());
        // Chunk 跳过整图结构/可达性检查，仅查局部不变量 → 应通过
        assert!(TopDownBackend
            .validate(&result, &normalized, &[], ValidationScope::Chunk)
            .is_ok());
    }
}
