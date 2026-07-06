// 图结构定义
// 提供拓扑生成阶段的基础辅助函数

use crate::config::{NormalizedConfig, RangeU16};
use crate::model::room::{BranchId, RoomEdgeId, RoomId};
use crate::rng::StableRng;

/// 从闭区间范围中采样一个 `u16`。
pub fn sample_range_u16(rng: &mut StableRng, range: RangeU16) -> u16 {
    if range.min == range.max {
        return range.min;
    }
    let sampled = rng.random_range(u32::from(range.min), u32::from(range.max) + 1);
    sampled as u16
}

/// 生成稳定的房间 ID。
pub fn room_id(index: usize) -> RoomId {
    format!("room-{index:03}")
}

/// 生成稳定的房间边 ID。
pub fn edge_id(index: usize) -> RoomEdgeId {
    format!("edge-{index:03}")
}

/// 生成稳定的分支 ID。
pub fn branch_id(index: usize) -> BranchId {
    format!("branch-{index:03}")
}

/// 获取配置中的主题标签；若为空则回退到 `default`。
pub fn theme_tags(config: &NormalizedConfig) -> Vec<String> {
    if config.config.theme_tags.is_empty() {
        vec!["default".to_string()]
    } else {
        config.config.theme_tags.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_range_u16_includes_u16_max_upper_bound() {
        let range = RangeU16::new(u16::MAX - 1, u16::MAX);
        let saw_upper_bound = (0..4096).any(|seed| {
            let mut rng = StableRng::from_seed(seed);
            sample_range_u16(&mut rng, range) == u16::MAX
        });

        assert!(saw_upper_bound, "闭区间采样必须能返回 u16::MAX 上界");
    }
}
