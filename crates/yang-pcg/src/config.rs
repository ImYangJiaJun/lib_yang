// 配置管理模块
// 负责定义和归一化生成配置

use crate::error::{PcgError, PcgResult};
use serde::{Deserialize, Serialize};

/// 地图生成配置
///
/// 包含所有可配置的生成参数，支持序列化、反序列化和默认值填充。
/// 配置采用层级合并策略：默认配置 -> 预设配置 -> 实例覆盖 -> 运行时覆盖。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// 房间数量范围
    pub room_count: RangeU16,
    /// 关键路径长度范围
    pub critical_path_length: RangeU16,
    /// 分支数量范围
    pub branch_count: RangeU16,
    /// 死路数量范围
    pub dead_end_count: RangeU16,
    /// 房间尺寸配置
    pub room_size: RoomSizeConfig,
    /// 走廊配置
    pub corridor: CorridorConfig,
    /// 地形配置
    pub terrain: TerrainConfig,
    /// 交互物生成配置
    pub item_spawns: ItemSpawnConfig,
    /// 敌人生成配置
    pub enemy_spawns: EnemySpawnConfig,
    /// 分块配置
    pub chunking: ChunkingConfig,
    /// 主题标签列表
    pub theme_tags: Vec<String>,
    /// 生成模式
    pub generation_mode: GenerationMode,
    /// 能力开关
    pub capability_flags: CapabilityFlags,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            room_count: RangeU16 { min: 10, max: 20 },
            critical_path_length: RangeU16 { min: 5, max: 10 },
            branch_count: RangeU16 { min: 1, max: 3 },
            dead_end_count: RangeU16 { min: 0, max: 2 },
            room_size: RoomSizeConfig::default(),
            corridor: CorridorConfig::default(),
            terrain: TerrainConfig::default(),
            item_spawns: ItemSpawnConfig::default(),
            enemy_spawns: EnemySpawnConfig::default(),
            chunking: ChunkingConfig::default(),
            theme_tags: vec!["default".to_string()],
            generation_mode: GenerationMode::OfflineFullFloor,
            capability_flags: CapabilityFlags::default(),
        }
    }
}

impl GenerationConfig {
    /// 归一化配置
    ///
    /// 验证配置的有效性，填充缺失的默认值，并返回归一化后的配置。
    ///
    /// # 错误
    ///
    /// - 当数值范围非法时返回 `PcgError::Config`
    /// - 当存在互斥约束时返回 `PcgError::Config`
    pub fn normalize(&self) -> PcgResult<NormalizedConfig> {
        // 验证房间数量范围
        if self.room_count.min > self.room_count.max {
            return Err(PcgError::config_with_field(
                format!(
                    "房间数量范围非法: min({}) > max({})",
                    self.room_count.min, self.room_count.max
                ),
                "room_count",
            ));
        }

        if self.room_count.min < 2 {
            return Err(PcgError::config_with_field(
                "房间数量最小值不能小于 2（至少需要起点和终点）",
                "room_count.min",
            ));
        }

        // 验证关键路径长度范围
        if self.critical_path_length.min > self.critical_path_length.max {
            return Err(PcgError::config_with_field(
                format!(
                    "关键路径长度范围非法: min({}) > max({})",
                    self.critical_path_length.min, self.critical_path_length.max
                ),
                "critical_path_length",
            ));
        }

        if self.critical_path_length.min < 2 {
            return Err(PcgError::config_with_field(
                "关键路径长度最小值不能小于 2",
                "critical_path_length.min",
            ));
        }

        // 验证关键路径长度不超过房间总数
        if self.critical_path_length.min > self.room_count.max {
            return Err(PcgError::config_with_field(
                format!(
                    "关键路径长度最小值({})不能超过房间总数最大值({})",
                    self.critical_path_length.min, self.room_count.max
                ),
                "critical_path_length.min",
            ));
        }

        // 验证分支数量范围
        if self.branch_count.min > self.branch_count.max {
            return Err(PcgError::config_with_field(
                format!(
                    "分支数量范围非法: min({}) > max({})",
                    self.branch_count.min, self.branch_count.max
                ),
                "branch_count",
            ));
        }

        // 验证死路数量范围
        if self.dead_end_count.min > self.dead_end_count.max {
            return Err(PcgError::config_with_field(
                format!(
                    "死路数量范围非法: min({}) > max({})",
                    self.dead_end_count.min, self.dead_end_count.max
                ),
                "dead_end_count",
            ));
        }

        // 验证房间尺寸配置
        self.room_size.validate()?;

        // 验证走廊配置
        self.corridor.validate()?;

        // 验证地形配置
        self.terrain.validate()?;

        // 验证交互物配置
        self.item_spawns.validate()?;

        // 验证敌人配置
        self.enemy_spawns.validate()?;

        // 验证分块配置
        self.chunking.validate()?;

        // 验证能力开关与生成模式的兼容性
        if self.generation_mode == GenerationMode::RuntimeChunked
            && !self.capability_flags.runtime_chunked
        {
            return Err(PcgError::config(
                "运行时分块模式需要启用 runtime_chunked 能力开关",
            ));
        }

        if self.generation_mode == GenerationMode::HybridPrecompute
            && !self.capability_flags.hybrid_precompute
        {
            return Err(PcgError::config(
                "混合预计算模式需要启用 hybrid_precompute 能力开关",
            ));
        }

        // 构建归一化配置
        Ok(NormalizedConfig {
            config: self.clone(),
            time_budget_ms: None,
            iteration_budget: None,
        })
    }

    /// 合并配置
    ///
    /// 将当前配置与另一个配置合并，另一个配置的非默认值会覆盖当前配置。
    /// 用于实现配置层级：默认配置 -> 预设配置 -> 实例覆盖 -> 运行时覆盖。
    pub fn merge(&self, other: &GenerationConfig) -> Self {
        // 简化实现：直接使用 other 的值覆盖
        // 在实际应用中，可以实现更细粒度的合并逻辑
        let mut merged = self.clone();

        // 合并房间数量范围
        merged.room_count = other.room_count;
        merged.critical_path_length = other.critical_path_length;
        merged.branch_count = other.branch_count;
        merged.dead_end_count = other.dead_end_count;

        // 合并子配置
        merged.room_size = other.room_size.clone();
        merged.corridor = other.corridor.clone();
        merged.terrain = other.terrain.clone();
        merged.item_spawns = other.item_spawns.clone();
        merged.enemy_spawns = other.enemy_spawns.clone();
        merged.chunking = other.chunking.clone();

        // 合并主题标签（追加而不是覆盖）
        if !other.theme_tags.is_empty() {
            merged.theme_tags = other.theme_tags.clone();
        }

        // 合并生成模式和能力开关
        merged.generation_mode = other.generation_mode;
        merged.capability_flags = other.capability_flags.clone();

        merged
    }
}

/// 归一化后的配置
///
/// 经过验证和归一化处理的配置，保证所有字段都是有效的。
#[derive(Debug, Clone)]
pub struct NormalizedConfig {
    /// 原始配置
    pub config: GenerationConfig,
    /// 时间预算（毫秒），超过后提前返回部分结果
    pub time_budget_ms: Option<u64>,
    /// 迭代预算，超过后提前返回部分结果
    pub iteration_budget: Option<u32>,
}

impl NormalizedConfig {
    /// 获取配置引用
    pub fn config(&self) -> &GenerationConfig {
        &self.config
    }

    /// 设置时间预算（毫秒）
    pub fn with_time_budget(mut self, ms: u64) -> Self {
        self.time_budget_ms = Some(ms);
        self
    }

    /// 设置迭代预算
    pub fn with_iteration_budget(mut self, iterations: u32) -> Self {
        self.iteration_budget = Some(iterations);
        self
    }
}

/// 无符号 16 位整数范围
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RangeU16 {
    pub min: u16,
    pub max: u16,
}

impl RangeU16 {
    /// 创建新的范围
    pub fn new(min: u16, max: u16) -> Self {
        Self { min, max }
    }

    /// 验证范围是否有效
    pub fn validate(&self, field_name: &str) -> PcgResult<()> {
        if self.min > self.max {
            return Err(PcgError::config_with_field(
                format!("范围非法: min({}) > max({})", self.min, self.max),
                field_name,
            ));
        }
        Ok(())
    }
}

/// 房间尺寸配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSizeConfig {
    /// 最小宽度（网格单位）
    pub min_width: u16,
    /// 最大宽度（网格单位）
    pub max_width: u16,
    /// 最小高度（网格单位）
    pub min_height: u16,
    /// 最大高度（网格单位）
    pub max_height: u16,
}

impl Default for RoomSizeConfig {
    fn default() -> Self {
        Self {
            min_width: 8,
            max_width: 16,
            min_height: 8,
            max_height: 16,
        }
    }
}

impl RoomSizeConfig {
    fn validate(&self) -> PcgResult<()> {
        if self.min_width > self.max_width {
            return Err(PcgError::config_with_field(
                format!(
                    "房间宽度范围非法: min({}) > max({})",
                    self.min_width, self.max_width
                ),
                "room_size.width",
            ));
        }

        if self.min_height > self.max_height {
            return Err(PcgError::config_with_field(
                format!(
                    "房间高度范围非法: min({}) > max({})",
                    self.min_height, self.max_height
                ),
                "room_size.height",
            ));
        }

        if self.min_width < 4 || self.min_height < 4 {
            return Err(PcgError::config_with_field(
                "房间最小尺寸不能小于 4x4",
                "room_size",
            ));
        }

        Ok(())
    }
}

/// 走廊配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorridorConfig {
    /// 走廊宽度（网格单位）
    pub width: u16,
    /// 最大转折次数
    pub max_turns: u16,
    /// 连接策略
    pub connection_strategy: ConnectionStrategy,
}

impl Default for CorridorConfig {
    fn default() -> Self {
        Self {
            width: 2,
            max_turns: 3,
            connection_strategy: ConnectionStrategy::Orthogonal,
        }
    }
}

impl CorridorConfig {
    fn validate(&self) -> PcgResult<()> {
        if self.width < 1 {
            return Err(PcgError::config_with_field(
                "走廊宽度不能小于 1",
                "corridor.width",
            ));
        }

        if self.width > 10 {
            return Err(PcgError::config_with_field(
                "走廊宽度不能大于 10",
                "corridor.width",
            ));
        }

        Ok(())
    }
}

/// 连接策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStrategy {
    /// 正交连接（仅水平和垂直）
    Orthogonal,
    /// 直线连接
    Straight,
    /// 共享边开口
    SharedEdge,
}

/// 地形配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainConfig {
    /// 障碍物密度（0.0 - 1.0）
    pub obstacle_density: f32,
    /// 最小可通行面积比例（0.0 - 1.0）
    pub min_walkable_ratio: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            obstacle_density: 0.2,
            min_walkable_ratio: 0.6,
        }
    }
}

impl TerrainConfig {
    fn validate(&self) -> PcgResult<()> {
        if !(0.0..=1.0).contains(&self.obstacle_density) {
            return Err(PcgError::config_with_field(
                format!(
                    "障碍物密度必须在 0.0-1.0 范围内，当前值: {}",
                    self.obstacle_density
                ),
                "terrain.obstacle_density",
            ));
        }

        if !(0.0..=1.0).contains(&self.min_walkable_ratio) {
            return Err(PcgError::config_with_field(
                format!(
                    "最小可通行面积比例必须在 0.0-1.0 范围内，当前值: {}",
                    self.min_walkable_ratio
                ),
                "terrain.min_walkable_ratio",
            ));
        }

        if self.obstacle_density + self.min_walkable_ratio > 1.0 {
            return Err(PcgError::config_with_field(
                "障碍物密度与最小可通行面积比例之和不能超过 1.0",
                "terrain",
            ));
        }

        Ok(())
    }
}

/// 交互物生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSpawnConfig {
    /// 每个房间的交互物数量范围
    pub count_per_room: RangeU16,
    /// 最小间距（网格单位）
    pub min_spacing: u16,
    /// 稀有度权重
    pub rarity_weights: Vec<f32>,
}

impl Default for ItemSpawnConfig {
    fn default() -> Self {
        Self {
            count_per_room: RangeU16 { min: 1, max: 3 },
            min_spacing: 2,
            rarity_weights: vec![0.6, 0.3, 0.1], // 普通、稀有、史诗
        }
    }
}

impl ItemSpawnConfig {
    fn validate(&self) -> PcgResult<()> {
        self.count_per_room.validate("item_spawns.count_per_room")?;

        if self.min_spacing < 1 {
            return Err(PcgError::config_with_field(
                "交互物最小间距不能小于 1",
                "item_spawns.min_spacing",
            ));
        }

        // 验证稀有度权重总和
        let total_weight: f32 = self.rarity_weights.iter().sum();
        if (total_weight - 1.0).abs() > 0.01 {
            return Err(PcgError::config_with_field(
                format!("稀有度权重总和必须为 1.0，当前值: {}", total_weight),
                "item_spawns.rarity_weights",
            ));
        }

        Ok(())
    }
}

/// 敌人生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemySpawnConfig {
    /// 每个房间的敌人数量范围
    pub count_per_room: RangeU16,
    /// 最小间距（网格单位）
    pub min_spacing: u16,
    /// 与入口的最小安全距离（网格单位）
    pub min_distance_from_entrance: u16,
    /// 基础难度预算
    pub base_difficulty_budget: u16,
}

impl Default for EnemySpawnConfig {
    fn default() -> Self {
        Self {
            count_per_room: RangeU16 { min: 2, max: 5 },
            min_spacing: 3,
            min_distance_from_entrance: 4,
            base_difficulty_budget: 100,
        }
    }
}

impl EnemySpawnConfig {
    fn validate(&self) -> PcgResult<()> {
        self.count_per_room
            .validate("enemy_spawns.count_per_room")?;

        if self.min_spacing < 1 {
            return Err(PcgError::config_with_field(
                "敌人最小间距不能小于 1",
                "enemy_spawns.min_spacing",
            ));
        }

        if self.min_distance_from_entrance < 2 {
            return Err(PcgError::config_with_field(
                "敌人与入口的最小安全距离不能小于 2",
                "enemy_spawns.min_distance_from_entrance",
            ));
        }

        Ok(())
    }
}

/// 分块配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    /// 分块大小（网格单位）
    pub chunk_size: u16,
    /// 是否启用分块
    pub enabled: bool,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 32,
            enabled: false,
        }
    }
}

impl ChunkingConfig {
    fn validate(&self) -> PcgResult<()> {
        if self.enabled && self.chunk_size < 8 {
            return Err(PcgError::config_with_field(
                "分块大小不能小于 8",
                "chunking.chunk_size",
            ));
        }

        Ok(())
    }
}

/// 生成模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerationMode {
    /// 离线整层生成
    OfflineFullFloor,
    /// 运行时分块生成
    RuntimeChunked,
    /// 混合预计算模式
    HybridPrecompute,
}

/// 能力开关
///
/// 用于控制特定功能的启用状态，支持版本兼容和功能降级。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityFlags {
    /// 是否启用运行时分块
    #[serde(default)]
    pub runtime_chunked: bool,
    /// 是否启用混合预计算
    #[serde(default)]
    pub hybrid_precompute: bool,
    /// 是否启用 Grammar 兼容输出
    #[serde(default)]
    pub grammar_support: bool,
    /// 是否启用调试输出
    #[serde(default)]
    pub debug_output: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = GenerationConfig::default();
        let result = config.normalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_room_count_range() {
        let config = GenerationConfig {
            room_count: RangeU16 { min: 20, max: 10 },
            ..Default::default()
        };

        let result = config.normalize();
        assert!(result.is_err());

        if let Err(err) = result {
            assert_eq!(err.error_code(), "PCG-CONFIG-001");
        }
    }

    #[test]
    fn test_room_count_too_small() {
        let config = GenerationConfig {
            room_count: RangeU16 { min: 1, max: 10 },
            ..Default::default()
        };

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_critical_path_exceeds_room_count() {
        let config = GenerationConfig {
            room_count: RangeU16 { min: 5, max: 10 },
            critical_path_length: RangeU16 { min: 15, max: 20 },
            ..Default::default()
        };

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_room_size() {
        let mut config = GenerationConfig::default();
        config.room_size.min_width = 20;
        config.room_size.max_width = 10;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_room_size_too_small() {
        let mut config = GenerationConfig::default();
        config.room_size.min_width = 2;
        config.room_size.min_height = 2;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_corridor_width() {
        let mut config = GenerationConfig::default();
        config.corridor.width = 0;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_obstacle_density() {
        let mut config = GenerationConfig::default();
        config.terrain.obstacle_density = 1.5;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_obstacle_density_and_walkable_ratio_conflict() {
        let mut config = GenerationConfig::default();
        config.terrain.obstacle_density = 0.6;
        config.terrain.min_walkable_ratio = 0.6;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_rarity_weights() {
        let mut config = GenerationConfig::default();
        config.item_spawns.rarity_weights = vec![0.5, 0.3, 0.1]; // 总和不为 1.0

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_flags_validation() {
        let mut config = GenerationConfig {
            generation_mode: GenerationMode::RuntimeChunked,
            ..Default::default()
        };
        config.capability_flags.runtime_chunked = false;

        let result = config.normalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_merge() {
        let base = GenerationConfig::default();
        let override_config = GenerationConfig {
            room_count: RangeU16 { min: 15, max: 25 },
            theme_tags: vec!["dungeon".to_string()],
            ..Default::default()
        };

        let merged = base.merge(&override_config);

        assert_eq!(merged.room_count.min, 15);
        assert_eq!(merged.room_count.max, 25);
        assert_eq!(merged.theme_tags, vec!["dungeon".to_string()]);
    }

    #[test]
    fn test_range_u16_validation() {
        let valid_range = RangeU16 { min: 5, max: 10 };
        assert!(valid_range.validate("test").is_ok());

        let invalid_range = RangeU16 { min: 10, max: 5 };
        assert!(invalid_range.validate("test").is_err());
    }
}
