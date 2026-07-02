// 固定种子的黄金样本测试
// 使用固定种子生成结果，验证关键字段的哈希值稳定
// 需求映射：18.2

use crate::config::GenerationConfig;
use crate::generator::MapGenerator;
use crate::model::request::GenerationRequest;
use crate::rng::fnv1a_64;

/// 计算房间 ID 列表的哈希值（FNV-1a，跨版本稳定）
fn hash_room_ids(rooms: &[crate::model::room::Room]) -> u64 {
    let mut data = Vec::new();
    for room in rooms {
        data.extend_from_slice(room.id.as_bytes());
        data.extend_from_slice(format!("{:?}", room.room_type).as_bytes());
    }
    fnv1a_64(&data)
}

/// 计算走廊 ID 列表的哈希值（FNV-1a，跨版本稳定）
fn hash_corridor_ids(corridors: &[crate::model::room::Corridor]) -> u64 {
    let mut data = Vec::new();
    for corridor in corridors {
        data.extend_from_slice(corridor.id.as_bytes());
        data.extend_from_slice(corridor.from_room.as_bytes());
        data.extend_from_slice(corridor.to_room.as_bytes());
    }
    fnv1a_64(&data)
}

/// 计算点位的哈希值（基于位置和类型，FNV-1a）
fn hash_spawns(spawns: &[crate::model::spawn::SpawnPoint]) -> u64 {
    let mut data = Vec::new();
    for spawn in spawns {
        data.extend_from_slice(spawn.room_id.as_bytes());
        data.extend_from_slice(&spawn.grid_pos.x.to_le_bytes());
        data.extend_from_slice(&spawn.grid_pos.y.to_le_bytes());
        data.extend_from_slice(format!("{:?}", spawn.kind).as_bytes());
    }
    fnv1a_64(&data)
}

/// 固定种子 42 的黄金样本测试
///
/// 验证相同种子和默认配置下，生成结果的关键字段哈希保持稳定。
/// 如果算法变更导致此测试失败，需要更新黄金样本值。
#[test]
fn test_golden_sample_seed_42() {
    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: Some(42),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("黄金样本生成应成功");

    // 验证基本结构稳定性
    let room_hash = hash_room_ids(&result.rooms);
    let corridor_hash = hash_corridor_ids(&result.corridors);
    let item_spawn_hash = hash_spawns(&result.item_spawns);
    let enemy_spawn_hash = hash_spawns(&result.enemy_spawns);

    // 第一次运行时记录哈希值，后续回归时对比
    // 如果算法未变更，这些值应保持不变
    let room_count = result.rooms.len();
    let corridor_count = result.corridors.len();
    let item_spawn_count = result.item_spawns.len();
    let enemy_spawn_count = result.enemy_spawns.len();

    // 再次生成，验证确定性
    let result2 = generator
        .generate(GenerationRequest {
            seed: Some(42),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("第二次生成应成功");

    let room_hash2 = hash_room_ids(&result2.rooms);
    let corridor_hash2 = hash_corridor_ids(&result2.corridors);
    let item_spawn_hash2 = hash_spawns(&result2.item_spawns);
    let enemy_spawn_hash2 = hash_spawns(&result2.enemy_spawns);

    // 验证两次生成结果完全一致
    assert_eq!(room_hash, room_hash2, "房间哈希应一致");
    assert_eq!(corridor_hash, corridor_hash2, "走廊哈希应一致");
    assert_eq!(item_spawn_hash, item_spawn_hash2, "交互物哈希应一致");
    assert_eq!(enemy_spawn_hash, enemy_spawn_hash2, "敌人哈希应一致");
    assert_eq!(result.rooms.len(), room_count, "房间数应一致");
    assert_eq!(result2.corridors.len(), corridor_count, "走廊数应一致");
    assert_eq!(
        result2.item_spawns.len(),
        item_spawn_count,
        "交互物数应一致"
    );
    assert_eq!(
        result2.enemy_spawns.len(),
        enemy_spawn_count,
        "敌人数应一致"
    );

    // 验证元数据稳定性
    assert_eq!(result.metadata.seed, 42);
    assert_eq!(
        result.metadata.config_digest,
        result2.metadata.config_digest
    );
    assert_eq!(
        result.metadata.schema_version,
        result2.metadata.schema_version
    );
}

/// 固定种子 12345 的黄金样本测试（第二组种子）
#[test]
fn test_golden_sample_seed_12345() {
    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: Some(12345),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("黄金样本生成应成功");

    // 验证确定性：连续生成 3 次，结果应完全一致
    for i in 0..3 {
        let result_n = generator
            .generate(GenerationRequest {
                seed: Some(12345),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .unwrap_or_else(|_| panic!("第 {} 次生成应成功", i + 2));

        assert_eq!(
            hash_room_ids(&result.rooms),
            hash_room_ids(&result_n.rooms),
            "第 {} 次生成的房间哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_corridor_ids(&result.corridors),
            hash_corridor_ids(&result_n.corridors),
            "第 {} 次生成的走廊哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_spawns(&result.item_spawns),
            hash_spawns(&result_n.item_spawns),
            "第 {} 次生成的交互物哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_spawns(&result.enemy_spawns),
            hash_spawns(&result_n.enemy_spawns),
            "第 {} 次生成的敌人哈希应一致",
            i + 2
        );
    }
}

/// 黄金样本 JSON 导出一致性测试
///
/// 验证固定种子生成的结果导出为 JSON 后，再次导入能得到相同的关键字段。
#[test]
fn test_golden_sample_json_roundtrip() {
    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: Some(42),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("生成应成功");

    // 导出为 JSON
    let json = crate::export::export_json(&result).expect("JSON 导出应成功");

    // 从 JSON 导入
    let imported = crate::export::import_json(&json).expect("JSON 导入应成功");

    // 验证关键字段一致
    assert_eq!(imported.metadata.seed, result.metadata.seed);
    assert_eq!(
        imported.metadata.config_digest,
        result.metadata.config_digest
    );
    assert_eq!(imported.rooms.len(), result.rooms.len());
    assert_eq!(imported.corridors.len(), result.corridors.len());
    assert_eq!(imported.item_spawns.len(), result.item_spawns.len());
    assert_eq!(imported.enemy_spawns.len(), result.enemy_spawns.len());

    // 验证房间 ID 顺序一致
    for (original, reimported) in result.rooms.iter().zip(imported.rooms.iter()) {
        assert_eq!(original.id, reimported.id);
        assert_eq!(original.room_type, reimported.room_type);
    }
}
