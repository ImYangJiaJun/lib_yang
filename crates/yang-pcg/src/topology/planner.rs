// 拓扑规划器

use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::model::room::{Branch, Room, RoomEdge, RoomGraph, RoomType};
use crate::rng::StableRng;

use super::graph::{branch_id, edge_id, room_id, sample_range_u16, theme_tags};
use super::room_types::{apply_branch_room_types, assign_critical_room_type, branch_purpose};

/// 拓扑生成器。
#[non_exhaustive]
pub struct TopologyGenerator<'a> {
    config: &'a NormalizedConfig,
}

impl<'a> TopologyGenerator<'a> {
    pub fn new(config: &'a NormalizedConfig) -> Self {
        Self { config }
    }

    /// 生成最小可运行的房间拓扑。
    ///
    /// 当前策略采用“关键路径 + 分支”的启发式构造：
    /// 1. 先构建关键路径。
    /// 2. 再把剩余房间挂到若干分支上。
    /// 3. 最后分配房型和基础难度。
    pub fn generate(&self, rng: &mut StableRng) -> PcgResult<RoomGraph> {
        let room_count = usize::from(sample_range_u16(rng, self.config.config.room_count));
        let critical_path_len = usize::from(
            sample_range_u16(rng, self.config.config.critical_path_length)
                .clamp(2, room_count as u16),
        );
        let extra_room_count = room_count.saturating_sub(critical_path_len);
        let requested_branch_count =
            usize::from(sample_range_u16(rng, self.config.config.branch_count));
        let branch_count = if extra_room_count == 0 {
            0
        } else {
            requested_branch_count.max(1).min(extra_room_count)
        };

        let mut nodes = Vec::with_capacity(room_count);
        let mut critical_path = Vec::with_capacity(critical_path_len);
        let themes = theme_tags(self.config);

        for index in 0..critical_path_len {
            let room_type = assign_critical_room_type(index, critical_path_len);
            let id = room_id(index);
            critical_path.push(id.clone());
            nodes.push(Room {
                id,
                room_type,
                depth_from_start: index as u16,
                branch_id: None,
                difficulty: (index as u16) * 10,
                theme_tags: themes.clone(),
                bounds: None,
                template_ref: None,
                grammar_token: None,
            });
        }

        let mut edges = Vec::with_capacity(room_count.saturating_sub(1));
        let mut edge_index = 0usize;
        for pair in critical_path.windows(2) {
            edges.push(RoomEdge {
                id: edge_id(edge_index),
                from_room: pair[0].clone(),
                to_room: pair[1].clone(),
                is_critical: true,
            });
            edge_index += 1;
        }

        let mut branches = Vec::with_capacity(branch_count);
        let mut next_room_index = critical_path_len;

        for branch_index in 0..branch_count {
            let remaining_rooms = room_count - next_room_index;
            let remaining_branches = branch_count - branch_index;
            let branch_room_count = (remaining_rooms / remaining_branches).max(1);
            let parent_critical_index = if critical_path_len > 2 {
                1 + (branch_index % (critical_path_len - 2))
            } else {
                0
            };
            let parent_room_id = critical_path[parent_critical_index].clone();
            let branch_identifier = branch_id(branch_index);
            let purpose = branch_purpose(branch_index).to_string();
            let mut branch_room_ids = Vec::with_capacity(branch_room_count);
            let mut previous_room_id = parent_room_id.clone();

            for branch_room_offset in 0..branch_room_count {
                let id = room_id(next_room_index);
                let depth = (parent_critical_index + branch_room_offset + 1) as u16;
                nodes.push(Room {
                    id: id.clone(),
                    room_type: RoomType::Combat,
                    depth_from_start: depth,
                    branch_id: Some(branch_identifier.clone()),
                    difficulty: depth * 10,
                    theme_tags: themes.clone(),
                    bounds: None,
                    template_ref: None,
                    grammar_token: None,
                });
                edges.push(RoomEdge {
                    id: edge_id(edge_index),
                    from_room: previous_room_id.clone(),
                    to_room: id.clone(),
                    is_critical: false,
                });
                edge_index += 1;
                previous_room_id = id.clone();
                branch_room_ids.push(id);
                next_room_index += 1;
            }

            let end_room = branch_room_ids
                .last()
                .cloned()
                .unwrap_or_else(|| parent_room_id.clone());

            branches.push(Branch {
                id: branch_identifier,
                start_room: parent_room_id,
                end_room,
                room_ids: branch_room_ids,
                purpose,
            });
        }

        apply_branch_room_types(&mut nodes, &branches);

        Ok(RoomGraph {
            nodes,
            edges,
            critical_path,
            branches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;

    #[test]
    fn test_generate_topology_produces_start_and_boss() {
        let config = GenerationConfig::default().normalize().unwrap();
        let mut rng = StableRng::from_seed(7);
        let graph = TopologyGenerator::new(&config).generate(&mut rng).unwrap();

        assert!(!graph.nodes.is_empty());
        assert_eq!(graph.nodes.first().unwrap().room_type, RoomType::Start);
        assert!(graph
            .nodes
            .iter()
            .any(|room| matches!(room.room_type, RoomType::Boss)));
        assert!(!graph.edges.is_empty());
        assert!(!graph.critical_path.is_empty());
    }
}
