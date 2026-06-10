//! # yang-pcg
//!
//! YANG Procedural Content Generation - UE5 Roguelike 地图生成算法库
//!
//! 本库提供可复现、可测试、可缓存、可导出、可分块加载的地图生成核心，
//! 并通过独立适配层与 UE5 PCG 工作流对接。
//!
//! ## 核心特性
//!
//! - **确定性生成**：相同种子和配置生成相同结果
//! - **分层架构**：算法核心与 UE5 集成解耦
//! - **可观测性**：完整的调试和追踪支持
//! - **可扩展性**：支持约束、模板、分块等增强能力
//!
//! ## 模块组织
//!
//! - `config`: 配置管理与归一化
//! - `generator`: 核心生成器编排
//! - `rng`: 确定性随机数生成
//! - `topology`: 房间拓扑与路径规划
//! - `layout`: 空间布局与走廊生成
//! - `terrain`: 房间地形生成
//! - `spawn`: 交互物与敌人点位生成
//! - `constraint`: 约束求解（锚点、排除区、模板）
//! - `model`: 核心数据模型
//! - `ue`: UE5 适配层
//! - `debug`: 调试与分析输出
//! - `cache`: 缓存管理
//! - `error`: 错误类型定义
//! - `validation`: 结果验证与不变量检查
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use yang_pcg::{GenerationRequest, GenerationConfig, MapGenerator};
//!
//! // 创建生成请求
//! let request = GenerationRequest {
//!     seed: Some(12345),
//!     config: GenerationConfig::default(),
//!     constraints: vec![],
//!     runtime_context: None,
//!     trace_id: None,
//! };
//!
//! // 生成地图
//! let generator = MapGenerator::new();
//! let result = generator.generate(request)?;
//!
//! // 导出为 JSON
//! let json = serde_json::to_string_pretty(&result)?;
//! ```

// 核心模块
pub mod backend;
pub mod cache;
pub mod chunked;
pub mod config;
pub mod debug;
pub mod digest;
pub mod error;
pub mod export;
pub mod generator;
pub mod grammar;
pub mod rng;
pub mod validation;

// 生成流程模块
pub mod constraint;
pub mod layout;
pub mod spawn;
pub mod terrain;
pub mod topology;

// 数据模型
pub mod model;

// UE5 集成
pub mod ue;

// 重新导出常用类型
pub use chunked::{ChunkDetailResult, TopologyResult};
pub use config::{GenerationConfig, GenerationMode, NormalizedConfig};
pub use digest::ConfigDigest;
pub use error::{PcgError, PcgResult};
pub use export::{
    export_binary, export_json, export_json_compact, import_binary, import_json,
    CURRENT_SCHEMA_VERSION,
};
pub use generator::MapGenerator;
pub use grammar::{GrammarContext, GrammarRule, WeightedRuleSelector};
pub use model::{
    request::{GenerationRequest, RuntimeContext},
    result::{GenerationResult, ResultMetadata},
    room::{Room, RoomType},
};

// 任务 26 测试模块
#[cfg(test)]
mod tests_task26;

// 任务 27 属性测试模块
#[cfg(test)]
mod tests_task27;

// 基础功能测试
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationConfig;

    #[test]
    fn test_basic_types_creation() {
        // 测试基本类型可以创建
        let config = GenerationConfig::default();
        assert_eq!(config.room_count.min, 10);
        assert_eq!(config.room_count.max, 20);

        let request = GenerationRequest {
            seed: Some(12345),
            config,
            constraints: vec![],
            runtime_context: None,
            trace_id: None,
        };
        assert_eq!(request.seed, Some(12345));

        let generator = MapGenerator::new();
        // 注意：实际生成功能尚未实现，这里只测试类型创建
        let _ = generator;
    }

    #[test]
    fn test_room_types() {
        use crate::model::room::RoomType;

        // 测试房间类型枚举
        let room_types = vec![
            RoomType::Start,
            RoomType::Combat,
            RoomType::Treasure,
            RoomType::Shop,
            RoomType::Elite,
            RoomType::Puzzle,
            RoomType::Safe,
            RoomType::Boss,
            RoomType::Event,
            RoomType::Secret,
        ];

        assert_eq!(room_types.len(), 10);
        assert_eq!(room_types[0], RoomType::Start);
        assert_eq!(room_types[7], RoomType::Boss);
    }

    #[test]
    fn test_generation_modes() {
        use crate::config::GenerationMode;

        let modes = [
            GenerationMode::OfflineFullFloor,
            GenerationMode::RuntimeChunked,
            GenerationMode::HybridPrecompute,
        ];

        assert_eq!(modes.len(), 3);
        assert_eq!(modes[0], GenerationMode::OfflineFullFloor);
    }
}
