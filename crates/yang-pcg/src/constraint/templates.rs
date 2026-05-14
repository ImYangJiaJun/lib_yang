// 模板引用

use crate::model::request::{Constraint, TemplateConstraint};
use crate::model::room::Room;

/// 应用模板引用约束。
pub fn apply_template_constraints(rooms: &mut [Room], constraints: &[Constraint]) {
    for constraint in constraints {
        let Constraint::Template(TemplateConstraint {
            room_id,
            room_type,
            template_ref,
        }) = constraint
        else {
            continue;
        };

        if let Some(target_room_id) = room_id {
            if let Some(room) = rooms.iter_mut().find(|room| room.id == *target_room_id) {
                room.template_ref = Some(template_ref.clone());
                continue;
            }
        }

        if let Some(target_room_type) = room_type {
            if let Some(room) = rooms
                .iter_mut()
                .find(|room| room.room_type == *target_room_type && room.template_ref.is_none())
            {
                room.template_ref = Some(template_ref.clone());
            }
        }
    }
}
