// 生成请求数据模型
// 定义地图生成请求的输入参数

use crate::config::GenerationConfig;
use crate::model::geometry::{GridPoint, WorldPoint};
use crate::model::room::RoomType;
use serde::{Deserialize, Serialize};

/// 单次地图生成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// 随机种子(可选,未提供时自动生成)
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
