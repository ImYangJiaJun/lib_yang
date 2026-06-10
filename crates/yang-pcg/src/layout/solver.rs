// 布局求解器

use std::collections::HashMap;

use crate::config::NormalizedConfig;
use crate::model::geometry::{GridPoint, RoomBounds};
use crate::model::room::{Room, RoomGraph};
use crate::rng::StableRng;
use crate::topology::graph::sample_range_u16;

/// 房间之间的最小间隔（格）。
///
/// 防重叠检测在候选边界四周外扩此值后再比对，使房间不仅「不重叠」还「不贴边」，
/// 为走廊/地形预留呼吸空间。`RoomBounds::intersects` 边缘相切不算重叠，
/// 故 margin=0 即可满足 `validate_no_overlap`；这里取 1 仅为更干净的布局。
const ROOM_MARGIN: i32 = 1;

/// 求解房间边界布局。
///
/// 关键路径沿 x 轴单调铺排（天然不自重叠）；分支沿竖直方向错位。
/// 分支放置采用**确定性防重叠**：维护已放置边界累加集，候选边界若与任一已放置
/// 边界相距过近，则沿分支竖直方向按 `row_spacing` 步进外推，直至清空冲突。
/// 整个外推过程**不消耗任何随机数**——width/height 抽取顺序与重构前完全一致，
/// 仅个别会重叠的分支房间的 y 坐标发生确定性偏移，故确定性契约不变。
pub fn solve_room_bounds(
    graph: &RoomGraph,
    config: &NormalizedConfig,
    rng: &mut StableRng,
) -> HashMap<String, RoomBounds> {
    let mut bounds_map = HashMap::with_capacity(graph.nodes.len());
    // 已放置边界累加集：防重叠检测的依据（关键路径 + 先前分支）。
    let mut placed: Vec<RoomBounds> = Vec::with_capacity(graph.nodes.len());
    let mut critical_cursor_x = 0i32;
    let row_spacing =
        i32::from(config.config.room_size.max_height + config.config.corridor.width + 8);

    for room_id in &graph.critical_path {
        let width = i32::from(sample_range_u16(
            rng,
            crate::config::RangeU16::new(
                config.config.room_size.min_width,
                config.config.room_size.max_width,
            ),
        ));
        let height = i32::from(sample_range_u16(
            rng,
            crate::config::RangeU16::new(
                config.config.room_size.min_height,
                config.config.room_size.max_height,
            ),
        ));
        let bounds = RoomBounds {
            min: GridPoint {
                x: critical_cursor_x,
                y: -height / 2,
            },
            max: GridPoint {
                x: critical_cursor_x + width,
                y: -height / 2 + height,
            },
        };
        critical_cursor_x = bounds.max.x + i32::from(config.config.corridor.width) + 6;
        placed.push(bounds);
        bounds_map.insert(room_id.clone(), bounds);
    }

    for (branch_index, branch) in graph.branches.iter().enumerate() {
        let parent_bounds = bounds_map
            .get(&branch.start_room)
            .copied()
            .unwrap_or(RoomBounds {
                min: GridPoint { x: 0, y: 0 },
                max: GridPoint { x: 12, y: 12 },
            });
        let vertical_direction = if branch_index % 2 == 0 { 1 } else { -1 };
        let base_y = if vertical_direction > 0 {
            parent_bounds.max.y + row_spacing
        } else {
            parent_bounds.min.y - row_spacing
        };
        let mut branch_cursor_x = parent_bounds.center().x;

        for room_id in &branch.room_ids {
            let width = i32::from(sample_range_u16(
                rng,
                crate::config::RangeU16::new(
                    config.config.room_size.min_width,
                    config.config.room_size.max_width,
                ),
            ));
            let height = i32::from(sample_range_u16(
                rng,
                crate::config::RangeU16::new(
                    config.config.room_size.min_height,
                    config.config.room_size.max_height,
                ),
            ));

            let initial_min_y = if vertical_direction > 0 {
                base_y
            } else {
                base_y - height
            };
            let mut bounds = RoomBounds {
                min: GridPoint {
                    x: branch_cursor_x,
                    y: initial_min_y,
                },
                max: GridPoint {
                    x: branch_cursor_x + width,
                    y: initial_min_y + height,
                },
            };

            // 确定性防重叠：沿分支竖直方向步进，直至与所有已放置房间不再过近。
            bounds = nudge_clear(bounds, &placed, vertical_direction, row_spacing);

            branch_cursor_x = bounds.max.x + i32::from(config.config.corridor.width) + 4;
            placed.push(bounds);
            bounds_map.insert(room_id.clone(), bounds);
        }
    }

    bounds_map
}

/// 把候选边界沿竖直方向确定性外推，直至不与任何已放置边界过近。
///
/// 每步沿 `vertical_direction`（+1 向上 / -1 向下）平移 `row_spacing`。
/// 因 `row_spacing` 大于单个房间高度且已放置集合有限、纵向有界，循环必然终止；
/// 仍设防御性上限：超限则一次性平移到所有已放置房间之外，保证清空。
fn nudge_clear(
    mut bounds: RoomBounds,
    placed: &[RoomBounds],
    vertical_direction: i32,
    row_spacing: i32,
) -> RoomBounds {
    let step = vertical_direction * row_spacing;
    let max_iters = placed.len() + 4;
    for _ in 0..max_iters {
        if !overlaps_any(&bounds, placed) {
            return bounds;
        }
        bounds.min.y += step;
        bounds.max.y += step;
    }

    // 防御性兜底：平移到所有已放置房间之外（保证不再相交）。
    if overlaps_any(&bounds, placed) {
        let height = bounds.max.y - bounds.min.y;
        if vertical_direction > 0 {
            let ceiling = placed.iter().map(|b| b.max.y).max().unwrap_or(0);
            bounds.min.y = ceiling + ROOM_MARGIN + 1;
        } else {
            let floor = placed.iter().map(|b| b.min.y).min().unwrap_or(0);
            bounds.min.y = floor - ROOM_MARGIN - 1 - height;
        }
        bounds.max.y = bounds.min.y + height;
    }
    bounds
}

/// 候选边界（四周外扩 `ROOM_MARGIN`）是否与任一已放置边界相交。
fn overlaps_any(candidate: &RoomBounds, placed: &[RoomBounds]) -> bool {
    let inflated = RoomBounds {
        min: GridPoint {
            x: candidate.min.x - ROOM_MARGIN,
            y: candidate.min.y - ROOM_MARGIN,
        },
        max: GridPoint {
            x: candidate.max.x + ROOM_MARGIN,
            y: candidate.max.y + ROOM_MARGIN,
        },
    };
    placed.iter().any(|b| inflated.intersects(b))
}

/// 将边界写回房间列表。
pub fn apply_room_bounds(rooms: &[Room], bounds_map: &HashMap<String, RoomBounds>) -> Vec<Room> {
    rooms
        .iter()
        .cloned()
        .map(|mut room| {
            room.bounds = bounds_map.get(&room.id).copied();
            room
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;
    use crate::model::room::{Branch, RoomEdge};

    /// 构造「单父多分支」的密集图：3 条分支都挂在同一关键房上，
    /// 触发同父分支堆叠这一最主要的重叠源。
    fn dense_same_parent_graph() -> RoomGraph {
        let critical_path = vec!["c0".to_string(), "c1".to_string()];
        let mut branches = Vec::new();
        for b in 0..3 {
            branches.push(Branch {
                id: format!("br-{b}"),
                start_room: "c0".to_string(),
                end_room: format!("br-{b}-r1"),
                room_ids: vec![format!("br-{b}-r0"), format!("br-{b}-r1")],
                purpose: "reward".to_string(),
            });
        }
        RoomGraph {
            nodes: vec![],
            edges: Vec::<RoomEdge>::new(),
            critical_path,
            branches,
        }
    }

    fn assert_no_pairwise_overlap(bounds_map: &HashMap<String, RoomBounds>) {
        let all: Vec<(&String, &RoomBounds)> = bounds_map.iter().collect();
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert!(
                    !all[i].1.intersects(all[j].1),
                    "房间 {} 与 {} 边界重叠: {:?} vs {:?}",
                    all[i].0,
                    all[j].0,
                    all[i].1,
                    all[j].1
                );
            }
        }
    }

    #[test]
    fn test_solve_room_bounds_no_overlap_same_parent_branches() {
        // 验证需求: 4.7 - 同父多分支不再堆叠重叠
        let normalized = GenerationConfig::default()
            .normalize()
            .expect("配置应归一化");
        let graph = dense_same_parent_graph();
        for seed in 0u64..32 {
            let mut rng = StableRng::from_seed(seed);
            let bounds_map = solve_room_bounds(&graph, &normalized, &mut rng);
            assert_eq!(bounds_map.len(), 2 + 3 * 2, "应放置全部关键+分支房间");
            assert_no_pairwise_overlap(&bounds_map);
        }
    }

    #[test]
    fn test_solve_room_bounds_deterministic() {
        // 同 seed 两次结果一致（确定性外推不破坏可复现性）
        let normalized = GenerationConfig::default()
            .normalize()
            .expect("配置应归一化");
        let graph = dense_same_parent_graph();
        let mut rng1 = StableRng::from_seed(99);
        let mut rng2 = StableRng::from_seed(99);
        let a = solve_room_bounds(&graph, &normalized, &mut rng1);
        let b = solve_room_bounds(&graph, &normalized, &mut rng2);
        assert_eq!(a, b, "同 seed 布局应完全一致");
    }
}
