// 错误类型定义
// 提供统一的错误处理机制，包含丰富的上下文信息

use thiserror::Error;

/// PCG 库的统一错误类型
///
/// 每个错误变体都携带足够的上下文信息，包括：
/// - 阶段名称（stage）
/// - 随机种子（seed）
/// - 追踪 ID（trace_id）
/// - 字段路径（field_path）
/// - 房间 ID（room_id）
/// - 分块 ID（chunk_id）
///
/// 错误信息使用中文描述，并提供稳定的机器可读错误码。
///
/// 注意：为了减小 Result 类型的大小，建议在函数签名中使用 `Box<PcgError>`。
#[derive(Error, Debug, Clone)]
pub enum PcgError {
    /// 配置错误
    ///
    /// 当配置参数非法、超出范围或存在互斥约束时触发。
    ///
    /// **错误码**: `PCG-CONFIG-001`
    #[error("[PCG-CONFIG-001] 配置错误: {message}")]
    Config {
        /// 错误描述
        message: String,
        /// 错误字段路径（如 "room_count.min"）
        field_path: Option<String>,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
    },

    /// 约束不满足错误
    ///
    /// 当输入约束彼此冲突或无法满足时触发。
    ///
    /// **错误码**: `PCG-CONSTRAINT-001`
    #[error("[PCG-CONSTRAINT-001] 约束不满足: {message}")]
    Constraint {
        /// 错误描述
        message: String,
        /// 约束类型（如 "anchor", "exclusion_zone"）
        constraint_type: Option<String>,
        /// 冲突详情
        conflict_details: Option<String>,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 相关房间 ID
        room_id: Option<String>,
    },

    /// 拓扑生成错误
    ///
    /// 当房间图生成失败、关键路径无法构建或房型分配失败时触发。
    ///
    /// **错误码**: `PCG-TOPOLOGY-001`
    #[error("[PCG-TOPOLOGY-001] 拓扑生成错误: {message}")]
    Topology {
        /// 错误描述
        message: String,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 当前房间数量
        room_count: Option<usize>,
        /// 目标房间数量
        target_room_count: Option<usize>,
    },

    /// 布局求解错误
    ///
    /// 当房间边界计算失败、门锚点生成失败或走廊路径无法构建时触发。
    ///
    /// **错误码**: `PCG-LAYOUT-001`
    #[error("[PCG-LAYOUT-001] 布局求解错误: {message}")]
    Layout {
        /// 错误描述
        message: String,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 相关房间 ID
        room_id: Option<String>,
        /// 碰撞详情
        collision_details: Option<String>,
    },

    /// 地形生成错误
    ///
    /// 当房间地形生成失败、连通性验证失败或障碍物布局失败时触发。
    ///
    /// **错误码**: `PCG-TERRAIN-001`
    #[error("[PCG-TERRAIN-001] 地形生成错误: {message}")]
    Terrain {
        /// 错误描述
        message: String,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 相关房间 ID
        room_id: Option<String>,
        /// 地形策略
        strategy: Option<String>,
        /// 连通性检查结果
        connectivity_failed: Option<bool>,
    },

    /// 点位生成错误
    ///
    /// 当交互物或敌人点位生成失败、间距验证失败或预算耗尽时触发。
    ///
    /// **错误码**: `PCG-SPAWN-001`
    #[error("[PCG-SPAWN-001] 点位生成错误: {message}")]
    Spawn {
        /// 错误描述
        message: String,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 相关房间 ID
        room_id: Option<String>,
        /// 点位类型（如 "item", "enemy"）
        spawn_type: Option<String>,
        /// 候选点位数量
        candidate_count: Option<usize>,
        /// 目标点位数量
        target_count: Option<usize>,
    },

    /// 导出错误
    ///
    /// 当结果序列化、导出或格式转换失败时触发。
    ///
    /// **错误码**: `PCG-EXPORT-001`
    #[error("[PCG-EXPORT-001] 导出错误: {message}")]
    Export {
        /// 错误描述
        message: String,
        /// 导出格式（如 "json", "binary"）
        format: Option<String>,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 底层错误信息
        source_error: Option<String>,
    },

    /// 预算耗尽错误
    ///
    /// 当时间预算、迭代预算或内存预算耗尽时触发。
    ///
    /// **错误码**: `PCG-BUDGET-001`
    #[error("[PCG-BUDGET-001] 预算耗尽: {message}")]
    BudgetExhausted {
        /// 错误描述
        message: String,
        /// 预算类型（如 "time", "iteration", "memory"）
        budget_type: String,
        /// 已消耗量
        consumed: Option<u64>,
        /// 预算上限
        limit: Option<u64>,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
    },

    /// 迭代次数超限错误
    ///
    /// 当重试次数超过配置上限时触发。
    ///
    /// **错误码**: `PCG-ITERATION-001`
    #[error("[PCG-ITERATION-001] 迭代次数超限: {message}")]
    IterationLimit {
        /// 错误描述
        message: String,
        /// 当前迭代次数
        current_iteration: u32,
        /// 最大迭代次数
        max_iteration: u32,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 相关房间 ID
        room_id: Option<String>,
    },

    /// 数据损坏错误
    ///
    /// 当缓存数据、导入数据或中间结果损坏时触发。
    ///
    /// **错误码**: `PCG-CORRUPTED-001`
    #[error("[PCG-CORRUPTED-001] 数据损坏: {message}")]
    CorruptedData {
        /// 错误描述
        message: String,
        /// 数据类型（如 "cache", "import", "intermediate"）
        data_type: Option<String>,
        /// 预期版本
        expected_version: Option<String>,
        /// 实际版本
        actual_version: Option<String>,
        /// 生成阶段
        stage: Option<String>,
        /// 随机种子
        seed: Option<u64>,
        /// 追踪 ID
        trace_id: Option<String>,
        /// 分块 ID
        chunk_id: Option<String>,
    },

    /// 能力不可用错误
    ///
    /// 当请求的功能在当前配置或版本中不可用时触发。
    ///
    /// **错误码**: `PCG-CAPABILITY-001`
    #[error("[PCG-CAPABILITY-001] 能力不可用: {message}")]
    CapabilityUnavailable {
        /// 错误描述
        message: String,
        /// 能力名称（如 "runtime_chunked", "grammar"）
        capability: String,
        /// 最低要求版本
        required_version: Option<String>,
        /// 当前版本
        current_version: Option<String>,
        /// 生成阶段
        stage: Option<String>,
        /// 追踪 ID
        trace_id: Option<String>,
    },
}

/// PCG 库的统一结果类型
///
/// 使用 `Box<PcgError>` 来减小 Result 的栈大小，避免 clippy 警告。
pub type PcgResult<T> = Result<T, Box<PcgError>>;

impl PcgError {
    /// 获取错误码
    ///
    /// 返回稳定的机器可读错误码，用于错误分类和处理。
    pub fn error_code(&self) -> &'static str {
        match self {
            PcgError::Config { .. } => "PCG-CONFIG-001",
            PcgError::Constraint { .. } => "PCG-CONSTRAINT-001",
            PcgError::Topology { .. } => "PCG-TOPOLOGY-001",
            PcgError::Layout { .. } => "PCG-LAYOUT-001",
            PcgError::Terrain { .. } => "PCG-TERRAIN-001",
            PcgError::Spawn { .. } => "PCG-SPAWN-001",
            PcgError::Export { .. } => "PCG-EXPORT-001",
            PcgError::BudgetExhausted { .. } => "PCG-BUDGET-001",
            PcgError::IterationLimit { .. } => "PCG-ITERATION-001",
            PcgError::CorruptedData { .. } => "PCG-CORRUPTED-001",
            PcgError::CapabilityUnavailable { .. } => "PCG-CAPABILITY-001",
        }
    }

    /// 获取生成阶段
    ///
    /// 返回错误发生时的生成阶段名称。
    pub fn stage(&self) -> Option<&str> {
        match self {
            PcgError::Config { stage, .. }
            | PcgError::Constraint { stage, .. }
            | PcgError::Topology { stage, .. }
            | PcgError::Layout { stage, .. }
            | PcgError::Terrain { stage, .. }
            | PcgError::Spawn { stage, .. }
            | PcgError::Export { stage, .. }
            | PcgError::BudgetExhausted { stage, .. }
            | PcgError::IterationLimit { stage, .. }
            | PcgError::CorruptedData { stage, .. }
            | PcgError::CapabilityUnavailable { stage, .. } => stage.as_deref(),
        }
    }

    /// 获取随机种子
    ///
    /// 返回错误发生时使用的随机种子。
    pub fn seed(&self) -> Option<u64> {
        match self {
            PcgError::Config { seed, .. }
            | PcgError::Constraint { seed, .. }
            | PcgError::Topology { seed, .. }
            | PcgError::Layout { seed, .. }
            | PcgError::Terrain { seed, .. }
            | PcgError::Spawn { seed, .. }
            | PcgError::Export { seed, .. }
            | PcgError::BudgetExhausted { seed, .. }
            | PcgError::IterationLimit { seed, .. }
            | PcgError::CorruptedData { seed, .. } => *seed,
            PcgError::CapabilityUnavailable { .. } => None,
        }
    }

    /// 获取追踪 ID
    ///
    /// 返回错误发生时的追踪标识，用于串联日志、缓存与导出结果。
    pub fn trace_id(&self) -> Option<&str> {
        match self {
            PcgError::Config { trace_id, .. }
            | PcgError::Constraint { trace_id, .. }
            | PcgError::Topology { trace_id, .. }
            | PcgError::Layout { trace_id, .. }
            | PcgError::Terrain { trace_id, .. }
            | PcgError::Spawn { trace_id, .. }
            | PcgError::Export { trace_id, .. }
            | PcgError::BudgetExhausted { trace_id, .. }
            | PcgError::IterationLimit { trace_id, .. }
            | PcgError::CorruptedData { trace_id, .. }
            | PcgError::CapabilityUnavailable { trace_id, .. } => trace_id.as_deref(),
        }
    }

    /// 创建配置错误的便捷方法
    pub fn config(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::config_unboxed(message))
    }

    /// 创建配置错误（未装箱）
    pub fn config_unboxed(message: impl Into<String>) -> Self {
        PcgError::Config {
            message: message.into(),
            field_path: None,
            stage: None,
            seed: None,
            trace_id: None,
        }
    }

    /// 创建配置错误并指定字段路径
    pub fn config_with_field(
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Box<Self> {
        Box::new(Self::config_with_field_unboxed(message, field_path))
    }

    /// 创建配置错误并指定字段路径（未装箱）
    pub fn config_with_field_unboxed(
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        PcgError::Config {
            message: message.into(),
            field_path: Some(field_path.into()),
            stage: None,
            seed: None,
            trace_id: None,
        }
    }

    /// 创建约束错误的便捷方法
    pub fn constraint(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::constraint_unboxed(message))
    }

    /// 创建约束错误（未装箱）
    pub fn constraint_unboxed(message: impl Into<String>) -> Self {
        PcgError::Constraint {
            message: message.into(),
            constraint_type: None,
            conflict_details: None,
            stage: None,
            seed: None,
            trace_id: None,
            room_id: None,
        }
    }

    /// 创建拓扑错误的便捷方法
    pub fn topology(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::topology_unboxed(message))
    }

    /// 创建拓扑错误（未装箱）
    pub fn topology_unboxed(message: impl Into<String>) -> Self {
        PcgError::Topology {
            message: message.into(),
            stage: None,
            seed: None,
            trace_id: None,
            room_count: None,
            target_room_count: None,
        }
    }

    /// 创建布局错误的便捷方法
    pub fn layout(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::layout_unboxed(message))
    }

    /// 创建布局错误（未装箱）
    pub fn layout_unboxed(message: impl Into<String>) -> Self {
        PcgError::Layout {
            message: message.into(),
            stage: None,
            seed: None,
            trace_id: None,
            room_id: None,
            collision_details: None,
        }
    }

    /// 创建地形错误的便捷方法
    pub fn terrain(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::terrain_unboxed(message))
    }

    /// 创建地形错误（未装箱）
    pub fn terrain_unboxed(message: impl Into<String>) -> Self {
        PcgError::Terrain {
            message: message.into(),
            stage: None,
            seed: None,
            trace_id: None,
            room_id: None,
            strategy: None,
            connectivity_failed: None,
        }
    }

    /// 创建点位错误的便捷方法
    pub fn spawn(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::spawn_unboxed(message))
    }

    /// 创建点位错误（未装箱）
    pub fn spawn_unboxed(message: impl Into<String>) -> Self {
        PcgError::Spawn {
            message: message.into(),
            stage: None,
            seed: None,
            trace_id: None,
            room_id: None,
            spawn_type: None,
            candidate_count: None,
            target_count: None,
        }
    }

    /// 创建导出错误的便捷方法
    pub fn export(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::export_unboxed(message))
    }

    /// 创建导出错误（未装箱）
    pub fn export_unboxed(message: impl Into<String>) -> Self {
        PcgError::Export {
            message: message.into(),
            format: None,
            stage: None,
            seed: None,
            trace_id: None,
            source_error: None,
        }
    }

    /// 创建导出错误并指定格式和底层错误信息
    pub fn export_with_format(
        message: impl Into<String>,
        format: impl Into<String>,
        source_error: Option<String>,
    ) -> Box<Self> {
        Box::new(PcgError::Export {
            message: message.into(),
            format: Some(format.into()),
            stage: Some("export".to_string()),
            seed: None,
            trace_id: None,
            source_error,
        })
    }

    /// 创建预算耗尽错误的便捷方法
    pub fn budget_exhausted(
        message: impl Into<String>,
        budget_type: impl Into<String>,
    ) -> Box<Self> {
        Box::new(Self::budget_exhausted_unboxed(message, budget_type))
    }

    /// 创建预算耗尽错误（未装箱）
    pub fn budget_exhausted_unboxed(
        message: impl Into<String>,
        budget_type: impl Into<String>,
    ) -> Self {
        PcgError::BudgetExhausted {
            message: message.into(),
            budget_type: budget_type.into(),
            consumed: None,
            limit: None,
            stage: None,
            seed: None,
            trace_id: None,
        }
    }

    /// 创建迭代超限错误的便捷方法
    pub fn iteration_limit(message: impl Into<String>, current: u32, max: u32) -> Box<Self> {
        Box::new(Self::iteration_limit_unboxed(message, current, max))
    }

    /// 创建迭代超限错误（未装箱）
    pub fn iteration_limit_unboxed(message: impl Into<String>, current: u32, max: u32) -> Self {
        PcgError::IterationLimit {
            message: message.into(),
            current_iteration: current,
            max_iteration: max,
            stage: None,
            seed: None,
            trace_id: None,
            room_id: None,
        }
    }

    /// 创建数据损坏错误的便捷方法
    pub fn corrupted_data(message: impl Into<String>) -> Box<Self> {
        Box::new(Self::corrupted_data_unboxed(message))
    }

    /// 创建数据损坏错误（未装箱）
    pub fn corrupted_data_unboxed(message: impl Into<String>) -> Self {
        PcgError::CorruptedData {
            message: message.into(),
            data_type: None,
            expected_version: None,
            actual_version: None,
            stage: None,
            seed: None,
            trace_id: None,
            chunk_id: None,
        }
    }

    /// 创建数据损坏错误并指定数据类型
    pub fn corrupted_data_with_type(
        message: impl Into<String>,
        data_type: impl Into<String>,
    ) -> Box<Self> {
        Box::new(PcgError::CorruptedData {
            message: message.into(),
            data_type: Some(data_type.into()),
            expected_version: None,
            actual_version: None,
            stage: None,
            seed: None,
            trace_id: None,
            chunk_id: None,
        })
    }

    /// 创建数据损坏错误并指定版本信息
    pub fn corrupted_data_with_version(
        message: impl Into<String>,
        data_type: impl Into<String>,
        expected_version: &str,
        actual_version: &str,
    ) -> Box<Self> {
        Box::new(PcgError::CorruptedData {
            message: message.into(),
            data_type: Some(data_type.into()),
            expected_version: Some(expected_version.to_string()),
            actual_version: Some(actual_version.to_string()),
            stage: None,
            seed: None,
            trace_id: None,
            chunk_id: None,
        })
    }

    /// 创建能力不可用错误的便捷方法
    pub fn capability_unavailable(
        message: impl Into<String>,
        capability: impl Into<String>,
    ) -> Box<Self> {
        Box::new(Self::capability_unavailable_unboxed(message, capability))
    }

    /// 创建能力不可用错误（未装箱）
    pub fn capability_unavailable_unboxed(
        message: impl Into<String>,
        capability: impl Into<String>,
    ) -> Self {
        PcgError::CapabilityUnavailable {
            message: message.into(),
            capability: capability.into(),
            required_version: None,
            current_version: None,
            stage: None,
            trace_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code() {
        let err = *PcgError::config("测试配置错误");
        assert_eq!(err.error_code(), "PCG-CONFIG-001");

        let err = *PcgError::constraint("测试约束错误");
        assert_eq!(err.error_code(), "PCG-CONSTRAINT-001");

        let err = *PcgError::topology("测试拓扑错误");
        assert_eq!(err.error_code(), "PCG-TOPOLOGY-001");
    }

    #[test]
    fn test_config_error_with_context() {
        let err = PcgError::Config {
            message: "房间数量超出范围".to_string(),
            field_path: Some("room_count.max".to_string()),
            stage: Some("config_validation".to_string()),
            seed: Some(12345),
            trace_id: Some("trace-001".to_string()),
        };

        assert_eq!(err.error_code(), "PCG-CONFIG-001");
        assert_eq!(err.stage(), Some("config_validation"));
        assert_eq!(err.seed(), Some(12345));
        assert_eq!(err.trace_id(), Some("trace-001"));
    }

    #[test]
    fn test_constraint_error_with_room_id() {
        let err = PcgError::Constraint {
            message: "锚点冲突".to_string(),
            constraint_type: Some("anchor".to_string()),
            conflict_details: Some("Boss 房间与商店房间锚点重叠".to_string()),
            stage: Some("constraint_check".to_string()),
            seed: Some(67890),
            trace_id: Some("trace-002".to_string()),
            room_id: Some("room-5".to_string()),
        };

        assert_eq!(err.error_code(), "PCG-CONSTRAINT-001");
        assert_eq!(err.seed(), Some(67890));
    }

    #[test]
    fn test_budget_exhausted_error() {
        let err = *PcgError::budget_exhausted("时间预算耗尽", "time");
        assert_eq!(err.error_code(), "PCG-BUDGET-001");
    }

    #[test]
    fn test_iteration_limit_error() {
        let err = *PcgError::iteration_limit("布局求解失败", 100, 100);
        assert_eq!(err.error_code(), "PCG-ITERATION-001");

        if let PcgError::IterationLimit {
            current_iteration,
            max_iteration,
            ..
        } = err
        {
            assert_eq!(current_iteration, 100);
            assert_eq!(max_iteration, 100);
        } else {
            panic!("错误类型不匹配");
        }
    }

    #[test]
    fn test_capability_unavailable_error() {
        let err = *PcgError::capability_unavailable("运行时分块模式不可用", "runtime_chunked");
        assert_eq!(err.error_code(), "PCG-CAPABILITY-001");
    }

    #[test]
    fn test_error_display() {
        let err = *PcgError::config("测试错误");
        let display = format!("{}", err);
        assert!(display.contains("PCG-CONFIG-001"));
        assert!(display.contains("测试错误"));
    }

    #[test]
    fn test_convenience_methods() {
        let err = *PcgError::config_with_field("字段错误", "test.field");
        if let PcgError::Config { field_path, .. } = err {
            assert_eq!(field_path, Some("test.field".to_string()));
        } else {
            panic!("错误类型不匹配");
        }
    }
}
