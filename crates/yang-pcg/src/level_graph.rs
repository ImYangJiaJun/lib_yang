// 生成关卡拓扑结构

/// 房间类型
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum RoomType {
    ///  开始
    Start,
    ///  战斗
    Combat,
    ///  宝物
    Treasure,
    ///  商店
    Shop,
    ///  精英怪
    Elite,
    ///  解密
    Puzzle,
    ///  boss房
    Boss,
}

#[derive(Clone, Debug)]
/// 房间节点
#[non_exhaustive]
pub struct RoomNode {
    ///  房间id
    pub id: usize,
    /// 房间类型
    pub room_type: RoomType,
    /// 难度
    pub difficulty: u8,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Edge {
    pub from: usize,
    pub to: usize,
}

#[non_exhaustive]
pub struct LevelGraph {
    pub nodes: Vec<RoomNode>,
    pub edges: Vec<Edge>,
}
