// 验证逻辑
// 负责验证配置、请求与生成结果的不变量

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::config::NormalizedConfig;
use crate::error::{PcgError, PcgResult};
use crate::model::geometry::GridPoint;
use crate::model::request::{Constraint, ExclusionZoneConstraint, GenerationRequest};
use crate::model::result::GenerationResult;
use crate::model::room::{Room, RoomGraph, RoomType};
use crate::model::spawn::SpawnPoint;
use crate::model::terrain::{Grid2D, Terrain, TileKind};

/// 验证生成请求。
///
/// 当前阶段主要检查：
/// 1. 配置是否可以成功归一化。
/// 2. `RuntimeChunked` 模式下是否提供了运行时上下文。
pub fn validate_request(request: &GenerationRequest) -> PcgResult<NormalizedConfig> {
    let normalized = request.config.normalize()?;

    if matches!(
        normalized.config.generation_mode,
        crate::config::GenerationMode::RuntimeChunked
    ) && request.runtime_context.is_none()
    {
        return Err(PcgError::config_with_field(
            "运行时分块模式需要提供 runtime_context",
            "runtime_context",
        ));
    }

    Ok(normalized)
}

/// 验证房间可达性（BFS 从 Start 房间出发）
///
/// 从拓扑图中找到 Start 类型的房间作为起点，使用 BFS 遍历所有可达房间。
/// 如果存在不可达的房间，返回包含不可达房间 ID 列表的拓扑错误。
///
/// # 参数
/// - `graph`: 房间拓扑图
///
/// # 返回
/// - `Ok(())`: 所有房间从 Start 可达
/// - `Err(PcgError::Topology)`: 存在不可达房间，错误信息包含不可达房间 ID
///
/// # 需求映射
/// - 需求 3.2: 确保所有 Room 从 Start 房间可达
/// - 需求 18.3: 验证拓扑连通性不变量
pub fn validate_reachability(graph: &RoomGraph) -> PcgResult<()> {
    // 查找 Start 房间
    let start_room = graph
        .nodes
        .iter()
        .find(|room| room.room_type == RoomType::Start);

    let start_room = match start_room {
        Some(room) => room,
        None => {
            return Err(PcgError::topology(
                "拓扑图中不存在 Start 类型的房间，无法验证可达性",
            ));
        }
    };

    let start_id = &start_room.id;

    // 构建邻接表（无向图，因为拓扑边表示双向连通）
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in &graph.nodes {
        adjacency.entry(node.id.as_str()).or_default();
    }
    for edge in &graph.edges {
        adjacency
            .entry(edge.from_room.as_str())
            .or_default()
            .push(edge.to_room.as_str());
        adjacency
            .entry(edge.to_room.as_str())
            .or_default()
            .push(edge.from_room.as_str());
    }

    // BFS 遍历
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    visited.insert(start_id.as_str());
    queue.push_back(start_id.as_str());

    while let Some(current) = queue.pop_front() {
        if let Some(neighbors) = adjacency.get(current) {
            for &neighbor in neighbors {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // 检查是否所有房间都被访问到
    let all_room_ids: HashSet<&str> = graph.nodes.iter().map(|r| r.id.as_str()).collect();
    let unreachable: Vec<&str> = all_room_ids
        .difference(&visited)
        .copied()
        .collect();

    if unreachable.is_empty() {
        Ok(())
    } else {
        let unreachable_list = unreachable.join(", ");
        Err(Box::new(PcgError::Topology {
            message: format!(
                "存在 {} 个不可达房间: [{}]",
                unreachable.len(),
                unreachable_list
            ),
            stage: Some("validation".to_string()),
            seed: None,
            trace_id: None,
            room_count: Some(graph.nodes.len()),
            target_room_count: None,
        }))
    }
}

/// 验证房间边界不重叠（AABB 碰撞检测）
///
/// 遍历所有房间对，检测是否存在 AABB 重叠。跳过没有边界信息的房间。
/// 如果检测到重叠，返回包含冲突房间对信息的布局错误。
///
/// # 参数
/// - `rooms`: 房间列表
///
/// # 返回
/// - `Ok(())`: 所有房间边界不重叠
/// - `Err(PcgError::Layout)`: 存在重叠的房间对，错误信息包含冲突详情
///
/// # 需求映射
/// - 需求 4.7: 房间边界不重叠
/// - 需求 18.3: 验证布局不变量
pub fn validate_no_overlap(rooms: &[Room]) -> PcgResult<()> {
    // 收集所有冲突房间对
    let mut conflicts: Vec<(String, String)> = Vec::new();

    for i in 0..rooms.len() {
        // 跳过没有边界的房间
        let bounds_a = match &rooms[i].bounds {
            Some(b) => b,
            None => continue,
        };

        for j in (i + 1)..rooms.len() {
            // 跳过没有边界的房间
            let bounds_b = match &rooms[j].bounds {
                Some(b) => b,
                None => continue,
            };

            // 使用 AABB 碰撞检测
            if bounds_a.intersects(bounds_b) {
                conflicts.push((rooms[i].id.clone(), rooms[j].id.clone()));
            }
        }
    }

    if conflicts.is_empty() {
        Ok(())
    } else {
        // 构建冲突详情字符串
        let conflict_details: Vec<String> = conflicts
            .iter()
            .map(|(a, b)| format!("({}, {})", a, b))
            .collect();
        let details_str = conflict_details.join(", ");

        Err(Box::new(PcgError::Layout {
            message: format!(
                "检测到 {} 对房间边界重叠: [{}]",
                conflicts.len(),
                details_str
            ),
            stage: Some("validation".to_string()),
            seed: None,
            trace_id: None,
            room_id: None,
            collision_details: Some(details_str),
        }))
    }
}

/// 验证地形连通性（BFS 验证所有门口瓦片互相可达）
///
/// 对每个地形网格，找到所有 Doorway 类型的瓦片作为入口/出口点，
/// 使用 BFS 在可通行瓦片（Floor、Doorway、Reserved）上遍历，
/// 验证所有 Doorway 瓦片属于同一连通区域。
///
/// # 参数
/// - `terrains`: 地形列表
///
/// # 返回
/// - `Ok(())`: 所有地形的门口瓦片互相可达
/// - `Err(PcgError::Terrain)`: 存在不连通的门口瓦片，错误信息包含房间 ID
///
/// # 需求映射
/// - 需求 5.4: 房间内所有入口到出口存在可通行路径
/// - 需求 18.3: 验证地形连通性不变量
pub fn validate_terrain_connectivity(terrains: &[Terrain]) -> PcgResult<()> {
    for terrain in terrains {
        // 收集所有 Doorway 瓦片坐标
        let doorways = collect_doorway_tiles(&terrain.tiles);

        // 如果没有门口或只有一个门口，无需验证连通性
        if doorways.len() <= 1 {
            continue;
        }

        // 从第一个门口出发进行 BFS，检查是否能到达所有其他门口
        let start = doorways[0];
        let reachable = bfs_walkable(&terrain.tiles, start);

        // 检查所有门口是否都在可达集合中
        let unreachable_doors: Vec<&GridPoint> = doorways
            .iter()
            .filter(|p| !reachable.contains(p))
            .collect();

        if !unreachable_doors.is_empty() {
            return Err(Box::new(PcgError::Terrain {
                message: format!(
                    "房间 {} 的地形存在 {} 个不可达门口瓦片，连通性验证失败",
                    terrain.room_id,
                    unreachable_doors.len()
                ),
                stage: Some("validation".to_string()),
                seed: None,
                trace_id: None,
                room_id: Some(terrain.room_id.clone()),
                strategy: None,
                connectivity_failed: Some(true),
            }));
        }
    }

    Ok(())
}

/// 收集网格中所有 Doorway 类型的瓦片坐标
fn collect_doorway_tiles(grid: &Grid2D<TileKind>) -> Vec<GridPoint> {
    let mut doorways = Vec::new();
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            if grid.get(x, y) == Some(&TileKind::Doorway) {
                doorways.push(GridPoint { x, y });
            }
        }
    }
    doorways
}

/// 从起点出发，BFS 遍历所有可通行瓦片，返回可达点集合
fn bfs_walkable(grid: &Grid2D<TileKind>, start: GridPoint) -> HashSet<GridPoint> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        // 四方向邻居
        let neighbors = [
            GridPoint { x: current.x + 1, y: current.y },
            GridPoint { x: current.x - 1, y: current.y },
            GridPoint { x: current.x, y: current.y + 1 },
            GridPoint { x: current.x, y: current.y - 1 },
        ];

        for neighbor in neighbors {
            if visited.contains(&neighbor) {
                continue;
            }
            // 检查邻居是否为可通行瓦片
            if let Some(tile) = grid.get(neighbor.x, neighbor.y) {
                if is_tile_walkable(*tile) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    visited
}

/// 判断瓦片是否可通行（Floor、Doorway、Reserved）
fn is_tile_walkable(tile: TileKind) -> bool {
    matches!(tile, TileKind::Floor | TileKind::Doorway | TileKind::Reserved)
}

/// 默认最小间距（曼哈顿距离，单位：格）
const DEFAULT_MIN_SPACING: i32 = 2;

/// 点位间距违规详情
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SpacingViolation {
    /// 第一个点位 ID
    pub spawn_a_id: String,
    /// 第二个点位 ID
    pub spawn_b_id: String,
    /// 所属房间 ID
    pub room_id: String,
    /// 实际距离
    pub actual_distance: i32,
    /// 要求的最小间距
    pub required_spacing: i32,
}

/// 排除区违规详情
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExclusionViolation {
    /// 违规点位 ID
    pub spawn_id: String,
    /// 所属房间 ID
    pub room_id: String,
    /// 点位坐标
    pub position: GridPoint,
    /// 排除区标签
    pub zone_label: String,
}

/// 计算两点之间的曼哈顿距离
fn manhattan_distance(a: GridPoint, b: GridPoint) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// 判断点位是否在排除区内
fn is_point_in_exclusion_zone(point: GridPoint, zone: &ExclusionZoneConstraint) -> bool {
    point.x >= zone.min.x
        && point.x < zone.max.x
        && point.y >= zone.min.y
        && point.y < zone.max.y
}

/// 验证点位间距和禁布规则
///
/// 对所有交互物和敌人点位执行以下验证：
/// 1. 同一房间内的点位之间满足最小间距约束（曼哈顿距离）
/// 2. 所有点位不在排除区域内
///
/// # 参数
/// - `spawns`: 所有点位列表（包含交互物和敌人）
/// - `constraints`: 约束列表（用于提取排除区）
/// - `min_spacing`: 最小间距（曼哈顿距离），传 `None` 时使用默认值 2
///
/// # 返回
/// - `Ok(())`: 所有点位满足间距和禁布约束
/// - `Err(PcgError::Spawn)`: 存在违规点位，错误信息包含违规详情
///
/// # 需求映射
/// - 需求 7.4: 交互物之间保持最小间距
/// - 需求 8.3: 敌人点位与入口、交互物点位之间保持最小安全间距
/// - 需求 18.3: 验证点位间距不变量
pub fn validate_spawn_spacing(
    spawns: &[SpawnPoint],
    constraints: &[Constraint],
    min_spacing: Option<i32>,
) -> PcgResult<()> {
    let spacing = min_spacing.unwrap_or(DEFAULT_MIN_SPACING);

    // 1. 按房间分组点位
    let mut room_spawns: HashMap<&str, Vec<&SpawnPoint>> = HashMap::new();
    for spawn in spawns {
        room_spawns
            .entry(spawn.room_id.as_str())
            .or_default()
            .push(spawn);
    }

    // 2. 检查同一房间内点位间距
    let mut spacing_violations: Vec<SpacingViolation> = Vec::new();
    for (room_id, points) in &room_spawns {
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                let dist = manhattan_distance(points[i].grid_pos, points[j].grid_pos);
                if dist < spacing {
                    spacing_violations.push(SpacingViolation {
                        spawn_a_id: points[i].id.clone(),
                        spawn_b_id: points[j].id.clone(),
                        room_id: room_id.to_string(),
                        actual_distance: dist,
                        required_spacing: spacing,
                    });
                }
            }
        }
    }

    // 3. 提取排除区约束（仅 exclude_spawns 为 true 的）
    let exclusion_zones: Vec<&ExclusionZoneConstraint> = constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::ExclusionZone(zone) if zone.exclude_spawns => Some(zone),
            _ => None,
        })
        .collect();

    // 4. 检查点位是否在排除区内
    let mut exclusion_violations: Vec<ExclusionViolation> = Vec::new();
    for spawn in spawns {
        for zone in &exclusion_zones {
            if is_point_in_exclusion_zone(spawn.grid_pos, zone) {
                exclusion_violations.push(ExclusionViolation {
                    spawn_id: spawn.id.clone(),
                    room_id: spawn.room_id.clone(),
                    position: spawn.grid_pos,
                    zone_label: zone.label.clone(),
                });
            }
        }
    }

    // 5. 汇总错误
    if spacing_violations.is_empty() && exclusion_violations.is_empty() {
        return Ok(());
    }

    // 构建错误信息
    let mut details = Vec::new();

    if !spacing_violations.is_empty() {
        let spacing_details: Vec<String> = spacing_violations
            .iter()
            .map(|v| {
                format!(
                    "房间 {} 中点位 {} 与 {} 间距 {} < {}",
                    v.room_id, v.spawn_a_id, v.spawn_b_id, v.actual_distance, v.required_spacing
                )
            })
            .collect();
        details.push(format!(
            "间距违规 {} 处: [{}]",
            spacing_violations.len(),
            spacing_details.join("; ")
        ));
    }

    if !exclusion_violations.is_empty() {
        let exclusion_details: Vec<String> = exclusion_violations
            .iter()
            .map(|v| {
                format!(
                    "点位 {} (房间 {}, 坐标 ({},{})) 在排除区 '{}' 内",
                    v.spawn_id, v.room_id, v.position.x, v.position.y, v.zone_label
                )
            })
            .collect();
        details.push(format!(
            "排除区违规 {} 处: [{}]",
            exclusion_violations.len(),
            exclusion_details.join("; ")
        ));
    }

    let message = details.join("; ");

    Err(Box::new(PcgError::Spawn {
        message,
        stage: Some("validation".to_string()),
        seed: None,
        trace_id: None,
        room_id: None,
        spawn_type: None,
        candidate_count: None,
        target_count: None,
    }))
}

/// 验证生成结果的基础不变量。
///
/// 当前实现先覆盖最关键的结构一致性检查，后续可以继续补充：
/// - 房间可达性
/// - 门口连通性
/// - 点位间距
/// - 约束满足报告
pub fn validate_result(result: &GenerationResult) -> PcgResult<()> {
    if result.rooms.len() != result.topology.nodes.len() {
        return Err(PcgError::topology(
            "生成结果中的 rooms 数量与 topology.nodes 数量不一致",
        ));
    }

    if !result.topology.edges.is_empty() && result.corridors.len() != result.topology.edges.len() {
        return Err(PcgError::layout(
            "生成结果中的 corridors 数量与 topology.edges 数量不一致",
        ));
    }

    if result.door_anchors.len() < result.corridors.len() {
        return Err(PcgError::layout("走廊数量超过门锚点数量，结果不一致"));
    }

    if result.metadata.schema_version.trim().is_empty() {
        return Err(PcgError::corrupted_data("生成结果缺少 schema_version"));
    }

    if result.metadata.algorithm_version.trim().is_empty() {
        return Err(PcgError::corrupted_data("生成结果缺少 algorithm_version"));
    }

    Ok(())
}

// ========== 结构化验证报告 ==========

/// 单项验证结果
///
/// 记录单个不变量检查的通过/失败状态及可选错误信息。
///
/// # 需求映射
/// - 需求 6.6: 约束验证报告
/// - 需求 15.3: 输出约束验证报告，说明哪些不变量被满足或被拒绝
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValidationItem {
    /// 验证项名称（如 "reachability"、"no_overlap" 等）
    pub name: String,
    /// 是否通过验证
    pub passed: bool,
    /// 失败时的错误信息（通过时为 None）
    pub error_message: Option<String>,
}

impl ValidationItem {
    /// 创建一个通过的验证项
    pub fn passed(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            error_message: None,
        }
    }

    /// 创建一个失败的验证项
    pub fn failed(name: impl Into<String>, error_message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            error_message: Some(error_message.into()),
        }
    }
}

/// 结构化约束验证报告
///
/// 包含各不变量检查的通过/失败状态，用于调试输出和约束满足性分析。
/// 每个字段对应一个不变量验证项，记录该项是否通过以及失败原因。
///
/// # 需求映射
/// - 需求 6.6: 约束验证报告
/// - 需求 15.3: 输出约束验证报告，说明哪些不变量被满足或被拒绝
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValidationReport {
    /// 房间可达性验证（BFS 从 Start 出发）
    pub reachability: ValidationItem,
    /// 房间边界不重叠验证（AABB 碰撞检测）
    pub no_overlap: ValidationItem,
    /// 地形连通性验证（门口瓦片互相可达）
    pub terrain_connectivity: ValidationItem,
    /// 点位间距和禁布规则验证
    pub spawn_spacing: ValidationItem,
    /// 所有验证项是否全部通过
    pub all_passed: bool,
    /// 通过的验证项数量
    pub passed_count: usize,
    /// 失败的验证项数量
    pub failed_count: usize,
}

impl ValidationReport {
    /// 获取所有验证项的列表
    pub fn items(&self) -> Vec<&ValidationItem> {
        vec![
            &self.reachability,
            &self.no_overlap,
            &self.terrain_connectivity,
            &self.spawn_spacing,
        ]
    }
}

/// 执行完整的不变量验证，生成结构化验证报告
///
/// 依次运行所有不变量检查（可达性、边界不重叠、地形连通性、点位间距），
/// 将每项结果汇总为 `ValidationReport`。即使某项验证失败，也会继续执行后续验证，
/// 以便一次性收集所有问题。
///
/// # 参数
/// - `result`: 生成结果
/// - `constraints`: 约束列表（用于点位间距验证中的排除区检查）
/// - `min_spacing`: 最小间距（可选，传 None 使用默认值）
///
/// # 返回
/// - `ValidationReport`: 包含各项验证结果的结构化报告
///
/// # 需求映射
/// - 需求 6.6: 约束验证报告
/// - 需求 15.3: 输出约束验证报告
pub fn run_full_validation(
    result: &GenerationResult,
    constraints: &[Constraint],
    min_spacing: Option<i32>,
) -> ValidationReport {
    // 1. 验证房间可达性
    let reachability = match validate_reachability(&result.topology) {
        Ok(()) => ValidationItem::passed("reachability"),
        Err(e) => ValidationItem::failed("reachability", format!("{}", e)),
    };

    // 2. 验证房间边界不重叠
    let no_overlap = match validate_no_overlap(&result.rooms) {
        Ok(()) => ValidationItem::passed("no_overlap"),
        Err(e) => ValidationItem::failed("no_overlap", format!("{}", e)),
    };

    // 3. 验证地形连通性
    let terrain_connectivity = match validate_terrain_connectivity(&result.terrains) {
        Ok(()) => ValidationItem::passed("terrain_connectivity"),
        Err(e) => ValidationItem::failed("terrain_connectivity", format!("{}", e)),
    };

    // 4. 验证点位间距和禁布规则
    // 合并交互物和敌人点位
    let all_spawns: Vec<SpawnPoint> = result
        .item_spawns
        .iter()
        .chain(result.enemy_spawns.iter())
        .cloned()
        .collect();

    let spawn_spacing = match validate_spawn_spacing(&all_spawns, constraints, min_spacing) {
        Ok(()) => ValidationItem::passed("spawn_spacing"),
        Err(e) => ValidationItem::failed("spawn_spacing", format!("{}", e)),
    };

    // 5. 汇总统计
    let items = [&reachability, &no_overlap, &terrain_connectivity, &spawn_spacing];
    let passed_count = items.iter().filter(|item| item.passed).count();
    let failed_count = items.len() - passed_count;
    let all_passed = failed_count == 0;

    ValidationReport {
        reachability,
        no_overlap,
        terrain_connectivity,
        spawn_spacing,
        all_passed,
        passed_count,
        failed_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;
    use crate::model::geometry::{GridPoint, RoomBounds};
    use crate::model::request::RuntimeContext;
    use crate::model::result::ResultMetadata;
    use crate::model::room::{Room, RoomEdge, RoomGraph, RoomType};

    #[test]
    fn test_validate_request_with_default_config() {
        let request = GenerationRequest {
            seed: Some(42),
            config: GenerationConfig::default(),
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        };

        let result = validate_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_request_requires_runtime_context_for_chunked_mode() {
        let mut config = GenerationConfig::default();
        config.generation_mode = crate::config::GenerationMode::RuntimeChunked;
        config.capability_flags.runtime_chunked = true;

        let request = GenerationRequest {
            seed: Some(42),
            config,
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        };

        let err = validate_request(&request).unwrap_err();
        assert_eq!(err.error_code(), "PCG-CONFIG-001");

        let request = GenerationRequest {
            runtime_context: Some(RuntimeContext {
                focus_position: None,
                interest_radius: Some(100.0),
                requested_chunks: vec!["chunk-0".to_string()],
                caller_tag: Some("test".to_string()),
            }),
            ..request
        };

        assert!(validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_result_rejects_mismatched_room_counts() {
        let result = GenerationResult {
            metadata: ResultMetadata {
                seed: 1,
                config_digest: "digest".to_string(),
                schema_version: "1.0.0".to_string(),
                algorithm_version: "0.1.0".to_string(),
                target_engine_version: None,
                trace_id: None,
            },
            topology: RoomGraph {
                nodes: vec![crate::model::room::Room {
                    id: "room-0".to_string(),
                    room_type: crate::model::room::RoomType::Start,
                    depth_from_start: 0,
                    branch_id: None,
                    difficulty: 0,
                    theme_tags: vec![],
                    bounds: None,
                    template_ref: None,
                    grammar_token: None,
                }],
                edges: vec![],
                critical_path: vec![],
                branches: vec![],
            },
            rooms: vec![],
            door_anchors: vec![],
            corridors: vec![],
            terrains: vec![],
            item_spawns: vec![],
            enemy_spawns: vec![],
            chunks: vec![],
            debug: None,
        };

        let err = validate_result(&result).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
    }

    // ========== validate_reachability 测试 ==========

    /// 辅助函数：创建一个简单的房间节点
    fn make_room(id: &str, room_type: RoomType) -> Room {
        Room {
            id: id.to_string(),
            room_type,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec![],
            bounds: None,
            template_ref: None,
            grammar_token: None,
        }
    }

    /// 辅助函数：创建一条边
    fn make_edge(id: &str, from: &str, to: &str) -> RoomEdge {
        RoomEdge {
            id: id.to_string(),
            from_room: from.to_string(),
            to_room: to.to_string(),
            is_critical: false,
        }
    }

    #[test]
    fn test_reachability_all_connected() {
        // 验证需求: 3.2 - 所有房间从 Start 可达
        // 构建线性连通图: Start -> Combat -> Boss
        let graph = RoomGraph {
            nodes: vec![
                make_room("room-0", RoomType::Start),
                make_room("room-1", RoomType::Combat),
                make_room("room-2", RoomType::Boss),
            ],
            edges: vec![
                make_edge("edge-0", "room-0", "room-1"),
                make_edge("edge-1", "room-1", "room-2"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let result = validate_reachability(&graph);
        assert!(result.is_ok(), "所有房间应从 Start 可达");
    }

    #[test]
    fn test_reachability_with_unreachable_rooms() {
        // 验证需求: 3.2 - 不可达房间应返回错误
        // room-2 没有任何边连接，不可达
        let graph = RoomGraph {
            nodes: vec![
                make_room("room-0", RoomType::Start),
                make_room("room-1", RoomType::Combat),
                make_room("room-2", RoomType::Boss),
            ],
            edges: vec![
                make_edge("edge-0", "room-0", "room-1"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let err = validate_reachability(&graph).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
        let msg = format!("{}", err);
        assert!(msg.contains("room-2"), "错误信息应包含不可达房间 ID");
        assert!(msg.contains("1 个不可达房间"), "错误信息应包含不可达房间数量");
    }

    #[test]
    fn test_reachability_no_start_room() {
        // 验证需求: 3.2 - 没有 Start 房间时应返回错误
        let graph = RoomGraph {
            nodes: vec![
                make_room("room-0", RoomType::Combat),
                make_room("room-1", RoomType::Boss),
            ],
            edges: vec![
                make_edge("edge-0", "room-0", "room-1"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let err = validate_reachability(&graph).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
        let msg = format!("{}", err);
        assert!(msg.contains("Start"), "错误信息应提及 Start 房间缺失");
    }

    #[test]
    fn test_reachability_single_start_room() {
        // 验证需求: 3.2 - 只有一个 Start 房间且无边时应通过
        let graph = RoomGraph {
            nodes: vec![
                make_room("room-0", RoomType::Start),
            ],
            edges: vec![],
            critical_path: vec![],
            branches: vec![],
        };

        let result = validate_reachability(&graph);
        assert!(result.is_ok(), "单个 Start 房间应通过可达性验证");
    }

    #[test]
    fn test_reachability_branching_graph() {
        // 验证需求: 3.2 - 分支图中所有房间可达
        // Start -> Combat1 -> Boss
        //       -> Treasure
        let graph = RoomGraph {
            nodes: vec![
                make_room("start", RoomType::Start),
                make_room("combat1", RoomType::Combat),
                make_room("boss", RoomType::Boss),
                make_room("treasure", RoomType::Treasure),
            ],
            edges: vec![
                make_edge("e1", "start", "combat1"),
                make_edge("e2", "combat1", "boss"),
                make_edge("e3", "start", "treasure"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let result = validate_reachability(&graph);
        assert!(result.is_ok(), "分支图中所有房间应从 Start 可达");
    }

    #[test]
    fn test_reachability_multiple_unreachable() {
        // 验证需求: 3.2 - 多个不可达房间
        let graph = RoomGraph {
            nodes: vec![
                make_room("start", RoomType::Start),
                make_room("combat", RoomType::Combat),
                make_room("isolated1", RoomType::Treasure),
                make_room("isolated2", RoomType::Shop),
            ],
            edges: vec![
                make_edge("e1", "start", "combat"),
                // isolated1 和 isolated2 互相连接但与主图断开
                make_edge("e2", "isolated1", "isolated2"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let err = validate_reachability(&graph).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
        let msg = format!("{}", err);
        assert!(msg.contains("2 个不可达房间"), "应报告 2 个不可达房间");
    }

    // ========== validate_no_overlap 测试 ==========

    /// 辅助函数：创建带边界的房间
    fn make_room_with_bounds(id: &str, min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Room {
        Room {
            id: id.to_string(),
            room_type: RoomType::Combat,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec![],
            bounds: Some(RoomBounds {
                min: GridPoint { x: min_x, y: min_y },
                max: GridPoint { x: max_x, y: max_y },
            }),
            template_ref: None,
            grammar_token: None,
        }
    }

    /// 辅助函数：创建无边界的房间
    fn make_room_without_bounds(id: &str) -> Room {
        Room {
            id: id.to_string(),
            room_type: RoomType::Combat,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec![],
            bounds: None,
            template_ref: None,
            grammar_token: None,
        }
    }

    #[test]
    fn test_no_overlap_non_overlapping_rooms() {
        // 验证需求: 4.7 - 不重叠的房间应通过验证
        let rooms = vec![
            make_room_with_bounds("room-0", 0, 0, 5, 5),
            make_room_with_bounds("room-1", 10, 0, 15, 5),
            make_room_with_bounds("room-2", 0, 10, 5, 15),
        ];

        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "不重叠的房间应通过验证");
    }

    #[test]
    fn test_no_overlap_overlapping_rooms() {
        // 验证需求: 4.7 - 重叠的房间应返回错误
        let rooms = vec![
            make_room_with_bounds("room-0", 0, 0, 10, 10),
            make_room_with_bounds("room-1", 5, 5, 15, 15),
        ];

        let err = validate_no_overlap(&rooms).unwrap_err();
        assert_eq!(err.error_code(), "PCG-LAYOUT-001");
        let msg = format!("{}", err);
        assert!(msg.contains("room-0"), "错误信息应包含冲突房间 ID");
        assert!(msg.contains("room-1"), "错误信息应包含冲突房间 ID");
        assert!(msg.contains("1 对房间边界重叠"), "错误信息应包含冲突数量");
    }

    #[test]
    fn test_no_overlap_adjacent_rooms_not_overlapping() {
        // 验证需求: 4.7 - 相邻但不重叠的房间应通过验证
        // room-0 的 max.x == room-1 的 min.x，边界相切不算重叠
        let rooms = vec![
            make_room_with_bounds("room-0", 0, 0, 5, 5),
            make_room_with_bounds("room-1", 5, 0, 10, 5),
        ];

        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "相邻但不重叠的房间应通过验证");
    }

    #[test]
    fn test_no_overlap_rooms_without_bounds_skipped() {
        // 验证需求: 4.7 - 没有边界的房间应被跳过
        let rooms = vec![
            make_room_with_bounds("room-0", 0, 0, 10, 10),
            make_room_without_bounds("room-1"),
            make_room_with_bounds("room-2", 20, 20, 30, 30),
        ];

        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "没有边界的房间应被跳过，不影响验证");
    }

    #[test]
    fn test_no_overlap_multiple_conflicts() {
        // 验证需求: 4.7 - 多对冲突应全部报告
        let rooms = vec![
            make_room_with_bounds("room-0", 0, 0, 10, 10),
            make_room_with_bounds("room-1", 5, 5, 15, 15),
            make_room_with_bounds("room-2", 8, 8, 18, 18),
        ];

        let err = validate_no_overlap(&rooms).unwrap_err();
        assert_eq!(err.error_code(), "PCG-LAYOUT-001");
        let msg = format!("{}", err);
        // room-0 与 room-1 重叠，room-0 与 room-2 重叠，room-1 与 room-2 重叠
        assert!(msg.contains("3 对房间边界重叠"), "应报告 3 对冲突: {}", msg);
    }

    #[test]
    fn test_no_overlap_empty_rooms() {
        // 验证需求: 4.7 - 空房间列表应通过验证
        let rooms: Vec<Room> = vec![];
        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "空房间列表应通过验证");
    }

    #[test]
    fn test_no_overlap_single_room() {
        // 验证需求: 4.7 - 单个房间应通过验证
        let rooms = vec![make_room_with_bounds("room-0", 0, 0, 10, 10)];
        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "单个房间应通过验证");
    }

    #[test]
    fn test_no_overlap_all_rooms_without_bounds() {
        // 验证需求: 4.7 - 所有房间都没有边界时应通过验证
        let rooms = vec![
            make_room_without_bounds("room-0"),
            make_room_without_bounds("room-1"),
            make_room_without_bounds("room-2"),
        ];

        let result = validate_no_overlap(&rooms);
        assert!(result.is_ok(), "所有房间都没有边界时应通过验证");
    }

    // ========== validate_terrain_connectivity 测试 ==========

    use crate::model::terrain::{ConnectivitySummary, Grid2D, Terrain, TileKind};
    use crate::model::geometry::GridSize;

    /// 辅助函数：创建一个地形对象
    fn make_terrain(room_id: &str, width: u32, height: u32, tiles: Vec<TileKind>) -> Terrain {
        Terrain {
            room_id: room_id.to_string(),
            grid_size: GridSize { width, height },
            tiles: Grid2D {
                width,
                height,
                data: tiles,
            },
            reserved_zones: vec![],
            connectivity_summary: ConnectivitySummary {
                all_doors_connected: true,
                walkable_tile_count: 0,
                total_tile_count: width * height,
                connected_region_count: 1,
            },
        }
    }

    #[test]
    fn test_terrain_connectivity_all_doors_connected() {
        // 验证需求: 5.4 - 所有门口通过地板连通
        // 5x3 网格：
        // D F F F D
        // W W W W W  (墙)
        // W W W W W  (墙)
        // 实际上第一行全部可通行，两个门口连通
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Floor, TileKind::Floor, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain = make_terrain("room-0", 5, 3, tiles);

        let result = validate_terrain_connectivity(&[terrain]);
        assert!(result.is_ok(), "所有门口连通时应通过验证");
    }

    #[test]
    fn test_terrain_connectivity_disconnected_doors() {
        // 验证需求: 5.4 - 门口被墙体隔断时应返回错误
        // 5x3 网格：
        // D F W F D
        // W W W W W
        // W W W W W
        // 中间墙体将两个门口隔断
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Wall, TileKind::Floor, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain = make_terrain("room-1", 5, 3, tiles);

        let err = validate_terrain_connectivity(&[terrain]).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TERRAIN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("room-1"), "错误信息应包含房间 ID");
        assert!(msg.contains("不可达门口瓦片"), "错误信息应描述连通性失败");
    }

    #[test]
    fn test_terrain_connectivity_single_door() {
        // 验证需求: 5.4 - 只有一个门口时无需验证，直接通过
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Floor,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain = make_terrain("room-2", 3, 3, tiles);

        let result = validate_terrain_connectivity(&[terrain]);
        assert!(result.is_ok(), "只有一个门口时应直接通过验证");
    }

    #[test]
    fn test_terrain_connectivity_no_doors() {
        // 验证需求: 5.4 - 没有门口时无需验证，直接通过
        let tiles = vec![
            TileKind::Floor, TileKind::Floor, TileKind::Floor,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain = make_terrain("room-3", 3, 3, tiles);

        let result = validate_terrain_connectivity(&[terrain]);
        assert!(result.is_ok(), "没有门口时应直接通过验证");
    }

    #[test]
    fn test_terrain_connectivity_empty_terrains() {
        // 验证需求: 5.4 - 空地形列表应通过验证
        let result = validate_terrain_connectivity(&[]);
        assert!(result.is_ok(), "空地形列表应通过验证");
    }

    #[test]
    fn test_terrain_connectivity_multiple_terrains_one_fails() {
        // 验证需求: 5.4 - 多个地形中有一个不连通应返回错误
        // 第一个地形连通
        let tiles_ok = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain_ok = make_terrain("room-ok", 3, 3, tiles_ok);

        // 第二个地形不连通
        let tiles_bad = vec![
            TileKind::Doorway, TileKind::Wall, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrain_bad = make_terrain("room-bad", 3, 3, tiles_bad);

        let err = validate_terrain_connectivity(&[terrain_ok, terrain_bad]).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TERRAIN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("room-bad"), "错误信息应包含失败的房间 ID");
    }

    #[test]
    fn test_terrain_connectivity_doors_connected_via_reserved() {
        // 验证需求: 5.4 - 门口通过 Reserved 瓦片连通也应通过
        // 3x1 网格：D R D
        let tiles = vec![
            TileKind::Doorway, TileKind::Reserved, TileKind::Doorway,
        ];
        let terrain = make_terrain("room-4", 3, 1, tiles);

        let result = validate_terrain_connectivity(&[terrain]);
        assert!(result.is_ok(), "门口通过 Reserved 瓦片连通应通过验证");
    }

    #[test]
    fn test_terrain_connectivity_three_doors_partial_disconnect() {
        // 验证需求: 5.4 - 三个门口中有一个不可达
        // 5x3 网格：
        // D F F W D
        // W W W W W
        // D F F W W
        // 左上角门口和左下角门口不连通（因为第二行全是墙），右上角门口也不可达
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Floor, TileKind::Wall, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Doorway, TileKind::Floor, TileKind::Floor, TileKind::Wall, TileKind::Wall,
        ];
        let terrain = make_terrain("room-5", 5, 3, tiles);

        let err = validate_terrain_connectivity(&[terrain]).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TERRAIN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("room-5"), "错误信息应包含房间 ID");
    }

    // ========== validate_spawn_spacing 测试 ==========

    use crate::model::spawn::{SpawnKind, SpawnMetadata, SpawnPoint};
    use crate::model::request::{Constraint, ExclusionZoneConstraint};

    /// 辅助函数：创建一个点位
    fn make_spawn(id: &str, room_id: &str, x: i32, y: i32, kind: SpawnKind) -> SpawnPoint {
        SpawnPoint {
            id: id.to_string(),
            room_id: room_id.to_string(),
            kind,
            grid_pos: GridPoint { x, y },
            world_transform: None,
            metadata: SpawnMetadata {
                spawn_tag: "test".to_string(),
                rarity_tier: None,
                enemy_pool_tag: None,
                encounter_id: None,
                wave_id: None,
                difficulty: None,
                seed: 0,
            },
        }
    }

    /// 辅助函数：创建排除区约束
    fn make_exclusion_zone(
        label: &str,
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
    ) -> Constraint {
        Constraint::ExclusionZone(ExclusionZoneConstraint {
            label: label.to_string(),
            min: GridPoint { x: min_x, y: min_y },
            max: GridPoint { x: max_x, y: max_y },
            exclude_rooms: false,
            exclude_spawns: true,
        })
    }

    #[test]
    fn test_spawn_spacing_all_valid() {
        // 验证需求: 7.4 - 满足间距的点位应通过验证
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 5, 0, SpawnKind::Item),
            make_spawn("s3", "room-0", 0, 5, SpawnKind::Enemy),
        ];

        let result = validate_spawn_spacing(&spawns, &[], Some(2));
        assert!(result.is_ok(), "满足间距的点位应通过验证");
    }

    #[test]
    fn test_spawn_spacing_violation() {
        // 验证需求: 7.4 - 间距不足的点位应返回错误
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 1, 0, SpawnKind::Item),
        ];

        let err = validate_spawn_spacing(&spawns, &[], Some(2)).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("间距违规"), "错误信息应包含间距违规描述");
        assert!(msg.contains("s1"), "错误信息应包含违规点位 ID");
        assert!(msg.contains("s2"), "错误信息应包含违规点位 ID");
    }

    #[test]
    fn test_spawn_spacing_different_rooms_no_conflict() {
        // 验证需求: 7.4 - 不同房间的点位不检查间距
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-1", 0, 0, SpawnKind::Item),
        ];

        let result = validate_spawn_spacing(&spawns, &[], Some(2));
        assert!(result.is_ok(), "不同房间的点位不应检查间距");
    }

    #[test]
    fn test_spawn_spacing_exclusion_zone_violation() {
        // 验证需求: 8.3 - 点位在排除区内应返回错误
        let spawns = vec![
            make_spawn("s1", "room-0", 3, 3, SpawnKind::Enemy),
        ];
        let constraints = vec![
            make_exclusion_zone("boss-area", 0, 0, 5, 5),
        ];

        let err = validate_spawn_spacing(&spawns, &constraints, Some(2)).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("排除区违规"), "错误信息应包含排除区违规描述");
        assert!(msg.contains("boss-area"), "错误信息应包含排除区标签");
    }

    #[test]
    fn test_spawn_spacing_exclusion_zone_boundary() {
        // 验证需求: 8.3 - 排除区边界外的点位应通过
        // 排除区 [0,0) ~ [5,5)，点位在 (5,5) 刚好在边界外
        let spawns = vec![
            make_spawn("s1", "room-0", 5, 5, SpawnKind::Item),
        ];
        let constraints = vec![
            make_exclusion_zone("zone-a", 0, 0, 5, 5),
        ];

        let result = validate_spawn_spacing(&spawns, &constraints, Some(2));
        assert!(result.is_ok(), "排除区边界外的点位应通过验证");
    }

    #[test]
    fn test_spawn_spacing_default_min_spacing() {
        // 验证需求: 7.4 - 使用默认间距（2）
        // 曼哈顿距离 = |1-0| + |0-0| = 1 < 2
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 1, 0, SpawnKind::Enemy),
        ];

        let err = validate_spawn_spacing(&spawns, &[], None).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
    }

    #[test]
    fn test_spawn_spacing_exact_boundary() {
        // 验证需求: 7.4 - 恰好等于最小间距时应通过
        // 曼哈顿距离 = |2-0| + |0-0| = 2 == min_spacing(2)
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 2, 0, SpawnKind::Item),
        ];

        let result = validate_spawn_spacing(&spawns, &[], Some(2));
        assert!(result.is_ok(), "恰好等于最小间距时应通过验证");
    }

    #[test]
    fn test_spawn_spacing_empty_spawns() {
        // 验证需求: 7.4 - 空点位列表应通过验证
        let result = validate_spawn_spacing(&[], &[], Some(2));
        assert!(result.is_ok(), "空点位列表应通过验证");
    }

    #[test]
    fn test_spawn_spacing_mixed_violations() {
        // 验证需求: 7.4, 8.3 - 同时存在间距违规和排除区违规
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 1, 0, SpawnKind::Item),
            make_spawn("s3", "room-0", 3, 3, SpawnKind::Enemy),
        ];
        let constraints = vec![
            make_exclusion_zone("danger-zone", 2, 2, 5, 5),
        ];

        let err = validate_spawn_spacing(&spawns, &constraints, Some(2)).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("间距违规"), "错误信息应包含间距违规");
        assert!(msg.contains("排除区违规"), "错误信息应包含排除区违规");
    }

    #[test]
    fn test_spawn_spacing_non_spawn_exclusion_zone_ignored() {
        // 验证需求: 8.3 - exclude_spawns 为 false 的排除区不影响点位
        let spawns = vec![
            make_spawn("s1", "room-0", 3, 3, SpawnKind::Item),
        ];
        let constraints = vec![
            Constraint::ExclusionZone(ExclusionZoneConstraint {
                label: "room-only-zone".to_string(),
                min: GridPoint { x: 0, y: 0 },
                max: GridPoint { x: 5, y: 5 },
                exclude_rooms: true,
                exclude_spawns: false,
            }),
        ];

        let result = validate_spawn_spacing(&spawns, &constraints, Some(2));
        assert!(result.is_ok(), "exclude_spawns 为 false 的排除区不应影响点位验证");
    }

    #[test]
    fn test_spawn_spacing_multiple_rooms_independent() {
        // 验证需求: 7.4 - 多个房间独立验证间距
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 5, 5, SpawnKind::Item),
            make_spawn("s3", "room-1", 0, 0, SpawnKind::Enemy),
            make_spawn("s4", "room-1", 1, 0, SpawnKind::Enemy),
        ];

        let err = validate_spawn_spacing(&spawns, &[], Some(2)).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
        let msg = format!("{}", err);
        // room-0 的点位间距足够，room-1 的点位间距不足
        assert!(msg.contains("room-1"), "错误信息应包含违规房间 ID");
        assert!(msg.contains("s3"), "错误信息应包含违规点位 ID");
        assert!(msg.contains("s4"), "错误信息应包含违规点位 ID");
    }

    // ========== ValidationReport 和 run_full_validation 测试 ==========

    /// 辅助函数：创建一个最小合法的 GenerationResult 用于验证报告测试
    fn make_valid_generation_result() -> GenerationResult {
        use crate::model::geometry::{CardinalDir, GridPoint, GridSize, RoomBounds};
        use crate::model::room::{
            Corridor, CorridorPath, DoorAnchor, Room, RoomEdge, RoomGraph, RoomType,
        };
        use crate::model::result::ResultMetadata;
        use crate::model::spawn::{SpawnKind, SpawnMetadata, SpawnPoint};
        use crate::model::terrain::{ConnectivitySummary, Grid2D, Terrain, TileKind};

        let rooms = vec![
            Room {
                id: "room-0".to_string(),
                room_type: RoomType::Start,
                depth_from_start: 0,
                branch_id: None,
                difficulty: 0,
                theme_tags: vec![],
                bounds: Some(RoomBounds {
                    min: GridPoint { x: 0, y: 0 },
                    max: GridPoint { x: 10, y: 10 },
                }),
                template_ref: None,
                grammar_token: None,
            },
            Room {
                id: "room-1".to_string(),
                room_type: RoomType::Boss,
                depth_from_start: 1,
                branch_id: None,
                difficulty: 1,
                theme_tags: vec![],
                bounds: Some(RoomBounds {
                    min: GridPoint { x: 20, y: 0 },
                    max: GridPoint { x: 30, y: 10 },
                }),
                template_ref: None,
                grammar_token: None,
            },
        ];

        let topology = RoomGraph {
            nodes: rooms.clone(),
            edges: vec![RoomEdge {
                id: "edge-0".to_string(),
                from_room: "room-0".to_string(),
                to_room: "room-1".to_string(),
                is_critical: true,
            }],
            critical_path: vec!["room-0".to_string(), "room-1".to_string()],
            branches: vec![],
        };

        // 创建连通的地形（两个门口通过地板连接）
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Floor, TileKind::Floor, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall,
        ];
        let terrains = vec![
            Terrain {
                room_id: "room-0".to_string(),
                grid_size: GridSize { width: 5, height: 3 },
                tiles: Grid2D { width: 5, height: 3, data: tiles.clone() },
                reserved_zones: vec![],
                connectivity_summary: ConnectivitySummary {
                    all_doors_connected: true,
                    walkable_tile_count: 5,
                    total_tile_count: 15,
                    connected_region_count: 1,
                },
            },
            Terrain {
                room_id: "room-1".to_string(),
                grid_size: GridSize { width: 5, height: 3 },
                tiles: Grid2D { width: 5, height: 3, data: tiles },
                reserved_zones: vec![],
                connectivity_summary: ConnectivitySummary {
                    all_doors_connected: true,
                    walkable_tile_count: 5,
                    total_tile_count: 15,
                    connected_region_count: 1,
                },
            },
        ];

        // 创建间距足够的点位
        let item_spawns = vec![SpawnPoint {
            id: "item-0".to_string(),
            room_id: "room-0".to_string(),
            grid_pos: GridPoint { x: 2, y: 0 },
            kind: SpawnKind::Item,
            metadata: SpawnMetadata {
                spawn_tag: "test".to_string(),
                rarity_tier: None,
                enemy_pool_tag: None,
                encounter_id: None,
                wave_id: None,
                difficulty: None,
                seed: 0,
            },
            world_transform: None,
        }];

        let enemy_spawns = vec![SpawnPoint {
            id: "enemy-0".to_string(),
            room_id: "room-1".to_string(),
            grid_pos: GridPoint { x: 2, y: 0 },
            kind: SpawnKind::Enemy,
            metadata: SpawnMetadata {
                spawn_tag: "test".to_string(),
                rarity_tier: None,
                enemy_pool_tag: None,
                encounter_id: None,
                wave_id: None,
                difficulty: None,
                seed: 0,
            },
            world_transform: None,
        }];

        GenerationResult {
            metadata: ResultMetadata {
                seed: 42,
                config_digest: "test-digest".to_string(),
                schema_version: "1.0.0".to_string(),
                algorithm_version: "0.1.0".to_string(),
                target_engine_version: None,
                trace_id: None,
            },
            topology,
            rooms,
            door_anchors: vec![DoorAnchor {
                id: "door-0".to_string(),
                room_id: "room-0".to_string(),
                edge_id: "edge-0".to_string(),
                grid_pos: GridPoint { x: 10, y: 5 },
                facing: CardinalDir::East,
                width_tiles: 2,
                socket_tag: None,
            }],
            corridors: vec![Corridor {
                id: "corridor-0".to_string(),
                from_room: "room-0".to_string(),
                to_room: "room-1".to_string(),
                from_anchor: "door-0".to_string(),
                to_anchor: "door-1".to_string(),
                width_tiles: 2,
                path: CorridorPath::Straight(vec![
                    GridPoint { x: 10, y: 5 },
                    GridPoint { x: 20, y: 5 },
                ]),
                segment_tags: Vec::new(),
            }],
            terrains,
            item_spawns,
            enemy_spawns,
            chunks: vec![],
            debug: None,
        }
    }

    #[test]
    fn test_validation_item_passed() {
        // 验证需求: 6.6 - ValidationItem 通过状态构造
        let item = ValidationItem::passed("reachability");
        assert!(item.passed);
        assert_eq!(item.name, "reachability");
        assert!(item.error_message.is_none());
    }

    #[test]
    fn test_validation_item_failed() {
        // 验证需求: 6.6 - ValidationItem 失败状态构造
        let item = ValidationItem::failed("no_overlap", "存在重叠房间");
        assert!(!item.passed);
        assert_eq!(item.name, "no_overlap");
        assert_eq!(item.error_message.as_deref(), Some("存在重叠房间"));
    }

    #[test]
    fn test_validation_report_all_passed() {
        // 验证需求: 6.6, 15.3 - 所有验证通过时报告状态正确
        let result = make_valid_generation_result();
        let report = run_full_validation(&result, &[], None);

        assert!(report.all_passed, "所有验证应通过");
        assert_eq!(report.passed_count, 4);
        assert_eq!(report.failed_count, 0);
        assert!(report.reachability.passed);
        assert!(report.no_overlap.passed);
        assert!(report.terrain_connectivity.passed);
        assert!(report.spawn_spacing.passed);
    }

    #[test]
    fn test_validation_report_with_failures() {
        // 验证需求: 6.6, 15.3 - 存在失败时报告状态正确
        let mut result = make_valid_generation_result();

        // 添加一个不可达的房间
        result.topology.nodes.push(Room {
            id: "isolated".to_string(),
            room_type: RoomType::Treasure,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec![],
            bounds: Some(RoomBounds {
                min: GridPoint { x: 50, y: 50 },
                max: GridPoint { x: 60, y: 60 },
            }),
            template_ref: None,
            grammar_token: None,
        });
        result.rooms.push(result.topology.nodes.last().unwrap().clone());

        let report = run_full_validation(&result, &[], None);

        assert!(!report.all_passed, "应有验证失败");
        assert!(!report.reachability.passed, "可达性验证应失败");
        assert!(report.reachability.error_message.is_some());
        assert!(report.no_overlap.passed, "边界不重叠验证应通过");
        assert!(report.terrain_connectivity.passed, "地形连通性验证应通过");
        assert!(report.spawn_spacing.passed, "点位间距验证应通过");
        assert_eq!(report.passed_count, 3);
        assert_eq!(report.failed_count, 1);
    }

    #[test]
    fn test_validation_report_items_method() {
        // 验证需求: 6.6 - items() 方法返回所有验证项
        let result = make_valid_generation_result();
        let report = run_full_validation(&result, &[], None);

        let items = report.items();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].name, "reachability");
        assert_eq!(items[1].name, "no_overlap");
        assert_eq!(items[2].name, "terrain_connectivity");
        assert_eq!(items[3].name, "spawn_spacing");
    }

    #[test]
    fn test_validation_report_serializable() {
        // 验证需求: 6.6 - ValidationReport 可序列化
        let result = make_valid_generation_result();
        let report = run_full_validation(&result, &[], None);

        let json = serde_json::to_string(&report).expect("序列化应成功");
        assert!(json.contains("reachability"));
        assert!(json.contains("all_passed"));

        let deserialized: ValidationReport =
            serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(deserialized.all_passed, report.all_passed);
        assert_eq!(deserialized.passed_count, report.passed_count);
        assert_eq!(deserialized.failed_count, report.failed_count);
    }

    #[test]
    fn test_debug_bundle_with_validation_report() {
        // 验证需求: 15.3 - DebugBundle 包含验证报告
        use crate::debug::DebugBundle;

        let result = make_valid_generation_result();
        let report = run_full_validation(&result, &[], None);

        let bundle = DebugBundle {
            trace_id: None,
            stage_stats: vec![],
            notes: vec![],
            validation_report: Some(report),
            debug_channels: None,
        };

        assert!(bundle.validation_report.is_some());
        let report = bundle.validation_report.unwrap();
        assert!(report.all_passed);
        assert_eq!(report.passed_count, 4);
    }

    // ========== 补充边界条件测试 ==========

    #[test]
    fn test_reachability_empty_graph() {
        // 验证需求: 3.2 - 空图（无节点）应返回错误（无 Start 房间）
        let graph = RoomGraph {
            nodes: vec![],
            edges: vec![],
            critical_path: vec![],
            branches: vec![],
        };

        let err = validate_reachability(&graph).unwrap_err();
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
        let msg = format!("{}", err);
        assert!(msg.contains("Start"), "空图应报告缺少 Start 房间");
    }

    #[test]
    fn test_reachability_cycle_graph() {
        // 验证需求: 3.2 - 环形图中所有房间可达
        // Start -> A -> B -> Start（形成环）
        let graph = RoomGraph {
            nodes: vec![
                make_room("start", RoomType::Start),
                make_room("a", RoomType::Combat),
                make_room("b", RoomType::Treasure),
            ],
            edges: vec![
                make_edge("e1", "start", "a"),
                make_edge("e2", "a", "b"),
                make_edge("e3", "b", "start"),
            ],
            critical_path: vec![],
            branches: vec![],
        };

        let result = validate_reachability(&graph);
        assert!(result.is_ok(), "环形图中所有房间应从 Start 可达");
    }

    #[test]
    fn test_no_overlap_room_contained_within_another() {
        // 验证需求: 4.7 - 一个房间完全包含在另一个房间内应检测为重叠
        let rooms = vec![
            make_room_with_bounds("outer", 0, 0, 20, 20),
            make_room_with_bounds("inner", 5, 5, 10, 10),
        ];

        let err = validate_no_overlap(&rooms).unwrap_err();
        assert_eq!(err.error_code(), "PCG-LAYOUT-001");
        let msg = format!("{}", err);
        assert!(msg.contains("outer"), "错误信息应包含外层房间 ID");
        assert!(msg.contains("inner"), "错误信息应包含内层房间 ID");
    }

    #[test]
    fn test_no_overlap_identical_bounds() {
        // 验证需求: 4.7 - 两个房间边界完全相同应检测为重叠
        let rooms = vec![
            make_room_with_bounds("room-a", 0, 0, 10, 10),
            make_room_with_bounds("room-b", 0, 0, 10, 10),
        ];

        let err = validate_no_overlap(&rooms).unwrap_err();
        assert_eq!(err.error_code(), "PCG-LAYOUT-001");
    }

    #[test]
    fn test_spawn_spacing_single_spawn() {
        // 验证需求: 7.4 - 单个点位无需比较间距，应通过
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
        ];

        let result = validate_spawn_spacing(&spawns, &[], Some(100));
        assert!(result.is_ok(), "单个点位应通过间距验证");
    }

    #[test]
    fn test_spawn_spacing_min_spacing_one() {
        // 验证需求: 7.4 - 最小间距为 1 时，相邻点位（距离=1）不满足
        let spawns = vec![
            make_spawn("s1", "room-0", 0, 0, SpawnKind::Item),
            make_spawn("s2", "room-0", 1, 0, SpawnKind::Item),
        ];

        // 距离 = 1，min_spacing = 1，1 < 1 为 false，应通过
        let result = validate_spawn_spacing(&spawns, &[], Some(1));
        assert!(result.is_ok(), "距离等于最小间距时应通过验证");
    }

    #[test]
    fn test_spawn_spacing_same_position() {
        // 验证需求: 7.4 - 两个点位在同一位置（距离=0）应违规
        let spawns = vec![
            make_spawn("s1", "room-0", 5, 5, SpawnKind::Item),
            make_spawn("s2", "room-0", 5, 5, SpawnKind::Enemy),
        ];

        let err = validate_spawn_spacing(&spawns, &[], Some(1)).unwrap_err();
        assert_eq!(err.error_code(), "PCG-SPAWN-001");
        let msg = format!("{}", err);
        assert!(msg.contains("间距违规"), "同一位置的点位应报告间距违规");
    }

    #[test]
    fn test_terrain_connectivity_large_connected_grid() {
        // 验证需求: 5.4 - 较大网格中门口通过复杂路径连通
        // 7x3 网格：
        // D F F F F F D
        // W W W W W F W
        // W W W W W F W
        // 右侧门口需要绕行才能到达
        let tiles = vec![
            TileKind::Doorway, TileKind::Floor, TileKind::Floor, TileKind::Floor, TileKind::Floor, TileKind::Floor, TileKind::Doorway,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Floor, TileKind::Wall,
            TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Wall, TileKind::Floor, TileKind::Wall,
        ];
        let terrain = make_terrain("room-large", 7, 3, tiles);

        let result = validate_terrain_connectivity(&[terrain]);
        assert!(result.is_ok(), "通过复杂路径连通的门口应通过验证");
    }

    #[test]
    fn test_run_full_validation_multiple_failures() {
        // 验证需求: 6.6, 15.3 - 多项验证同时失败时报告正确
        let mut result = make_valid_generation_result();

        // 1. 添加不可达房间（可达性失败）
        result.topology.nodes.push(Room {
            id: "isolated".to_string(),
            room_type: RoomType::Treasure,
            depth_from_start: 0,
            branch_id: None,
            difficulty: 0,
            theme_tags: vec![],
            bounds: Some(RoomBounds {
                min: GridPoint { x: 50, y: 50 },
                max: GridPoint { x: 60, y: 60 },
            }),
            template_ref: None,
            grammar_token: None,
        });
        result.rooms.push(result.topology.nodes.last().unwrap().clone());

        // 2. 制造重叠（边界不重叠失败）
        result.rooms[0].bounds = Some(RoomBounds {
            min: GridPoint { x: 0, y: 0 },
            max: GridPoint { x: 25, y: 10 },
        });
        result.topology.nodes[0].bounds = result.rooms[0].bounds;

        let report = run_full_validation(&result, &[], None);

        assert!(!report.all_passed, "应有多项验证失败");
        assert!(!report.reachability.passed, "可达性验证应失败");
        assert!(!report.no_overlap.passed, "边界不重叠验证应失败");
        assert!(report.failed_count >= 2, "至少应有 2 项失败");
        assert_eq!(
            report.passed_count + report.failed_count, 4,
            "通过数 + 失败数应等于总验证项数"
        );
    }
}
