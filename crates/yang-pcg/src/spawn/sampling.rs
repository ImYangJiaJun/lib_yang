// 点位采样算法

use crate::debug::RejectionReason;
use crate::model::geometry::GridPoint;
use crate::rng::StableRng;

/// 采样结果，包含选中的点位和拒绝信息。
#[derive(Debug, Clone)]
pub struct SamplingResult {
    /// 最终选中的点位列表
    pub selected: Vec<GridPoint>,
    /// 被拒绝的点位及原因列表
    pub rejections: Vec<RejectionReason>,
}

/// 从候选点中选出满足最小间距的若干点。
pub fn select_spaced_points(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    rng: &mut StableRng,
) -> Vec<GridPoint> {
    select_spaced_points_tracked(candidates, desired_count, min_spacing, rng).selected
}

/// 从候选点中选出满足最小间距的若干点，同时记录被拒绝的点位和原因。
///
/// 与 `select_spaced_points` 功能相同，但额外返回拒绝信息，
/// 用于调试模式下的点位生成报告。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
/// - 需求 15.5: 失败阶段与失败约束输出
pub fn select_spaced_points_tracked(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    rng: &mut StableRng,
) -> SamplingResult {
    if candidates.is_empty() || desired_count == 0 {
        return SamplingResult {
            selected: Vec::new(),
            rejections: Vec::new(),
        };
    }

    let mut shuffled = candidates.to_vec();
    rng.shuffle(&mut shuffled);

    let mut selected = Vec::with_capacity(desired_count.min(shuffled.len()));
    let mut rejections = Vec::new();
    let min_distance_sq = i32::from(min_spacing).pow(2);

    for point in shuffled {
        if selected.len() == desired_count {
            break;
        }
        if selected
            .iter()
            .all(|existing| distance_sq(*existing, point) >= min_distance_sq)
        {
            selected.push(point);
        } else {
            rejections.push(RejectionReason {
                position: point,
                reason: format!(
                    "间距不足: 与已选点位距离小于最小间距 {}",
                    min_spacing
                ),
            });
        }
    }

    SamplingResult {
        selected,
        rejections,
    }
}

fn distance_sq(a: GridPoint, b: GridPoint) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}
