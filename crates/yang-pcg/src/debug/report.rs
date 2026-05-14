// 调试报告生成

use serde::{Deserialize, Serialize};

use super::profiling::StageStat;
use crate::model::geometry::GridPoint;
use crate::validation::ValidationReport;

/// 单个被拒绝点位的原因记录。
///
/// 记录在点位生成阶段中被拒绝的候选点位及其拒绝原因，
/// 用于调试和参数调优。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
/// - 需求 15.5: 失败阶段与失败约束输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionReason {
    /// 被拒绝的候选点位坐标
    pub position: GridPoint,
    /// 拒绝原因描述
    pub reason: String,
}

/// 点位生成调试信息。
///
/// 记录点位生成阶段的候选点位数、拒绝数、拒绝原因和最终接受数，
/// 用于分析点位采样效率和约束影响。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
/// - 需求 15.5: 失败阶段与失败约束输出
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpawnDebugInfo {
    /// 候选点位总数
    pub candidate_count: usize,
    /// 被拒绝的候选点位数
    pub rejected_count: usize,
    /// 每个被拒绝点位的原因列表
    pub rejection_reasons: Vec<RejectionReason>,
    /// 最终接受的点位数
    pub accepted_count: usize,
}

/// 调试通道数据。
///
/// 包含生成过程中的关键路径节点、门锚点坐标、走廊中心线和被拒绝房间等调试信息，
/// 用于可视化和问题排查。
///
/// # 需求映射
/// - 需求 15.1: 调试通道输出
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebugChannels {
    /// 关键路径上的房间 ID 列表
    pub critical_path_nodes: Vec<String>,
    /// 所有门锚点的网格坐标
    pub door_anchor_positions: Vec<GridPoint>,
    /// 每条走廊的中心线点序列
    pub corridor_centerlines: Vec<Vec<GridPoint>>,
    /// 生成过程中被拒绝的房间 ID 列表
    pub rejected_rooms: Vec<String>,
    /// 点位生成调试信息（可选）
    ///
    /// 记录候选点位数、拒绝数、拒绝原因和接受数，
    /// 仅在调试模式下填充。
    ///
    /// # 需求映射
    /// - 需求 15.3: 输出被拒绝点位
    /// - 需求 15.5: 失败阶段与失败约束输出
    pub spawn_debug: Option<SpawnDebugInfo>,
}

/// 调试信息包。
///
/// 包含生成过程中的阶段统计、备注信息、约束验证报告和调试通道数据。
/// `trace_id` 字段用于串联日志、缓存与导出结果，实现全链路追踪。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebugBundle {
    /// 追踪标识（可选）
    ///
    /// 从 `GenerationRequest.trace_id` 传入，贯穿整个生成流程，
    /// 用于串联日志、缓存键与导出元数据。
    ///
    /// # 需求映射
    /// - 需求 15.6: 支持为单次生成附加追踪标识
    pub trace_id: Option<String>,
    /// 各阶段统计信息
    pub stage_stats: Vec<StageStat>,
    /// 备注信息
    pub notes: Vec<String>,
    /// 约束验证报告（可选）
    ///
    /// 包含各不变量检查的通过/失败状态，仅在调试模式下生成。
    ///
    /// # 需求映射
    /// - 需求 6.6: 约束验证报告
    /// - 需求 15.3: 输出约束验证报告
    pub validation_report: Option<ValidationReport>,
    /// 调试通道数据（可选）
    ///
    /// 包含关键路径节点、门锚点坐标、走廊中心线、被拒绝房间等信息，
    /// 仅在调试模式下填充。
    ///
    /// # 需求映射
    /// - 需求 15.1: 调试通道输出
    pub debug_channels: Option<DebugChannels>,
}
