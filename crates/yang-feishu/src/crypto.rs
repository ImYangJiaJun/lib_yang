//! 飞书外部选项/事件订阅的 AES-256-CBC 原语与摘要。
//!
//! 与飞书官方 Go 参考实现保持一致：密钥取配置原文的 SHA-256 摘要；IV 为 16 字节
//! 随机值并**前置**拼接；PKCS#7 填充（长度已对齐时仍追加整块）；输出为
//! `base64(IV ‖ 密文)`。
//!
//! 只搬通用原语：`seal` / `issue_token` / `unseal_verified` 等凭据生命周期留在宿主。

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::{GeneralPurpose, STANDARD};
use base64::Engine;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// AES 块大小（字节）。
const BLOCK_SIZE: usize = 16;

/// base64 STANDARD 编码器（飞书 AES 输出用标准字母表）。
const BASE64: GeneralPurpose = STANDARD;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// SHA-256 十六进制摘要的长度。
const HASH_HEX_LEN: usize = 64;

/// 加密/解密失败。
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// 加密失败（填充异常）。
    #[error("AES-CBC 加密失败")]
    Encrypt,
    /// 密文不是合法 base64。
    #[error("密文不是合法 base64")]
    InvalidBase64,
    /// 密文长度非法（必须至少一个 IV 块 + 一个整块密文）。
    #[error("密文长度非法")]
    InvalidLength,
    /// 解密失败（换钥匙、篡改或填充非法）。
    #[error("密文解密失败")]
    Decrypt,
    /// JSON 序列化失败。
    #[error("JSON 序列化失败: {0}")]
    Serialize(String),
}

/// 由配置原文派生 256 位密钥（SHA-256）。
pub fn derive_key(raw: &str) -> [u8; 32] {
    let digest = Sha256::digest(raw.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

/// 加密字节串，返回 `base64(IV ‖ 密文)`。
pub fn encrypt_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<String, CryptoError> {
    let mut iv = [0u8; BLOCK_SIZE];
    OsRng.fill_bytes(&mut iv);

    // 预留一个整块：PKCS#7 在明文已对齐时还要再追加整块。
    let mut buffer = vec![0u8; plaintext.len() + BLOCK_SIZE];
    buffer[..plaintext.len()].copy_from_slice(plaintext);

    let cipher = Aes256CbcEnc::new(key.into(), (&iv).into());
    let ciphertext = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|_| CryptoError::Encrypt)?;

    let mut output = Vec::with_capacity(BLOCK_SIZE + ciphertext.len());
    output.extend_from_slice(&iv);
    output.extend_from_slice(ciphertext);
    Ok(BASE64.encode(output))
}

/// 加密一个 JSON 值，返回 `base64(IV ‖ 密文)`。
pub fn encrypt_json(value: &serde_json::Value, key: &[u8; 32]) -> Result<String, CryptoError> {
    let plaintext =
        serde_json::to_vec(value).map_err(|error| CryptoError::Serialize(error.to_string()))?;
    encrypt_bytes(&plaintext, key)
}

/// 解密 `base64(IV ‖ 密文)`，剥除 PKCS#7 填充。
pub fn decrypt_bytes(encoded: &str, key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    let raw = BASE64
        .decode(encoded)
        .map_err(|_| CryptoError::InvalidBase64)?;
    if raw.len() <= BLOCK_SIZE || (raw.len() - BLOCK_SIZE) % BLOCK_SIZE != 0 {
        return Err(CryptoError::InvalidLength);
    }
    let (iv, ciphertext) = raw.split_at(BLOCK_SIZE);
    let iv: [u8; BLOCK_SIZE] = iv.try_into().map_err(|_| CryptoError::InvalidLength)?;

    let mut buffer = ciphertext.to_vec();
    let cipher = Aes256CbcDec::new(key.into(), (&iv).into());
    let plain = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| CryptoError::Decrypt)?;
    Ok(plain.to_vec())
}

/// 计算 token 的 SHA-256 摘要，返回 64 字符小写 hex。
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(HASH_HEX_LEN);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    hex
}

/// 常数时间校验 token 是否匹配库存摘要；库存值不是 64 位 hex 时 fail-closed。
pub fn verify_token(presented: &str, stored_hash: &str) -> bool {
    let normalized = stored_hash.trim().to_ascii_lowercase();
    if normalized.len() != HASH_HEX_LEN || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let computed = hash_token(presented);
    computed.as_bytes().ct_eq(normalized.as_bytes()).into()
}

/// 常数时间比较两个明文 Token；长度不同直接 false。
pub fn tokens_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    left.ct_eq(right).into()
}

/// 解密 CBC 密文但不剥除填充（仅供测试验证 PKCS#7）。
#[cfg(test)]
fn decrypt_cbc(ciphertext: &[u8], iv: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.is_empty() || ciphertext.len() % BLOCK_SIZE != 0 {
        return Err(CryptoError::InvalidLength);
    }
    if iv.len() != BLOCK_SIZE {
        return Err(CryptoError::InvalidLength);
    }
    let iv: [u8; BLOCK_SIZE] = iv.try_into().map_err(|_| CryptoError::InvalidLength)?;
    let mut buffer = ciphertext.to_vec();
    let cipher = Aes256CbcDec::new(key.into(), (&iv).into());
    let decrypted = cipher
        .decrypt_padded_mut::<aes::cipher::block_padding::NoPadding>(&mut buffer)
        .map_err(|_| CryptoError::Decrypt)?;
    Ok(decrypted.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_derivation_is_sha256_of_raw_text() {
        // sha256("") 的已知摘要：证明 derive_key 没有做 hex/base64 猜测。
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(derive_key(""), expected);
    }

    #[test]
    fn round_trip_preserves_bytes() {
        let key = derive_key("secret");
        let plaintext = "飞书外部选项".as_bytes();
        let cipher = encrypt_bytes(plaintext, &key).unwrap_or_else(|error| panic!("{error}"));
        let decoded = decrypt_bytes(&cipher, &key).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(decoded, plaintext);
    }

    #[test]
    fn aligned_plaintext_still_appends_a_full_padding_block() {
        let key = derive_key("secret");
        let plaintext = [7u8; 16];
        let encrypted = encrypt_bytes(&plaintext, &key).unwrap();
        let raw = BASE64.decode(encrypted).unwrap();
        assert_eq!((raw.len() - 16) % 16, 0);
        // 16 字节明文 + PKCS#7 整块 = 32 字节密文
        assert_eq!(raw.len() - 16, 32);

        let (iv, ciphertext) = raw.split_at(16);
        let padded = decrypt_cbc(ciphertext, iv, &key).unwrap();
        assert_eq!(&padded[16..], &[16u8; 16]);
    }

    #[test]
    fn partial_block_padding_is_pkcs7() {
        let key = derive_key("secret");
        let plaintext = [3u8; 20];
        let encrypted = encrypt_bytes(&plaintext, &key).unwrap();
        let raw = BASE64.decode(encrypted).unwrap();
        let (iv, ciphertext) = raw.split_at(16);
        let padded = decrypt_cbc(ciphertext, iv, &key).unwrap();
        assert_eq!(padded.len(), 32);
        assert_eq!(&padded[20..], &[12u8; 12]);
    }

    #[test]
    fn iv_is_prefixed_and_random() {
        let key = derive_key("secret");
        let one = encrypt_bytes(b"same", &key).unwrap();
        let two = encrypt_bytes(b"same", &key).unwrap();
        assert_ne!(one, two);
        let raw = BASE64.decode(one).unwrap();
        assert_eq!(raw.len(), 16 + 16);
    }

    #[test]
    fn encrypt_json_is_deterministic_in_shape() {
        let key = derive_key("secret");
        let value = serde_json::json!({"code": 0});
        let cipher = encrypt_json(&value, &key).unwrap();
        let plain = decrypt_bytes(&cipher, &key).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plain).unwrap(),
            value
        );
    }

    #[test]
    fn wrong_key_or_tampering_never_returns_the_original() {
        let key = derive_key("secret");
        let plaintext = b"payload";
        let cipher = encrypt_bytes(plaintext, &key).unwrap();

        // CBC 的填充校验是概率性的（随机块约 1/256 恰好合法），因此这里断言
        // 「绝不还原出原文」，而不是硬要求 Err。
        if let Ok(bytes) = decrypt_bytes(&cipher, &derive_key("other")) {
            assert_ne!(bytes, plaintext);
        }

        let mut raw = BASE64.decode(&cipher).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 1;
        if let Ok(bytes) = decrypt_bytes(&BASE64.encode(raw), &key) {
            assert_ne!(bytes, plaintext);
        }

        // 确定性的畸形密文必须失败。
        let mut truncated = BASE64.decode(&cipher).unwrap();
        truncated.pop();
        assert!(decrypt_bytes(&BASE64.encode(truncated), &key).is_err());
    }

    #[test]
    fn malformed_ciphertexts_are_rejected() {
        let key = derive_key("secret");
        assert!(matches!(
            decrypt_bytes("not-base64!!", &key),
            Err(CryptoError::InvalidBase64)
        ));
        assert!(matches!(
            decrypt_bytes(&BASE64.encode([0u8; 16]), &key),
            Err(CryptoError::InvalidLength)
        ));
    }

    #[test]
    fn hash_is_lowercase_hex_sha256() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_token_is_fail_closed_and_normalizes_the_stored_value() {
        let stored = hash_token("token");
        assert!(verify_token("token", &stored));
        assert!(verify_token("token", &stored.to_uppercase()));
        assert!(verify_token("token", &format!("  {stored}\n")));
        assert!(!verify_token("other", &stored));
        assert!(!verify_token("token", "not-a-hash"));
        assert!(!verify_token("token", &"a".repeat(63)));
    }

    #[test]
    fn tokens_equal_is_length_safe() {
        assert!(tokens_equal("feishu-token", "feishu-token"));
        assert!(!tokens_equal("feishu-token", "feishu-tokeN"));
        assert!(!tokens_equal("short", "longer"));
    }
}
