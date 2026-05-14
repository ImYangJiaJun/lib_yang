// 生成结果数据模型
// 定义地图生成的输出结果

use crate::debug::DebugBundle;
use crate::model::chunk::Chunk;
use crate::model::room::{Corridor, DoorAnchor, Room, RoomGraph};
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::Terrain;
use serde::{Deserialize, Serialize};

/// 单次地图生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    /// 结果元数据
    pub metadata: ResultMetadata,
    /// 拓扑图
    pub topology: RoomGraph,
    /// 房间列表
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

/// 结果元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
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
