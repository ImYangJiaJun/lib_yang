// 分块数据模型测试
// 验证需求: 11.2, 11.4, 12.1, 12.3

use crate::model::chunk::*;
use crate::model::geometry::{GridPoint, RoomBounds};

#[test]
fn test_chunk_creation() {
    // 测试分块创建
    let chunk = Chunk {
        id: "chunk-001".to_string(),
        bounds: RoomBounds {
            min: GridPoint { x: 0, y: 0 },
            max: GridPoint { x: 100, y: 100 },
        },
        room_ids: vec![
            "room-001".to_string(),
            "room-002".to_string(),
            "room-003".to_string(),
        ],
        dependencies: vec!["chunk-000".to_string()],
        streaming_metadata: StreamingMetadata {
            data_layer: Some("Gameplay".to_string()),
            external_data_layer: None,
            hlod_layer: None,
            streaming_priority: Some(100),
        },
    };

    assert_eq!(chunk.id, "chunk-001");
    assert_eq!(chunk.room_ids.len(), 3);
    assert_eq!(chunk.dependencies.len(), 1);
}

#[test]
fn test_streaming_metadata_full() {
    // 测试完整流式元数据
    let metadata = StreamingMetadata {
        data_layer: Some("MainLevel".to_string()),
        external_data_layer: Some("ExternalContent".to_string()),
        hlod_layer: Some("HLOD0".to_string()),
        streaming_priority: Some(200),
    };

    assert_eq!(metadata.data_layer, Some("MainLevel".to_string()));
    assert_eq!(
        metadata.external_data_layer,
        Some("ExternalContent".to_string())
    );
    assert_eq!(metadata.hlod_layer, Some("HLOD0".to_string()));
    assert_eq!(metadata.streaming_priority, Some(200));
}

#[test]
fn test_streaming_metadata_minimal() {
    // 测试最小流式元数据
    let metadata = StreamingMetadata {
        data_layer: None,
        external_data_layer: None,
        hlod_layer: None,
        streaming_priority: None,
    };

    assert!(metadata.data_layer.is_none());
    assert!(metadata.external_data_layer.is_none());
    assert!(metadata.hlod_layer.is_none());
    assert!(metadata.streaming_priority.is_none());
}

#[test]
fn test_chunk_with_dependencies() {
    // 测试带依赖的分块
    let chunk = Chunk {
        id: "chunk-005".to_string(),
        bounds: RoomBounds {
            min: GridPoint { x: 200, y: 200 },
            max: GridPoint { x: 300, y: 300 },
        },
        room_ids: vec!["room-010".to_string()],
        dependencies: vec![
            "chunk-001".to_string(),
            "chunk-002".to_string(),
            "chunk-003".to_string(),
        ],
        streaming_metadata: StreamingMetadata {
            data_layer: Some("DynamicContent".to_string()),
            external_data_layer: None,
            hlod_layer: None,
            streaming_priority: Some(50),
        },
    };

    assert_eq!(chunk.dependencies.len(), 3);
    assert!(chunk.dependencies.contains(&"chunk-001".to_string()));
}

#[test]
fn test_chunk_without_dependencies() {
    // 测试无依赖的分块
    let chunk = Chunk {
        id: "chunk-start".to_string(),
        bounds: RoomBounds {
            min: GridPoint { x: 0, y: 0 },
            max: GridPoint { x: 50, y: 50 },
        },
        room_ids: vec!["room-start".to_string()],
        dependencies: vec![],
        streaming_metadata: StreamingMetadata {
            data_layer: Some("StartArea".to_string()),
            external_data_layer: None,
            hlod_layer: None,
            streaming_priority: Some(1000),
        },
    };

    assert!(chunk.dependencies.is_empty());
    assert_eq!(chunk.streaming_metadata.streaming_priority, Some(1000));
}
