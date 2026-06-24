// UE5 适配器

use std::collections::BTreeMap;

use crate::error::PcgResult;
use crate::model::geometry::{Bounds3, Transform3, WorldPoint};
use crate::model::result::GenerationResult;
use crate::model::room::CorridorPath;
use crate::model::terrain::TileKind;

use super::channels::{ChannelKind, NamedChannel, Polyline3};
use super::points::{PcgPoint, PropertyValue};

/// 将生成结果导出为 UE5 兼容的具名通道。
///
/// 当 `ResultMetadata.trace_id` 存在时，会将其写入每个通道的元数据中，
/// 以便下游系统通过 `trace_id` 串联日志与导出结果。
pub fn export_named_channels(result: &GenerationResult) -> PcgResult<Vec<NamedChannel>> {
    let mut channels = vec![
        export_room_channel(result),
        export_door_channel(result),
        export_corridor_channel(result),
        export_item_channel(result),
        export_enemy_channel(result),
    ];

    let (floor_channel, wall_channel) = export_tile_channels(result);
    channels.push(floor_channel);
    channels.push(wall_channel);

    // 将 trace_id 写入每个通道的元数据，实现追踪标识串联
    if let Some(ref trace_id) = result.metadata.trace_id {
        for channel in &mut channels {
            channel.metadata.insert(
                "trace_id".to_string(),
                PropertyValue::String(trace_id.clone()),
            );
        }
    }

    Ok(channels)
}

/// 将生成结果导出为 UE5 具名通道，并序列化为 JSON 字符串。
///
/// 这是 [`export_named_channels`] 的便捷封装：先生成 `Vec<NamedChannel>`，
/// 再用 `serde_json` 序列化，使具名通道可直接落盘供 UE5 侧读取。
///
/// # 示例
///
/// ```rust,ignore
/// let json = yang_pcg::ue::export_named_channels_json(&result)?;
/// std::fs::write("channels.json", json)?;
/// ```
pub fn export_named_channels_json(result: &GenerationResult) -> PcgResult<String> {
    let channels = export_named_channels(result)?;
    serde_json::to_string(&channels).map_err(|e| {
        crate::error::PcgError::export_err(
            format!("具名通道序列化失败: {}", e),
            "ue_channels_json",
            e,
        )
    })
}

fn export_room_channel(result: &GenerationResult) -> NamedChannel {
    let points = result
        .rooms
        .iter()
        .filter_map(|room| {
            let bounds = room.bounds?;
            Some(PcgPoint {
                transform: grid_transform(bounds.center()),
                bounds: room_bounds3(bounds),
                density: 1.0,
                seed: result.metadata.seed,
                attributes: BTreeMap::from([
                    (
                        "room_id".to_string(),
                        PropertyValue::String(room.id.clone()),
                    ),
                    (
                        "room_type".to_string(),
                        PropertyValue::String(format!("{:?}", room.room_type)),
                    ),
                    (
                        "difficulty".to_string(),
                        PropertyValue::Int(i64::from(room.difficulty)),
                    ),
                ]),
            })
        })
        .collect();

    NamedChannel {
        name: "rooms".to_string(),
        kind: ChannelKind::Rooms,
        points,
        polylines: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn export_door_channel(result: &GenerationResult) -> NamedChannel {
    let points = result
        .door_anchors
        .iter()
        .map(|anchor| PcgPoint {
            transform: grid_transform(anchor.grid_pos),
            bounds: Bounds3 {
                min: grid_world_point(anchor.grid_pos),
                max: grid_world_point(anchor.grid_pos),
            },
            density: 1.0,
            seed: result.metadata.seed,
            attributes: BTreeMap::from([
                (
                    "room_id".to_string(),
                    PropertyValue::String(anchor.room_id.clone()),
                ),
                (
                    "edge_id".to_string(),
                    PropertyValue::String(anchor.edge_id.clone()),
                ),
                (
                    "facing".to_string(),
                    PropertyValue::String(format!("{:?}", anchor.facing)),
                ),
            ]),
        })
        .collect();

    NamedChannel {
        name: "doors".to_string(),
        kind: ChannelKind::Doors,
        points,
        polylines: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn export_corridor_channel(result: &GenerationResult) -> NamedChannel {
    let polylines: Vec<Polyline3> = result
        .corridors
        .iter()
        .map(|corridor| match &corridor.path {
            CorridorPath::Straight(points)
            | CorridorPath::Orthogonal(points)
            | CorridorPath::Polyline(points) => points
                .iter()
                .map(|point| grid_world_point(*point))
                .collect(),
        })
        .collect();

    NamedChannel {
        name: "corridors".to_string(),
        kind: ChannelKind::Corridors,
        points: Vec::new(),
        polylines,
        metadata: BTreeMap::new(),
    }
}

fn export_item_channel(result: &GenerationResult) -> NamedChannel {
    let points = result
        .item_spawns
        .iter()
        .map(|spawn| PcgPoint {
            transform: spawn
                .world_transform
                .unwrap_or_else(|| grid_transform(spawn.grid_pos)),
            bounds: Bounds3 {
                min: grid_world_point(spawn.grid_pos),
                max: grid_world_point(spawn.grid_pos),
            },
            density: 1.0,
            seed: spawn.metadata.seed,
            attributes: BTreeMap::from([
                (
                    "room_id".to_string(),
                    PropertyValue::String(spawn.room_id.clone()),
                ),
                (
                    "spawn_tag".to_string(),
                    PropertyValue::String(spawn.metadata.spawn_tag.clone()),
                ),
            ]),
        })
        .collect();

    NamedChannel {
        name: "spawn_items".to_string(),
        kind: ChannelKind::ItemSpawns,
        points,
        polylines: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn export_enemy_channel(result: &GenerationResult) -> NamedChannel {
    let points = result
        .enemy_spawns
        .iter()
        .map(|spawn| PcgPoint {
            transform: spawn
                .world_transform
                .unwrap_or_else(|| grid_transform(spawn.grid_pos)),
            bounds: Bounds3 {
                min: grid_world_point(spawn.grid_pos),
                max: grid_world_point(spawn.grid_pos),
            },
            density: 1.0,
            seed: spawn.metadata.seed,
            attributes: BTreeMap::from([
                (
                    "room_id".to_string(),
                    PropertyValue::String(spawn.room_id.clone()),
                ),
                (
                    "enemy_pool_tag".to_string(),
                    PropertyValue::String(
                        spawn
                            .metadata
                            .enemy_pool_tag
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                ),
            ]),
        })
        .collect();

    NamedChannel {
        name: "spawn_enemies".to_string(),
        kind: ChannelKind::EnemySpawns,
        points,
        polylines: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn export_tile_channels(result: &GenerationResult) -> (NamedChannel, NamedChannel) {
    let mut floor_points = Vec::new();
    let mut wall_points = Vec::new();

    for terrain in &result.terrains {
        let Some(room) = result.rooms.iter().find(|room| room.id == terrain.room_id) else {
            continue;
        };
        let Some(bounds) = room.bounds else {
            continue;
        };

        for y in 0..terrain.grid_size.height as i32 {
            for x in 0..terrain.grid_size.width as i32 {
                let world_grid = crate::model::geometry::GridPoint {
                    x: bounds.min.x + x,
                    y: bounds.min.y + y,
                };
                let point = PcgPoint {
                    transform: grid_transform(world_grid),
                    bounds: Bounds3 {
                        min: grid_world_point(world_grid),
                        max: grid_world_point(world_grid),
                    },
                    density: 1.0,
                    seed: result.metadata.seed,
                    attributes: BTreeMap::from([(
                        "room_id".to_string(),
                        PropertyValue::String(room.id.clone()),
                    )]),
                };
                match terrain.tiles.get(x, y).copied() {
                    Some(TileKind::Floor | TileKind::Doorway | TileKind::Reserved) => {
                        floor_points.push(point)
                    }
                    Some(TileKind::Wall | TileKind::Obstacle) => wall_points.push(point),
                    _ => {}
                }
            }
        }
    }

    (
        NamedChannel {
            name: "floor_tiles".to_string(),
            kind: ChannelKind::FloorTiles,
            points: floor_points,
            polylines: Vec::new(),
            metadata: BTreeMap::new(),
        },
        NamedChannel {
            name: "wall_tiles".to_string(),
            kind: ChannelKind::WallTiles,
            points: wall_points,
            polylines: Vec::new(),
            metadata: BTreeMap::new(),
        },
    )
}

fn room_bounds3(bounds: crate::model::geometry::RoomBounds) -> Bounds3 {
    Bounds3 {
        min: grid_world_point(bounds.min),
        max: grid_world_point(bounds.max),
    }
}

fn grid_transform(point: crate::model::geometry::GridPoint) -> Transform3 {
    Transform3 {
        position: grid_world_point(point),
        ..Transform3::default()
    }
}

fn grid_world_point(point: crate::model::geometry::GridPoint) -> WorldPoint {
    WorldPoint {
        x: point.x as f32,
        y: point.y as f32,
        z: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;
    use crate::generator::MapGenerator;
    use crate::model::request::GenerationRequest;

    #[test]
    fn test_export_named_channels_from_generated_result() {
        let result = MapGenerator::new()
            .generate(GenerationRequest {
                seed: Some(101),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: None,
            })
            .expect("应能生成测试地图");

        let channels = export_named_channels(&result).expect("应能导出具名通道");
        assert!(channels.iter().any(|channel| channel.name == "rooms"));
        assert!(channels.iter().any(|channel| channel.name == "doors"));
        assert!(channels.iter().any(|channel| channel.name == "corridors"));
        assert!(channels.iter().any(|channel| channel.name == "spawn_items"));
        assert!(channels
            .iter()
            .any(|channel| channel.name == "spawn_enemies"));
    }

    #[test]
    fn test_named_channels_json_roundtrip() {
        let result = MapGenerator::new()
            .generate(GenerationRequest {
                seed: Some(202),
                config: GenerationConfig::default(),
                constraints: vec![],
                runtime_context: None,
                trace_id: Some("rt".to_string()),
            })
            .expect("应能生成测试地图");

        // 序列化为 JSON 字符串
        let json = export_named_channels_json(&result).expect("应能序列化具名通道为 JSON");
        assert!(!json.is_empty());

        // 反序列化回 Vec<NamedChannel> 并比对关键字段，验证 Serialize/Deserialize 闭环
        let restored: Vec<NamedChannel> =
            serde_json::from_str(&json).expect("应能从 JSON 反序列化具名通道");
        let original = export_named_channels(&result).expect("应能导出具名通道");

        assert_eq!(restored.len(), original.len());
        for (a, b) in restored.iter().zip(original.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.points.len(), b.points.len());
            assert_eq!(a.polylines.len(), b.polylines.len());
        }
    }
}
