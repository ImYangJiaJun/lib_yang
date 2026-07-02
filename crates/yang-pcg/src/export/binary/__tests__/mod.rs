// 二进制导出与导入模块测试
// 验证需求: 14.1, 14.2, 14.3, 16.2

use crate::config::GenerationConfig;
use crate::debug::DebugBundle;
use crate::export::binary::{export_binary, import_binary};
use crate::generator::MapGenerator;
use crate::model::request::GenerationRequest;
use crate::model::result::{GenerationResult, ResultMetadata};
use crate::model::room::RoomGraph;

/// 创建测试用的最小 GenerationResult
fn create_test_result() -> GenerationResult {
    GenerationResult {
        metadata: ResultMetadata {
            seed: 42,
            config_digest: "test-digest-binary".to_string(),
            schema_version: "1.0.0".to_string(),
            algorithm_version: "0.2.0".to_string(),
            target_engine_version: Some("UE5.5".to_string()),
            trace_id: Some("trace-binary-test".to_string()),
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

/// 使用 MapGenerator 生成包含完整数据的真实结果
fn generate_full_result() -> GenerationResult {
    let generator = MapGenerator::new();
    generator
        .generate(GenerationRequest {
            seed: Some(12345),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: Some("binary-test".to_string()),
        })
        .expect("生成应成功")
}

// ============================================================
// 29.1 二进制格式定义测试
// 验证需求: 14.1
// ============================================================

#[test]
fn test_binary_format_magic_bytes() {
    // 验证导出的二进制数据以 "YPCG" magic 标识开头
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    assert_eq!(&bytes[0..4], b"YPCG", "前 4 字节应为 Magic 标识 'YPCG'");
}

#[test]
fn test_binary_format_version_header() {
    // 验证版本头包含正确的 schema 版本号
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // Schema 主版本号 (u16 LE) 在偏移 4-5
    let major = u16::from_le_bytes([bytes[4], bytes[5]]);
    assert_eq!(major, 1, "Schema 主版本号应为 1");

    // Schema 次版本号 (u16 LE) 在偏移 6-7
    let minor = u16::from_le_bytes([bytes[6], bytes[7]]);
    assert_eq!(minor, 0, "Schema 次版本号应为 0");
}

#[test]
fn test_binary_format_reserved_bytes() {
    // 验证保留字段为全零
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // 保留字段在偏移 8-11
    assert_eq!(&bytes[8..12], &[0u8; 4], "保留字段应为全零");
}

#[test]
fn test_binary_format_body_length() {
    // 验证 body 长度字段正确
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // Body 长度在偏移 12-15 (u32 LE)
    let body_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;

    // 总长度 = header(16) + body_len + crc32(4)
    assert_eq!(
        bytes.len(),
        16 + body_len + 4,
        "总长度应等于 header + body + crc32"
    );
}

#[test]
fn test_binary_format_minimum_size() {
    // 验证导出的数据至少包含 header + crc32
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // 最小大小: header(16) + 至少 1 字节 body + crc32(4) = 21
    assert!(
        bytes.len() >= 21,
        "导出数据应至少 21 字节，实际 {} 字节",
        bytes.len()
    );
}

// ============================================================
// 29.2 二进制序列化与反序列化测试
// 验证需求: 14.1, 14.3
// ============================================================

#[test]
fn test_binary_roundtrip_minimal() {
    // 验证最小数据的序列化/反序列化往返一致性
    let original = create_test_result();
    let bytes = export_binary(&original).expect("导出应成功");
    let imported = import_binary(&bytes).expect("导入应成功");

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
fn test_binary_roundtrip_full_data() {
    // 验证完整生成数据的序列化/反序列化往返一致性
    let original = generate_full_result();
    let bytes = export_binary(&original).expect("导出应成功");
    let imported = import_binary(&bytes).expect("导入应成功");

    // 验证元数据
    assert_eq!(imported.metadata.seed, original.metadata.seed);
    assert_eq!(
        imported.metadata.config_digest,
        original.metadata.config_digest
    );

    // 验证拓扑
    assert_eq!(imported.topology.nodes.len(), original.topology.nodes.len());
    assert_eq!(imported.topology.edges.len(), original.topology.edges.len());
    assert_eq!(
        imported.topology.critical_path,
        original.topology.critical_path
    );

    // 验证房间
    assert_eq!(imported.rooms.len(), original.rooms.len());
    for (i, (orig, imp)) in original.rooms.iter().zip(imported.rooms.iter()).enumerate() {
        assert_eq!(imp.id, orig.id, "房间 {} 的 ID 应一致", i);
        assert_eq!(imp.room_type, orig.room_type, "房间 {} 的类型应一致", i);
    }

    // 验证走廊
    assert_eq!(imported.corridors.len(), original.corridors.len());

    // 验证地形
    assert_eq!(imported.terrains.len(), original.terrains.len());
    for (i, (orig, imp)) in original
        .terrains
        .iter()
        .zip(imported.terrains.iter())
        .enumerate()
    {
        assert_eq!(imp.room_id, orig.room_id, "地形 {} 的房间 ID 应一致", i);
        assert_eq!(
            imp.tiles.data, orig.tiles.data,
            "地形 {} 的网格数据应一致",
            i
        );
    }

    // 验证点位
    assert_eq!(imported.item_spawns.len(), original.item_spawns.len());
    assert_eq!(imported.enemy_spawns.len(), original.enemy_spawns.len());

    // 验证分块
    assert_eq!(imported.chunks.len(), original.chunks.len());
}

#[test]
fn test_binary_roundtrip_with_debug() {
    // 验证带调试信息的数据也能正确往返
    let mut original = create_test_result();
    original.debug = Some(DebugBundle::default());

    let bytes = export_binary(&original).expect("导出应成功");
    let imported = import_binary(&bytes).expect("导入应成功");

    assert!(imported.debug.is_some(), "导入后应保留调试信息");
}

#[test]
fn test_binary_has_structured_header_overhead() {
    // 验证二进制格式包含结构化头部和 CRC32 校验和的额外开销
    // 二进制格式的价值在于结构化头部（magic + 版本 + 保留字段）和 CRC32 完整性校验
    let result = generate_full_result();
    let binary = export_binary(&result).expect("二进制导出应成功");
    let json = crate::export::export_json_compact(&result).expect("JSON 导出应成功");

    // 二进制格式 = header(16) + body(JSON 字节) + crc32(4)
    // 因此比紧凑 JSON 多 20 字节
    let overhead = binary.len() as i64 - json.len() as i64;
    assert_eq!(
        overhead, 20,
        "二进制格式应比紧凑 JSON 多 20 字节（header + crc32），实际差异: {}",
        overhead
    );
}

// ============================================================
// 29.3 版本兼容性测试
// 验证需求: 14.2
// ============================================================

#[test]
fn test_binary_version_mismatch_rejected() {
    // 验证主版本号不匹配时拒绝导入
    let result = create_test_result();
    let mut bytes = export_binary(&result).expect("导出应成功");

    // 篡改主版本号为 2（偏移 4-5）
    bytes[4] = 2;
    bytes[5] = 0;

    // 重新计算 CRC32（因为修改了数据）
    let payload_len = bytes.len() - 4;
    let new_crc = crc32fast::hash(&bytes[..payload_len]);
    let crc_bytes = new_crc.to_le_bytes();
    bytes[payload_len] = crc_bytes[0];
    bytes[payload_len + 1] = crc_bytes[1];
    bytes[payload_len + 2] = crc_bytes[2];
    bytes[payload_len + 3] = crc_bytes[3];

    let err = import_binary(&bytes).expect_err("版本不匹配应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("主版本号不兼容"),
        "错误信息应包含版本不兼容描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_minor_version_difference_accepted() {
    // 验证次版本号不同但主版本号相同时应成功导入
    let result = create_test_result();
    let mut bytes = export_binary(&result).expect("导出应成功");

    // 篡改次版本号为 5（偏移 6-7）
    bytes[6] = 5;
    bytes[7] = 0;

    // 重新计算 CRC32
    let payload_len = bytes.len() - 4;
    let new_crc = crc32fast::hash(&bytes[..payload_len]);
    let crc_bytes = new_crc.to_le_bytes();
    bytes[payload_len] = crc_bytes[0];
    bytes[payload_len + 1] = crc_bytes[1];
    bytes[payload_len + 2] = crc_bytes[2];
    bytes[payload_len + 3] = crc_bytes[3];

    // 次版本号不同应仍可导入（向前兼容）
    let imported = import_binary(&bytes).expect("次版本号不同应允许导入");
    assert_eq!(imported.metadata.seed, 42);
}

#[test]
fn test_binary_reserved_bytes_ignored_on_import() {
    // 验证保留字段非零时仍可正常导入（向前兼容）
    let result = create_test_result();
    let mut bytes = export_binary(&result).expect("导出应成功");

    // 篡改保留字段为非零值（偏移 8-11）
    bytes[8] = 0xFF;
    bytes[9] = 0x01;
    bytes[10] = 0x02;
    bytes[11] = 0x03;

    // 重新计算 CRC32
    let payload_len = bytes.len() - 4;
    let new_crc = crc32fast::hash(&bytes[..payload_len]);
    let crc_bytes = new_crc.to_le_bytes();
    bytes[payload_len] = crc_bytes[0];
    bytes[payload_len + 1] = crc_bytes[1];
    bytes[payload_len + 2] = crc_bytes[2];
    bytes[payload_len + 3] = crc_bytes[3];

    // 保留字段非零应仍可导入
    let imported = import_binary(&bytes).expect("保留字段非零应允许导入");
    assert_eq!(imported.metadata.seed, 42);
}

// ============================================================
// 29.4 CRC32 校验和测试
// 验证需求: 16.2
// ============================================================

#[test]
fn test_binary_crc32_detects_corruption_in_body() {
    // 验证 body 数据被篡改时 CRC32 校验失败
    let result = create_test_result();
    let mut bytes = export_binary(&result).expect("导出应成功");

    // 篡改 body 中的一个字节（偏移 20，在 body 区域内）
    if bytes.len() > 20 {
        bytes[20] ^= 0xFF;
    }

    let err = import_binary(&bytes).expect_err("数据损坏应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("CRC32 校验和不匹配"),
        "错误信息应包含 CRC32 校验失败描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_crc32_detects_corruption_in_header() {
    // 验证 header 数据被篡改时 CRC32 校验失败
    let result = create_test_result();
    let mut bytes = export_binary(&result).expect("导出应成功");

    // 篡改 body 长度字段（偏移 12）但不更新 CRC32
    bytes[12] ^= 0x01;

    let err = import_binary(&bytes).expect_err("header 损坏应返回错误");
    let err_msg = format!("{}", err);
    // 可能是长度不匹配或 CRC32 不匹配
    assert!(
        err_msg.contains("CRC32") || err_msg.contains("长度不匹配"),
        "错误信息应包含损坏检测描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_crc32_detects_truncation() {
    // 验证数据被截断时检测到错误
    let result = create_test_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // 截断最后几个字节
    let truncated = &bytes[..bytes.len() - 2];

    let err = import_binary(truncated).expect_err("截断数据应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("长度不匹配") || err_msg.contains("CRC32"),
        "错误信息应包含损坏检测描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_import_too_short_data() {
    // 验证过短的数据返回错误
    let short_data = vec![0u8; 10];

    let err = import_binary(&short_data).expect_err("过短数据应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("数据长度不足"),
        "错误信息应包含长度不足描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_import_invalid_magic() {
    // 验证无效 magic 标识返回错误
    let mut data = vec![0u8; 30];
    // 写入错误的 magic
    data[0] = b'X';
    data[1] = b'Y';
    data[2] = b'Z';
    data[3] = b'W';

    let err = import_binary(&data).expect_err("无效 magic 应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("Magic 标识不匹配"),
        "错误信息应包含 Magic 不匹配描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_import_empty_data() {
    // 验证空数据返回错误
    let err = import_binary(&[]).expect_err("空数据应返回错误");
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("数据长度不足"),
        "错误信息应包含长度不足描述: {}",
        err_msg
    );
}

#[test]
fn test_binary_crc32_valid_on_correct_data() {
    // 验证正确数据的 CRC32 校验通过
    let result = generate_full_result();
    let bytes = export_binary(&result).expect("导出应成功");

    // 验证最后 4 字节是有效的 CRC32
    let payload = &bytes[..bytes.len() - 4];
    let stored_crc = u32::from_le_bytes([
        bytes[bytes.len() - 4],
        bytes[bytes.len() - 3],
        bytes[bytes.len() - 2],
        bytes[bytes.len() - 1],
    ]);
    let computed_crc = crc32fast::hash(payload);

    assert_eq!(stored_crc, computed_crc, "存储的 CRC32 应与计算值一致");
}

#[test]
fn test_binary_deterministic_output() {
    // 验证相同输入产生相同的二进制输出
    let result = create_test_result();
    let bytes1 = export_binary(&result).expect("第一次导出应成功");
    let bytes2 = export_binary(&result).expect("第二次导出应成功");

    assert_eq!(bytes1, bytes2, "相同输入应产生相同的二进制输出");
}
