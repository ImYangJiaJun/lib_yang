// 分块数据模型
// 定义运行时分块和流式加载相关的数据结构

use crate::model::geometry::RoomBounds;
use crate::model::room::RoomId;
use serde::{Deserialize, Serialize};

/// 分块标识符（统一定义在 `model::ChunkId`）
pub use super::ChunkId;

/// 分块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Chunk {
    /// 分块 ID
    pub id: ChunkId,
    /// 分块边界
    pub bounds: RoomBounds,
    /// 包含的房间 ID 列表
    pub room_ids: Vec<RoomId>,
    /// 依赖的分块 ID 列表
    pub dependencies: Vec<ChunkId>,
    /// 流式元数据
    pub streaming_metadata: StreamingMetadata,
}

/// 流式元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StreamingMetadata {
    /// Data Layer 名称(可选)
    pub data_layer: Option<String>,
    /// External Data Layer 名称(可选)
    pub external_data_layer: Option<String>,
    /// HLOD Layer 名称(可选)
    pub hlod_layer: Option<String>,
    /// 流式优先级(可选)
    pub streaming_priority: Option<i32>,
}
