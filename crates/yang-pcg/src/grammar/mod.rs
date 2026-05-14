// Grammar 兼容模块
// 提供确定性权重选择器和 Grammar 规则映射支持

pub mod selector;

pub use selector::{GrammarContext, GrammarRule, WeightedRuleSelector};
