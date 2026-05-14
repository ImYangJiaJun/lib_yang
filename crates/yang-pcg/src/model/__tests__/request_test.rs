// 生成请求数据模型测试
// 验证需求: 1.1, 2.1, 10.1

use crate::config::GenerationConfig;
use crate::model::geometry::WorldPoint;
use crate::model::request::*;

#[test]
fn test_generation_request_creation() {
    // 测试生成请求创建
    let request = GenerationRequest {
        seed: Some(12345),
        config: GenerationConfig::default(),
        constraints: vec![],
        runtime_context: None,
        trace_id: Some("test-trace-001".to_string()),
    };

    assert_eq!(request.seed, Some(12345));
    assert_eq!(request.trace_id, Some("test-trace-001".to_string()));
    assert!(request.constraints.is_empty());
    assert!(request.runtime_context.is_none());
}

#[test]
fn test_runtime_context_creation() {
    // 测试运行时上下文创建
    let context = RuntimeContext {
        focus_position: Some(WorldPoint {
            x: 100.0,
            y: 200.0,
            z: 0.0,
        }),
        interest_radius: Some(500.0),
        requested_chunks: vec!["chunk-1".to_string(), "chunk-2".to_string()],
        caller_tag: Some("player".to_string()),
    };

    assert!(context.focus_position.is_some());
    assert_eq!(context.interest_radius, Some(500.0));
    assert_eq!(context.requested_chunks.len(), 2);
    assert_eq!(context.caller_tag, Some("player".to_string()));
}

#[test]
fn test_generation_request_with_runtime_context() {
    // 测试带运行时上下文的生成请求
    let context = RuntimeContext {
        focus_position: Some(WorldPoint {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }),
        interest_radius: Some(1000.0),
        requested_chunks: vec![],
        caller_tag: None,
    };

    let request = GenerationRequest {
        seed: None,
        config: GenerationConfig::default(),
        constraints: vec![],
        runtime_context: Some(context),
        trace_id: None,
    };

    assert!(request.runtime_context.is_some());
    assert!(request.seed.is_none());
}
