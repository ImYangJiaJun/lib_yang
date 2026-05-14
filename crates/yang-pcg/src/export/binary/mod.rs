// 二进制导出与导入模块
// 提供紧凑的二进制格式导出/导入功能，包含 magic 标识、版本头、CRC32 校验和
//
// 二进制格式布局：
// - Bytes 0-3:   Magic 标识 "YPCG"
// - Bytes 4-5:   Schema 主版本号 (u16 LE)
// - Bytes 6-7:   Schema 次版本号 (u16 LE)
// - Bytes 8-11:  保留字段（用于未来扩展，当前填充 0）
// - Bytes 12-15: Body 长度 (u32 LE)
// - Bytes 16..16+body_len: 紧凑 JSON 编码的 body 数据
// - 最后 4 字节:  CRC32 校验和（覆盖前面所有字节）

use crate::error::{PcgError, PcgResult};
use crate::export::CURRENT_SCHEMA_VERSION;
use crate::model::result::GenerationResult;

/// 二进制格式的 Magic 标识
const BINARY_MAGIC: &[u8; 4] = b"YPCG";

/// 文件头固定长度（magic + version + reserved + body_length）
const HEADER_SIZE: usize = 16;

/// CRC32 校验和长度
const CRC32_SIZE: usize = 4;

/// 最小有效文件大小（header + 至少 1 字节 body + crc32）
const MIN_FILE_SIZE: usize = HEADER_SIZE + 1 + CRC32_SIZE;

/// 将生成结果导出为二进制格式
///
/// 使用紧凑的二进制布局，包含 magic 标识、版本头、保留字段、
/// 紧凑 JSON 编码的 body 数据和 CRC32 校验和。
///
/// # 参数
///
/// * `result` - 生成结果的引用
///
/// # 返回值
///
/// 成功时返回二进制数据的字节向量，失败时返回导出错误。
///
/// # 格式布局
///
/// | 偏移 | 长度 | 内容 |
/// |------|------|------|
/// | 0-3 | 4 | Magic "YPCG" |
/// | 4-5 | 2 | Schema 主版本号 (u16 LE) |
/// | 6-7 | 2 | Schema 次版本号 (u16 LE) |
/// | 8-11 | 4 | 保留字段 (全零) |
/// | 12-15 | 4 | Body 长度 (u32 LE) |
/// | 16..16+N | N | 紧凑 JSON 编码的 body |
/// | 最后 4 字节 | 4 | CRC32 校验和 |
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::export::binary::export_binary;
///
/// let result = generator.generate(request)?;
/// let bytes = export_binary(&result)?;
/// std::fs::write("output.ypcg", &bytes)?;
/// ```
pub fn export_binary(result: &GenerationResult) -> PcgResult<Vec<u8>> {
    // 使用紧凑 JSON 作为 body 编码
    // bincode 不支持 skip_serializing_if 等 serde 属性，
    // 因此使用 JSON 字节作为 body，保证完全兼容性。
    // 二进制格式的价值在于结构化头部和 CRC32 校验和。
    let body = serde_json::to_vec(result).map_err(|e| {
        PcgError::export_with_format(
            format!("二进制序列化失败: {}", e),
            "binary",
            Some(e.to_string()),
        )
    })?;

    let body_len = body.len();

    // 检查 body 长度是否超过 u32 最大值
    if body_len > u32::MAX as usize {
        return Err(PcgError::export_with_format(
            format!("数据体过大，超过 4GB 限制: {} 字节", body_len),
            "binary",
            None,
        ));
    }

    // 解析当前 schema 版本号
    let (major, minor) = parse_schema_version(CURRENT_SCHEMA_VERSION)?;

    // 组装二进制数据
    let total_size = HEADER_SIZE + body_len + CRC32_SIZE;
    let mut buffer = Vec::with_capacity(total_size);

    // 写入 Magic 标识
    buffer.extend_from_slice(BINARY_MAGIC);

    // 写入 Schema 版本号（小端序）
    buffer.extend_from_slice(&major.to_le_bytes());
    buffer.extend_from_slice(&minor.to_le_bytes());

    // 写入保留字段（4 字节全零，用于未来扩展）
    buffer.extend_from_slice(&[0u8; 4]);

    // 写入 Body 长度（小端序）
    buffer.extend_from_slice(&(body_len as u32).to_le_bytes());

    // 写入 Body 数据
    buffer.extend_from_slice(&body);

    // 计算并写入 CRC32 校验和（覆盖前面所有字节）
    let crc = crc32fast::hash(&buffer);
    buffer.extend_from_slice(&crc.to_le_bytes());

    Ok(buffer)
}

/// 从二进制数据导入生成结果
///
/// 验证 magic 标识、版本兼容性和 CRC32 校验和，然后反序列化 body 数据。
///
/// # 参数
///
/// * `data` - 二进制数据的字节切片
///
/// # 返回值
///
/// 成功时返回重建的 `GenerationResult`，失败时返回相应错误。
///
/// # 错误情况
///
/// - 数据长度不足（小于最小有效大小）
/// - Magic 标识不匹配
/// - Schema 主版本号不兼容
/// - CRC32 校验和不匹配（数据损坏）
/// - Body 反序列化失败
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::export::binary::import_binary;
///
/// let bytes = std::fs::read("output.ypcg")?;
/// let result = import_binary(&bytes)?;
/// println!("种子: {}", result.metadata.seed);
/// ```
pub fn import_binary(data: &[u8]) -> PcgResult<GenerationResult> {
    // 检查最小数据长度
    if data.len() < MIN_FILE_SIZE {
        return Err(PcgError::corrupted_data_with_type(
            format!(
                "数据长度不足: 期望至少 {} 字节，实际 {} 字节",
                MIN_FILE_SIZE,
                data.len()
            ),
            "binary",
        ));
    }

    // 验证 Magic 标识
    let magic = &data[0..4];
    if magic != BINARY_MAGIC {
        return Err(PcgError::corrupted_data_with_type(
            format!(
                "Magic 标识不匹配: 期望 {:?}，实际 {:?}",
                BINARY_MAGIC, magic
            ),
            "binary",
        ));
    }

    // 读取版本号
    let file_major = u16::from_le_bytes([data[4], data[5]]);
    let _file_minor = u16::from_le_bytes([data[6], data[7]]);

    // 验证主版本号兼容性
    let (current_major, _current_minor) = parse_schema_version(CURRENT_SCHEMA_VERSION)?;
    if file_major != current_major {
        return Err(PcgError::corrupted_data_with_version(
            format!(
                "Schema 主版本号不兼容: 文件版本 {}，当前版本 {}",
                file_major, current_major
            ),
            "binary",
            &format!("{}", current_major),
            &format!("{}", file_major),
        ));
    }

    // 读取保留字段（当前忽略，用于未来扩展）
    // data[8..12] 为保留字段

    // 读取 Body 长度
    let body_len = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;

    // 验证数据总长度是否匹配
    let expected_total = HEADER_SIZE + body_len + CRC32_SIZE;
    if data.len() != expected_total {
        return Err(PcgError::corrupted_data_with_type(
            format!(
                "数据长度不匹配: 头部声明 body 长度 {}，期望总长度 {}，实际总长度 {}",
                body_len, expected_total, data.len()
            ),
            "binary",
        ));
    }

    // 验证 CRC32 校验和
    let payload = &data[..data.len() - CRC32_SIZE];
    let stored_crc = u32::from_le_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    let computed_crc = crc32fast::hash(payload);

    if stored_crc != computed_crc {
        return Err(PcgError::corrupted_data_with_type(
            format!(
                "CRC32 校验和不匹配: 存储值 0x{:08X}，计算值 0x{:08X}，数据可能已损坏",
                stored_crc, computed_crc
            ),
            "binary",
        ));
    }

    // 反序列化 Body（从紧凑 JSON 字节解码）
    let body = &data[HEADER_SIZE..HEADER_SIZE + body_len];
    let result: GenerationResult = serde_json::from_slice(body).map_err(|e| {
        PcgError::export_with_format(
            format!("二进制反序列化失败: {}", e),
            "binary",
            Some(e.to_string()),
        )
    })?;

    Ok(result)
}

/// 解析 schema 版本字符串为 (major, minor) 元组
///
/// 支持 semver 格式（如 "1.0.0"），提取主版本号和次版本号。
fn parse_schema_version(version: &str) -> PcgResult<(u16, u16)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return Err(PcgError::export_with_format(
            format!("无效的版本格式: '{}'，期望 semver 格式", version),
            "binary",
            None,
        ));
    }

    let major = parts[0].parse::<u16>().map_err(|_| {
        PcgError::export_with_format(
            format!("无法解析主版本号: '{}'", parts[0]),
            "binary",
            None,
        )
    })?;

    let minor = parts[1].parse::<u16>().map_err(|_| {
        PcgError::export_with_format(
            format!("无法解析次版本号: '{}'", parts[1]),
            "binary",
            None,
        )
    })?;

    Ok((major, minor))
}

#[cfg(test)]
mod __tests__;
