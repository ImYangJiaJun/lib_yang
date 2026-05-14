// 调试开关不影响玩法结果测试
// 验证同种子开启/关闭调试时，玩法通道（rooms、corridors、spawns）完全一致
// 需求映射：2.6, 15.4

use crate::config::GenerationConfig;
use crate::generator::MapGenerator;
use crate::model::request::GenerationRequest;

/// 验证调试开关不影响房间生成结果
#[test]
fn test_debug_toggle_rooms_identical() {
    let config = GenerationConfig::default();

    // 关闭调试生成
    let generator_off = MapGenerator::new();
    let result_off = generator_off
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("关闭调试生成应成功");

    // 开启调试生成
    let mut generator_on = MapGenerator::new();
    generator_on.set_debug(true);
    let result_on = generator_on
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("开启调试生成应成功");

    // 验证房间数量一致
    assert_eq!(
        result_off.rooms.len(),
        result_on.rooms.len(),
        "调试开关不应影响房间数量"
    );

    // 验证每个房间的 ID、类型、边界完全一致
    for (room_off, room_on) in result_off.rooms.iter().zip(result_on.rooms.iter()) {
        assert_eq!(room_off.id, room_on.id, "房间 ID 应一致");
        assert_eq!(room_off.room_type, room_on.room_type, "房间类型应一致");
        assert_eq!(room_off.bounds, room_on.bounds, "房间边界应一致");
        assert_eq!(
            room_off.depth_from_start, room_on.depth_from_start,
            "房间深度应一致"
        );
        assert_eq!(room_off.difficulty, room_on.difficulty, "房间难度应一致");
    }
}

/// 验证调试开关不影响走廊生成结果
#[test]
fn test_debug_toggle_corridors_identical() {
    let config = GenerationConfig::default();

    let generator_off = MapGenerator::new();
    let result_off = generator_off
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("关闭调试生成应成功");

    let mut generator_on = MapGenerator::new();
    generator_on.set_debug(true);
    let result_on = generator_on
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("开启调试生成应成功");

    // 验证走廊数量一致
    assert_eq!(
        result_off.corridors.len(),
        result_on.corridors.len(),
        "调试开关不应影响走廊数量"
    );

    // 验证每条走廊的关键字段一致
    for (corr_off, corr_on) in result_off.corridors.iter().zip(result_on.corridors.iter()) {
        assert_eq!(corr_off.id, corr_on.id, "走廊 ID 应一致");
        assert_eq!(corr_off.from_room, corr_on.from_room, "走廊起点应一致");
        assert_eq!(corr_off.to_room, corr_on.to_room, "走廊终点应一致");
        assert_eq!(
            corr_off.width_tiles, corr_on.width_tiles,
            "走廊宽度应一致"
        );
    }
}

/// 验证调试开关不影响点位生成结果
#[test]
fn test_debug_toggle_spawns_identical() {
    let config = GenerationConfig::default();

    let generator_off = MapGenerator::new();
    let result_off = generator_off
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("关闭调试生成应成功");

    let mut generator_on = MapGenerator::new();
    generator_on.set_debug(true);
    let result_on = generator_on
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("开启调试生成应成功");

    // 验证交互物点位数量一致
    assert_eq!(
        result_off.item_spawns.len(),
        result_on.item_spawns.len(),
        "调试开关不应影响交互物数量"
    );

    // 验证敌人点位数量一致
    assert_eq!(
        result_off.enemy_spawns.len(),
        result_on.enemy_spawns.len(),
        "调试开关不应影响敌人数量"
    );

    // 验证交互物点位位置和类型一致
    for (spawn_off, spawn_on) in result_off.item_spawns.iter().zip(result_on.item_spawns.iter()) {
        assert_eq!(spawn_off.room_id, spawn_on.room_id, "交互物房间 ID 应一致");
        assert_eq!(
            spawn_off.grid_pos, spawn_on.grid_pos,
            "交互物网格位置应一致"
        );
        assert_eq!(spawn_off.kind, spawn_on.kind, "交互物类型应一致");
    }

    // 验证敌人点位位置和类型一致
    for (spawn_off, spawn_on) in result_off
        .enemy_spawns
        .iter()
        .zip(result_on.enemy_spawns.iter())
    {
        assert_eq!(spawn_off.room_id, spawn_on.room_id, "敌人房间 ID 应一致");
        assert_eq!(
            spawn_off.grid_pos, spawn_on.grid_pos,
            "敌人网格位置应一致"
        );
        assert_eq!(spawn_off.kind, spawn_on.kind, "敌人类型应一致");
    }
}

/// 验证调试开关不影响拓扑结构
#[test]
fn test_debug_toggle_topology_identical() {
    let config = GenerationConfig::default();

    let generator_off = MapGenerator::new();
    let result_off = generator_off
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("关闭调试生成应成功");

    let mut generator_on = MapGenerator::new();
    generator_on.set_debug(true);
    let result_on = generator_on
        .generate(GenerationRequest {
            seed: Some(42),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("开启调试生成应成功");

    // 验证拓扑节点一致
    assert_eq!(
        result_off.topology.nodes.len(),
        result_on.topology.nodes.len(),
        "拓扑节点数应一致"
    );

    // 验证关键路径一致
    assert_eq!(
        result_off.topology.critical_path,
        result_on.topology.critical_path,
        "关键路径应一致"
    );

    // 验证拓扑边一致
    assert_eq!(
        result_off.topology.edges.len(),
        result_on.topology.edges.len(),
        "拓扑边数应一致"
    );

    for (edge_off, edge_on) in result_off
        .topology
        .edges
        .iter()
        .zip(result_on.topology.edges.iter())
    {
        assert_eq!(edge_off.id, edge_on.id, "边 ID 应一致");
        assert_eq!(edge_off.from_room, edge_on.from_room, "边起点应一致");
        assert_eq!(edge_off.to_room, edge_on.to_room, "边终点应一致");
    }
}

/// 使用不同种子验证调试隔离性（种子 99）
#[test]
fn test_debug_toggle_different_seed() {
    let config = GenerationConfig::default();

    let generator_off = MapGenerator::new();
    let result_off = generator_off
        .generate(GenerationRequest {
            seed: Some(99),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("关闭调试生成应成功");

    let mut generator_on = MapGenerator::new();
    generator_on.set_debug(true);
    let result_on = generator_on
        .generate(GenerationRequest {
            seed: Some(99),
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("开启调试生成应成功");

    // 验证所有玩法通道一致
    assert_eq!(result_off.rooms.len(), result_on.rooms.len());
    assert_eq!(result_off.corridors.len(), result_on.corridors.len());
    assert_eq!(result_off.item_spawns.len(), result_on.item_spawns.len());
    assert_eq!(result_off.enemy_spawns.len(), result_on.enemy_spawns.len());

    // 验证调试模式确实产生了调试输出
    assert!(result_off.debug.is_none(), "关闭调试不应有调试输出");
    assert!(result_on.debug.is_some(), "开启调试应有调试输出");
}
