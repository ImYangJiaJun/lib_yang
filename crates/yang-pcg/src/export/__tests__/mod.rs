// JSON 导出与导入模块测试
// 验证需求: 14.2, 14.3

use crate::debug::DebugBundle;
use crate::export::{export_json, export_json_compact, import_json, CURRENT_SCHEMA_VERSION};
use crate::model::result::{GenerationResult, ResultMetadata};
use crate::model::room::RoomGraph;

/// 创建测试用的最小 GenerationResult
fn create_test_result() -> GenerationResult {
    GenerationResult {
        metadata: ResultMetadata {
            seed: 42,
            config_digest: "test-digest-abc123".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.2.0".to_string(),
            target_engine_version: Some("UE5.5".to_string()),
            trace_id: Some("trace-export-test".to_string()),
        },
        topology: RoomGraph {
            nodes: vec![],
            edges: vec![],
            critical_path: vec![],
            branches: vec![],
        },
        rooms: vec![],
        door_anchors: vec![],
        corridors: vec![],
        terrains: vec![],
        item_spawns: vec![],
        enemy_spawns: vec![],
        chunks: vec![],
        debug: None,
    }
}

/// 创建不含 target_engine_version 的测试结果
fn create_test_result_without_engine_version() -> GenerationResult {
    GenerationResult {
        metadata: ResultMetadata {
            seed: 99,
            config_digest: "no-engine-digest".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.1.0".to_string(),
            target_engine_version: None,
            trace_id: None,
        },
        topology: RoomGraph {
            nodes: vec![],
            edges: vec![],
            critical_path: vec![],
            branches: vec![],
        },
        rooms: vec![],
        door_anchors: vec![],
        corridors: vec![],
        terrains: vec![],
        item_spawns: vec![],
        enemy_spawns: vec![],
        chunks: vec![],
        debug: None,
    }
}

#[test]
fn test_export_json_contains_schema_version() {
    // 验证导出的 JSON 包含 schema_version 字段
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"1.0.0\""));
}

#[test]
fn test_export_json_contains_algorithm_version() {
    // 验证导出的 JSON 包含 algorithm_version 字段
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"algorithm_version\""));
    assert!(json.contains("\"0.2.0\""));
}

#[test]
fn test_export_json_contains_seed() {
    // 验证导出的 JSON 包含 seed 字段
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"seed\""));
    assert!(json.contains("42"));
}

#[test]
fn test_export_json_contains_config_digest() {
    // 验证导出的 JSON 包含 config_digest 字段
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"config_digest\""));
    assert!(json.contains("\"test-digest-abc123\""));
}

#[test]
fn test_export_json_contains_target_engine_version() {
    // 验证导出的 JSON 包含 target_engine_version 字段
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"target_engine_version\""));
    assert!(json.contains("\"UE5.5\""));
}

#[test]
fn test_export_json_null_target_engine_version() {
    // 验证当 target_engine_version 为 None 时，JSON 中该字段为 null
    let result = create_test_result_without_engine_version();
    let json = export_json(&result).expect("导出应成功");

    assert!(json.contains("\"target_engine_version\""));
    assert!(json.contains("null"));
}

#[test]
fn test_export_json_is_pretty_formatted() {
    // 验证 export_json 输出格式化的 JSON（包含换行和缩进）
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    // 格式化 JSON 应包含换行符
    assert!(json.contains('\n'));
    // 格式化 JSON 应包含缩进空格
    assert!(json.contains("  "));
}

#[test]
fn test_export_json_compact_no_extra_whitespace() {
    // 验证 export_json_compact 输出紧凑的 JSON（无多余换行）
    let result = create_test_result();
    let json = export_json_compact(&result).expect("导出应成功");

    // 紧凑 JSON 不应包含换行符
    assert!(!json.contains('\n'));
}

#[test]
fn test_export_json_compact_contains_all_metadata() {
    // 验证紧凑格式同样包含完整元数据
    let result = create_test_result();
    let json = export_json_compact(&result).expect("导出应成功");

    assert!(json.contains("\"schema_version\":\"1.0.0\""));
    assert!(json.contains("\"algorithm_version\":\"0.2.0\""));
    assert!(json.contains("\"seed\":42"));
    assert!(json.contains("\"config_digest\":\"test-digest-abc123\""));
    assert!(json.contains("\"target_engine_version\":\"UE5.5\""));
}

#[test]
fn test_export_json_roundtrip() {
    // 验证导出的 JSON 可以反序列化回 GenerationResult
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    let restored: GenerationResult =
        serde_json::from_str(&json).expect("应可从导出的 JSON 反序列化");

    assert_eq!(restored.metadata.seed, 42);
    assert_eq!(restored.metadata.config_digest, "test-digest-abc123");
    assert_eq!(restored.metadata.schema_version, "1.0.0");
    assert_eq!(restored.metadata.algorithm_version, "0.2.0");
    assert_eq!(
        restored.metadata.target_engine_version,
        Some("UE5.5".to_string())
    );
}

#[test]
fn test_export_json_compact_roundtrip() {
    // 验证紧凑格式导出的 JSON 同样可以反序列化
    let result = create_test_result();
    let json = export_json_compact(&result).expect("导出应成功");

    let restored: GenerationResult = serde_json::from_str(&json).expect("应可从紧凑 JSON 反序列化");

    assert_eq!(restored.metadata.seed, result.metadata.seed);
    assert_eq!(
        restored.metadata.config_digest,
        result.metadata.config_digest
    );
    assert_eq!(
        restored.metadata.schema_version,
        result.metadata.schema_version
    );
    assert_eq!(
        restored.metadata.algorithm_version,
        result.metadata.algorithm_version
    );
    assert_eq!(
        restored.metadata.target_engine_version,
        result.metadata.target_engine_version
    );
}

#[test]
fn test_export_json_with_debug_bundle() {
    // 验证带调试信息的结果也能正确导出
    let mut result = create_test_result();
    result.debug = Some(DebugBundle::default());

    let json = export_json(&result).expect("带调试信息的导出应成功");
    assert!(json.contains("\"debug\""));

    let restored: GenerationResult = serde_json::from_str(&json).expect("应可反序列化");
    assert!(restored.debug.is_some());
}

#[test]
fn test_export_json_valid_json_structure() {
    // 验证导出结果是合法的 JSON，且包含完整元数据结构
    let result = create_test_result();
    let json = export_json(&result).expect("导出应成功");

    // 尝试解析为通用 JSON Value 验证结构合法性
    let value: serde_json::Value = serde_json::from_str(&json).expect("导出结果应为合法 JSON");

    // 验证顶层结构包含 metadata 字段
    assert!(value.get("metadata").is_some());
    let metadata = value.get("metadata").unwrap();

    // 验证 metadata 中包含所有必需字段
    assert!(metadata.get("schema_version").is_some());
    assert!(metadata.get("algorithm_version").is_some());
    assert!(metadata.get("seed").is_some());
    assert!(metadata.get("config_digest").is_some());
    assert!(metadata.get("target_engine_version").is_some());
}

// ============================================================
// import_json 测试
// 验证需求: 14.3
// ============================================================

#[test]
fn test_import_json_success() {
    // 验证正常导入：导出后再导入，结果应一致
    let original = create_test_result();
    let json = export_json(&original).expect("导出应成功");

    let (imported, warnings) = import_json(&json).expect("导入应成功");
    assert!(warnings.is_empty(), "版本匹配时不应有警告");

    assert_eq!(imported.metadata.seed, original.metadata.seed);
    assert_eq!(
        imported.metadata.config_digest,
        original.metadata.config_digest
    );
    assert_eq!(
        imported.metadata.schema_version,
        original.metadata.schema_version
    );
    assert_eq!(
        imported.metadata.algorithm_version,
        original.metadata.algorithm_version
    );
    assert_eq!(
        imported.metadata.target_engine_version,
        original.metadata.target_engine_version
    );
    assert_eq!(imported.metadata.trace_id, original.metadata.trace_id);
    assert_eq!(imported.rooms.len(), original.rooms.len());
    assert_eq!(imported.corridors.len(), original.corridors.len());
}

#[test]
fn test_import_json_compact_success() {
    // 验证从紧凑格式 JSON 导入也能成功
    let original = create_test_result();
    let json = export_json_compact(&original).expect("导出应成功");

    let (imported, warnings) = import_json(&json).expect("从紧凑 JSON 导入应成功");
    assert!(warnings.is_empty(), "版本匹配时不应有警告");

    assert_eq!(imported.metadata.seed, 42);
    assert_eq!(imported.metadata.schema_version, "1.0.0");
}

#[test]
fn test_import_json_schema_version_mismatch() {
    // 验证主版本号不一致时返回错误
    let mut result = create_test_result();
    result.metadata.schema_version = "2.0.0".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let err = import_json(&json).expect_err("应返回版本不兼容错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("schema_version 不兼容"),
        "错误信息应包含不兼容描述: {}",
        err_msg
    );
    assert!(
        err_msg.contains("2.0.0"),
        "错误信息应包含导入版本号: {}",
        err_msg
    );
    // 验证错误码为 CorruptedData 而非 Export
    assert_eq!(
        err.error_code(),
        "PCG-CORRUPTED-001",
        "版本不兼容应返回 CorruptedData 错误码"
    );
}

#[test]
fn test_import_json_schema_version_minor_difference_ok() {
    // 验证次版本号不同但主版本号相同时应成功
    let mut result = create_test_result();
    result.metadata.schema_version = "1.2.3".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let (imported, warnings) = import_json(&json).expect("次版本号不同应允许导入");
    assert!(warnings.is_empty(), "版本匹配时不应有警告");
    assert_eq!(imported.metadata.schema_version, "1.2.3");
}

#[test]
fn test_import_json_invalid_json() {
    // 验证无效 JSON 返回错误
    let invalid_json = "{ this is not valid json }";

    let err = import_json(invalid_json).expect_err("无效 JSON 应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("JSON 反序列化失败"),
        "错误信息应包含反序列化失败: {}",
        err_msg
    );
}

#[test]
fn test_import_json_empty_string() {
    // 验证空字符串返回错误
    let err = import_json("").expect_err("空字符串应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("JSON 反序列化失败"),
        "错误信息应包含反序列化失败: {}",
        err_msg
    );
}

#[test]
fn test_import_json_missing_fields() {
    // 验证缺少必要字段的 JSON 返回错误
    let incomplete_json = r#"{"metadata": {"seed": 42}}"#;

    let err = import_json(incomplete_json).expect_err("缺少字段应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("JSON 反序列化失败"),
        "错误信息应包含反序列化失败: {}",
        err_msg
    );
}

#[test]
fn test_import_json_with_debug_bundle() {
    // 验证带调试信息的结果也能正确导入
    let mut result = create_test_result();
    result.debug = Some(DebugBundle::default());
    let json = export_json(&result).expect("导出应成功");

    let (imported, _warnings) = import_json(&json).expect("带调试信息的导入应成功");
    assert!(imported.debug.is_some());
}

#[test]
fn test_current_schema_version_constant() {
    // 验证常量定义正确
    assert_eq!(CURRENT_SCHEMA_VERSION, "1.0.0");
    // 验证常量符合 semver 格式
    let parts: Vec<&str> = CURRENT_SCHEMA_VERSION.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(parts[0].parse::<u32>().is_ok());
    assert!(parts[1].parse::<u32>().is_ok());
    assert!(parts[2].parse::<u32>().is_ok());
}

#[test]
fn test_import_json_malformed_version() {
    // 验证畸形 schema_version 被拒绝：非三段 semver 格式
    let malformed_versions = vec!["abc", "1.2", "1.2.3.4", ""];
    for bad_version in malformed_versions {
        let mut result = create_test_result();
        result.metadata.schema_version = bad_version.to_string();
        let json = serde_json::to_string(&result).expect("序列化应成功");

        let err = import_json(&json).expect_err(&format!("畸形版本 '{}' 应被拒绝", bad_version));
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("schema_version 格式非法"),
            "版本 '{}' 的错误信息应包含格式描述: {}",
            bad_version,
            err_msg
        );
        assert_eq!(
            err.error_code(),
            "PCG-CORRUPTED-001",
            "畸形版本 '{}' 应返回 CorruptedData 错误码",
            bad_version
        );
    }
}

#[test]
fn test_schema_version_compat() {
    // 主版本号相同 → 兼容（无论次版本/修订号）
    let compatible_versions = vec!["1.0.0", "1.0.99", "1.99.0", "1.2.3"];
    for good_version in compatible_versions {
        let mut result = create_test_result();
        result.metadata.schema_version = good_version.to_string();
        let json = serde_json::to_string(&result).expect("序列化应成功");

        let (imported, _warnings) = import_json(&json)
            .unwrap_or_else(|_| panic!("主版本相同的版本 '{}' 应被接受", good_version));
        assert_eq!(imported.metadata.schema_version, good_version);
    }

    // 主版本号不同 → 不兼容
    let incompatible_versions = vec!["0.0.0", "0.9.9", "2.0.0", "99.0.0"];
    for bad_version in incompatible_versions {
        let mut result = create_test_result();
        result.metadata.schema_version = bad_version.to_string();
        let json = serde_json::to_string(&result).expect("序列化应成功");

        let err =
            import_json(&json).expect_err(&format!("主版本不同的版本 '{}' 应被拒绝", bad_version));
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("schema_version 不兼容"),
            "版本 '{}' 的错误信息应包含不兼容描述: {}",
            bad_version,
            err_msg
        );
    }
}

// ============================================================
// algorithm_version 检查测试（OPT-S-10）
// 验证需求: import 仅校验 schema 主版本，不检查 algorithm_version
// ============================================================

#[test]
fn test_import_algorithm_version_mismatch_produces_warning() {
    // 验证 algorithm_version 主版本不一致时返回警告而非错误
    let mut result = create_test_result();
    // 当前 crate 版本是 0.1.0，用主版本 9 模拟不匹配
    result.metadata.algorithm_version = "9.0.0".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let (imported, warnings) = import_json(&json).expect("algorithm_version 不匹配不应阻止导入");
    assert_eq!(imported.metadata.algorithm_version, "9.0.0");
    assert_eq!(warnings.len(), 1, "应有一条 algorithm_version 警告");
    assert!(
        warnings[0].contains("algorithm_version 主版本不一致"),
        "警告应描述 algorithm_version 不一致: {}",
        warnings[0]
    );
    assert!(
        warnings[0].contains("9.0.0"),
        "警告应包含导入版本号: {}",
        warnings[0]
    );
}

#[test]
fn test_import_algorithm_version_major_match_no_warning() {
    // 验证 algorithm_version 主版本一致时无警告
    let mut result = create_test_result();
    // 当前 crate 版本是 0.1.0，同主版本不同次版本
    result.metadata.algorithm_version = "0.99.0".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let (imported, warnings) = import_json(&json).expect("导入应成功");
    assert_eq!(imported.metadata.algorithm_version, "0.99.0");
    assert!(warnings.is_empty(), "主版本匹配时不应有警告");
}

#[test]
fn test_import_algorithm_version_exact_match_no_warning() {
    // 验证 algorithm_version 完全匹配时无警告
    let result = create_test_result();
    // 用当前 crate 版本覆盖 algorithm_version
    let mut result = result;
    result.metadata.algorithm_version = env!("CARGO_PKG_VERSION").to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let (imported, warnings) = import_json(&json).expect("导入应成功");
    assert_eq!(
        imported.metadata.algorithm_version,
        env!("CARGO_PKG_VERSION")
    );
    assert!(warnings.is_empty(), "完全匹配时不应有警告");
}

#[test]
fn test_import_algorithm_version_invalid_format_no_warning() {
    // 验证 algorithm_version 格式非法时静默跳过（不产生警告）
    let mut result = create_test_result();
    result.metadata.algorithm_version = "invalid-version".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    let (imported, warnings) = import_json(&json).expect("非法 algorithm_version 不应阻止导入");
    assert_eq!(imported.metadata.algorithm_version, "invalid-version");
    assert!(warnings.is_empty(), "非法格式应静默跳过，不产生警告");
}

#[test]
fn test_import_schema_and_algorithm_both_mismatch() {
    // 验证 schema_version 和 algorithm_version 同时不匹配时，
    // schema_version 报错（硬错误），algorithm_version 警告不触发
    let mut result = create_test_result();
    result.metadata.schema_version = "2.0.0".to_string();
    result.metadata.algorithm_version = "9.0.0".to_string();
    let json = serde_json::to_string(&result).expect("序列化应成功");

    // schema_version 不兼容应直接返回错误
    let err = import_json(&json).expect_err("schema_version 不兼容应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("schema_version 不兼容"),
        "错误信息应包含 schema_version 不兼容: {}",
        err_msg
    );
}

// ============================================================
// 导出/导入一致性测试（使用 MapGenerator 生成真实数据）
// 验证需求: 14.3, 18.5
// ============================================================

use crate::config::GenerationConfig;
use crate::generator::MapGenerator;
use crate::model::request::GenerationRequest;

/// 使用 MapGenerator 生成包含完整数据的真实结果
fn generate_full_result() -> GenerationResult {
    let generator = MapGenerator::new();
    generator
        .generate(GenerationRequest {
            seed: Some(12345),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: Some("consistency-test".to_string()),
        })
        .expect("生成应成功")
}

/// 使用 MapGenerator 生成带调试信息的完整结果
fn generate_full_result_with_debug() -> GenerationResult {
    let mut generator = MapGenerator::new();
    generator.set_debug(true);
    generator
        .generate(GenerationRequest {
            seed: Some(67890),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: Some("debug-consistency-test".to_string()),
        })
        .expect("调试模式生成应成功")
}

#[test]
fn test_consistency_metadata_roundtrip() {
    // 验证 serialize → deserialize 后元数据语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    // 验证所有元数据字段
    assert_eq!(imported.metadata.seed, original.metadata.seed);
    assert_eq!(
        imported.metadata.config_digest,
        original.metadata.config_digest
    );
    assert_eq!(
        imported.metadata.schema_version,
        original.metadata.schema_version
    );
    assert_eq!(
        imported.metadata.algorithm_version,
        original.metadata.algorithm_version
    );
    assert_eq!(
        imported.metadata.target_engine_version,
        original.metadata.target_engine_version
    );
    assert_eq!(imported.metadata.trace_id, original.metadata.trace_id);
}

#[test]
fn test_consistency_topology_roundtrip() {
    // 验证 serialize → deserialize 后拓扑结构语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    // 验证节点数量一致
    assert_eq!(
        imported.topology.nodes.len(),
        original.topology.nodes.len(),
        "拓扑节点数量应一致"
    );
    // 验证边数量一致
    assert_eq!(
        imported.topology.edges.len(),
        original.topology.edges.len(),
        "拓扑边数量应一致"
    );
    // 验证关键路径一致
    assert_eq!(
        imported.topology.critical_path, original.topology.critical_path,
        "关键路径应一致"
    );
    // 验证分支数量一致
    assert_eq!(
        imported.topology.branches.len(),
        original.topology.branches.len(),
        "分支数量应一致"
    );
    // 验证每个节点的 ID 和类型一致
    for (i, (orig_node, imp_node)) in original
        .topology
        .nodes
        .iter()
        .zip(imported.topology.nodes.iter())
        .enumerate()
    {
        assert_eq!(imp_node.id, orig_node.id, "节点 {} 的 ID 应一致", i);
        assert_eq!(
            imp_node.room_type, orig_node.room_type,
            "节点 {} 的类型应一致",
            i
        );
        assert_eq!(
            imp_node.depth_from_start, orig_node.depth_from_start,
            "节点 {} 的深度应一致",
            i
        );
        assert_eq!(
            imp_node.difficulty, orig_node.difficulty,
            "节点 {} 的难度应一致",
            i
        );
    }
    // 验证每条边的源和目标一致
    for (i, (orig_edge, imp_edge)) in original
        .topology
        .edges
        .iter()
        .zip(imported.topology.edges.iter())
        .enumerate()
    {
        assert_eq!(
            imp_edge.from_room, orig_edge.from_room,
            "边 {} 的源应一致",
            i
        );
        assert_eq!(imp_edge.to_room, orig_edge.to_room, "边 {} 的目标应一致", i);
    }
}

#[test]
fn test_consistency_rooms_roundtrip() {
    // 验证 serialize → deserialize 后房间列表语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    assert_eq!(imported.rooms.len(), original.rooms.len(), "房间数量应一致");
    for (i, (orig, imp)) in original.rooms.iter().zip(imported.rooms.iter()).enumerate() {
        assert_eq!(imp.id, orig.id, "房间 {} 的 ID 应一致", i);
        assert_eq!(imp.room_type, orig.room_type, "房间 {} 的类型应一致", i);
        assert_eq!(
            imp.depth_from_start, orig.depth_from_start,
            "房间 {} 的深度应一致",
            i
        );
        assert_eq!(imp.difficulty, orig.difficulty, "房间 {} 的难度应一致", i);
        assert_eq!(
            imp.theme_tags, orig.theme_tags,
            "房间 {} 的主题标签应一致",
            i
        );
        assert_eq!(imp.branch_id, orig.branch_id, "房间 {} 的分支 ID 应一致", i);
    }
}

#[test]
fn test_consistency_corridors_roundtrip() {
    // 验证 serialize → deserialize 后走廊列表语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    assert_eq!(
        imported.corridors.len(),
        original.corridors.len(),
        "走廊数量应一致"
    );
    for (i, (orig, imp)) in original
        .corridors
        .iter()
        .zip(imported.corridors.iter())
        .enumerate()
    {
        assert_eq!(imp.id, orig.id, "走廊 {} 的 ID 应一致", i);
        assert_eq!(imp.from_room, orig.from_room, "走廊 {} 的源房间应一致", i);
        assert_eq!(imp.to_room, orig.to_room, "走廊 {} 的目标房间应一致", i);
        assert_eq!(imp.width_tiles, orig.width_tiles, "走廊 {} 的宽度应一致", i);
    }
}

#[test]
fn test_consistency_terrains_roundtrip() {
    // 验证 serialize → deserialize 后地形数据语义一致（包含网格数据）
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    assert_eq!(
        imported.terrains.len(),
        original.terrains.len(),
        "地形数量应一致"
    );
    for (i, (orig, imp)) in original
        .terrains
        .iter()
        .zip(imported.terrains.iter())
        .enumerate()
    {
        assert_eq!(imp.room_id, orig.room_id, "地形 {} 的房间 ID 应一致", i);
        assert_eq!(imp.grid_size, orig.grid_size, "地形 {} 的网格尺寸应一致", i);
        // 验证网格数据完全一致
        assert_eq!(
            imp.tiles.width, orig.tiles.width,
            "地形 {} 的网格宽度应一致",
            i
        );
        assert_eq!(
            imp.tiles.height, orig.tiles.height,
            "地形 {} 的网格高度应一致",
            i
        );
        assert_eq!(
            imp.tiles.data.len(),
            orig.tiles.data.len(),
            "地形 {} 的网格数据长度应一致",
            i
        );
        assert_eq!(
            imp.tiles.data, orig.tiles.data,
            "地形 {} 的网格瓦片数据应完全一致",
            i
        );
        // 验证连通性摘要一致
        assert_eq!(
            imp.connectivity_summary.all_doors_connected,
            orig.connectivity_summary.all_doors_connected,
            "地形 {} 的连通性应一致",
            i
        );
        assert_eq!(
            imp.connectivity_summary.walkable_tile_count,
            orig.connectivity_summary.walkable_tile_count,
            "地形 {} 的可通行瓦片数应一致",
            i
        );
    }
}

#[test]
fn test_consistency_spawns_roundtrip() {
    // 验证 serialize → deserialize 后点位数据语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    // 验证交互物点位
    assert_eq!(
        imported.item_spawns.len(),
        original.item_spawns.len(),
        "交互物点位数量应一致"
    );
    for (i, (orig, imp)) in original
        .item_spawns
        .iter()
        .zip(imported.item_spawns.iter())
        .enumerate()
    {
        assert_eq!(imp.id, orig.id, "交互物点位 {} 的 ID 应一致", i);
        assert_eq!(
            imp.room_id, orig.room_id,
            "交互物点位 {} 的房间 ID 应一致",
            i
        );
        assert_eq!(imp.kind, orig.kind, "交互物点位 {} 的类型应一致", i);
        assert_eq!(
            imp.grid_pos, orig.grid_pos,
            "交互物点位 {} 的网格位置应一致",
            i
        );
        assert_eq!(
            imp.metadata.spawn_tag, orig.metadata.spawn_tag,
            "交互物点位 {} 的标签应一致",
            i
        );
    }

    // 验证敌人点位
    assert_eq!(
        imported.enemy_spawns.len(),
        original.enemy_spawns.len(),
        "敌人点位数量应一致"
    );
    for (i, (orig, imp)) in original
        .enemy_spawns
        .iter()
        .zip(imported.enemy_spawns.iter())
        .enumerate()
    {
        assert_eq!(imp.id, orig.id, "敌人点位 {} 的 ID 应一致", i);
        assert_eq!(imp.room_id, orig.room_id, "敌人点位 {} 的房间 ID 应一致", i);
        assert_eq!(imp.kind, orig.kind, "敌人点位 {} 的类型应一致", i);
        assert_eq!(
            imp.grid_pos, orig.grid_pos,
            "敌人点位 {} 的网格位置应一致",
            i
        );
    }
}

#[test]
fn test_consistency_chunks_roundtrip() {
    // 验证 serialize → deserialize 后分块数据语义一致
    let original = generate_full_result();
    let json = export_json(&original).expect("导出应成功");
    let (imported, _warnings) = import_json(&json).expect("导入应成功");

    assert_eq!(
        imported.chunks.len(),
        original.chunks.len(),
        "分块数量应一致"
    );
    for (i, (orig, imp)) in original
        .chunks
        .iter()
        .zip(imported.chunks.iter())
        .enumerate()
    {
        assert_eq!(imp.id, orig.id, "分块 {} 的 ID 应一致", i);
        assert_eq!(imp.room_ids, orig.room_ids, "分块 {} 的房间列表应一致", i);
        assert_eq!(
            imp.dependencies, orig.dependencies,
            "分块 {} 的依赖列表应一致",
            i
        );
        assert_eq!(
            imp.streaming_metadata.data_layer, orig.streaming_metadata.data_layer,
            "分块 {} 的 data_layer 应一致",
            i
        );
        assert_eq!(
            imp.streaming_metadata.streaming_priority, orig.streaming_metadata.streaming_priority,
            "分块 {} 的 streaming_priority 应一致",
            i
        );
    }
}

#[test]
fn test_consistency_pretty_and_compact_produce_same_result() {
    // 验证 pretty 和 compact 两种格式导出后导入结果语义一致
    let original = generate_full_result();

    let json_pretty = export_json(&original).expect("pretty 导出应成功");
    let json_compact = export_json_compact(&original).expect("compact 导出应成功");

    let (imported_pretty, _warnings_p) =
        import_json(&json_pretty).expect("从 pretty JSON 导入应成功");
    let (imported_compact, _warnings_c) =
        import_json(&json_compact).expect("从 compact JSON 导入应成功");

    // 元数据一致
    assert_eq!(
        imported_pretty.metadata.seed,
        imported_compact.metadata.seed
    );
    assert_eq!(
        imported_pretty.metadata.config_digest,
        imported_compact.metadata.config_digest
    );
    // 拓扑一致
    assert_eq!(
        imported_pretty.topology.nodes.len(),
        imported_compact.topology.nodes.len()
    );
    assert_eq!(
        imported_pretty.topology.edges.len(),
        imported_compact.topology.edges.len()
    );
    assert_eq!(
        imported_pretty.topology.critical_path,
        imported_compact.topology.critical_path
    );
    // 房间一致
    assert_eq!(imported_pretty.rooms.len(), imported_compact.rooms.len());
    for (p, c) in imported_pretty
        .rooms
        .iter()
        .zip(imported_compact.rooms.iter())
    {
        assert_eq!(p.id, c.id);
        assert_eq!(p.room_type, c.room_type);
    }
    // 走廊一致
    assert_eq!(
        imported_pretty.corridors.len(),
        imported_compact.corridors.len()
    );
    // 地形一致
    assert_eq!(
        imported_pretty.terrains.len(),
        imported_compact.terrains.len()
    );
    for (p, c) in imported_pretty
        .terrains
        .iter()
        .zip(imported_compact.terrains.iter())
    {
        assert_eq!(p.tiles.data, c.tiles.data, "地形网格数据应一致");
    }
    // 点位一致
    assert_eq!(
        imported_pretty.item_spawns.len(),
        imported_compact.item_spawns.len()
    );
    assert_eq!(
        imported_pretty.enemy_spawns.len(),
        imported_compact.enemy_spawns.len()
    );
    // 分块一致
    assert_eq!(imported_pretty.chunks.len(), imported_compact.chunks.len());
}

#[test]
fn test_consistency_reexport_idempotency() {
    // 验证导入后重新导出产生相同的 JSON（幂等性）
    let original = generate_full_result();

    // 第一次导出
    let json_first = export_json(&original).expect("第一次导出应成功");
    // 导入
    let (imported, _warnings) = import_json(&json_first).expect("导入应成功");
    // 第二次导出
    let json_second = export_json(&imported).expect("重新导出应成功");

    // 两次导出的 JSON 应完全相同
    assert_eq!(
        json_first, json_second,
        "导入后重新导出的 JSON 应与原始导出完全一致（幂等性）"
    );
}

#[test]
fn test_consistency_compact_reexport_idempotency() {
    // 验证紧凑格式的导入后重新导出也满足幂等性
    let original = generate_full_result();

    let json_first = export_json_compact(&original).expect("第一次紧凑导出应成功");
    let (imported, _warnings) = import_json(&json_first).expect("导入应成功");
    let json_second = export_json_compact(&imported).expect("重新紧凑导出应成功");

    assert_eq!(
        json_first, json_second,
        "紧凑格式导入后重新导出的 JSON 应完全一致（幂等性）"
    );
}

#[test]
fn test_consistency_with_debug_data_roundtrip() {
    // 验证带调试信息的完整结果也能正确往返
    let original = generate_full_result_with_debug();
    assert!(original.debug.is_some(), "调试模式应产生 DebugBundle");

    let json = export_json(&original).expect("带调试信息的导出应成功");
    let (imported, _warnings) = import_json(&json).expect("带调试信息的导入应成功");

    // 验证调试信息存在
    assert!(imported.debug.is_some(), "导入后应保留调试信息");
    let orig_debug = original.debug.as_ref().unwrap();
    let imp_debug = imported.debug.as_ref().unwrap();

    // 验证 trace_id 一致
    assert_eq!(imp_debug.trace_id, orig_debug.trace_id);
    // 验证阶段统计数量一致
    assert_eq!(
        imp_debug.stage_stats.len(),
        orig_debug.stage_stats.len(),
        "阶段统计数量应一致"
    );
    // 验证各阶段名称和产出数一致
    for (i, (orig_stat, imp_stat)) in orig_debug
        .stage_stats
        .iter()
        .zip(imp_debug.stage_stats.iter())
        .enumerate()
    {
        assert_eq!(
            imp_stat.stage_name, orig_stat.stage_name,
            "阶段 {} 的名称应一致",
            i
        );
        assert_eq!(
            imp_stat.produced_count, orig_stat.produced_count,
            "阶段 {} 的产出数应一致",
            i
        );
    }
}

#[test]
fn test_consistency_generated_data_is_non_trivial() {
    // 确保测试使用的生成数据是非平凡的（包含实际内容）
    let result = generate_full_result();

    // 验证生成的数据确实包含有意义的内容
    assert!(!result.rooms.is_empty(), "生成结果应包含房间");
    assert!(!result.corridors.is_empty(), "生成结果应包含走廊");
    assert!(!result.terrains.is_empty(), "生成结果应包含地形");
    assert!(!result.chunks.is_empty(), "生成结果应包含分块");
    assert!(!result.topology.nodes.is_empty(), "生成结果应包含拓扑节点");
    assert!(!result.topology.edges.is_empty(), "生成结果应包含拓扑边");
    assert!(
        !result.topology.critical_path.is_empty(),
        "生成结果应包含关键路径"
    );
    // 验证地形网格数据非空
    for terrain in &result.terrains {
        assert!(!terrain.tiles.data.is_empty(), "地形网格数据不应为空");
    }
}
