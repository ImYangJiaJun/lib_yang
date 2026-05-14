// 锚点约束

use crate::error::{PcgError, PcgResult};
use crate::model::request::{AnchorConstraint, Constraint};
use crate::model::room::Room;

/// 校验锚点约束。
pub fn validate_anchor_constraints(constraints: &[Constraint]) -> PcgResult<()> {
    for constraint in constraints {
        let Constraint::Anchor(anchor) = constraint else {
            continue;
        };

        if anchor.room_id.is_none() && anchor.room_type.is_none() {
            return Err(PcgError::constraint(
                "锚点约束必须至少指定 room_id 或 room_type",
            ));
        }
    }
    Ok(())
}

/// 将锚点约束应用到房间列表。
///
/// 当前 MVP 只处理“按房间类型固定房型”这一类轻量约束；
/// 坐标级约束会在后续布局阶段增强。
pub fn apply_anchor_constraints(rooms: &mut [Room], constraints: &[Constraint]) {
    for constraint in constraints {
        let Constraint::Anchor(AnchorConstraint {
            room_id, room_type, ..
        }) = constraint
        else {
            continue;
        };

        if let (Some(target_room_id), Some(target_room_type)) = (room_id, room_type) {
            if let Some(room) = rooms.iter_mut().find(|room| room.id == *target_room_id) {
                room.room_type = *target_room_type;
            }
        }
    }
}
