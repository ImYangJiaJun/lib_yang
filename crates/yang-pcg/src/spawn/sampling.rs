// 点位采样算法

use crate::debug::RejectionReason;
use crate::model::geometry::GridPoint;
use crate::rng::StableRng;

/// 采样结果，包含选中的点位和拒绝信息。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(crate) struct SamplingResult {
    /// 最终选中的点位列表
    pub(crate) selected: Vec<GridPoint>,
    /// 被拒绝的点位及原因列表
    pub(crate) rejections: Vec<RejectionReason>,
}

/// 从候选点中选出满足最小间距的若干点。
pub(crate) fn select_spaced_points(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    rng: &mut StableRng,
) -> Vec<GridPoint> {
    select_spaced_points_inner(candidates, desired_count, min_spacing, &[], 0, rng)
}

/// 从候选点中选出满足最小间距、且与「已占用点」保持跨类型间距的若干点。
///
/// `occupied` 为另一类型（如先放置的交互物）已占用的局部坐标，`occupied_spacing`
/// 为跨类型间距阈值；候选点需同时满足「距已选点 ≥ min_spacing」与
/// 「距所有 occupied ≥ occupied_spacing」（均为欧氏距离）。
///
/// 注意：RNG 仅消耗于候选洗牌，**与 occupied 无关**——传空 occupied 时行为与
/// `select_spaced_points_tracked` 完全一致，故不影响既有确定性。
pub(crate) fn select_spaced_points_excluding(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    occupied: &[GridPoint],
    occupied_spacing: u16,
    rng: &mut StableRng,
) -> Vec<GridPoint> {
    select_spaced_points_inner(
        candidates,
        desired_count,
        min_spacing,
        occupied,
        occupied_spacing,
        rng,
    )
}

/// 从候选点中选出满足最小间距的若干点，同时记录被拒绝的点位和原因。
///
/// 与 `select_spaced_points` 功能相同，但额外返回拒绝信息，
/// 用于调试模式下的点位生成报告。
///
/// # 需求映射
/// - 需求 15.3: 输出被拒绝点位
/// - 需求 15.5: 失败阶段与失败约束输出
pub(crate) fn select_spaced_points_tracked(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    rng: &mut StableRng,
) -> SamplingResult {
    // 委托给带占用集合的实现，传空 occupied 即与原行为字节一致（RNG 仅消耗于洗牌）。
    select_spaced_points_tracked_inner(candidates, desired_count, min_spacing, &[], 0, rng)
}

/// `select_spaced_points_tracked` 的跨类型间距版本。
///
/// 在原有「距已选点 ≥ min_spacing」之外，追加「距 `occupied` 中所有点 ≥ occupied_spacing」
/// 的接受条件。RNG 仅消耗于候选洗牌，与 occupied 无关。
pub(crate) fn select_spaced_points_tracked_excluding(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    occupied: &[GridPoint],
    occupied_spacing: u16,
    rng: &mut StableRng,
) -> SamplingResult {
    select_spaced_points_tracked_inner(
        candidates,
        desired_count,
        min_spacing,
        occupied,
        occupied_spacing,
        rng,
    )
}

/// 生产路径：仅返回选中点，不构建 rejection 字符串。
fn select_spaced_points_inner(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    occupied: &[GridPoint],
    occupied_spacing: u16,
    rng: &mut StableRng,
) -> Vec<GridPoint> {
    if candidates.is_empty() || desired_count == 0 {
        return Vec::new();
    }

    let mut shuffled = candidates.to_vec();
    rng.shuffle(&mut shuffled);

    let mut selected = Vec::with_capacity(desired_count.min(shuffled.len()));
    let min_distance_sq = i64::from(min_spacing) * i64::from(min_spacing);
    let occupied_distance_sq = i64::from(occupied_spacing) * i64::from(occupied_spacing);

    for point in shuffled {
        if selected.len() == desired_count {
            break;
        }
        let ok_selected = selected
            .iter()
            .all(|existing| distance_sq(*existing, point) >= min_distance_sq);
        let ok_occupied = occupied
            .iter()
            .all(|occ| distance_sq(*occ, point) >= occupied_distance_sq);
        if ok_selected && ok_occupied {
            selected.push(point);
        }
    }

    selected
}

/// 调试路径：保留 rejection String 用于调试报告。
fn select_spaced_points_tracked_inner(
    candidates: &[GridPoint],
    desired_count: usize,
    min_spacing: u16,
    occupied: &[GridPoint],
    occupied_spacing: u16,
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
    let min_distance_sq = i64::from(min_spacing) * i64::from(min_spacing);
    let occupied_distance_sq = i64::from(occupied_spacing) * i64::from(occupied_spacing);

    for point in shuffled {
        if selected.len() == desired_count {
            break;
        }
        let ok_selected = selected
            .iter()
            .all(|existing| distance_sq(*existing, point) >= min_distance_sq);
        let ok_occupied = occupied
            .iter()
            .all(|occ| distance_sq(*occ, point) >= occupied_distance_sq);
        if ok_selected && ok_occupied {
            selected.push(point);
        } else {
            rejections.push(RejectionReason {
                position: point,
                reason: format!(
                    "间距不足: 与已选点位或已占用点位距离小于间距阈值（min={}, cross={}）",
                    min_spacing, occupied_spacing
                ),
            });
        }
    }

    SamplingResult {
        selected,
        rejections,
    }
}

fn distance_sq(a: GridPoint, b: GridPoint) -> i64 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    dx * dx + dy * dy
}
