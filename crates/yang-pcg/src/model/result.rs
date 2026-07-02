// 生成结果数据模型
// 定义地图生成的输出结果

use crate::debug::DebugBundle;
use crate::digest::ConfigDigest;
use crate::model::chunk::Chunk;
use crate::model::room::{Corridor, DoorAnchor, Room, RoomGraph};
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use serde::{Deserialize, Serialize};

/// 单次地图生成结果
///
/// # `rooms` 与 `topology.nodes` 的关系
///
/// 两者存储同一批房间的不同状态：
/// - `topology.nodes`：拓扑阶段产物，`bounds` 为 `None`（尚未布局）；在分块模式下
///   可能包含当前切片未涉及的房间（全层拓扑图的子集或全集，视模式而定）。
/// - `rooms`：布局阶段之后的产物，`bounds` 为 `Some(...)`；在整图模式下数量等于
///   `topology.nodes`，在分块模式下严格少于 `topology.nodes`（仅含当前切片已布局房间）。
///
/// 需要按 ID 查找房间时，**优先使用 [`GenerationResult::room_by_id`]**
/// （搜索 `rooms`，保证 bounds 已填充）；仅在拓扑遍历场景下才直接访问
/// `topology.nodes`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GenerationResult {
    /// 结果元数据
    pub metadata: ResultMetadata,
    /// 拓扑图
    ///
    /// `nodes` 包含拓扑阶段确定的房间（`bounds` 为 `None`，布局后由 [`Self::rooms`]
    /// 提供带边界版本）。分块模式下 `nodes` 可能包含未在当前切片布局的房间。
    /// 详见结构体级文档。
    pub topology: RoomGraph,
    /// 房间列表（布局阶段之后，`bounds` 已填充）
    ///
    /// 整图模式下数量等于 `topology.nodes`；分块模式下严格少于 `topology.nodes`。
    /// 按 ID 查找房间请使用 [`Self::room_by_id`]。
    pub rooms: Vec<Room>,
    /// 门锚点列表
    pub door_anchors: Vec<DoorAnchor>,
    /// 走廊列表
    pub corridors: Vec<Corridor>,
    /// 地形列表
    pub terrains: Vec<Terrain>,
    /// 交互物点位列表
    pub item_spawns: Vec<SpawnPoint>,
    /// 敌人点位列表
    pub enemy_spawns: Vec<SpawnPoint>,
    /// 分块列表
    pub chunks: Vec<Chunk>,
    /// 调试信息(可选)
    pub debug: Option<DebugBundle>,
}

impl GenerationResult {
    /// 按 ID 查找房间
    ///
    /// 在 `rooms`（布局后的完整房间列表）中按 ID 线性查找。
    /// 返回的 `Room` 保证 `bounds` 已填充（整图和分块模式均如此）。
    ///
    /// 如果需要遍历拓扑结构（含未布局房间），请直接使用 `topology.nodes`。
    pub fn room_by_id(&self, id: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }
}

/// 结果元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResultMetadata {
    /// 使用的随机种子
    pub seed: u64,
    /// 配置摘要
    pub config_digest: String,
    /// 数据模式版本
    pub schema_version: String,
    /// 算法版本
    pub algorithm_version: String,
    /// 目标引擎版本(可选)
    pub target_engine_version: Option<String>,
    /// 追踪标识(可选)
    pub trace_id: Option<String>,
}

impl ResultMetadata {
    /// 返回类型化的配置摘要
    ///
    /// 将内部存储的 `String` 转换为 [`ConfigDigest`]，可直接调用
    /// [`ConfigDigest::matches`] 等类型安全方法。
    pub fn config_digest_typed(&self) -> ConfigDigest {
        ConfigDigest::from_string(self.config_digest.clone())
    }
}
