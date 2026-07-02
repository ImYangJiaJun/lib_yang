// 配置摘要生成
// 用于生成配置的稳定哈希，用于缓存和回归验证

use crate::config::GenerationConfig;
use crate::error::{PcgError, PcgResult};
use crate::rng::fnv1a_64;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 验证配置中所有 f32 字段不含 NaN。
///
/// serde_json 将 f32::NAN 序列化为 `null`（JSON 不支持 NaN），
/// 这会导致两个问题：(1) 序列化后的 JSON 丢失原始值信息；
/// (2) 不同 NaN 位的 NaN 值产生相同摘要，违反摘要唯一性契约。
/// 本函数在序列化前做显式校验，提供清晰的错误信息。
fn validate_no_nan(config: &GenerationConfig) -> PcgResult<()> {
    if config.terrain.obstacle_density.is_nan() {
        return Err(PcgError::config_with_field(
            "obstacle_density 不能为 NaN",
            "terrain.obstacle_density",
        ));
    }
    if config.terrain.min_walkable_ratio.is_nan() {
        return Err(PcgError::config_with_field(
            "min_walkable_ratio 不能为 NaN",
            "terrain.min_walkable_ratio",
        ));
    }
    if config.item_spawns.rarity_weights.iter().any(|w| w.is_nan()) {
        return Err(PcgError::config_with_field(
            "rarity_weights 不能包含 NaN",
            "item_spawns.rarity_weights",
        ));
    }
    Ok(())
}

/// 配置摘要
///
/// 为生成配置生成稳定的哈希值，用于：
/// - 缓存键生成
/// - 回归验证
/// - 导出签名
///
/// 摘要保证：
/// - 相同配置生成相同摘要（同一编译二进制内确定）
/// - 不同配置生成不同摘要（高概率）
/// - 跨 Rust 版本稳定性取决于 FNV-1a (fnv1a_64) 的算法不变性——FNV 是固定规范，不随 Rust 版本变化
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ConfigDigest {
    /// 十六进制哈希字符串
    hash: String,
}

impl ConfigDigest {
    /// 从配置生成摘要
    ///
    /// # 错误
    ///
    /// - 当配置包含 NaN 的 `f32` 字段时返回 `PcgError::Config`
    /// - 当配置序列化失败时返回 `PcgError::Config`
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_pcg::config::GenerationConfig;
    /// use yang_pcg::digest::ConfigDigest;
    ///
    /// let config = GenerationConfig::default();
    /// let digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化");
    /// println!("配置摘要: {}", digest.as_str());
    /// ```
    pub fn from_config(config: &GenerationConfig) -> PcgResult<Self> {
        let (_, hash) = Self::seed_and_digest_from_config(config)?;
        Ok(Self { hash })
    }

    /// 从配置派生确定性种子（u64）。
    ///
    /// 当生成请求未显式提供 `seed` 时，用本方法从配置派生一个**确定性**的兜底种子，
    /// 保证「相同 config + 不提供 seed」始终产出相同地图——符合确定性库的契约。
    /// 与 [`from_config`](Self::from_config) 使用相同的稳定哈希逻辑。
    ///
    /// # 错误
    ///
    /// - 当配置包含 NaN 的 `f32` 字段时返回 `PcgError::Config`
    /// - 当配置序列化失败时返回 `PcgError::Config`
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_pcg::config::GenerationConfig;
    /// use yang_pcg::digest::ConfigDigest;
    ///
    /// let config = GenerationConfig::default();
    /// // 同一 config 多次派生得到相同种子
    /// assert_eq!(
    ///     ConfigDigest::seed_from_config(&config).unwrap(),
    ///     ConfigDigest::seed_from_config(&config).unwrap()
    /// );
    /// ```
    pub fn seed_from_config(config: &GenerationConfig) -> PcgResult<u64> {
        let (seed, _) = Self::seed_and_digest_from_config(config)?;
        Ok(seed)
    }

    /// 一次性从配置派生种子和摘要字符串。
    ///
    /// 内部只做一次 `serde_json::to_string`，避免调用方分别调用
    /// `seed_from_config` + `from_config` 导致重复序列化。
    ///
    /// # 错误
    ///
    /// - 当配置包含 NaN 的 `f32` 字段时返回 `PcgError::Config`
    /// - 当配置序列化失败时返回 `PcgError::Config`
    pub fn seed_and_digest_from_config(config: &GenerationConfig) -> PcgResult<(u64, String)> {
        validate_no_nan(config)?;
        let json = serde_json::to_string(config)
            .map_err(|e| PcgError::config(format!("GenerationConfig 序列化失败: {}", e)))?;
        let hash_value = fnv1a_64(json.as_bytes());
        Ok((hash_value, format!("{:016x}", hash_value)))
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
        Self::from_config(config).map_or(false, |expected| self.hash == expected.hash)
    }
}

impl std::fmt::Display for ConfigDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hash)
    }
}

impl Serialize for ConfigDigest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hash)
    }
}

impl<'de> Deserialize<'de> for ConfigDigest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hash = String::deserialize(deserializer)?;
        Ok(Self::from_string(hash))
    }
}

impl From<&GenerationConfig> for ConfigDigest {
    fn from(config: &GenerationConfig) -> Self {
        Self::from_config(config).expect("默认配置生成摘要不应失败")
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
        let digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化");

        // 摘要应该是 16 个十六进制字符
        assert_eq!(digest.as_str().len(), 16);
        assert!(digest.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_digest_stability() {
        let config = GenerationConfig::default();

        // 多次生成摘要应该得到相同结果
        let digest1 = ConfigDigest::from_config(&config).expect("默认配置应可序列化");
        let digest2 = ConfigDigest::from_config(&config).expect("默认配置应可序列化");

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

        let digest1 = ConfigDigest::from_config(&config1).expect("配置应可序列化");
        let digest2 = ConfigDigest::from_config(&config2).expect("配置应可序列化");

        // 不同配置应该生成不同摘要
        assert_ne!(digest1, digest2);
        assert_ne!(digest1.as_str(), digest2.as_str());
    }

    #[test]
    fn test_digest_matches() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化");

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
        let digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化");

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

        let digest1 = ConfigDigest::from_config(&config1).expect("配置应可序列化");
        let digest2 = ConfigDigest::from_config(&config2).expect("配置应可序列化");

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

        let digest1 = ConfigDigest::from_config(&config1).expect("配置应可序列化");
        let digest2 = ConfigDigest::from_config(&config2).expect("配置应可序列化");

        // 主题标签的变化也应该影响摘要
        assert_ne!(digest1, digest2);
    }

    #[test]
    fn test_digest_into_string() {
        let config = GenerationConfig::default();
        let digest = ConfigDigest::from_config(&config).expect("默认配置应可序列化");
        let hash_str = digest.as_str().to_string();

        let owned_str = digest.into_string();
        assert_eq!(owned_str, hash_str);
    }
}
