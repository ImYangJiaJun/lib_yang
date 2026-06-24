// 默认雕刻策略
// 将现有的 carve_room_terrain 逻辑封装为 TerrainStrategy 实现

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::Terrain;
use crate::rng::StableRng;

use super::carve::carve_room_terrain_with_config;
use super::strategy::TerrainStrategy;

/// 默认雕刻策略
///
/// 基于现有的 `carve_room_terrain` 逻辑实现，适用于大多数房间类型。
/// 生成标准的墙体边框、门口通行区、保留区和随机障碍物。
///
/// # 适用场景
///
/// - 战斗房间（Combat）
/// - 精英房间（Elite）
/// - 谜题房间（Puzzle）
/// - 事件房间（Event）
/// - 秘密房间（Secret）
/// - 其他未指定特殊策略的房间
#[non_exhaustive]
pub struct DefaultCarveStrategy;

impl TerrainStrategy for DefaultCarveStrategy {
    fn name(&self) -> &str {
        "default_carve"
    }

    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain> {
        carve_room_terrain_with_config(room, anchors, config, rng)
    }
}
