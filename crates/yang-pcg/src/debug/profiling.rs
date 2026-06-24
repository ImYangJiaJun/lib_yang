// 性能分析

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// 单个阶段的统计信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StageStat {
    /// 阶段名称
    pub stage_name: String,
    /// 阶段耗时（毫秒）
    pub duration_ms: u64,
    /// 迭代次数
    pub iterations: u32,
    /// 产出数量
    pub produced_count: usize,
}

/// 构建简易阶段统计（不含耗时）。
pub fn stage_stat(stage_name: impl Into<String>, produced_count: usize) -> StageStat {
    StageStat {
        stage_name: stage_name.into(),
        duration_ms: 0,
        iterations: 1,
        produced_count,
    }
}

/// 构建包含耗时的阶段统计。
///
/// # 参数
/// - `stage_name`: 阶段名称
/// - `produced_count`: 该阶段产出的元素数量
/// - `duration_ms`: 阶段耗时（毫秒）
pub fn stage_stat_timed(
    stage_name: impl Into<String>,
    produced_count: usize,
    duration_ms: u64,
) -> StageStat {
    StageStat {
        stage_name: stage_name.into(),
        duration_ms,
        iterations: 1,
        produced_count,
    }
}

/// 计时工具：将 `Instant` 起始时间转换为已经过的毫秒数。
#[inline]
pub fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}
