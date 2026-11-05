//! 审批实例的幂等键派生。
//!
//! `derive_uuid` 的输出已持久化在宿主的任务表与组表里；泛化时**必须保持四元组
//! `|` 拼接的逐字节输出**，否则蓝绿切换后存量任务按 uuid 回捞全部 1390003。

use sha2::{Digest, Sha256};

/// 由任意字符串组派生 36 字符 UUID（v8 变体位）。
///
/// 实现：`parts.join("|")` → SHA-256 前 16 字节 → 置版本/变体位。
/// `parts` 不得含 `|`（否则不同元组可能拼出同一输入）。
pub fn derive_idempotency_key(parts: &[&str]) -> String {
    debug_assert!(
        !parts.iter().any(|part| part.contains('|')),
        "幂等键组成部分不得包含 '|'"
    );
    let joined = parts.join("|");
    let digest = Sha256::digest(joined.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 4122 v8 + 变体位
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

/// 一条审批任务/聚合组的稳定幂等键。
pub fn derive_uuid(
    base_token: &str,
    table_id: &str,
    approval_code: &str,
    record_id: &str,
) -> String {
    derive_idempotency_key(&[base_token, table_id, approval_code, record_id])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_has_canonical_shape() {
        let uuid = derive_uuid("base", "table", "code", "record");
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[8..9], "-");
        assert_eq!(&uuid[13..14], "-");
        assert_eq!(&uuid[18..19], "-");
        assert_eq!(&uuid[23..24], "-");
        let version = u8::from_str_radix(&uuid[14..15], 16).unwrap_or(0);
        assert_eq!(version, 8);
    }

    #[test]
    fn uuid_is_stable_and_depends_on_every_part() {
        let uuid = derive_uuid("base", "table", "code", "record");
        assert_eq!(uuid, derive_uuid("base", "table", "code", "record"));
        assert_ne!(uuid, derive_uuid("base2", "table", "code", "record"));
        assert_ne!(uuid, derive_uuid("base", "table2", "code", "record"));
        assert_ne!(uuid, derive_uuid("base", "table", "code2", "record"));
        assert_ne!(uuid, derive_uuid("base", "table", "code", "record2"));
    }

    #[test]
    fn four_part_join_is_byte_compatible_with_the_persisted_algorithm() {
        // 黄金向量：与宿主 approval_uuid 的现有算法逐字节一致。
        // 变更这一行意味着存量任务无法按 uuid 回捞。
        let uuid = derive_uuid("appbcbWCzen6", "tblsRc9GRRX", "4202AD96-9EC1", "recqwIwhc6");
        assert_eq!(uuid, "57801135-b6d4-8a98-949c-12de67e20ca8");
    }
}
