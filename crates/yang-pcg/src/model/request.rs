// 生成请求数据模型
// 定义地图生成请求的输入参数

use crate::config::GenerationConfig;
use crate::model::geometry::{GridPoint, WorldPoint};
use crate::model::room::RoomType;
use serde::{Deserialize, Serialize};

/// 单次地图生成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// 随机种子。
    ///
    /// - `Some(s)`：使用显式种子 `s`。相同 `s` + 相同 `config` 必产出相同地图。
    /// - `None`：从 `config` 派生**确定性**兜底种子（见 `ConfigDigest::seed_from_config`），
    ///   因此相同 `config` 即使不提供种子也会复现同一地图。想要不同结果时，请显式提供种子或修改配置。
    ///
    /// 注意：确定性保证在**同一生成模式内**成立；不同 `generation_mode` 即使同种子也会产出不同地图。
    pub seed: Option<u64>,
    /// 生成配置
    pub config: GenerationConfig,
    /// 约束列表
    pub constraints: Vec<Constraint>,
    /// 运行时上下文(可选)
    pub runtime_context: Option<RuntimeContext>,
    /// 追踪标识(可选)
    pub trace_id: Option<String>,
}

/// 运行时生成上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeContext {
    /// 关注位置(世界坐标)
    pub focus_position: Option<WorldPoint>,
    /// 兴趣半径
    pub interest_radius: Option<f32>,
    /// 请求的分块 ID 列表
    pub requested_chunks: Vec<ChunkId>,
    /// 调用方标签
    pub caller_tag: Option<String>,
}

/// 分块标识符
pub type ChunkId = String;

/// 约束类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    /// 锚点约束
    Anchor(AnchorConstraint),
    /// 排除区约束
    ExclusionZone(ExclusionZoneConstraint),
    /// 模板引用约束
    Template(TemplateConstraint),
}

/// 锚点约束
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorConstraint {
    /// 约束标签
    pub label: String,
    /// 指定房间 ID（可选）
    pub room_id: Option<String>,
    /// 指定房间类型（可选）
    pub room_type: Option<RoomType>,
    /// 目标逻辑坐标（可选）
    pub target_grid_pos: Option<GridPoint>,
}

/// 排除区约束
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExclusionZoneConstraint {
    /// 约束标签
    pub label: String,
    /// 最小网格坐标（包含）
    pub min: GridPoint,
    /// 最大网格坐标（不包含）
    pub max: GridPoint,
    /// 是否排除房间
    pub exclude_rooms: bool,
    /// 是否排除点位
    pub exclude_spawns: bool,
}

/// 模板引用约束
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConstraint {
    /// 指定房间 ID（可选）
    pub room_id: Option<String>,
    /// 指定房间类型（可选）
    pub room_type: Option<RoomType>,
    /// 模板引用
    pub template_ref: String,
}
