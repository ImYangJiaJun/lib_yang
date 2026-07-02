// Grammar 确定性权重选择器
// 根据朝向、房间主题、走廊长度和房间类型进行确定性规则选择
// 需求映射：13.4

use crate::error::{PcgError, PcgResult};
use crate::model::geometry::CardinalDir;
use crate::model::room::RoomType;
use crate::rng::StableRng;

/// Grammar 规则定义
///
/// 表示一条可选的 Grammar 规则，包含名称和基础权重。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GrammarRule {
    /// 规则名称（对应外部 Grammar 系统的规则标识）
    pub name: String,
    /// 基础权重（未经上下文调整的原始权重）
    pub base_weight: f64,
}

/// Grammar 上下文
///
/// 提供确定性权重选择所需的上下文信息，包括朝向、房间主题、走廊长度和房间类型。
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct GrammarContext {
    /// 朝向（影响规则权重调整）
    pub facing: Option<CardinalDir>,
    /// 房间主题标签列表
    pub theme_tags: Vec<String>,
    /// 走廊长度（瓦片数，可选）
    pub corridor_length: Option<u16>,
    /// 房间类型
    pub room_type: Option<RoomType>,
}

/// 确定性权重选择器
///
/// 在 Grammar 模式下根据上下文信息对规则权重进行确定性调整，
/// 并使用 `StableRng` 进行确定性选择，确保同种子下结果可复现。
///
/// # 权重调整规则
///
/// 1. **朝向匹配**：规则名称包含朝向关键词时权重 ×2.0
/// 2. **主题匹配**：规则名称包含房间主题标签时权重 ×1.5
/// 3. **走廊长度**：长走廊（>10）偏好 "long" 规则 ×1.8，短走廊（≤5）偏好 "short" 规则 ×1.8
/// 4. **房间类型**：规则名称包含房间类型关键词时权重 ×1.5
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::grammar::{GrammarContext, GrammarRule, WeightedRuleSelector};
/// use yang_pcg::model::geometry::CardinalDir;
/// use yang_pcg::rng::StableRng;
///
/// let rules = vec![
///     GrammarRule { name: "corridor_north_long".to_string(), base_weight: 1.0 },
///     GrammarRule { name: "corridor_south_short".to_string(), base_weight: 1.0 },
/// ];
/// let context = GrammarContext {
///     facing: Some(CardinalDir::North),
///     corridor_length: Some(15),
///     ..Default::default()
/// };
/// let mut rng = StableRng::from_seed(42);
/// let selector = WeightedRuleSelector;
/// let selected = selector.select(&rules, &context, &mut rng).unwrap();
/// ```
#[non_exhaustive]
pub struct WeightedRuleSelector;

impl WeightedRuleSelector {
    /// 根据上下文确定性地选择一条 Grammar 规则
    ///
    /// # 参数
    ///
    /// * `rules` - 候选规则列表
    /// * `context` - Grammar 上下文（朝向、主题、走廊长度、房间类型）
    /// * `rng` - 确定性随机数生成器
    ///
    /// # 返回
    ///
    /// 返回选中规则的索引，如果规则列表为空或所有权重为零则返回错误。
    ///
    /// # 错误
    ///
    /// 当规则列表为空或所有规则的调整后权重为零时，返回 `PcgError::CapabilityUnavailable`。
    pub fn select(
        &self,
        rules: &[GrammarRule],
        context: &GrammarContext,
        rng: &mut StableRng,
    ) -> PcgResult<usize> {
        if rules.is_empty() {
            return Err(PcgError::capability_unavailable(
                "Grammar 规则列表为空，无法进行选择",
                "grammar",
            ));
        }

        // 计算调整后的权重
        let adjusted_weights: Vec<f64> = rules
            .iter()
            .map(|rule| self.compute_adjusted_weight(rule, context))
            .collect();

        let total_weight: f64 = adjusted_weights.iter().sum();
        if !total_weight.is_finite() || total_weight <= 0.0 {
            return Err(PcgError::capability_unavailable(
                "所有 Grammar 规则的调整后权重为零",
                "grammar",
            ));
        }

        // 使用 StableRng 进行确定性选择
        let mut random_value = rng.gen_f64() * total_weight;
        for (i, &weight) in adjusted_weights.iter().enumerate() {
            random_value -= weight;
            if random_value <= 0.0 {
                return Ok(i);
            }
        }

        // 浮点精度兜底：返回最后一个有效规则
        Ok(rules.len() - 1)
    }

    /// 计算单条规则的调整后权重
    ///
    /// 根据上下文信息对基础权重进行乘法调整。
    fn compute_adjusted_weight(&self, rule: &GrammarRule, context: &GrammarContext) -> f64 {
        let mut weight = rule.base_weight;
        if weight <= 0.0 {
            return 0.0;
        }

        let rule_name_lower = rule.name.to_lowercase();

        // 规则 1：朝向匹配
        if let Some(facing) = &context.facing {
            let facing_keyword = facing_to_keyword(facing);
            if rule_name_lower.contains(facing_keyword) {
                weight *= 2.0;
            }
        }

        // 规则 2：主题匹配
        for tag in &context.theme_tags {
            if rule_name_lower.contains(&tag.to_lowercase()) {
                weight *= 1.5;
                break; // 只应用一次主题加成
            }
        }

        // 规则 3：走廊长度偏好
        if let Some(length) = context.corridor_length {
            let length_matches = (length > 10 && rule_name_lower.contains("long"))
                || (length <= 5 && rule_name_lower.contains("short"));
            if length_matches {
                weight *= 1.8;
            }
        }

        // 规则 4：房间类型匹配
        if let Some(room_type) = &context.room_type {
            let type_keyword = room_type_to_keyword(room_type);
            if rule_name_lower.contains(type_keyword) {
                weight *= 1.5;
            }
        }

        weight
    }
}

/// 将朝向转换为规则名称中的关键词
fn facing_to_keyword(facing: &CardinalDir) -> &'static str {
    match facing {
        CardinalDir::North => "north",
        CardinalDir::South => "south",
        CardinalDir::East => "east",
        CardinalDir::West => "west",
    }
}

/// 将房间类型转换为规则名称中的关键词
fn room_type_to_keyword(room_type: &RoomType) -> &'static str {
    match room_type {
        RoomType::Start => "start",
        RoomType::Combat => "combat",
        RoomType::Treasure => "treasure",
        RoomType::Shop => "shop",
        RoomType::Elite => "elite",
        RoomType::Puzzle => "puzzle",
        RoomType::Safe => "safe",
        RoomType::Boss => "boss",
        RoomType::Event => "event",
        RoomType::Secret => "secret",
    }
}

#[cfg(test)]
mod __tests__ {
    use super::*;

    #[test]
    fn test_select_single_rule() {
        // 验证需求 13.4：单条规则时直接选中
        let rules = vec![GrammarRule {
            name: "default_room".to_string(),
            base_weight: 1.0,
        }];
        let context = GrammarContext::default();
        let mut rng = StableRng::from_seed(42);
        let selector = WeightedRuleSelector;

        let result = selector.select(&rules, &context, &mut rng);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_select_deterministic() {
        // 验证需求 13.4：相同种子下结果可复现
        let rules = vec![
            GrammarRule {
                name: "rule_a".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "rule_b".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "rule_c".to_string(),
                base_weight: 1.0,
            },
        ];
        let context = GrammarContext::default();

        let mut rng1 = StableRng::from_seed(12345);
        let mut rng2 = StableRng::from_seed(12345);
        let selector = WeightedRuleSelector;

        let result1 = selector.select(&rules, &context, &mut rng1).unwrap();
        let result2 = selector.select(&rules, &context, &mut rng2).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_facing_weight_boost() {
        // 验证需求 13.4：朝向匹配时权重提升
        let rules = vec![
            GrammarRule {
                name: "corridor_north".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "corridor_south".to_string(),
                base_weight: 1.0,
            },
        ];
        let context = GrammarContext {
            facing: Some(CardinalDir::North),
            ..Default::default()
        };
        let selector = WeightedRuleSelector;

        // 统计多次选择的分布
        let mut north_count = 0;
        let mut rng = StableRng::from_seed(42);
        for _ in 0..1000 {
            let idx = selector.select(&rules, &context, &mut rng).unwrap();
            if idx == 0 {
                north_count += 1;
            }
        }

        // 朝向匹配的规则应被选中更多次（权重 2.0 vs 1.0，约 66%）
        let ratio = north_count as f64 / 1000.0;
        assert!(
            ratio > 0.55,
            "朝向匹配规则应被更频繁选中，实际比例: {ratio}"
        );
    }

    #[test]
    fn test_theme_weight_boost() {
        // 验证需求 13.4：主题匹配时权重提升
        let rules = vec![
            GrammarRule {
                name: "room_dungeon_dark".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "room_forest_light".to_string(),
                base_weight: 1.0,
            },
        ];
        let context = GrammarContext {
            theme_tags: vec!["dungeon".to_string()],
            ..Default::default()
        };
        let selector = WeightedRuleSelector;

        let mut dungeon_count = 0;
        let mut rng = StableRng::from_seed(42);
        for _ in 0..1000 {
            let idx = selector.select(&rules, &context, &mut rng).unwrap();
            if idx == 0 {
                dungeon_count += 1;
            }
        }

        // 主题匹配的规则应被选中更多次（权重 1.5 vs 1.0，约 60%）
        let ratio = dungeon_count as f64 / 1000.0;
        assert!(
            ratio > 0.50,
            "主题匹配规则应被更频繁选中，实际比例: {ratio}"
        );
    }

    #[test]
    fn test_corridor_length_weight_boost() {
        // 验证需求 13.4：走廊长度影响权重
        let rules = vec![
            GrammarRule {
                name: "corridor_long_variant".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "corridor_short_variant".to_string(),
                base_weight: 1.0,
            },
        ];

        // 长走廊上下文
        let long_context = GrammarContext {
            corridor_length: Some(15),
            ..Default::default()
        };
        let selector = WeightedRuleSelector;

        let mut long_count = 0;
        let mut rng = StableRng::from_seed(42);
        for _ in 0..1000 {
            let idx = selector.select(&rules, &long_context, &mut rng).unwrap();
            if idx == 0 {
                long_count += 1;
            }
        }

        // 长走廊应偏好 "long" 规则（权重 1.8 vs 1.0，约 64%）
        let ratio = long_count as f64 / 1000.0;
        assert!(ratio > 0.55, "长走廊应偏好 long 规则，实际比例: {ratio}");
    }

    #[test]
    fn test_room_type_weight_boost() {
        // 验证需求 13.4：房间类型匹配时权重提升
        let rules = vec![
            GrammarRule {
                name: "module_boss_arena".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "module_combat_basic".to_string(),
                base_weight: 1.0,
            },
        ];
        let context = GrammarContext {
            room_type: Some(RoomType::Boss),
            ..Default::default()
        };
        let selector = WeightedRuleSelector;

        let mut boss_count = 0;
        let mut rng = StableRng::from_seed(42);
        for _ in 0..1000 {
            let idx = selector.select(&rules, &context, &mut rng).unwrap();
            if idx == 0 {
                boss_count += 1;
            }
        }

        // Boss 类型匹配的规则应被选中更多次
        let ratio = boss_count as f64 / 1000.0;
        assert!(
            ratio > 0.50,
            "房间类型匹配规则应被更频繁选中，实际比例: {ratio}"
        );
    }

    #[test]
    fn test_empty_rules_returns_error() {
        // 验证需求 13.5：空规则列表返回错误
        let rules: Vec<GrammarRule> = vec![];
        let context = GrammarContext::default();
        let mut rng = StableRng::from_seed(42);
        let selector = WeightedRuleSelector;

        let result = selector.select(&rules, &context, &mut rng);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_weight_rules_returns_error() {
        // 验证需求 13.5：所有权重为零时返回错误
        let rules = vec![
            GrammarRule {
                name: "rule_a".to_string(),
                base_weight: 0.0,
            },
            GrammarRule {
                name: "rule_b".to_string(),
                base_weight: 0.0,
            },
        ];
        let context = GrammarContext::default();
        let mut rng = StableRng::from_seed(42);
        let selector = WeightedRuleSelector;

        let result = selector.select(&rules, &context, &mut rng);
        assert!(result.is_err());
    }

    #[test]
    fn test_combined_context_boost() {
        // 验证需求 13.4：多个上下文因素叠加
        let rules = vec![
            GrammarRule {
                name: "corridor_north_long_dungeon".to_string(),
                base_weight: 1.0,
            },
            GrammarRule {
                name: "corridor_south_short_forest".to_string(),
                base_weight: 1.0,
            },
        ];
        let context = GrammarContext {
            facing: Some(CardinalDir::North),
            theme_tags: vec!["dungeon".to_string()],
            corridor_length: Some(15),
            room_type: None,
        };
        let selector = WeightedRuleSelector;

        // 第一条规则匹配朝向(×2.0)、主题(×1.5)、长度(×1.8) = 5.4
        // 第二条规则无匹配 = 1.0
        // 第一条应被选中约 84% 的时间
        let mut first_count = 0;
        let mut rng = StableRng::from_seed(42);
        for _ in 0..1000 {
            let idx = selector.select(&rules, &context, &mut rng).unwrap();
            if idx == 0 {
                first_count += 1;
            }
        }

        let ratio = first_count as f64 / 1000.0;
        assert!(
            ratio > 0.75,
            "多因素叠加规则应被高频选中，实际比例: {ratio}"
        );
    }

    #[test]
    fn test_invalid_grammar_rule_returns_capability_error() {
        // 验证需求 13.5：当 Grammar 规则无效时返回 CapabilityUnavailable 错误
        let rules: Vec<GrammarRule> = vec![];
        let context = GrammarContext::default();
        let mut rng = StableRng::from_seed(42);
        let selector = WeightedRuleSelector;

        let result = selector.select(&rules, &context, &mut rng);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.error_code(), "PCG-CAPABILITY-001");

        // 验证错误信息包含 "grammar" 能力标识
        if let PcgError::CapabilityUnavailable { capability, .. } = err.as_ref() {
            assert_eq!(capability, "grammar");
        } else {
            panic!("应返回 CapabilityUnavailable 错误");
        }
    }

    #[test]
    fn test_invalid_module_reference_returns_capability_error() {
        // 验证需求 13.5：所有规则权重为零时返回结构化错误
        let rules = vec![
            GrammarRule {
                name: "invalid_module".to_string(),
                base_weight: 0.0,
            },
            GrammarRule {
                name: "broken_ref".to_string(),
                base_weight: -1.0,
            },
        ];
        let context = GrammarContext::default();
        let mut rng = StableRng::from_seed(42);
        let selector = WeightedRuleSelector;

        let result = selector.select(&rules, &context, &mut rng);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.error_code(), "PCG-CAPABILITY-001");
    }
}
