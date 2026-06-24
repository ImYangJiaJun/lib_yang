// Named Channel 定义

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::geometry::WorldPoint;

use super::points::{PcgPoint, PropertyValue};

/// 简化的折线表示。
pub type Polyline3 = Vec<WorldPoint>;

/// 通道类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ChannelKind {
    Rooms,
    Doors,
    Corridors,
    FloorTiles,
    WallTiles,
    ItemSpawns,
    EnemySpawns,
    Debug,
}

/// 具名通道。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NamedChannel {
    pub name: String,
    pub kind: ChannelKind,
    pub points: Vec<PcgPoint>,
    pub polylines: Vec<Polyline3>,
    pub metadata: BTreeMap<String, PropertyValue>,
}
