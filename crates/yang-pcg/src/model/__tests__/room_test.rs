// 房间数据模型测试
// 验证需求: 3.1, 3.4, 3.5, 4.2, 4.3

use crate::model::geometry::{CardinalDir, GridPoint};
use crate::model::room::*;

#[test]
fn test_room_type_variants() {
    // 测试房间类型枚举所有变体
    let types = vec![
        RoomType::Start,
        RoomType::Combat,
        RoomType::Treasure,
        RoomType::Shop,
        RoomType::Elite,
        RoomType::Puzzle,
        RoomType::Safe,
        RoomType::Boss,
        RoomType::Event,
        RoomType::Secret,
    ];

    assert_eq!(types.len(), 10);
    assert_eq!(types[0], RoomType::Start);
    assert_eq!(types[7], RoomType::Boss);
}

#[test]
fn test_room_creation() {
    // 测试房间创建
    let room = Room {
        id: "room-001".to_string(),
        room_type: RoomType::Combat,
        depth_from_start: 3,
        branch_id: None,
        difficulty: 5,
        theme_tags: vec!["dungeon".to_string(), "dark".to_string()],
        bounds: None,
        template_ref: None,
        grammar_token: None,
    };

    assert_eq!(room.id, "room-001");
    assert_eq!(room.room_type, RoomType::Combat);
    assert_eq!(room.depth_from_start, 3);
    assert_eq!(room.difficulty, 5);
    assert_eq!(room.theme_tags.len(), 2);
}

#[test]
fn test_room_graph_creation() {
    // 测试房间图创建
    let graph = RoomGraph {
        nodes: vec![],
        edges: vec![],
        critical_path: vec!["room-start".to_string(), "room-boss".to_string()],
        branches: vec![],
    };

    assert_eq!(graph.critical_path.len(), 2);
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}

#[test]
fn test_room_edge_creation() {
    // 测试房间边创建
    let edge = RoomEdge {
        id: "edge-001".to_string(),
        from_room: "room-001".to_string(),
        to_room: "room-002".to_string(),
        is_critical: true,
    };

    assert_eq!(edge.from_room, "room-001");
    assert_eq!(edge.to_room, "room-002");
    assert!(edge.is_critical);
}

#[test]
fn test_branch_creation() {
    // 测试分支创建
    let branch = Branch {
        id: "branch-001".to_string(),
        start_room: "room-005".to_string(),
        end_room: "room-008".to_string(),
        room_ids: vec![
            "room-005".to_string(),
            "room-006".to_string(),
            "room-007".to_string(),
            "room-008".to_string(),
        ],
        purpose: BranchPurpose::Reward,
    };

    assert_eq!(branch.purpose, BranchPurpose::Reward);
    assert_eq!(branch.room_ids.len(), 4);
}

#[test]
fn test_door_anchor_creation() {
    // 测试门锚点创建
    let anchor = DoorAnchor {
        id: "anchor-001".to_string(),
        room_id: "room-001".to_string(),
        edge_id: "edge-001".to_string(),
        grid_pos: GridPoint { x: 10, y: 5 },
        facing: CardinalDir::North,
        width_tiles: 2,
        socket_tag: None,
    };

    assert_eq!(anchor.room_id, "room-001");
    assert_eq!(anchor.facing, CardinalDir::North);
    assert_eq!(anchor.width_tiles, 2);
}

#[test]
fn test_corridor_creation() {
    // 测试走廊创建
    let path = CorridorPath::Straight(vec![
        GridPoint { x: 0, y: 0 },
        GridPoint { x: 5, y: 0 },
        GridPoint { x: 10, y: 0 },
    ]);

    let corridor = Corridor {
        id: "corridor-001".to_string(),
        from_room: "room-001".to_string(),
        to_room: "room-002".to_string(),
        from_anchor: "anchor-001".to_string(),
        to_anchor: "anchor-002".to_string(),
        width_tiles: 3,
        path,
        segment_tags: Vec::new(),
    };

    assert_eq!(corridor.width_tiles, 3);
    assert_eq!(corridor.from_room, "room-001");
    assert_eq!(corridor.to_room, "room-002");
}

#[test]
fn test_corridor_path_variants() {
    // 测试走廊路径类型
    let straight = CorridorPath::Straight(vec![GridPoint { x: 0, y: 0 }]);
    let orthogonal = CorridorPath::Orthogonal(vec![GridPoint { x: 0, y: 0 }]);
    let polyline = CorridorPath::Polyline(vec![GridPoint { x: 0, y: 0 }]);

    match straight {
        CorridorPath::Straight(_) => {}
        _ => panic!("Expected Straight variant"),
    }

    match orthogonal {
        CorridorPath::Orthogonal(_) => {}
        _ => panic!("Expected Orthogonal variant"),
    }

    match polyline {
        CorridorPath::Polyline(_) => {}
        _ => panic!("Expected Polyline variant"),
    }
}
