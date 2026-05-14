// 排除区约束

use crate::model::geometry::GridPoint;
use crate::model::request::{Constraint, ExclusionZoneConstraint};
use crate::model::spawn::SpawnPoint;

/// 校验排除区约束。
pub fn validate_exclusion_constraints(constraints: &[Constraint]) -> crate::error::PcgResult<()> {
    for constraint in constraints {
        let Constraint::ExclusionZone(exclusion) = constraint else {
            continue;
        };
        if exclusion.min.x >= exclusion.max.x || exclusion.min.y >= exclusion.max.y {
            return Err(crate::error::PcgError::constraint("排除区范围非法"));
        }
    }
    Ok(())
}

/// 过滤落在排除区内的点位。
pub fn filter_spawns_by_exclusion(
    spawns: Vec<SpawnPoint>,
    constraints: &[Constraint],
) -> Vec<SpawnPoint> {
    let exclusion_zones: Vec<&ExclusionZoneConstraint> = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            Constraint::ExclusionZone(zone) if zone.exclude_spawns => Some(zone),
            _ => None,
        })
        .collect();

    if exclusion_zones.is_empty() {
        return spawns;
    }

    spawns
        .into_iter()
        .filter(|spawn| {
            exclusion_zones
                .iter()
                .all(|zone| !contains(zone, spawn.grid_pos))
        })
        .collect()
}

fn contains(zone: &ExclusionZoneConstraint, point: GridPoint) -> bool {
    point.x >= zone.min.x && point.x < zone.max.x && point.y >= zone.min.y && point.y < zone.max.y
}
