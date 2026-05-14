# 错误处理指南

## 概述

`yang-pcg` 使用统一的错误类型 `PcgError` 来处理所有可能的错误情况。每个错误都携带丰富的上下文信息，包括阶段名称、随机种子、追踪 ID、字段路径、房间 ID 和分块 ID 等。

## 错误类型

### 错误分类

`PcgError` 按生成阶段分为以下几类：

1. **配置错误** (`Config`) - 错误码: `PCG-CONFIG-001`
   - 配置参数非法、超出范围或存在互斥约束

2. **约束错误** (`Constraint`) - 错误码: `PCG-CONSTRAINT-001`
   - 输入约束彼此冲突或无法满足

3. **拓扑错误** (`Topology`) - 错误码: `PCG-TOPOLOGY-001`
   - 房间图生成失败、关键路径无法构建或房型分配失败

4. **布局错误** (`Layout`) - 错误码: `PCG-LAYOUT-001`
   - 房间边界计算失败、门锚点生成失败或走廊路径无法构建

5. **地形错误** (`Terrain`) - 错误码: `PCG-TERRAIN-001`
   - 房间地形生成失败、连通性验证失败或障碍物布局失败

6. **点位错误** (`Spawn`) - 错误码: `PCG-SPAWN-001`
   - 交互物或敌人点位生成失败、间距验证失败或预算耗尽

7. **导出错误** (`Export`) - 错误码: `PCG-EXPORT-001`
   - 结果序列化、导出或格式转换失败

8. **预算耗尽** (`BudgetExhausted`) - 错误码: `PCG-BUDGET-001`
   - 时间预算、迭代预算或内存预算耗尽

9. **迭代超限** (`IterationLimit`) - 错误码: `PCG-ITERATION-001`
   - 重试次数超过配置上限

10. **数据损坏** (`CorruptedData`) - 错误码: `PCG-CORRUPTED-001`
    - 缓存数据、导入数据或中间结果损坏

11. **能力不可用** (`CapabilityUnavailable`) - 错误码: `PCG-CAPABILITY-001`
    - 请求的功能在当前配置或版本中不可用

## 使用示例

### 基本用法

```rust
use yang_pcg::error::{PcgError, PcgResult};

fn validate_room_count(count: usize) -> PcgResult<()> {
    if count == 0 {
        return Err(PcgError::config_with_field(
            "房间数量不能为 0",
            "room_count"
        ));
    }
    Ok(())
}
```

### 带上下文的错误

```rust
use yang_pcg::error::PcgError;

fn generate_topology(seed: u64, trace_id: &str) -> Result<(), Box<PcgError>> {
    // 生成失败时返回带上下文的错误
    Err(Box::new(PcgError::Topology {
        message: "无法生成关键路径".to_string(),
        stage: Some("topology_generation".to_string()),
        seed: Some(seed),
        trace_id: Some(trace_id.to_string()),
        room_count: Some(5),
        target_room_count: Some(10),
    }))
}
```

### 错误处理

```rust
use yang_pcg::error::PcgError;

fn handle_generation_error(err: Box<PcgError>) {
    // 获取错误码
    println!("错误码: {}", err.error_code());
    
    // 获取生成阶段
    if let Some(stage) = err.stage() {
        println!("发生阶段: {}", stage);
    }
    
    // 获取随机种子
    if let Some(seed) = err.seed() {
        println!("随机种子: {}", seed);
    }
    
    // 获取追踪 ID
    if let Some(trace_id) = err.trace_id() {
        println!("追踪 ID: {}", trace_id);
    }
    
    // 匹配具体错误类型
    match *err {
        PcgError::Config { message, field_path, .. } => {
            println!("配置错误: {}", message);
            if let Some(path) = field_path {
                println!("字段路径: {}", path);
            }
        }
        PcgError::Constraint { message, constraint_type, conflict_details, .. } => {
            println!("约束错误: {}", message);
            if let Some(ctype) = constraint_type {
                println!("约束类型: {}", ctype);
            }
            if let Some(details) = conflict_details {
                println!("冲突详情: {}", details);
            }
        }
        _ => {
            println!("其他错误: {}", err);
        }
    }
}
```

### 便捷方法

为了简化错误创建，`PcgError` 提供了一系列便捷方法：

```rust
use yang_pcg::error::PcgError;

// 创建配置错误
let err = PcgError::config("配置无效");

// 创建配置错误并指定字段路径
let err = PcgError::config_with_field("值超出范围", "room_count.max");

// 创建约束错误
let err = PcgError::constraint("锚点冲突");

// 创建拓扑错误
let err = PcgError::topology("无法生成连通图");

// 创建布局错误
let err = PcgError::layout("房间重叠");

// 创建地形错误
let err = PcgError::terrain("连通性验证失败");

// 创建点位错误
let err = PcgError::spawn("候选点位不足");

// 创建导出错误
let err = PcgError::export("序列化失败");

// 创建预算耗尽错误
let err = PcgError::budget_exhausted("时间预算耗尽", "time");

// 创建迭代超限错误
let err = PcgError::iteration_limit("布局求解失败", 100, 100);

// 创建数据损坏错误
let err = PcgError::corrupted_data("缓存数据损坏");

// 创建能力不可用错误
let err = PcgError::capability_unavailable("运行时分块不可用", "runtime_chunked");
```

## 错误上下文字段

每个错误变体都支持以下上下文字段（根据错误类型不同，可用字段有所差异）：

- `message`: 错误描述（必需）
- `stage`: 生成阶段名称
- `seed`: 随机种子
- `trace_id`: 追踪标识
- `field_path`: 字段路径（配置错误）
- `room_id`: 房间 ID
- `chunk_id`: 分块 ID
- `constraint_type`: 约束类型（约束错误）
- `conflict_details`: 冲突详情（约束错误）
- `collision_details`: 碰撞详情（布局错误）
- `strategy`: 地形策略（地形错误）
- `connectivity_failed`: 连通性检查结果（地形错误）
- `spawn_type`: 点位类型（点位错误）
- `candidate_count`: 候选点位数量（点位错误）
- `target_count`: 目标点位数量（点位错误）
- `format`: 导出格式（导出错误）
- `source_error`: 底层错误信息（导出错误）
- `budget_type`: 预算类型（预算耗尽）
- `consumed`: 已消耗量（预算耗尽）
- `limit`: 预算上限（预算耗尽）
- `current_iteration`: 当前迭代次数（迭代超限）
- `max_iteration`: 最大迭代次数（迭代超限）
- `data_type`: 数据类型（数据损坏）
- `expected_version`: 预期版本（数据损坏）
- `actual_version`: 实际版本（数据损坏）
- `capability`: 能力名称（能力不可用）
- `required_version`: 最低要求版本（能力不可用）
- `current_version`: 当前版本（能力不可用）

## 最佳实践

1. **使用便捷方法**：优先使用 `PcgError::config()` 等便捷方法创建错误，而不是直接构造枚举变体。

2. **提供足够的上下文**：在关键路径上创建错误时，尽可能提供 `stage`、`seed`、`trace_id` 等上下文信息。

3. **使用 `?` 操作符**：利用 Rust 的 `?` 操作符简化错误传播。

4. **记录错误日志**：在捕获错误时，使用 `error_code()`、`stage()`、`seed()` 等方法提取关键信息用于日志记录。

5. **错误恢复**：对于可恢复的错误（如迭代超限），考虑实现重试逻辑。

6. **错误聚合**：在批量操作中，考虑收集所有错误而不是在第一个错误时就停止。

## 性能考虑

为了减小 `Result` 类型的栈大小，`PcgResult<T>` 使用 `Box<PcgError>` 而不是直接使用 `PcgError`。这意味着：

- 错误创建时会有一次堆分配
- 错误传播的开销很小
- 符合 Rust 最佳实践，避免 clippy 警告

如果需要在性能关键路径上避免装箱，可以使用 `*_unboxed` 系列方法：

```rust
// 返回 Box<PcgError>
let err = PcgError::config("错误");

// 返回 PcgError（未装箱）
let err = PcgError::config_unboxed("错误");
```

## 需求映射

本错误处理设计满足以下需求：

- **需求 16.1**: 使用 `Result` 类型返回可能失败的操作
- **需求 16.2**: 区分配置错误、约束不满足、能力不可用、预算耗尽、序列化错误和数据损坏错误
- **需求 16.3**: 提供中文描述信息和稳定的机器可读错误码
- **需求 16.4**: 包含足够的上下文，如阶段名称、随机种子、相关房间或相关字段路径
- **需求 16.5**: 支持附带部分调试上下文
- **需求 16.6**: 实现 `std::error::Error` trait（通过 `thiserror` 自动实现）
