// 固定种子的黄金样本测试
// 使用固定种子生成结果，验证关键字段的哈希值稳定
// 需求映射：18.2

use crate::config::{
    CapabilityFlags, ChunkingConfig, GenerationConfig, GenerationMode,
};
use crate::digest::ConfigDigest;
use crate::generator::MapGenerator;
use crate::model::request::{GenerationRequest, RuntimeContext};
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

/// 创建 RuntimeChunked 模式的测试配置
fn make_runtime_chunked_config() -> GenerationConfig {
    GenerationConfig {
        generation_mode: GenerationMode::RuntimeChunked,
        capability_flags: CapabilityFlags {
            runtime_chunked: true,
            hybrid_precompute: false,
            grammar_support: false,
            debug_output: false,
        },
        chunking: ChunkingConfig {
            chunk_size: 32,
            enabled: true,
        },
        ..Default::default()
    }
}

/// 创建 HybridPrecompute 模式的测试配置
fn make_hybrid_config() -> GenerationConfig {
    GenerationConfig {
        generation_mode: GenerationMode::HybridPrecompute,
        capability_flags: CapabilityFlags {
            runtime_chunked: false,
            hybrid_precompute: true,
            grammar_support: false,
            debug_output: false,
        },
        chunking: ChunkingConfig {
            chunk_size: 32,
            enabled: true,
        },
        ..Default::default()
    }
}

// ============================================================================
// OPT-T-01: 固化黄金哈希常量（OfflineFullFloor seed-42 + 默认配置）
// ============================================================================
// 以下常量在 2026-07-02 由 OfflineFullFloor 模式、seed=42、默认配置产出。
// 如果算法变更导致这些值漂移，需要审查变更是否破坏了确定性契约，
// 然后在此更新常量并记录变更原因。

/// OfflineFullFloor seed-42 房间哈希（ID + room_type 拼接的 FNV-1a）
const GOLDEN_SEED42_ROOM_HASH: u64 = 0xbb9c3cdf7d452a0a;
/// OfflineFullFloor seed-42 走廊哈希（ID + from_room + to_room 拼接的 FNV-1a）
const GOLDEN_SEED42_CORRIDOR_HASH: u64 = 0x9f1b9dc978b92993;
/// OfflineFullFloor seed-42 交互物点位哈希
const GOLDEN_SEED42_ITEM_HASH: u64 = 0x3ffbe579cd1dd622;
/// OfflineFullFloor seed-42 敌人点位哈希
const GOLDEN_SEED42_ENEMY_HASH: u64 = 0x60fa4cbee9a988c3;

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

    // OPT-T-01: 验证哈希与固化常量一致（防止静默漂移）
    let room_hash = hash_room_ids(&result.rooms);
    let corridor_hash = hash_corridor_ids(&result.corridors);
    let item_spawn_hash = hash_spawns(&result.item_spawns);
    let enemy_spawn_hash = hash_spawns(&result.enemy_spawns);

    assert_eq!(room_hash, GOLDEN_SEED42_ROOM_HASH, "房间哈希与黄金常量不一致——算法可能已变更");
    assert_eq!(corridor_hash, GOLDEN_SEED42_CORRIDOR_HASH, "走廊哈希与黄金常量不一致");
    assert_eq!(item_spawn_hash, GOLDEN_SEED42_ITEM_HASH, "交互物哈希与黄金常量不一致");
    assert_eq!(enemy_spawn_hash, GOLDEN_SEED42_ENEMY_HASH, "敌人哈希与黄金常量不一致");

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

    // 临时打印哈希值用于固化常量
    println!("SEED12345_ROOM_HASH=0x{:016x}", hash_room_ids(&result.rooms));
    println!("SEED12345_CORRIDOR_HASH=0x{:016x}", hash_corridor_ids(&result.corridors));
    println!("SEED12345_ITEM_HASH=0x{:016x}", hash_spawns(&result.item_spawns));
    println!("SEED12345_ENEMY_HASH=0x{:016x}", hash_spawns(&result.enemy_spawns));

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

/// seed=None 稳定性测试
///
/// 验证不提供 seed 时，连续多次生成结果完全一致（确定性兜底种子）。
/// OPT-T-04
#[test]
fn test_golden_seed_none_stability() {
    let generator = MapGenerator::new();

    let first = generator
        .generate(GenerationRequest {
            seed: None,
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("seed=None 生成应成功");

    // 连续生成 4 次，结果应与首次完全一致
    for i in 0..4 {
        let repeat = generator
            .generate(GenerationRequest {
                seed: None,
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .unwrap_or_else(|_| panic!("第 {} 次 seed=None 生成应成功", i + 2));

        assert_eq!(
            first.rooms.len(),
            repeat.rooms.len(),
            "第 {} 次房间数应一致",
            i + 2
        );
        assert_eq!(
            first.corridors.len(),
            repeat.corridors.len(),
            "第 {} 次走廊数应一致",
            i + 2
        );
        assert_eq!(
            first.item_spawns.len(),
            repeat.item_spawns.len(),
            "第 {} 次交互物数应一致",
            i + 2
        );
        assert_eq!(
            first.enemy_spawns.len(),
            repeat.enemy_spawns.len(),
            "第 {} 次敌人数应一致",
            i + 2
        );
        assert_eq!(
            hash_room_ids(&first.rooms),
            hash_room_ids(&repeat.rooms),
            "第 {} 次房间哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_corridor_ids(&first.corridors),
            hash_corridor_ids(&repeat.corridors),
            "第 {} 次走廊哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_spawns(&first.item_spawns),
            hash_spawns(&repeat.item_spawns),
            "第 {} 次交互物哈希应一致",
            i + 2
        );
        assert_eq!(
            hash_spawns(&first.enemy_spawns),
            hash_spawns(&repeat.enemy_spawns),
            "第 {} 次敌人哈希应一致",
            i + 2
        );
        assert_eq!(
            first.metadata.seed, repeat.metadata.seed,
            "第 {} 次元数据种子应一致",
            i + 2
        );
        assert_eq!(
            first.metadata.config_digest, repeat.metadata.config_digest,
            "第 {} 次配置摘要应一致",
            i + 2
        );
    }
}

/// seed=None 时结果种子应等于 ConfigDigest::seed_from_config
///
/// 验证 generator 内部确实使用了配置派生种子作为兜底。
/// OPT-T-04
#[test]
fn test_seed_none_equals_seed_from_config() {
    let config = GenerationConfig::default();
    let expected_seed = ConfigDigest::seed_from_config(&config)
        .expect("默认配置派生种子不应失败");

    let generator = MapGenerator::new();
    let result = generator
        .generate(GenerationRequest {
            seed: None,
            config: config.clone(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        })
        .expect("seed=None 生成应成功");

    assert_eq!(
        result.metadata.seed, expected_seed,
        "seed=None 时结果种子应等于 ConfigDigest::seed_from_config 派生值"
    );
}

// ============================================================================
// OPT-T-02: 三模式黄金测试（RuntimeChunked + HybridPrecompute）
// ============================================================================
// OfflineFullFloor 的 seed-42 黄金哈希已在上方固化。
// 以下两个测试补齐另外两种生成模式的确定性验证。
// 注意：三种模式的 RNG 派生路径不同，同一 seed 产出不同地图——这是有意设计。

/// RuntimeChunked 模式 seed-42 黄金样本测试
///
/// 验证分块模式下相同种子的生成结果确定性（两次互比）。
/// RuntimeChunked 的 RNG 派生标签含 chunk/room 维度，与 OfflineFullFloor 不同，
/// 因此房间哈希不等于 GOLDEN_SEED42_ROOM_HASH——这是正常的。
#[test]
fn test_golden_runtime_chunked_seed_42() {
    let generator = MapGenerator::new();
    let config = make_runtime_chunked_config();

    let make_request = || GenerationRequest {
        seed: Some(42),
        config: config.clone(),
        constraints: vec![],
        runtime_context: Some(RuntimeContext {
            focus_position: None,
            interest_radius: None,
            requested_chunks: vec![], // 空列表 = 生成所有分块
            caller_tag: None,
        }),
        trace_id: None,
    };

    let result1 = generator
        .generate_chunk(make_request())
        .expect("RuntimeChunked 第一次生成应成功");

    let result2 = generator
        .generate_chunk(make_request())
        .expect("RuntimeChunked 第二次生成应成功");

    // 确定性验证：两次生成结果完全一致
    assert_eq!(result1.rooms.len(), result2.rooms.len(), "RuntimeChunked 房间数应一致");
    assert_eq!(
        hash_room_ids(&result1.rooms),
        hash_room_ids(&result2.rooms),
        "RuntimeChunked 房间哈希应一致"
    );
    assert_eq!(
        hash_corridor_ids(&result1.corridors),
        hash_corridor_ids(&result2.corridors),
        "RuntimeChunked 走廊哈希应一致"
    );
    assert_eq!(
        hash_spawns(&result1.item_spawns),
        hash_spawns(&result2.item_spawns),
        "RuntimeChunked 交互物哈希应一致"
    );
    assert_eq!(
        hash_spawns(&result1.enemy_spawns),
        hash_spawns(&result2.enemy_spawns),
        "RuntimeChunked 敌人哈希应一致"
    );
    assert_eq!(result1.metadata.seed, 42);
    assert_eq!(
        result1.metadata.config_digest, result2.metadata.config_digest,
        "RuntimeChunked 配置摘要应一致"
    );
}

/// HybridPrecompute 模式 seed-42 黄金样本测试
///
/// 验证混合模式下两阶段调用的确定性：
/// 1. generate_topology_only 两次调用返回相同拓扑
/// 2. fill_chunk_details 对同一 chunk 两次调用返回相同细节
#[test]
fn test_golden_hybrid_seed_42() {
    let generator = MapGenerator::new();
    let config = make_hybrid_config();

    let make_request = || GenerationRequest {
        seed: Some(42),
        config: config.clone(),
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    // 第一阶段：拓扑预计算
    let topo1 = generator
        .generate_topology_only(make_request())
        .expect("HybridPrecompute 拓扑第一次生成应成功");

    let topo2 = generator
        .generate_topology_only(make_request())
        .expect("HybridPrecompute 拓扑第二次生成应成功");

    // 拓扑确定性
    assert_eq!(topo1.seed, 42);
    assert_eq!(topo1.seed, topo2.seed, "HybridPrecompute 种子应一致");
    assert_eq!(
        topo1.topology.nodes.len(),
        topo2.topology.nodes.len(),
        "HybridPrecompute 拓扑节点数应一致"
    );
    assert_eq!(
        topo1.config_digest, topo2.config_digest,
        "HybridPrecompute 配置摘要应一致"
    );
    assert_eq!(
        topo1.chunks.len(),
        topo2.chunks.len(),
        "HybridPrecompute 分块数应一致"
    );

    // 第二阶段：逐块细节填充确定性
    let chunk_id = &topo1.chunks[0].id;
    let detail1 = generator
        .fill_chunk_details(&topo1, chunk_id)
        .expect("HybridPrecompute 第一次细节填充应成功");
    let detail2 = generator
        .fill_chunk_details(&topo2, chunk_id)
        .expect("HybridPrecompute 第二次细节填充应成功");

    assert_eq!(detail1.chunk_id, detail2.chunk_id, "分块 ID 应一致");
    assert_eq!(
        detail1.terrains.len(),
        detail2.terrains.len(),
        "HybridPrecompute 地形数应一致"
    );
    assert_eq!(
        detail1.item_spawns.len(),
        detail2.item_spawns.len(),
        "HybridPrecompute 交互物数应一致"
    );
    assert_eq!(
        detail1.enemy_spawns.len(),
        detail2.enemy_spawns.len(),
        "HybridPrecompute 敌人数应一致"
    );

    // 点位哈希一致性（跨两次独立拓扑预计算 + 细节填充）
    assert_eq!(
        hash_spawns(&detail1.item_spawns),
        hash_spawns(&detail2.item_spawns),
        "HybridPrecompute 交互物哈希应一致"
    );
    assert_eq!(
        hash_spawns(&detail1.enemy_spawns),
        hash_spawns(&detail2.enemy_spawns),
        "HybridPrecompute 敌人哈希应一致"
    );
}
