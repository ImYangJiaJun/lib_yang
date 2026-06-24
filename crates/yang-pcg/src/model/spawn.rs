// 点位数据模型
// 定义交互物和敌人点位的数据结构

use crate::model::geometry::{GridPoint, Transform3};
use crate::model::room::RoomId;
use serde::{Deserialize, Serialize};

/// 点位标识符
pub type SpawnPointId = String;

/// 点位
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpawnPoint {
    /// 点位 ID
    pub id: SpawnPointId,
    /// 所属房间 ID
    pub room_id: RoomId,
    /// 点位类型
    pub kind: SpawnKind,
    /// 网格位置
    pub grid_pos: GridPoint,
    /// 世界变换(可选)
    pub world_transform: Option<Transform3>,
    /// 点位元数据
    pub metadata: SpawnMetadata,
}

/// 点位类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SpawnKind {
    /// 交互物
    Item,
    /// 敌人
    Enemy,
    /// Boss
    Boss,
    /// 奖励
    Reward,
    /// 交互对象
    Interaction,
}

/// 点位元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpawnMetadata {
    /// 生成标签
    pub spawn_tag: String,
    /// 稀有度等级(可选)
    pub rarity_tier: Option<u8>,
    /// 敌人池标签(可选)
    pub enemy_pool_tag: Option<String>,
    /// 遭遇 ID(可选)
    pub encounter_id: Option<String>,
    /// 波次 ID(可选)
    pub wave_id: Option<String>,
    /// 难度(可选)
    pub difficulty: Option<u16>,
    /// 随机种子
    pub seed: u64,
}
