# 设计文档：yang-pcg UE5 Roguelike 地图生成算法库

## 概述

本文档定义 `yang-pcg` 的技术设计。`yang-pcg` 是一个纯 Rust 的程序化地图生成核心，用于为 UE5 Roguelike 项目生成可复现、可测试、可缓存、可导出、可分块加载的地图数据，并通过独立适配层与 UE5 PCG 工作流对接。

本设计与 [requirements.md](D:/code/lib_yang/.kiro/specs/ue5-roguelike-map-generator/requirements.md) 保持一一对应，重点解决以下问题：

1. 把“地图算法核心”与“UE5 集成接口”彻底解耦。
2. 把抽象的房间路径落地成房间边界、门锚点、走廊和玩法点位。
3. 让生成结果天然适合导出到 UE5 PCG 的 Point、Metadata、Spline 和 Named Channel。
4. 为运行时分块、World Partition、离线缓存、调试回放预留稳定边界。

## 修订结论

在整理现有规格目录后，有三个核心结论：

1. `requirements.md` 的方向已经接近可实施版本，但此前的 `design.md` 混入了大量“2D 平台跳跃”专用逻辑，和当前需求文档的通用 Roguelike 地图库定位不一致。
2. `tasks.md` 存在编码损坏、阶段边界模糊、与需求映射不完整的问题，不能直接作为迭代执行清单。
3. 当前最合适的策略不是继续在旧文档上追加细节，而是围绕现有需求重建一版更清晰的设计和实现任务。

因此，本设计采取以下裁剪原则：

1. 当前 MVP 默认面向“通用/Top-Down Roguelike”地图生成，不把 2D 平台跳跃的跳高、坠落、墙跳、轨迹避让纳入核心不变量。
2. 如果未来项目确认是 SideScroller Roguelike，可以在本设计之上追加 `SideScrollerProfile` 扩展，而不是把该假设写死到所有模块。
3. UE5 集成仅定义数据契约和能力边界，不在 Rust 核心中直接引入 Unreal Runtime 类型。

## 设计目标

1. **确定性**：相同 `seed + config + algorithm_version` 生成语义一致结果。
2. **分层性**：算法核心不依赖 UE5；UE 适配层不侵入核心算法。
3. **可观测性**：生成过程可调试、可追踪、可回放。
4. **可扩展性**：支持约束、模板引用、Grammar、运行时分块等增强能力。
5. **工程可行性**：模块边界清晰，便于单元测试、属性测试和分阶段实现。

## 非目标

1. 不直接在 Rust 核心中实例化 UE5 `AActor`、`UObject`、`UPCGComponent`。
2. 不负责最终 Static Mesh、Material、Niagara、NavMesh、AI 行为树等运行时内容。
3. 不在 MVP 中实现 2D 平台跳跃专用可达性模型。
4. 不在 MVP 中实现完整关卡叙事、任务链和战斗编排系统。

## 外部约束与参考

本设计默认对齐 UE5.5+ 的 PCG 能力边界，并参考以下官方资料：

1. [Procedural Content Generation Overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/procedural-content-generation-overview?application_version=5.6)
2. [PCG Development Guides](https://dev.epicgames.com/documentation/en-us/unreal-engine/pcg-development-guides)
3. [Using PCG Generation Modes](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-pcg-generation-modes)
4. [Unreal Engine 5.5 Release Notes](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-5-release-notes)
5. [Unreal Engine 5.6 Release Notes](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-6-release-notes)

基于这些资料，本设计采用以下现实假设：

1. UE5 PCG 下游更适合消费 Point、Spline、Metadata 和参数覆盖，而不是直接消费任意自定义 Rust 结构。
2. Runtime Generation、Hierarchical Generation、World Partition、Data Layer、Shape Grammar 是“适配层契约问题”，不是“核心算法耦合问题”。
3. UE 版本差异应通过能力开关处理，不能把高版本特性写死在核心结构里。

## 总体架构

### 分层

```text
┌────────────────────────────────────────────────────────────┐
│ UE5 消费层                                                │
│ PCG Graph / Blueprint / Editor Utility / Data Assets      │
└────────────────────────────────────────────────────────────┘
                           ↑
                    UE 兼容数据契约
                           ↑
┌────────────────────────────────────────────────────────────┐
│ 适配层                                                    │
│ ue::adapter / ue::export / ue::streaming                  │
└────────────────────────────────────────────────────────────┘
                           ↑
                   GenerationResult
                           ↑
┌────────────────────────────────────────────────────────────┐
│ 核心生成层                                                │
│ generator / topology / layout / terrain / spawn           │
│ constraint / validation / debug                           │
└────────────────────────────────────────────────────────────┘
                           ↑
                    Config / Request
                           ↑
┌────────────────────────────────────────────────────────────┐
│ 基础设施层                                                │
│ rng / math / grid / graph / serde / error / cache         │
└────────────────────────────────────────────────────────────┘
```

### 设计决策

1. **先拓扑，后几何，最后内容点位**：避免在早期就把地形、资源点和房间类型耦合在一起。
2. **门锚点是几何连通的权威来源**：拓扑边最终必须落到一对 `DoorAnchor` 上。
3. **逻辑网格和世界坐标分离**：核心统一在逻辑空间生成，UE 适配层再映射到世界空间。
4. **结果按通道输出**：房间、走廊、点位、调试数据都走具名通道，不混装。
5. **调试信息是旁路，不参与玩法结果**：开启调试不应改变地图内容。
6. **约束按阶段逐步收敛**：先校验可满足性，再在拓扑、布局、点位阶段分别施加约束。

## 建议的 crate 结构

```text
crates/yang-pcg/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── error.rs
    ├── config.rs
    ├── generator.rs
    ├── rng.rs
    ├── digest.rs
    ├── validation.rs
    ├── topology/
    │   ├── mod.rs
    │   ├── graph.rs
    │   ├── planner.rs
    │   └── room_types.rs
    ├── layout/
    │   ├── mod.rs
    │   ├── solver.rs
    │   ├── doors.rs
    │   └── corridors.rs
    ├── terrain/
    │   ├── mod.rs
    │   ├── grid.rs
    │   ├── carve.rs
    │   └── connectivity.rs
    ├── spawn/
    │   ├── mod.rs
    │   ├── items.rs
    │   ├── enemies.rs
    │   ├── sampling.rs
    │   └── budget.rs
    ├── constraint/
    │   ├── mod.rs
    │   ├── anchors.rs
    │   ├── exclusion.rs
    │   └── templates.rs
    ├── model/
    │   ├── mod.rs
    │   ├── geometry.rs
    │   ├── request.rs
    │   ├── result.rs
    │   ├── room.rs
    │   ├── terrain.rs
    │   ├── spawn.rs
    │   └── chunk.rs
    ├── ue/
    │   ├── mod.rs
    │   ├── adapter.rs
    │   ├── points.rs
    │   ├── channels.rs
    │   ├── params.rs
    │   └── streaming.rs
    ├── debug/
    │   ├── mod.rs
    │   ├── report.rs
    │   └── profiling.rs
    └── cache/
        ├── mod.rs
        ├── key.rs
        └── store.rs
```

## Rust 命名约定

需求文档沿用业务术语 `Map_Generator`、`Room_Type` 等表达；实现设计中，Rust 类型统一采用 idiomatic 命名：

1. `Map_Generator` -> `MapGenerator`
2. `Generation_Request` -> `GenerationRequest`
3. `Generation_Result` -> `GenerationResult`
4. `Room_Type` -> `RoomType`
5. `Door_Anchor` -> `DoorAnchor`

这样可以让需求术语和代码风格各自保持清晰。

## 核心流程设计

### 流程总览

```text
GenerationRequest
  -> validate_config
  -> normalize_config
  -> derive_rng_streams
  -> solve_constraints_precheck
  -> generate_topology
  -> solve_room_layout
  -> build_door_anchors
  -> carve_corridors
  -> generate_room_terrain
  -> generate_item_spawns
  -> generate_enemy_spawns
  -> validate_invariants
  -> assemble_named_channels
  -> build_debug_report
  -> GenerationResult
```

### 阶段说明

#### 1. 配置归一化阶段

输入：

1. `GenerationRequest`
2. `GenerationConfig`
3. 可选的运行时上下文、锚点、排除区

输出：

1. 归一化后的 `NormalizedConfig`
2. `ConfigDigest`
3. 能力开关集 `CapabilityFlags`

职责：

1. 填充默认值
2. 验证数值范围
3. 识别互斥配置
4. 将高层配置展开为子模块可直接消费的配置

#### 2. 拓扑生成阶段

输出：

1. `RoomGraph`
2. `CriticalPath`
3. `Branches`
4. 每个房间的 `RoomType`
5. 难度曲线与路径深度信息

设计重点：

1. 先满足可达性，再满足节奏和房型分布。
2. 关键路径是后续难度分配、Boss 房和奖励分支的参考轴。
3. 分支要保留“目的标签”，便于后续生成商店、奖励房或事件房。

#### 3. 空间布局阶段

输出：

1. 房间边界 `RoomBounds`
2. 门锚点 `DoorAnchor`
3. 走廊线 `CorridorPath`

设计重点：

1. 拓扑边必须映射为可落地的几何连接。
2. 房间之间允许配置“共享边开口”或“独立走廊”。
3. 门锚点是后续地形 carving、内容避让和 UE 样条导出的关键基准。

#### 4. 地形生成阶段

输出：

1. 每个房间的 `Terrain`
2. 地板、墙体、障碍物、保留区、门口通行区

设计重点：

1. 地形策略由房型、主题和模板共同决定。
2. 门口必须先落到网格上，再执行障碍物填充。
3. 生成完成后必须校验连通性。

#### 5. 点位生成阶段

输出：

1. 交互物点位
2. 敌人点位
3. 敌人预算和遭遇元数据

设计重点：

1. 点位候选应来自“可通行且不冲突”的逻辑瓦片。
2. 交互物和敌人要共享空间规则，但使用不同预算与过滤策略。
3. 点位生成先产生候选，再筛选、评分、裁剪。

#### 6. 结果组装阶段

输出：

1. `GenerationResult`
2. `NamedChannelSet`
3. 调试输出
4. 约束验证报告

设计重点：

1. 结果对象是缓存、导出、测试和 UE 适配的单一真相源。
2. 所有结果必须带上 `seed`、`config_digest`、`algorithm_version`。
3. 调试通道必须和玩法通道分离。

## 数据模型设计

下列结构不是最终代码，而是用于约束边界和字段含义。

### 生成请求与配置

```rust
pub struct GenerationRequest {
    pub seed: Option<u64>,
    pub config: GenerationConfig,
    pub constraints: Vec<Constraint>,
    pub runtime_context: Option<RuntimeContext>,
    pub trace_id: Option<String>,
}

pub struct GenerationConfig {
    pub room_count: RangeU16,
    pub critical_path_length: RangeU16,
    pub branch_count: RangeU16,
    pub room_size: RoomSizeRange,
    pub corridor: CorridorConfig,
    pub terrain: TerrainConfig,
    pub item_spawns: ItemSpawnConfig,
    pub enemy_spawns: EnemySpawnConfig,
    pub chunking: ChunkingConfig,
    pub theme_tags: Vec<String>,
    pub generation_mode: GenerationMode,
    pub capability_flags: CapabilityFlags,
}

pub enum GenerationMode {
    OfflineFullFloor,
    RuntimeChunked,
    HybridPrecompute,
}

pub struct RuntimeContext {
    pub focus_position: Option<WorldPoint>,
    pub interest_radius: Option<f32>,
    pub requested_chunks: Vec<ChunkId>,
    pub caller_tag: Option<String>,
}
```

### 房间与拓扑

```rust
pub struct Room {
    pub id: RoomId,
    pub room_type: RoomType,
    pub depth_from_start: u16,
    pub branch_id: Option<BranchId>,
    pub difficulty: u16,
    pub theme_tags: Vec<String>,
    pub bounds: Option<RoomBounds>,
    pub template_ref: Option<TemplateRef>,
}

pub enum RoomType {
    Start,
    Combat,
    Treasure,
    Shop,
    Elite,
    Puzzle,
    Safe,
    Boss,
    Event,
    Secret,
}

pub struct RoomGraph {
    pub nodes: Vec<Room>,
    pub edges: Vec<RoomEdge>,
    pub critical_path: Vec<RoomId>,
    pub branches: Vec<Branch>,
}
```

### 门锚点与走廊

```rust
pub struct DoorAnchor {
    pub id: DoorAnchorId,
    pub room_id: RoomId,
    pub edge_id: RoomEdgeId,
    pub grid_pos: GridPoint,
    pub facing: CardinalDir,
    pub width_tiles: u16,
}

pub struct Corridor {
    pub id: CorridorId,
    pub from_room: RoomId,
    pub to_room: RoomId,
    pub from_anchor: DoorAnchorId,
    pub to_anchor: DoorAnchorId,
    pub width_tiles: u16,
    pub path: CorridorPath,
}

pub enum CorridorPath {
    Straight(Vec<GridPoint>),
    Orthogonal(Vec<GridPoint>),
    Polyline(Vec<GridPoint>),
}
```

### 地形

```rust
pub struct Terrain {
    pub room_id: RoomId,
    pub grid_size: GridSize,
    pub tiles: Grid2D<TileKind>,
    pub reserved_zones: Vec<ReservedZone>,
    pub connectivity_summary: ConnectivitySummary,
}

pub enum TileKind {
    Empty,
    Floor,
    Wall,
    Obstacle,
    Reserved,
    Doorway,
}
```

### 点位

```rust
pub struct SpawnPoint {
    pub id: SpawnPointId,
    pub room_id: RoomId,
    pub kind: SpawnKind,
    pub grid_pos: GridPoint,
    pub world_transform: Option<Transform3>,
    pub metadata: SpawnMetadata,
}

pub enum SpawnKind {
    Item,
    Enemy,
    Boss,
    Reward,
    Interaction,
}

pub struct SpawnMetadata {
    pub spawn_tag: String,
    pub rarity_tier: Option<u8>,
    pub enemy_pool_tag: Option<String>,
    pub encounter_id: Option<String>,
    pub wave_id: Option<String>,
    pub difficulty: Option<u16>,
    pub seed: u64,
}
```

### UE 适配输出

```rust
pub struct PcgPoint {
    pub transform: Transform3,
    pub bounds: Bounds3,
    pub density: f32,
    pub seed: u64,
    pub attributes: BTreeMap<String, PropertyValue>,
}

pub struct NamedChannel {
    pub name: String,
    pub kind: ChannelKind,
    pub points: Vec<PcgPoint>,
    pub splines: Vec<Polyline3>,
    pub metadata: BTreeMap<String, PropertyValue>,
}

pub enum ChannelKind {
    Rooms,
    Doors,
    Corridors,
    FloorTiles,
    WallTiles,
    ItemSpawns,
    EnemySpawns,
    Debug,
}
```

### 结果对象

```rust
pub struct GenerationResult {
    pub metadata: ResultMetadata,
    pub topology: RoomGraph,
    pub rooms: Vec<Room>,
    pub corridors: Vec<Corridor>,
    pub terrains: Vec<Terrain>,
    pub item_spawns: Vec<SpawnPoint>,
    pub enemy_spawns: Vec<SpawnPoint>,
    pub chunks: Vec<Chunk>,
    pub channels: Vec<NamedChannel>,
    pub debug: Option<DebugBundle>,
}

pub struct ResultMetadata {
    pub seed: u64,
    pub config_digest: String,
    pub schema_version: String,
    pub algorithm_version: String,
    pub target_engine_version: Option<String>,
    pub trace_id: Option<String>,
}
```

## 模块设计

### 1. `config`

职责：

1. 定义所有可序列化配置结构。
2. 提供默认值、归一化和配置合并。
3. 提供能力开关和版本兼容映射。

关键接口：

```rust
impl GenerationConfig {
    pub fn normalize(&self) -> PcgResult<NormalizedConfig>;
    pub fn digest(&self) -> String;
}
```

### 2. `rng`

职责：

1. 提供稳定的、可派生子流的随机接口。
2. 屏蔽底层 PRNG 实现细节。

设计说明：

1. 不建议手写随机算法；优先基于成熟库实现稳定封装。
2. 对外只暴露“派生子流”和“稳定取样”语义。

建议：

1. 使用 `rand_pcg` 或等价确定性 RNG。
2. 提供 `derive_stream("topology")`、`derive_stream("terrain:room-12")` 这样的派生方式。

### 3. `topology`

职责：

1. 生成房间图。
2. 规划关键路径和分支。
3. 分配房型和难度。

建议算法：

1. 先生成候选房间节点。
2. 再构造连接图。
3. 用可达性约束修正图结构。
4. 再选关键路径、分支和房型。

实现注意：

1. 不要在这一层引入具体几何位置。
2. `Boss`、`Start`、`Safe`、`Treasure` 等房型分配应参考路径角色，而不是随机均匀散布。

### 4. `layout`

职责：

1. 将拓扑图映射到平面空间。
2. 解出房间边界。
3. 生成门锚点和走廊线。

建议策略：

1. 初始布局可以基于关键路径优先的启发式定位。
2. 碰撞修正采用 AABB 检测 + 迭代分离。
3. 走廊采用“正交折线优先、样条表现后置”的策略。

为什么不直接做样条：

1. 核心逻辑更关心连通性和占用关系。
2. UE5 表现层完全可以把折线再转换成样条或 Grammar 输入。

### 5. `terrain`

职责：

1. 为每个房间生成逻辑网格。
2. 预留门口、障碍物、保留区和中心战斗区。
3. 验证可通行性。

策略：

1. 地形策略由 `RoomType + theme_tags + template_ref` 决定。
2. Boss 房优先采用开放式策略。
3. Treasure/Shop/Safe 房应有更强的保留区约束。

### 6. `spawn`

职责：

1. 生成交互物点位。
2. 生成敌人点位和预算。
3. 验证与地形、走廊、门口的空间冲突。

建议流程：

1. 从地形中抽取候选瓦片。
2. 应用不可放置规则。
3. 做间距采样。
4. 基于权重和预算做最终筛选。

### 7. `constraint`

职责：

1. 处理锚点、排除区、保留房间和模板引用。
2. 在生成前做冲突预检查。
3. 在各阶段执行局部约束。

关键思想：

1. 约束不是一次性求解，而是“预检查 + 分阶段注入 + 末尾验证”。
2. 当约束不可满足时，应尽早失败并指出冲突字段。

### 8. `validation`

职责：

1. 在阶段结束后执行不变量检查。
2. 生成结构化验证报告。

建议覆盖：

1. 拓扑可达性
2. 房间几何不重叠
3. 地形连通性
4. 点位间距和安全区
5. 约束满足情况

### 9. `ue`

职责：

1. 把 `GenerationResult` 映射成 UE5 友好数据。
2. 输出 Point、Spline 和通道元数据。
3. 构建 Graph Parameters 映射、Chunk 元数据和流式层信息。

不做什么：

1. 不直接负责 UE 侧 Actor 生成。
2. 不负责编辑器脚本。
3. 不要求核心库依赖 Unreal SDK。

### 10. `debug`

职责：

1. 输出阶段耗时、候选/拒绝原因、关键路径和约束报告。
2. 支持开发期诊断和回归测试。

规则：

1. 调试开关只影响旁路数据，不影响玩法通道。
2. 调试对象要可序列化，方便存档回放。

### 11. `cache`

职责：

1. 基于 `seed + config_digest + algorithm_version` 构建缓存键。
2. 支持完整结果缓存和分块结果缓存。

缓存粒度：

1. Full floor result
2. Chunk result
3. 导出中间件结果

## 关键算法选择

### 确定性随机流

设计：

1. 根随机流来自 `seed`。
2. 每个阶段通过稳定标签派生子流。
3. 房间级、走廊级和点位级逻辑继续派生细粒度子流。

好处：

1. 调试时可以固定只重放某一阶段。
2. 新增调试逻辑不会意外改变玩法结果。

### 拓扑生成

建议采用启发式图生成，而不是一开始引入复杂 SAT/CP 求解：

1. 先采样目标房间数量。
2. 生成连通图骨架。
3. 用关键路径和分支规则修正结构。
4. 再做房型和难度分配。

原因：

1. 实现成本更低。
2. 更适合逐步加约束。
3. 便于后续替换具体策略而不影响外部接口。

### 空间布局

建议采用“启发式初始布局 + 迭代碰撞修正 + 正交走廊”：

1. 关键路径优先沿主轴排布。
2. 分支沿局部区域展开。
3. 通过迭代修正避免重叠。
4. 再基于几何关系推门口和走廊。

原因：

1. 对 Roguelike 房间图足够实用。
2. 结果更容易映射到模块化关卡或 PCG Spline。

### 地形生成

建议保留多种策略，但把接口统一为：

```rust
pub trait TerrainStrategy {
    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut dyn StableRng,
    ) -> PcgResult<Terrain>;
}
```

首批策略：

1. `OpenArenaStrategy`
2. `PillarStrategy`
3. `MazeStrategy`
4. `OrganicStrategy`

### 点位生成

建议使用“候选点提取 + 距离采样 + 评分筛选”三段式：

1. 先确定可放置区域。
2. 再按最小间距做采样。
3. 最后按房型、难度和预算做裁剪。

这样可以让道具与敌人共用候选区计算，但保留各自策略。

## UE5 集成设计

### Graph Parameters 映射

`GenerationConfig` 中可被图实例覆盖的字段，应映射为稳定参数键。例如：

1. `room_count.min`
2. `room_count.max`
3. `critical_path_length.min`
4. `critical_path_length.max`
5. `theme.primary`
6. `generation_mode`

原则：

1. 参数键稳定且可文档化。
2. 同一字段不要在多个地方使用不同命名。
3. 布尔能力开关必须显式命名。

### PCG Point 映射

约定：

1. 房间输出为中心点或边界采样点，并携带房型、尺寸、难度等元数据。
2. 门锚点输出为单独 `doors` 通道。
3. 走廊优先输出为 polyline/spline，必要时附加采样点。
4. 交互物与敌人分别输出为独立点通道。

### Runtime Chunking

分块模式下：

1. 拓扑可整层预计算。
2. 房间细节和点位可按 Chunk 增量补全。
3. `Chunk` 必须稳定标识，便于缓存和 World Partition 对接。

### World Partition / Data Layer

适配层应支持：

1. `chunk_id`
2. `data_layer`
3. `external_data_layer`
4. `hlod_layer`
5. `streaming_priority`

但这些字段是附加元数据，不应进入核心算法判定。

### Grammar 兼容

Grammar 兼容层不是单独生成器，而是结果导出策略：

1. 房间可以导出 `grammar_token`
2. 门口可以导出 `socket_tag`
3. 走廊可以导出长度段、转折类型和主题标签

这样既能兼容模块拼装，也不会把核心库绑死在某个 UE 工具链上。

## 错误模型

建议错误层次：

```rust
pub enum PcgError {
    Config(ConfigError),
    Constraint(ConstraintError),
    Topology(TopologyError),
    Layout(LayoutError),
    Terrain(TerrainError),
    Spawn(SpawnError),
    Export(ExportError),
    BudgetExhausted(BudgetContext),
    IterationLimit(IterationContext),
    CorruptedData(CorruptedDataContext),
}
```

错误要求：

1. 面向用户的描述使用中文。
2. 结构化字段保留阶段、字段路径、房间 ID、Chunk ID、种子等上下文。
3. 导出和缓存错误要和生成错误区分。

## 缓存与序列化

### 缓存键

建议：

```text
cache_key = hash(schema_version, algorithm_version, seed, config_digest, request_scope)
```

其中 `request_scope` 取值：

1. `full-floor`
2. `chunk:<id>`
3. `export:<format>`

### 导出格式

建议至少支持：

1. JSON：便于调试、差异比较、工具链消费。
2. 二进制：便于性能和缓存。

导出必须包含：

1. `schema_version`
2. `algorithm_version`
3. `seed`
4. `config_digest`
5. `channels`
6. `chunks`
7. `trace_id`

## 正确性与测试设计

### 必须成立的不变量

1. 同输入下输出稳定。
2. 所有房间从起点可达。
3. 房间边界不会出现未声明重叠。
4. 所有必达门口之间存在地形通路。
5. 点位满足最小间距。
6. 点位不会落在禁布区和禁止阻塞区。
7. Boss 房只落在允许位置。

### 测试层次

1. **单元测试**：模块内部纯逻辑。
2. **属性测试**：不变量验证。
3. **集成测试**：完整生成流程。
4. **基准测试**：不同规模配置的性能回归。
5. **黄金样本测试**：固定种子固定输出。

### 基准建议

1. `small`：10 房间级别
2. `medium`：20 房间级别
3. `large`：40 房间级别

基准记录：

1. 总耗时
2. 分阶段耗时
3. 房间数
4. 走廊数
5. Chunk 数
6. 峰值分配

## 可观察性设计

### 调试通道

建议至少输出：

1. `debug/room_centers`
2. `debug/critical_path`
3. `debug/branches`
4. `debug/door_anchors`
5. `debug/corridor_centerlines`
6. `debug/rejected_rooms`
7. `debug/rejected_spawn_points`
8. `debug/constraint_report`

### 阶段统计

建议记录：

1. 开始时间
2. 结束时间
3. 迭代次数
4. 候选数
5. 拒绝数
6. 重试次数

## 实施建议

### MVP 范围

优先实现：

1. `OfflineFullFloor`
2. 通用 Roguelike 拓扑
3. 房间边界、门锚点、正交走廊
4. 房间地形连通性
5. 交互物和敌人点位
6. JSON 导出
7. UE Point/Channel 基础适配

### 延后实现

可以在 MVP 之后补充：

1. `RuntimeChunked`
2. `HybridPrecompute`
3. Grammar 扩展输出
4. 二进制缓存
5. World Partition 深度集成
6. SideScroller 专用 profile

## 与需求文档的映射

1. 系统分层与接口：本设计的“总体架构”“crate 结构”“UE 适配层”
2. 随机种子与确定性：本设计的 `rng`、缓存键、稳定子流
3. 楼层拓扑：本设计的 `topology`
4. 房间边界、门锚点、走廊：本设计的 `layout`
5. 地形：本设计的 `terrain`
6. 手工约束：本设计的 `constraint`
7. 交互物点位：本设计的 `spawn`
8. 敌人点位与预算：本设计的 `spawn`、`budget`
9. UE5 PCG 数据契约：本设计的 `ue`
10. 配置管理与图参数映射：本设计的 `config`、`ue::params`
11. 运行时生成模式与分块：本设计的 `chunk`、`ue::streaming`
12. World Partition 与流式元数据：本设计的 `Chunk` 和流式元数据
13. Grammar 兼容：本设计的 Grammar 导出策略
14. 数据导出、缓存与重建：本设计的 `cache`、序列化
15. 调试与分析输出：本设计的 `debug`
16. 错误处理：本设计的错误模型
17. 性能与并发：本设计的基准和缓存策略
18. 测试支持：本设计的测试层次和不变量
