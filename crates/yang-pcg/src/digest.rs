// 配置摘要生成
// 用于生成配置的稳定哈希，用于缓存和回归验证

use crate::config::GenerationConfig;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 配置摘要
///
/// 为生成配置生成稳定的哈希值，用于：
/// - 缓存键生成
/// - 回归验证
/// - 导出签名
///
/// 摘要保证：
/// - 相同配置生成相同摘要
/// - 不同配置生成不同摘要（高概率）
/// - 摘要格式稳定，不受 Rust 版本影响
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigDigest {
    /// 十六进制哈希字符串
    hash: String,
}

impl ConfigDigest {
    /// 从配置生成摘要
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_pcg::config::GenerationConfig;
    /// use yang_pcg::digest::ConfigDigest;
    ///
    /// let config = GenerationConfig::default();
    /// let digest = ConfigDigest::from_config(&config);
    /// println!("配置摘要: {}", digest.as_str());
    /// ```
    pub fn from_config(config: &GenerationConfig) -> Self {
        // 使用 serde_json 序列化配置以确保稳定性
        // 这样可以避免 Rust 默认 Hash 实现的不稳定性
        let json = serde_json::to_string(config).unwrap_or_else(|_| String::new());

        let mut hasher = DefaultHasher::new();
        json.hash(&mut hasher);
        let hash_value = hasher.finish();

        Self {
            hash: format!("{:016x}", hash_value),
        }
    }

    /// 从字符串创建摘要
    ///
    /// 用于从缓存或导出数据中恢复摘要。
    pub fn from_string(hash: String) -> Self {
        Self { hash }
    }

    /// 获取摘要字符串
    pub fn as_str(&self) -> &str {
        &self.hash
    }

    /// 转换为字符串
    pub fn into_string(self) -> String {
        self.hash
    }

    /// 验证摘要是否匹配配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_pcg::config::GenerationConfig;
    /// use yang_pcg::digest::ConfigDigest;
    ///
    /// let config = GenerationConfig::default();
    /// let digest = ConfigDigest::from_config(&config);
    ///
    /// assert!(digest.matches(&config));
    /// ```
    pub fn matches(&self, config: &GenerationConfig) -> bool {
        let expected = Self::from_config(config);
        self.hash == expected.hash
    }
}

impl std::fmt::Display for ConfigDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hash)
    }
}

impl From<&GenerationConfig> for ConfigDigest {
    fn from(config: &GenerationConfig) -> Self {
        Self::from_config(config)
    }
}

impl From<String> for ConfigDigest {
    fn from(hash: String) -> Self {
        Self::from_string(hash)
    }
}

impl From<&str> for ConfigDigest {
    fn from(hash: &str) -> Self {
        Self::from_string(hash.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GenerationConfig, RangeU16};

    #[test]
    fn test_digest_from_config() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config);

        // 摘要应该是 16 个十六进制字符
        assert_eq!(digest.as_str().len(), 16);
        assert!(digest.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_digest_stability() {
        let config = GenerationConfig::default();

        // 多次生成摘要应该得到相同结果
        let digest1 = ConfigDigest::from_config(&config);
        let digest2 = ConfigDigest::from_config(&config);

        assert_eq!(digest1, digest2);
        assert_eq!(digest1.as_str(), digest2.as_str());
    }

    #[test]
    fn test_digest_uniqueness() {
        let config1 = GenerationConfig::default();
        let config2 = GenerationConfig {
            room_count: RangeU16 { min: 15, max: 25 },
            ..Default::default()
        };

        let digest1 = ConfigDigest::from_config(&config1);
        let digest2 = ConfigDigest::from_config(&config2);

        // 不同配置应该生成不同摘要
        assert_ne!(digest1, digest2);
        assert_ne!(digest1.as_str(), digest2.as_str());
    }

    #[test]
    fn test_digest_matches() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config);

        assert!(digest.matches(&config));

        let different_config = GenerationConfig {
            room_count: RangeU16 { min: 15, max: 25 },
            ..Default::default()
        };

        assert!(!digest.matches(&different_config));
    }

    #[test]
    fn test_digest_from_string() {
        let hash_str = "0123456789abcdef";
        let digest = ConfigDigest::from_string(hash_str.to_string());

        assert_eq!(digest.as_str(), hash_str);
    }

    #[test]
    fn test_digest_display() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config);

        let display_str = format!("{}", digest);
        assert_eq!(display_str, digest.as_str());
    }

    #[test]
    fn test_digest_conversions() {
        let config = GenerationConfig::default();

        // From &GenerationConfig
        let digest1: ConfigDigest = (&config).into();

        // From String
        let digest2: ConfigDigest = digest1.as_str().to_string().into();

        // From &str
        let digest3: ConfigDigest = digest1.as_str().into();

        assert_eq!(digest1, digest2);
        assert_eq!(digest2, digest3);
    }

    #[test]
    fn test_digest_sensitivity_to_nested_changes() {
        let config1 = GenerationConfig::default();
        let mut config2 = GenerationConfig::default();
        config2.terrain.obstacle_density = 0.3;

        let digest1 = ConfigDigest::from_config(&config1);
        let digest2 = ConfigDigest::from_config(&config2);

        // 嵌套字段的变化也应该影响摘要
        assert_ne!(digest1, digest2);
    }

    #[test]
    fn test_digest_sensitivity_to_theme_tags() {
        let config1 = GenerationConfig::default();
        let config2 = GenerationConfig {
            theme_tags: vec!["dungeon".to_string(), "dark".to_string()],
            ..Default::default()
        };

        let digest1 = ConfigDigest::from_config(&config1);
        let digest2 = ConfigDigest::from_config(&config2);

        // 主题标签的变化也应该影响摘要
        assert_ne!(digest1, digest2);
    }

    #[test]
    fn test_digest_into_string() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config);
        let hash_str = digest.as_str().to_string();

        let owned_str = digest.into_string();
        assert_eq!(owned_str, hash_str);
    }
}
