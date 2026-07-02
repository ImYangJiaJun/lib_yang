// JSON 导出与导入模块
// 提供将 GenerationResult 导出为 JSON 格式以及从 JSON 重建 GenerationResult 的功能

pub mod binary;

use crate::error::{PcgError, PcgResult};
use crate::model::result::GenerationResult;

// 重新导出二进制导出/导入函数
pub use binary::{export_binary, import_binary};

/// 当前数据模式版本
///
/// 用于导入时进行兼容性校验，主版本号不一致时拒绝导入。
pub const CURRENT_SCHEMA_VERSION: &str = "1.0.0";

/// 将生成结果导出为格式化的 JSON 字符串
///
/// 使用 `serde_json::to_string_pretty` 生成人类可读的 JSON 输出，
/// 包含完整的元数据信息：`schema_version`、`algorithm_version`、
/// `seed`、`config_digest`、`target_engine_version`。
///
/// # 参数
///
/// * `result` - 生成结果的引用
///
/// # 返回值
///
/// 成功时返回格式化的 JSON 字符串，失败时返回导出错误。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::export::export_json;
///
/// let result = generator.generate(request)?;
/// let json = export_json(&result)?;
/// println!("{}", json);
/// ```
pub fn export_json(result: &GenerationResult) -> PcgResult<String> {
    serde_json::to_string_pretty(result)
        .map_err(|e| PcgError::export_err(format!("JSON 序列化失败: {}", e), "json", e))
}

/// 将生成结果导出为紧凑的 JSON 字符串
///
/// 使用 `serde_json::to_string` 生成无多余空白的紧凑 JSON 输出，
/// 适用于网络传输或存储场景。包含与 `export_json` 相同的完整元数据。
///
/// # 参数
///
/// * `result` - 生成结果的引用
///
/// # 返回值
///
/// 成功时返回紧凑的 JSON 字符串，失败时返回导出错误。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::export::export_json_compact;
///
/// let result = generator.generate(request)?;
/// let json = export_json_compact(&result)?;
/// // 紧凑格式，适合存储和传输
/// std::fs::write("output.json", &json)?;
/// ```
pub fn export_json_compact(result: &GenerationResult) -> PcgResult<String> {
    serde_json::to_string(result)
        .map_err(|e| PcgError::export_err(format!("JSON 序列化失败: {}", e), "json", e))
}

/// 从 JSON 字符串重建生成结果
///
/// 将 JSON 字符串反序列化为 `GenerationResult`，并进行基本完整性校验：
/// - 检查 `schema_version` 的主版本号是否与当前版本兼容
/// - 主版本号不一致时返回错误
///
/// # 参数
///
/// * `json` - 包含生成结果的 JSON 字符串
///
/// # 返回值
///
/// 成功时返回重建的 `GenerationResult`，失败时返回导出错误。
///
/// # 错误情况
///
/// - JSON 解析失败（格式错误、字段缺失等）
/// - `schema_version` 主版本号与当前版本不兼容
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::export::import_json;
///
/// let json = std::fs::read_to_string("output.json")?;
/// let result = import_json(&json)?;
/// println!("种子: {}", result.metadata.seed);
/// ```
pub fn import_json(json: &str) -> PcgResult<GenerationResult> {
    // 反序列化 JSON 字符串
    let result: GenerationResult = serde_json::from_str(json)
        .map_err(|e| PcgError::export_err(format!("JSON 反序列化失败: {}", e), "json", e))?;

    // 校验 schema_version 格式（必须为合法的三段 semver）
    if !is_valid_semver(&result.metadata.schema_version) {
        return Err(PcgError::corrupted_data_with_version(
            format!(
                "schema_version 格式非法: '{}'（要求 X.Y.Z，三段均为非负整数）",
                result.metadata.schema_version,
            ),
            "json",
            CURRENT_SCHEMA_VERSION,
            &result.metadata.schema_version,
        ));
    }

    // 校验 schema_version 兼容性（主版本号必须一致）
    let imported_major = extract_major_version(&result.metadata.schema_version);
    let current_major = extract_major_version(CURRENT_SCHEMA_VERSION);

    if imported_major != current_major {
        return Err(PcgError::corrupted_data_with_version(
            format!(
                "schema_version 不兼容: 导入版本为 '{}'（主版本 {})，当前版本为 '{}'（主版本 {})",
                result.metadata.schema_version,
                imported_major.unwrap_or(0),
                CURRENT_SCHEMA_VERSION,
                current_major.unwrap_or(0),
            ),
            "json",
            CURRENT_SCHEMA_VERSION,
            &result.metadata.schema_version,
        ));
    }

    Ok(result)
}

/// 从版本字符串中提取主版本号
///
/// 支持 semver 格式（如 "1.0.0"、"2.1.3"），返回第一个点号前的数字。
fn extract_major_version(version: &str) -> Option<u32> {
    version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
}

/// 校验版本字符串是否为合法的三段 semver（X.Y.Z，每段均为非负整数）
///
/// 拒绝 "abc"、"1.2"、"1.2.3.4"、"" 等非法格式。
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

#[cfg(test)]
mod __tests__;
