// 约束求解模块
// 负责处理锚点、排除区、保留房间和模板引用

pub mod anchors;
pub mod exclusion;
pub mod templates;

use crate::error::PcgResult;
use crate::model::request::Constraint;
use crate::model::room::Room;
use crate::model::spawn::SpawnPoint;

/// 校验输入约束。
pub fn validate_constraints(constraints: &[Constraint]) -> PcgResult<()> {
    anchors::validate_anchor_constraints(constraints)?;
    exclusion::validate_exclusion_constraints(constraints)?;
    Ok(())
}

/// 将房间级约束应用到房间列表。
pub fn apply_room_constraints(rooms: &mut [Room], constraints: &[Constraint]) {
    anchors::apply_anchor_constraints(rooms, constraints);
    templates::apply_template_constraints(rooms, constraints);
}

/// 将点位级约束应用到交互物与敌人点位。
pub fn apply_spawn_constraints(
    item_spawns: Vec<SpawnPoint>,
    enemy_spawns: Vec<SpawnPoint>,
    constraints: &[Constraint],
) -> (Vec<SpawnPoint>, Vec<SpawnPoint>) {
    let item_spawns = exclusion::filter_spawns_by_exclusion(item_spawns, constraints);
    let enemy_spawns = exclusion::filter_spawns_by_exclusion(enemy_spawns, constraints);
    (item_spawns, enemy_spawns)
}
