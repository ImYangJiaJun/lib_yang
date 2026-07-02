# 配置管理指南

本文档介绍 `yang-pcg` 的配置管理功能，包括配置归一化、默认值、验证和摘要生成。

## 概述

`yang-pcg` 提供了完整的配置管理系统，支持：

- **默认配置**：开箱即用的合理默认值
- **配置验证**：自动验证配置的有效性
- **配置归一化**：填充缺失值并标准化配置
- **配置合并**：支持多层级配置覆盖
- **配置摘要**：生成稳定的配置哈希用于缓存

## 配置结构

### GenerationConfig

主配置结构包含以下字段：

```rust
pub struct GenerationConfig {
    pub room_count: RangeU16,              // 房间数量范围
    pub critical_path_length: RangeU16,    // 关键路径长度范围
    pub branch_count: RangeU16,            // 分支数量范围
    pub dead_end_count: RangeU16,          // 死路数量范围
    pub room_size: RoomSizeConfig,         // 房间尺寸配置
    pub corridor: CorridorConfig,          // 走廊配置
    pub terrain: TerrainConfig,            // 地形配置
    pub item_spawns: ItemSpawnConfig,      // 交互物生成配置
    pub enemy_spawns: EnemySpawnConfig,    // 敌人生成配置
    pub chunking: ChunkingConfig,          // 分块配置
    pub theme_tags: Vec<String>,           // 主题标签列表
    pub generation_mode: GenerationMode,   // 生成模式
    pub capability_flags: CapabilityFlags, // 能力开关
}
```

## 使用示例

### 1. 使用默认配置

```rust
use yang_pcg::GenerationConfig;

let config = GenerationConfig::default();
```

默认配置提供合理的初始值：
- 房间数量：10-20
- 关键路径长度：5-10
- 分支数量：1-3
- 房间尺寸：8x8 到 16x16

### 2. 创建自定义配置

```rust
use yang_pcg::{GenerationConfig, config::RangeU16};

let mut config = GenerationConfig::default();
config.room_count = RangeU16 { min: 15, max: 25 };
config.theme_tags = vec!["dungeon".to_string()];
config.terrain.obstacle_density = 0.3;
```

### 3. 配置验证与归一化

```rust
use yang_pcg::{GenerationConfig, PcgResult};

fn validate_config(config: &GenerationConfig) -> PcgResult<()> {
    let normalized = config.normalize()?;
    println!("配置验证通过");
    Ok(())
}
```

配置验证会检查：
- 数值范围是否有效（min <= max）
- 最小值是否满足约束（如房间数量 >= 2）
- 百分比值是否在 0.0-1.0 范围内
- 配置之间是否存在冲突

### 4. 配置合并

配置支持层级合并，用于实现：
- 默认配置 -> 预设配置
- 预设配置 -> 实例覆盖
- 实例覆盖 -> 运行时覆盖

```rust
use yang_pcg::{GenerationConfig, config::RangeU16};

let base_config = GenerationConfig::default();
let mut override_config = GenerationConfig::default();
override_config.room_count = RangeU16 { min: 20, max: 30 };

let merged = base_config.override_with(override_config);
```

### 5. 配置摘要

配置摘要用于缓存键生成和回归验证：

```rust
use yang_pcg::{GenerationConfig, ConfigDigest};

let config = GenerationConfig::default();
let digest = ConfigDigest::from_config(&config);
println!("配置摘要: {}", digest);

// 验证摘要是否匹配
assert!(digest.matches(&config));
```

摘要特性：
- 相同配置生成相同摘要
- 不同配置生成不同摘要（高概率）
- 摘要格式稳定，不受 Rust 版本影响

### 6. 配置序列化

配置支持 JSON 序列化和反序列化：

```rust
use yang_pcg::GenerationConfig;

let config = GenerationConfig::default();

// 序列化为 JSON
let json = serde_json::to_string_pretty(&config)?;

// 从 JSON 反序列化
let deserialized: GenerationConfig = serde_json::from_str(&json)?;
```

## 配置字段详解

### 房间尺寸配置 (RoomSizeConfig)

```rust
pub struct RoomSizeConfig {
    pub min_width: u16,   // 最小宽度（网格单位）
    pub max_width: u16,   // 最大宽度（网格单位）
    pub min_height: u16,  // 最小高度（网格单位）
    pub max_height: u16,  // 最大高度（网格单位）
}
```

约束：
- 最小尺寸不能小于 4x4
- min <= max

### 走廊配置 (CorridorConfig)

```rust
pub struct CorridorConfig {
    pub width: u16,                              // 走廊宽度
    pub max_turns: u16,                          // 最大转折次数
    pub connection_strategy: ConnectionStrategy, // 连接策略
}

pub enum ConnectionStrategy {
    Orthogonal,  // 正交连接
    Straight,    // 直线连接
    SharedEdge,  // 共享边开口
}
```

约束：
- 宽度范围：1-10

### 地形配置 (TerrainConfig)

```rust
pub struct TerrainConfig {
    pub obstacle_density: f32,      // 障碍物密度 (0.0-1.0)
    pub min_walkable_ratio: f32,    // 最小可通行面积比例 (0.0-1.0)
}
```

约束：
- 两个值都必须在 0.0-1.0 范围内
- 两者之和不能超过 1.0

### 交互物生成配置 (ItemSpawnConfig)

```rust
pub struct ItemSpawnConfig {
    pub count_per_room: RangeU16,   // 每个房间的交互物数量范围
    pub min_spacing: u16,           // 最小间距（网格单位）
    pub rarity_weights: Vec<f32>,   // 稀有度权重
}
```

约束：
- 最小间距 >= 1
- 稀有度权重总和必须为 1.0

### 敌人生成配置 (EnemySpawnConfig)

```rust
pub struct EnemySpawnConfig {
    pub count_per_room: RangeU16,           // 每个房间的敌人数量范围
    pub min_spacing: u16,                   // 最小间距
    pub min_distance_from_entrance: u16,    // 与入口的最小安全距离
    pub base_difficulty_budget: u16,        // 基础难度预算
}
```

约束：
- 最小间距 >= 1
- 与入口的最小安全距离 >= 2

### 能力开关 (CapabilityFlags)

```rust
pub struct CapabilityFlags {
    pub runtime_chunked: bool,      // 是否启用运行时分块
    pub hybrid_precompute: bool,    // 是否启用混合预计算
    pub grammar_support: bool,      // 是否启用 Grammar 兼容输出
    pub debug_output: bool,         // 是否启用调试输出
}
```

能力开关与生成模式的兼容性：
- `RuntimeChunked` 模式需要 `runtime_chunked = true`
- `HybridPrecompute` 模式需要 `hybrid_precompute = true`

## 错误处理

配置验证失败时会返回详细的错误信息：

```rust
use yang_pcg::{GenerationConfig, config::RangeU16};

let mut config = GenerationConfig::default();
config.room_count = RangeU16 { min: 30, max: 10 }; // 非法范围

match config.normalize() {
    Ok(_) => println!("配置有效"),
    Err(err) => {
        println!("错误码: {}", err.error_code());
        println!("错误信息: {}", err);
    }
}
```

错误信息包含：
- 错误码（如 `PCG-CONFIG-001`）
- 中文描述
- 字段路径（如 `room_count.min`）
- 上下文信息

## 最佳实践

1. **使用默认配置作为基础**：从 `GenerationConfig::default()` 开始，只覆盖需要修改的字段

2. **验证配置**：在使用配置前调用 `normalize()` 进行验证

3. **使用配置摘要**：为缓存和回归测试生成稳定的配置标识

4. **分层配置**：使用 `merge()` 实现配置的层级覆盖

5. **序列化配置**：将配置保存为 JSON 文件，便于版本控制和共享

## 参考

- [示例代码](../examples/config_normalization.rs) - 完整示例
- [AGENTS.md](../AGENTS.md) - 架构约定与已知状态
- [PRODUCTION_AUDIT_2026-06-24.md](PRODUCTION_AUDIT_2026-06-24.md) - 生产就绪度审计
