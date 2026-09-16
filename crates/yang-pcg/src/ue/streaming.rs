// 流式加载元数据

use crate::config::NormalizedConfig;
use crate::error::{PcgError, PcgResult};
use crate::model::chunk::{Chunk, ChunkId, StreamingMetadata};
use crate::model::geometry::RoomBounds;
use crate::model::room::Room;

/// 根据房间边界生成最小分块信息。
pub fn build_chunks(rooms: &[Room], config: &NormalizedConfig) -> PcgResult<Vec<Chunk>> {
    if rooms.is_empty() {
        return Ok(Vec::new());
    }

    if !config.config.chunking.enabled {
        let room_ids = rooms.iter().map(|room| room.id.clone()).collect();
        return Ok(vec![Chunk {
            id: "chunk-0-0".to_string(),
            bounds: aggregate_bounds(rooms)?,
            room_ids,
            dependencies: Vec::new(),
            streaming_metadata: StreamingMetadata {
                data_layer: None,
                external_data_layer: None,
                hlod_layer: None,
                streaming_priority: None,
            },
        }]);
    }

    let chunk_size = i32::from(config.config.chunking.chunk_size);
    let mut chunks = std::collections::BTreeMap::<ChunkId, Vec<&Room>>::new();

    for room in rooms {
        let Some(bounds) = room.bounds else {
            continue;
        };
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
            // chunk_rooms: Vec<&Room>，`.iter().copied()` 产出 &Room，零深拷贝
            let bounds = aggregate_bounds(chunk_rooms.iter().copied())?;
            Ok(Chunk {
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
            })
        })
        .collect()
}

fn aggregate_bounds<'a>(rooms: impl IntoIterator<Item = &'a Room>) -> PcgResult<RoomBounds> {
    let mut iter = rooms.into_iter().filter_map(|room| room.bounds);
    let Some(first) = iter.next() else {
        return Err(PcgError::layout(
            "无法聚合分块边界:该分块内没有任何房间具有 bounds".to_string(),
        ));
    };
    let mut min = first.min;
    let mut max = first.max;
    for bounds in iter {
        min.x = min.x.min(bounds.min.x);
        min.y = min.y.min(bounds.min.y);
        max.x = max.x.max(bounds.max.x);
        max.y = max.y.max(bounds.max.y);
    }
    Ok(RoomBounds { min, max })
}
