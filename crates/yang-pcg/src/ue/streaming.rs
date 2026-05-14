// 流式加载元数据

use crate::config::NormalizedConfig;
use crate::model::chunk::{Chunk, ChunkId, StreamingMetadata};
use crate::model::geometry::{GridPoint, RoomBounds};
use crate::model::room::Room;

/// 根据房间边界生成最小分块信息。
pub fn build_chunks(rooms: &[Room], config: &NormalizedConfig) -> Vec<Chunk> {
    if rooms.is_empty() {
        return Vec::new();
    }

    if !config.config.chunking.enabled {
        let room_ids = rooms.iter().map(|room| room.id.clone()).collect();
        return vec![Chunk {
            id: "chunk-0-0".to_string(),
            bounds: aggregate_bounds(rooms),
            room_ids,
            dependencies: Vec::new(),
            streaming_metadata: StreamingMetadata {
                data_layer: None,
                external_data_layer: None,
                hlod_layer: None,
                streaming_priority: None,
            },
        }];
    }

    let chunk_size = i32::from(config.config.chunking.chunk_size);
    let mut chunks = std::collections::BTreeMap::<ChunkId, Vec<&Room>>::new();

    for room in rooms.iter().filter(|room| room.bounds.is_some()) {
        let bounds = room.bounds.expect("已过滤 None");
        let center = bounds.center();
        let chunk_x = center.x.div_euclid(chunk_size);
        let chunk_y = center.y.div_euclid(chunk_size);
        chunks
            .entry(format!("chunk-{chunk_x}-{chunk_y}"))
            .or_default()
            .push(room);
    }

    chunks
        .into_iter()
        .map(|(id, chunk_rooms)| {
            let room_ids = chunk_rooms.iter().map(|room| room.id.clone()).collect();
            let bounds = aggregate_bounds(
                &chunk_rooms
                    .iter()
                    .map(|room| (*room).clone())
                    .collect::<Vec<_>>(),
            );
            Chunk {
                id,
                bounds,
                room_ids,
                dependencies: Vec::new(),
                streaming_metadata: StreamingMetadata {
                    data_layer: None,
                    external_data_layer: None,
                    hlod_layer: None,
                    streaming_priority: None,
                },
            }
        })
        .collect()
}

fn aggregate_bounds(rooms: &[Room]) -> RoomBounds {
    let mut min = GridPoint {
        x: i32::MAX,
        y: i32::MAX,
    };
    let mut max = GridPoint {
        x: i32::MIN,
        y: i32::MIN,
    };

    for bounds in rooms.iter().filter_map(|room| room.bounds) {
        min.x = min.x.min(bounds.min.x);
        min.y = min.y.min(bounds.min.y);
        max.x = max.x.max(bounds.max.x);
        max.y = max.y.max(bounds.max.y);
    }

    RoomBounds { min, max }
}
