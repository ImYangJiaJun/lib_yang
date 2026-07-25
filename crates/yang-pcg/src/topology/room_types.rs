// 房间类型分配

use crate::model::room::{Branch, BranchPurpose, Room, RoomId, RoomType};

const BRANCH_PURPOSES: [BranchPurpose; 4] = [
    BranchPurpose::Reward,
    BranchPurpose::Shop,
    BranchPurpose::Event,
    BranchPurpose::Secret,
];

/// 获取分支用途。
pub fn branch_purpose(index: usize) -> BranchPurpose {
    BRANCH_PURPOSES[index % BRANCH_PURPOSES.len()]
}

/// 为关键路径房间分配房间类型。
pub fn assign_critical_room_type(index: usize, critical_path_len: usize) -> RoomType {
    if index == 0 {
        RoomType::Start
    } else if index + 1 == critical_path_len {
        RoomType::Boss
    } else if index + 2 == critical_path_len {
        RoomType::Elite
    } else if index % 3 == 0 {
        RoomType::Puzzle
    } else {
        RoomType::Combat
    }
}

/// 为分支终点分配房间类型。
pub fn assign_branch_terminal_room_type(purpose: BranchPurpose) -> RoomType {
    match purpose {
        BranchPurpose::Reward => RoomType::Treasure,
        BranchPurpose::Shop => RoomType::Shop,
        BranchPurpose::Event => RoomType::Event,
        BranchPurpose::Secret => RoomType::Secret,
    }
}

/// 为房间设置与分支相关的房型信息。
pub fn apply_branch_room_types(rooms: &mut [Room], branches: &[Branch]) {
    for branch in branches {
        for (index, room_id) in branch.room_ids.iter().enumerate() {
            if let Some(room) = rooms.iter_mut().find(|room| room.id == *room_id) {
                room.branch_id = Some(branch.id.clone());
                if index + 1 == branch.room_ids.len() {
                    room.room_type = assign_branch_terminal_room_type(branch.purpose);
                } else if matches!(room.room_type, RoomType::Combat) && index % 2 == 1 {
                    room.room_type = RoomType::Elite;
                }
            }
        }
    }
}

/// 通过房间 ID 判断某个房间是否处于关键路径。
pub fn is_critical_room(room_id: &RoomId, critical_path: &[RoomId]) -> bool {
    critical_path
        .iter()
        .any(|critical_room_id| critical_room_id == room_id)
}
