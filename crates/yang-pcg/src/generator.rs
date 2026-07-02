// 核心生成器模块
// 负责编排整个地图生成流程

use crate::backend::{select_backend, ValidationScope};
use crate::debug::{elapsed_ms, stage_stat, stage_stat_timed, DebugBundle, DebugChannels};
use crate::digest::ConfigDigest;
use crate::error::{PcgError, PcgResult};
use crate::export::CURRENT_SCHEMA_VERSION;
use crate::model::request::GenerationRequest;
use crate::model::result::{GenerationResult, ResultMetadata};
use crate::model::room::CorridorPath;
use crate::rng::StableRng;
use crate::spawn::SpawnOutput;
use crate::validation::{run_full_validation, validate_request};
use crate::{chunked, constraint, topology, ue};
use std::time::Instant;

use crate::chunked::{ChunkDetailResult, TopologyResult};

/// 地图生成器
#[non_exhaustive]
pub struct MapGenerator {
    debug_enabled: bool,
}

impl MapGenerator {
    /// 创建新的地图生成器
    pub fn new() -> Self {
        Self {
            debug_enabled: false,
        }
    }

    /// 启用或禁用调试输出。
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_enabled = enabled;
    }

    /// 返回当前是否启用了调试输出。
    pub fn debug_enabled(&self) -> bool {
        self.debug_enabled
    }

    /// 生成地图
    pub fn generate(&self, request: GenerationRequest) -> PcgResult<GenerationResult> {
        // 如果是 RuntimeChunked 模式，委托给分块生成逻辑
        if request.config.generation_mode == crate::config::GenerationMode::RuntimeChunked {
            return self.generate_chunk(request);
        }

        // HybridPrecompute 模式必须通过 generate_topology_only() + fill_chunk_details() 两阶段调用
        if request.config.generation_mode == crate::config::GenerationMode::HybridPrecompute {
            return Err(PcgError::config(
                "HybridPrecompute须经generate_topology_only()+fill_chunk_details()两阶段调用",
            ));
        }

        let normalized = validate_request(&request)?;
        // seed 缺省时从配置派生确定性种子（而非系统时间），保证「相同 config 必产同图」。
        let seed = match request.seed {
            Some(s) => s,
            None => ConfigDigest::seed_from_config(&normalized.config)?,
        };
        let root_rng = StableRng::from_seed(seed);
        let config_digest = ConfigDigest::from_config(&normalized.config)?.into_string();

        constraint::validate_constraints(&request.constraints)?;

        // 选择管线 backend（本迭代恒为俯视角；编排代码与具体 backend 解耦）
        let backend = select_backend(&normalized);

        // 拓扑阶段
        let topology_start = self.debug_enabled.then(Instant::now);
        let mut topology_rng = root_rng.derive("topology");
        let mut graph = topology::generate_topology(&normalized, &mut topology_rng)?;
        constraint::apply_room_constraints(&mut graph.nodes, &request.constraints);
        let topology_ms = topology_start.map(elapsed_ms);

        // 布局阶段
        let layout_start = self.debug_enabled.then(Instant::now);
        let mut layout_rng = root_rng.derive("layout");
        let layout_output = backend.solve_layout(&graph, &normalized, &mut layout_rng)?;
        let layout_ms = layout_start.map(elapsed_ms);

        // 地形阶段
        let terrain_start = self.debug_enabled.then(Instant::now);
        let mut terrain_rng = root_rng.derive("terrain");
        let terrains = backend.generate_terrains(
            &layout_output.rooms,
            &layout_output.door_anchors,
            &normalized,
            &mut terrain_rng,
        )?;
        let terrain_ms = terrain_start.map(elapsed_ms);

        // 点位阶段
        let spawn_start = self.debug_enabled.then(Instant::now);
        let mut spawn_rng = root_rng.derive("spawn");
        // 调试模式下使用带跟踪的点位生成，记录候选数和拒绝原因
        let (item_spawns, enemy_spawns, spawn_debug_info) = if self.debug_enabled {
            let spawn_result = backend.generate_spawns_with_debug(
                &layout_output.rooms,
                &terrains,
                &normalized,
                &mut spawn_rng,
            )?;
            let (items, enemies) = constraint::apply_spawn_constraints(
                spawn_result.output.item_spawns,
                spawn_result.output.enemy_spawns,
                &request.constraints,
            );
            (items, enemies, Some(spawn_result.debug_info))
        } else {
            let SpawnOutput {
                item_spawns,
                enemy_spawns,
            } = backend.generate_spawns(
                &layout_output.rooms,
                &terrains,
                &normalized,
                &mut spawn_rng,
            )?;
            let (items, enemies) = constraint::apply_spawn_constraints(
                item_spawns,
                enemy_spawns,
                &request.constraints,
            );
            (items, enemies, None)
        };
        let spawn_ms = spawn_start.map(elapsed_ms);

        let chunks = ue::streaming::build_chunks(&layout_output.rooms, &normalized);

        let debug = self.debug_enabled.then(|| {
            // 构建调试通道数据
            let critical_path_nodes = graph.critical_path.clone();
            let door_anchor_positions = layout_output
                .door_anchors
                .iter()
                .map(|anchor| anchor.grid_pos)
                .collect();
            let corridor_centerlines = layout_output
                .corridors
                .iter()
                .map(|corridor| match &corridor.path {
                    CorridorPath::Straight(pts) => pts.clone(),
                    CorridorPath::Orthogonal(pts) => pts.clone(),
                    CorridorPath::Polyline(pts) => pts.clone(),
                })
                .collect();

            let debug_channels = DebugChannels {
                critical_path_nodes,
                door_anchor_positions,
                corridor_centerlines,
                // 当前流程中没有被拒绝的房间，预留空列表
                rejected_rooms: Vec::new(),
                // 填充点位生成调试信息
                spawn_debug: spawn_debug_info.clone(),
            };

            // 使用各阶段记录的实际耗时构建统计
            DebugBundle {
                trace_id: request.trace_id.clone(),
                stage_stats: vec![
                    stage_stat_timed("topology", graph.nodes.len(), topology_ms.unwrap_or(0)),
                    stage_stat_timed("layout", layout_output.rooms.len(), layout_ms.unwrap_or(0)),
                    stage_stat_timed("terrain", terrains.len(), terrain_ms.unwrap_or(0)),
                    stage_stat_timed("spawn_items", item_spawns.len(), spawn_ms.unwrap_or(0)),
                    stage_stat("spawn_enemies", enemy_spawns.len()),
                ],
                notes: vec!["MVP 生成流程已执行".to_string()],
                validation_report: None, // 将在结果组装后填充
                debug_channels: Some(debug_channels),
            }
        });

        let result = GenerationResult {
            metadata: ResultMetadata {
                seed,
                config_digest,
                schema_version: CURRENT_SCHEMA_VERSION.to_string(),
                algorithm_version: env!("CARGO_PKG_VERSION").to_string(),
                target_engine_version: None,
                trace_id: request.trace_id,
            },
            topology: graph,
            rooms: layout_output.rooms,
            door_anchors: layout_output.door_anchors,
            corridors: layout_output.corridors,
            terrains,
            item_spawns,
            enemy_spawns,
            chunks,
            debug,
        };

        // 全量硬校验进生产路径：结构一致性 + 可达 + 无重叠 + 地形连通 + 点位间距。
        // 失败返回 Err（不静默放行）。
        backend.validate(
            &result,
            &normalized,
            &request.constraints,
            ValidationScope::FullFloor,
        )?;

        // 在调试模式下，生成并填充验证报告
        let mut result = result;
        if result.debug.is_some() {
            let report = run_full_validation(&result, &request.constraints, None);
            if let Some(ref mut debug_bundle) = result.debug {
                debug_bundle.validation_report = Some(report);
            }
        }

        Ok(result)
    }
}

impl Default for MapGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// === 运行时分块生成方法 ===
impl MapGenerator {
    /// RuntimeChunked 模式的增量生成
    ///
    /// 仅生成请求范围内的房间细节和点位，复用已有拓扑结果。
    /// 支持时间预算和迭代预算限制。
    ///
    /// # 参数
    /// - `request`: 生成请求（必须包含 runtime_context）
    ///
    /// # 返回
    /// - `Ok(GenerationResult)`: 仅包含请求分块内容的生成结果
    pub fn generate_chunk(&self, request: GenerationRequest) -> PcgResult<GenerationResult> {
        // 模式守卫: generate_chunk() 仅适用于 RuntimeChunked 模式
        if request.config.generation_mode != crate::config::GenerationMode::RuntimeChunked {
            return Err(PcgError::config(
                "generate_chunk()仅适用于RuntimeChunked模式",
            ));
        }
        chunked::generate_chunk(request)
    }

    /// HybridPrecompute 模式第一阶段：仅生成拓扑和布局
    ///
    /// 生成完整的楼层拓扑图和空间布局，但不填充房间内部细节。
    /// 返回的 `TopologyResult` 可用于后续按需调用 `fill_chunk_details`。
    ///
    /// # 参数
    /// - `request`: 生成请求
    ///
    /// # 返回
    /// - `Ok(TopologyResult)`: 拓扑和布局预计算结果
    pub fn generate_topology_only(&self, request: GenerationRequest) -> PcgResult<TopologyResult> {
        chunked::generate_topology_only(request)
    }

    /// HybridPrecompute 模式第二阶段：按需填充分块细节
    ///
    /// 根据已有的拓扑结果，仅为指定分块内的房间生成地形和点位。
    /// 支持时间预算和迭代预算限制。
    ///
    /// # 参数
    /// - `topology_result`: 拓扑预计算结果
    /// - `chunk_id`: 要填充的分块 ID
    ///
    /// # 返回
    /// - `Ok(ChunkDetailResult)`: 分块细节
    pub fn fill_chunk_details(
        &self,
        topology_result: &TopologyResult,
        chunk_id: &str,
    ) -> PcgResult<ChunkDetailResult> {
        chunked::fill_chunk_details(topology_result, chunk_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;

    #[test]
    fn test_generate_satisfies_all_invariants_across_seeds() {
        // 验证需求 4.7/5.4/7.4/8.3 - 算法修复后，默认配置生成确定性满足全部硬不变量。
        // generate() 已无条件接入硬校验，成功本身即证明不变量满足；这里再用
        // run_full_validation 显式断言报告全绿，作为更直接的回归护栏。
        let generator = MapGenerator::new();
        for seed in [42u64, 12345, 7, 99, 256, 1024] {
            let result = generator
                .generate(GenerationRequest {
                    seed: Some(seed),
                    config: GenerationConfig::default(),
                    constraints: vec![],
                    runtime_context: None,
                    trace_id: None,
                })
                .unwrap_or_else(|e| panic!("seed {seed} 生成应成功并通过硬校验: {e}"));
            let report = crate::validation::run_full_validation(&result, &[], None);
            assert!(
                report.all_passed,
                "seed {seed} 应满足全部不变量，失败项: {:?}",
                report
                    .items()
                    .iter()
                    .filter(|i| !i.passed)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_generate_with_none_seed_is_deterministic() {
        // seed:None 现在从 config 派生确定性兜底种子，相同 config 必产同图（字节级一致）。
        let generator = MapGenerator::new();
        let make_request = || GenerationRequest {
            seed: None,
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        };
        let a = generator.generate(make_request()).expect("生成应成功");
        let b = generator.generate(make_request()).expect("生成应成功");
        let json_a = crate::export::export_json(&a).expect("导出应成功");
        let json_b = crate::export::export_json(&b).expect("导出应成功");
        assert_eq!(json_a, json_b, "seed:None 应从配置派生确定性结果");
    }

    #[test]
    fn test_generate_returns_non_empty_result() {
        let generator = MapGenerator::new();
        let config = GenerationConfig::default();
        let expected_digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化").into_string();

        let result = generator
            .generate(GenerationRequest {
                seed: Some(12345),
                config,
                constraints: vec![],
                runtime_context: None,
                trace_id: Some("trace-001".to_string()),
            })
            .expect("最小生成流程应该成功");

        assert_eq!(result.metadata.seed, 12345);
        assert_eq!(result.metadata.config_digest, expected_digest);
        assert_eq!(result.metadata.algorithm_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(result.metadata.trace_id.as_deref(), Some("trace-001"));
        assert!(!result.rooms.is_empty());
        assert!(!result.door_anchors.is_empty());
        assert!(!result.corridors.is_empty());
        assert!(!result.terrains.is_empty());
        assert!(!result.chunks.is_empty());
        assert!(result.debug.is_none());
    }

    #[test]
    fn test_generate_includes_debug_bundle_when_enabled() {
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(7),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("启用调试不应影响最小生成流程");

        assert!(generator.debug_enabled());
        assert!(result.debug.is_some());
        assert!(!result.rooms.is_empty());
    }

    #[test]
    fn test_generate_debug_records_stage_durations() {
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("调试模式生成应成功");

        let debug = result.debug.expect("调试模式应包含 DebugBundle");
        let stats = &debug.stage_stats;

        // 验证各阶段统计存在
        assert_eq!(stats.len(), 5);
        assert_eq!(stats[0].stage_name, "topology");
        assert_eq!(stats[1].stage_name, "layout");
        assert_eq!(stats[2].stage_name, "terrain");
        assert_eq!(stats[3].stage_name, "spawn_items");
        assert_eq!(stats[4].stage_name, "spawn_enemies");

        // 验证产出数量大于零
        assert!(stats[0].produced_count > 0, "拓扑阶段应有产出");
        assert!(stats[1].produced_count > 0, "布局阶段应有产出");
        assert!(stats[2].produced_count > 0, "地形阶段应有产出");

        // 注意：duration_ms 可能为 0（如果阶段执行极快），
        // 但至少不应该是负数或异常值，这里只验证字段已被设置
        // 在实际运行中，至少有一个阶段的耗时应大于 0
        let total_ms: u64 = stats.iter().map(|s| s.duration_ms).sum();
        // 总耗时应该是合理的（不超过 60 秒）
        assert!(total_ms < 60_000, "总耗时应在合理范围内");
    }

    #[test]
    fn test_generate_no_debug_has_zero_overhead() {
        // 非调试模式不应有 debug 输出
        let generator = MapGenerator::new();

        let result = generator
            .generate(GenerationRequest {
                seed: Some(99),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("非调试模式生成应成功");

        assert!(result.debug.is_none(), "非调试模式不应包含 DebugBundle");
    }

    #[test]
    fn test_generate_debug_channels_populated() {
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("调试模式生成应成功");

        let debug = result.debug.expect("调试模式应包含 DebugBundle");
        let channels = debug.debug_channels.expect("调试模式应包含 debug_channels");

        // 验证关键路径节点非空
        assert!(
            !channels.critical_path_nodes.is_empty(),
            "关键路径节点不应为空"
        );

        // 验证门锚点坐标非空
        assert!(
            !channels.door_anchor_positions.is_empty(),
            "门锚点坐标不应为空"
        );

        // 验证走廊中心线非空
        assert!(
            !channels.corridor_centerlines.is_empty(),
            "走廊中心线不应为空"
        );

        // 验证每条走廊中心线至少有两个点
        for (i, centerline) in channels.corridor_centerlines.iter().enumerate() {
            assert!(
                centerline.len() >= 2,
                "走廊 {} 的中心线应至少有 2 个点，实际有 {} 个",
                i,
                centerline.len()
            );
        }

        // 验证关键路径节点与拓扑一致
        assert_eq!(
            channels.critical_path_nodes, result.topology.critical_path,
            "调试通道的关键路径应与拓扑结果一致"
        );

        // 验证门锚点数量与实际门锚点一致
        assert_eq!(
            channels.door_anchor_positions.len(),
            result.door_anchors.len(),
            "调试通道的门锚点数量应与实际结果一致"
        );

        // 验证走廊中心线数量与实际走廊数量一致
        assert_eq!(
            channels.corridor_centerlines.len(),
            result.corridors.len(),
            "调试通道的走廊中心线数量应与实际结果一致"
        );
    }

    #[test]
    fn test_generate_debug_spawn_debug_info_populated() {
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("调试模式生成应成功");

        let debug = result.debug.expect("调试模式应包含 DebugBundle");
        let channels = debug.debug_channels.expect("调试模式应包含 debug_channels");
        let spawn_debug = channels.spawn_debug.expect("调试模式应包含 spawn_debug");

        // 验证候选点位数大于零（至少有一个房间产生了候选点）
        assert!(
            spawn_debug.candidate_count > 0,
            "候选点位数应大于 0，实际为 {}",
            spawn_debug.candidate_count
        );

        // 验证接受数大于零
        assert!(
            spawn_debug.accepted_count > 0,
            "接受点位数应大于 0，实际为 {}",
            spawn_debug.accepted_count
        );

        // 验证接受数 + 拒绝数 <= 候选数
        assert!(
            spawn_debug.accepted_count + spawn_debug.rejected_count <= spawn_debug.candidate_count,
            "接受数({}) + 拒绝数({}) 应不超过候选数({})",
            spawn_debug.accepted_count,
            spawn_debug.rejected_count,
            spawn_debug.candidate_count
        );

        // 验证拒绝原因列表长度与拒绝数一致
        assert_eq!(
            spawn_debug.rejection_reasons.len(),
            spawn_debug.rejected_count,
            "拒绝原因列表长度应与拒绝数一致"
        );

        // 验证每个拒绝原因都有非空的描述
        for reason in &spawn_debug.rejection_reasons {
            assert!(!reason.reason.is_empty(), "拒绝原因描述不应为空");
        }

        // 验证接受数与实际生成的点位数一致
        let total_spawns = result.item_spawns.len() + result.enemy_spawns.len();
        assert_eq!(
            spawn_debug.accepted_count, total_spawns,
            "接受数({}) 应与实际点位数({}) 一致",
            spawn_debug.accepted_count, total_spawns
        );
    }

    #[test]
    fn test_generate_no_debug_has_no_spawn_debug() {
        // 非调试模式不应有 spawn_debug 信息
        let generator = MapGenerator::new();

        let result = generator
            .generate(GenerationRequest {
                seed: Some(99),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("非调试模式生成应成功");

        assert!(result.debug.is_none(), "非调试模式不应包含 DebugBundle");
    }

    #[test]
    fn test_trace_id_propagation_through_pipeline() {
        // 验证 trace_id 从请求贯穿到结果元数据、调试包和导出通道
        let trace = "trace-propagation-test-001".to_string();
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: Some(trace.clone()),
            })
            .expect("生成应成功");

        // 1. 验证 trace_id 传播到 ResultMetadata
        assert_eq!(
            result.metadata.trace_id.as_deref(),
            Some(trace.as_str()),
            "trace_id 应传播到结果元数据"
        );

        // 2. 验证 trace_id 传播到 DebugBundle
        let debug = result.debug.as_ref().expect("调试模式应包含 DebugBundle");
        assert_eq!(
            debug.trace_id.as_deref(),
            Some(trace.as_str()),
            "trace_id 应传播到 DebugBundle"
        );

        // 3. 验证 trace_id 传播到导出通道元数据
        let channels = crate::ue::adapter::export_named_channels(&result).expect("导出应成功");
        for channel in &channels {
            let channel_trace = channel.metadata.get("trace_id");
            assert_eq!(
                channel_trace,
                Some(&crate::ue::points::PropertyValue::String(trace.clone())),
                "通道 '{}' 的元数据应包含 trace_id",
                channel.name
            );
        }
    }

    #[test]
    fn test_trace_id_none_does_not_pollute_export() {
        // 验证当 trace_id 为 None 时，导出通道不包含 trace_id 元数据
        let generator = MapGenerator::new();

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("生成应成功");

        assert!(result.metadata.trace_id.is_none());

        let channels = crate::ue::adapter::export_named_channels(&result).expect("导出应成功");
        for channel in &channels {
            assert!(
                !channel.metadata.contains_key("trace_id"),
                "通道 '{}' 在 trace_id 为 None 时不应包含 trace_id 元数据",
                channel.name
            );
        }
    }

    #[test]
    fn test_generate_debug_validation_report_populated() {
        // 验证约束验证报告在调试模式下被正确填充
        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let result = generator
            .generate(GenerationRequest {
                seed: Some(42),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("调试模式生成应成功");

        let debug = result.debug.expect("调试模式应包含 DebugBundle");
        let report = debug.validation_report.expect("调试模式应包含约束验证报告");

        // 验证报告包含验证项
        assert!(
            !report.items().is_empty(),
            "约束验证报告应包含至少一个验证项"
        );

        // 验证报告结构完整（passed_count + failed_count = 总验证项数）
        assert_eq!(
            report.passed_count + report.failed_count,
            report.items().len(),
            "通过数 + 失败数应等于总验证项数"
        );
    }
}
