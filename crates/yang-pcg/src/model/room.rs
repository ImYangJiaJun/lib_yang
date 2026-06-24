// 房间数据模型
// 定义房间的核心数据结构

use crate::model::geometry::RoomBounds;
use serde::{Deserialize, Serialize};

/// 房间标识符
pub type RoomId = String;

/// 分支标识符
pub type BranchId = String;

/// 房间边标识符
pub type RoomEdgeId = String;

/// 模板引用
pub type TemplateRef = String;

/// 房间
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Room {
    /// 房间 ID
    pub id: RoomId,
    /// 房间类型
    pub room_type: RoomType,
    /// 从起点的深度
    pub depth_from_start: u16,
    /// 所属分支 ID(可选)
    pub branch_id: Option<BranchId>,
    /// 难度等级
    pub difficulty: u16,
    /// 主题标签
    pub theme_tags: Vec<String>,
    /// 房间边界(可选,在布局阶段填充)
    pub bounds: Option<RoomBounds>,
    /// 模板引用(可选)
    pub template_ref: Option<TemplateRef>,
    /// Grammar 令牌(可选)
    ///
    /// 启用 Grammar 兼容模式时，用于标识房间对应的 Grammar 规则符号。
    /// 外部 Shape Grammar 或模块化拼接系统可根据此令牌选择具体模块资产。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar_token: Option<String>,
}

/// 房间类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RoomType {
    /// 起始房间
    Start,
    /// 战斗房间
    Combat,
    /// 宝藏房间
    Treasure,
    /// 商店房间
    Shop,
    /// 精英房间
    Elite,
    /// 谜题房间
    Puzzle,
    /// 安全房间
    Safe,
    /// Boss 房间
    Boss,
    /// 事件房间
    Event,
    /// 秘密房间
    Secret,
}

/// 房间图
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoomGraph {
    /// 房间节点列表
    pub nodes: Vec<Room>,
    /// 房间边列表
    pub edges: Vec<RoomEdge>,
    /// 关键路径(房间 ID 序列)
    pub critical_path: Vec<RoomId>,
    /// 分支列表
    pub branches: Vec<Branch>,
}

/// 房间边(拓扑连接)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoomEdge {
    /// 边 ID
    pub id: RoomEdgeId,
    /// 起始房间 ID
    pub from_room: RoomId,
    /// 目标房间 ID
    pub to_room: RoomId,
    /// 是否为关键路径的一部分
    pub is_critical: bool,
}

/// 分支
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Branch {
    /// 分支 ID
    pub id: BranchId,
    /// 分支起点房间 ID
    pub start_room: RoomId,
    /// 分支终点房间 ID
    pub end_room: RoomId,
    /// 分支房间 ID 列表
    pub room_ids: Vec<RoomId>,
    /// 分支目的(如 "reward", "shop", "event", "shortcut")
    pub purpose: String,
}

/// 门锚点标识符
pub type DoorAnchorId = String;

/// 门锚点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DoorAnchor {
    /// 锚点 ID
    pub id: DoorAnchorId,
    /// 所属房间 ID
    pub room_id: RoomId,
    /// 对应的边 ID
    pub edge_id: RoomEdgeId,
    /// 网格位置
    pub grid_pos: crate::model::geometry::GridPoint,
    /// 朝向
    pub facing: crate::model::geometry::CardinalDir,
    /// 宽度(瓦片数)
    pub width_tiles: u16,
    /// 插槽标签(可选)
    ///
    /// 启用 Grammar 兼容模式时，用于标识门锚点对应的模块插槽类型。
    /// 外部拼接系统可根据此标签匹配兼容的模块连接点。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_tag: Option<String>,
}

/// 走廊标识符
pub type CorridorId = String;

/// 走廊
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Corridor {
    /// 走廊 ID
    pub id: CorridorId,
    /// 起始房间 ID
    pub from_room: RoomId,
    /// 目标房间 ID
    pub to_room: RoomId,
    /// 起始锚点 ID
    pub from_anchor: DoorAnchorId,
    /// 目标锚点 ID
    pub to_anchor: DoorAnchorId,
    /// 宽度(瓦片数)
    pub width_tiles: u16,
    /// 路径
    pub path: CorridorPath,
    /// 分段标签列表
    ///
    /// 启用 Grammar 兼容模式时，标注走廊各段的长度、转折类型和主题。
    /// 外部 Grammar 系统可根据分段标签选择对应的走廊模块。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segment_tags: Vec<CorridorSegmentTag>,
}

/// 走廊路径
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CorridorPath {
    /// 直线路径
    Straight(Vec<crate::model::geometry::GridPoint>),
    /// 正交折线路径
    Orthogonal(Vec<crate::model::geometry::GridPoint>),
    /// 多段线路径
    Polyline(Vec<crate::model::geometry::GridPoint>),
}

/// 走廊分段标签
///
/// 描述走廊某一段的几何和主题属性，供外部 Grammar 系统选择对应模块。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CorridorSegmentTag {
    /// 段长度（瓦片数）
    pub length: u16,
    /// 转折类型
    pub turn_type: TurnType,
    /// 主题标签（可选）
    pub theme: Option<String>,
}

/// 走廊转折类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TurnType {
    /// 直行（无转折）
    Straight,
    /// 左转
    Left,
    /// 右转
    Right,
    /// T 形交叉
    TJunction,
    /// 十字交叉
    Cross,
}
