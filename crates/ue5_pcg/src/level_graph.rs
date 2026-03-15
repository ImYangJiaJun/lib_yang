// 生成关卡拓扑结构

/// 房间类型
/// Start       开始
/// Combat      战斗
/// Treasure    宝物
/// Shop        商店
/// Elite       精英怪
/// Puzzle      解密
/// Boss        boss房
#[derive(Clone, Copy, Debug)]
pub enum RoomType {
    Start,
    Combat,
    Treasure,
    Shop,
    Elite,
    Puzzle,
    Boss
}

/// 房间节点
/// id          房间id
/// room_type   房间类型
/// difficulty  难度
#[derive(Clone, Debug)]
pub struct RoomNode {
    pub id: usize,
    pub room_type: RoomType,
    pub difficulty: u8
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize
}

pub struct LevelGraph {
    pub nodes: Vec<RoomNode>,
    pub edges: Vec<Edge>
}

