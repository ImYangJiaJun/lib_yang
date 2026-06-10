# yang-pcg 双模式扩展设计：俯视角网格 + 横版平台跳跃

> 状态：设计待评审
> 日期：2026-06-10
> 关联：`crates/yang-pcg/docs/PRODUCTION_AUDIT_2026-06.md`（全量审核报告）
> 目标：把 yang-pcg 从单一「俯视角网格地图生成器」扩展为同时支持「俯视角网格地图（TopDownGrid）」与「横版平台跳跃（SidePlatformer）」两种地图种类的基础库。

---

## 0. 背景与动机

全量审核结论：当前库的几何模型（`GridPoint` 只有 x/y、无高度）、连通性（4 邻接 flood-fill）、瓦片语义（无单向平台/梯子）、房间拼接（上下行间距恒大于跳跃高度、垂直不可达）全部建立在俯视角假设上，对横版平台跳跃基本不适用。可复用的是 `RoomGraph` 拓扑骨架与确定性 RNG 编排。

本设计在**不破坏现有俯视角行为**的前提下，引入横版平台跳跃模式，并顺带修复审核中与「重写区域天然交叉」的若干问题。

### 已确认的关键决策（来自 brainstorming）

1. **目标定位**：通用基础库——先把核心几何/可达性模型做对，具体玩法子类型（线性/箱庭/Roguelite）后续再定。YAGNI：不为想象中的玩法过度设计。
2. **共享边界**：两模式共享 `RoomGraph` 拓扑 + `StableRng` 编排 + 错误/配置/导出框架；几何/布局/地形/可达性/校验按模式分层。
3. **可达性模型**：**简化运动学**——配置提供 `max_jump_height`/`max_jump_distance` 等标量，生成时保证必经平台间隙在跳跃能力内，校验用「重力下落 + 最大跳跃弧」做可达性分析。不做完整能力建模（二段跳/冲刺/爬墙）。
4. **API 形态**：单入口 `MapGenerator::generate` + 配置内 `MapKind` 枚举分派。现有俯视角调用零改动。
5. **mode 维度**：新开 `MapKind { TopDownGrid, SidePlatformer }`，与现有 `GenerationMode`（生成时机/分块，正交维度）分离。
6. **架构方案**：阶段级 `PipelineBackend` trait 抽象 + mode 分派（运行时 `Box<dyn>`）。
7. **范围切分**：本 spec 聚焦双模式架构，**只纳入与重写区域天然交叉的审核修复**（校验进生产路径、地形层吞错、几何高度维）。与模式无关的修复（DefaultHasher 跨版本、CacheKey 漏 constraints、API builder、死代码清理）另立清单后续单独做。

### 非目标（明确排除）

- 不做完整运动学/能力门控（二段跳、冲刺、爬墙能力图）。
- 不实现审核中与模式无关的修复（见上方范围切分）。
- 不拆分 crate；保持单 crate、单入口。
- 不改 RNG 派生标签链（守确定性契约与黄金测试）。

---

## 1. 整体架构与分层

核心思路：**编排不变，阶段分叉**。`generate()` 仍是 mode 无关的串联骨架，分叉发生在拓扑之后，由 `PipelineBackend` trait 多态承担。

```
                  MapGenerator::generate(request)
                            │
              validate_request + normalize（MapKind 感知）
                            │
              topology::generate_topology  ◄── 共享，产出 RoomGraph
                            │
              backend = select_backend(config.map_kind)
                            │
        ┌───────────────────┴───────────────────┐
   TopDownBackend                        SidePlatformerBackend
   （现有代码收拢）                        （新建）
   ├ layout（行列铺排）                    ├ layout（侧视：重力轴+平台高度）
   ├ terrain（俯视雕刻）                   ├ terrain（平台/间隙/梯子）
   ├ spawn（任意 floor）                   ├ spawn（平台顶面）
   └ validate（重叠/连通）                 └ validate（跳跃可达/落脚）
                            │
              组装 GenerationResult + backend.validate（进生产路径）
```

### 分层归属

| 层 | 归属 | 说明 |
|----|------|------|
| `topology/`、`rng`、`error`、`config` 框架、`export` | 共享 | RoomGraph 拓扑骨架、确定性编排，两模式不动 |
| `model/geometry`（升级）、`Grid2D<TileKind>` 容器 | 共享基元 | 几何基元含高度语义；瓦片容器统一，枚举扩展 |
| `trait PipelineBackend` | 新增契约 | 定义 layout/terrain/spawn/validate 四个阶段方法 |
| `backend/topdown/`（现有代码迁入）| 俯视角实现 | 行为与现状一致，黄金测试不变 |
| `backend/platformer/`（新建）| 平台跳跃实现 | 侧视几何、跳跃可达性 |

### 关键原则

- `generate()` 编排代码**只有一份**，靠 `Box<dyn PipelineBackend>` 运行时分派。
- 现有俯视角逻辑**整体迁入** `backend/topdown/`，对外行为零变化（现有 293 测试 + 黄金测试 seed 42/12345 必须继续全绿）。
- 新增模式 = 加一个 backend，不碰编排。
- `LayoutOutput`/`Terrain`/`SpawnOutput`/`GenerationResult` 等**数据类型共享**，trait 只分叉算法不分叉数据容器——UE 导出、序列化、结果组装均 mode 无关，差异已被 backend 吸收进数据内容。

---

## 2. 数据模型

「共享」与「分模式」的数据结构边界。

### 2.1 几何基元（共享，扩展语义）

`model/geometry.rs` 现状是纯 2D 俯视角。改造原则：**俯视角语义零变化，platformer 加纵向能力，二者不互相污染**。

```rust
// 共享：GridPoint 保持 {x, y}，含义按 MapKind 解释
//   - TopDownGrid：x/y = 平面横纵坐标
//   - SidePlatformer：x = 水平推进，y = 高度（重力轴，y 越大越高）
pub struct GridPoint { pub x: i32, pub y: i32 }   // 字段不变，避免破坏现有代码

// WorldPoint.z 在 platformer 下不再恒为 0：由 grid.y 映射出真实高度
//（修审核 §3.1：z 恒 0 在 platformer backend 解决；topdown 仍 z=0）
```

`GridPoint` 故意**不加第三维**——平台跳跃是 2D 横版（x 水平、y 垂直），二维足够。区别只在 y 的**语义**：俯视角是平面纵坐标，platformer 是重力高度轴。现有俯视角代码一行不改。

### 2.2 瓦片语义（共享容器，枚举扩展）

`TileKind` 扩展平台跳跃必需类型；俯视角不使用新增项：

```rust
pub enum TileKind {
    // —— 现有，两模式共用 ——
    Empty, Floor, Wall, Obstacle, Reserved, Doorway,
    // —— 新增，platformer 专用 ——
    Solid,           // 实心地块（可站立其上，剖面语义）
    OneWayPlatform,  // 单向平台（可从下穿过、可站立顶面）
    Ladder,          // 梯子（垂直通行）
    Hazard,          // 危险区（尖刺/岩浆等，致死或阻挡）
}
```

`Grid2D<TileKind>` 容器不变，两模式共用同一存储。`is_walkable` 判定分模式：俯视角 = Floor/Doorway/Reserved；platformer = 脚下为 Solid/OneWayPlatform 之上的格 + Ladder。

> 注意 `large_enum_variant` lint：新增变体均为无负载单元变体，不增大 enum 尺寸。

### 2.3 拓扑承载纵向语义（共享结构，platformer 填充新字段）

`RoomGraph`/`Room`/`RoomEdge` 骨架共享。审核 §3.4 指出 `RoomEdge` 无向、无法表达「需跳上去/是坠落口」。方案：加**可选**遍历语义字段，俯视角留 `None`：

```rust
pub struct RoomEdge {
    pub id, pub from_room, pub to_room, pub is_critical,  // 现有
    pub traversal: Option<TraversalKind>,   // 新增；TopDownGrid = None
}
pub enum TraversalKind {
    Horizontal,                 // 水平走廊/通道
    JumpUp { height: u16 },     // 需向上跳，记所需高度（用于校验 ≤ max_jump_height）
    DropDown,                   // 单向坠落
}
```

> `traversal` 为 `Option` 保证序列化向后兼容、俯视角行为不变。

### 2.4 配置（共享结构，加 MapKind 与 platformer 子配置）

```rust
pub struct GenerationConfig {
    // ... 现有字段不变 ...
    pub map_kind: MapKind,               // 新增，默认 TopDownGrid（向后兼容）
    pub platformer: PlatformerConfig,    // 新增，仅 SidePlatformer 读取
}

/// 地图种类，与 GenerationMode（生成时机/分块）正交。
pub enum MapKind { TopDownGrid, SidePlatformer }

pub struct PlatformerConfig {
    pub max_jump_height: u16,       // 简化运动学核心：最大跳跃高度（格）
    pub max_jump_distance: u16,     // 最大水平跳跃距离（格）
    pub min_platform_width: u16,    // 平台最小宽度，保证可落脚
    // gravity 方向固定为 y 向上，暂不参数化（YAGNI）
}
```

`map_kind` 默认 `TopDownGrid`，使现有 config 反序列化与行为完全不变。`normalize()` 按 `map_kind` 分别校验：platformer 下校验 `max_jump_*`/`min_platform_width` 合理且互洽（如平台间隙不会超出跳跃能力的物理可行域）。

---

## 3. PipelineBackend trait 契约

编排在拓扑之后选 backend，把后续阶段全部委托给它。trait 统一抽象两模式的 layout/terrain/spawn/validate，又不把俯视角假设泄进契约。

```rust
/// 拓扑之后的全部空间语义，按 MapKind 多态实现。
pub trait PipelineBackend {
    /// 布局：RoomGraph → 房间边界/门锚/连接。
    fn solve_layout(&self, graph: &RoomGraph, cfg: &NormalizedConfig, rng: &mut StableRng)
        -> PcgResult<LayoutOutput>;

    /// 地形：每房间网格雕刻。策略回退失败必须返回 Err（修审核 §4.2，不再静默丢房间）。
    fn generate_terrains(&self, rooms: &[Room], anchors: &[DoorAnchor],
        cfg: &NormalizedConfig, rng: &mut StableRng) -> PcgResult<Vec<Terrain>>;

    /// 点位：物品/敌人采样。
    fn generate_spawns(&self, rooms: &[Room], terrains: &[Terrain],
        cfg: &NormalizedConfig, rng: &mut StableRng) -> PcgResult<SpawnOutput>;

    /// 可达性 + 结构校验：进生产路径（修审核 §4.1）。
    /// TopDownGrid = 无重叠 + 4 邻接连通；SidePlatformer = 跳跃弧可达 + 落脚有效。
    fn validate(&self, result: &GenerationResult) -> PcgResult<()>;

    fn map_kind(&self) -> MapKind;
}
```

### 关键设计决定

1. **数据容器共享、仅算法分叉**：`LayoutOutput`/`Terrain`/`SpawnOutput` 跨模式同构，差异体现在内容（俯视角的 `Terrain.tiles` 不会出现 `OneWayPlatform`）。UE 导出、序列化、`GenerationResult` 组装因此全部 mode 无关。

2. **`validate` 进生产路径**（修审核 §4.1 high）：`generate()` 组装结果后**无条件**调 `backend.validate(&result)?`，失败返回 `Err`。
   - 俯视角 backend 的 `validate` 内含 `no_overlap` + `terrain_connectivity`——终结「默认路径不检测重叠/连通的静默吞错」。
   - platformer backend 的 `validate` 含跳跃可达性 + 落脚有效性。
   - debug-only 的 `run_full_validation` 报告**保留**作诊断，但不再是唯一校验。

3. **地形不再吞错**（修审核 §4.2）：trait 方法返回 `PcgResult`；backend 内部策略回退失败返回 `Err` 而非静默丢房间。

4. **分派方式 = 运行时 `Box<dyn>`**：`let backend = select_backend(cfg.map_kind);`。因为 `map_kind` 是运行时从 config 读的，泛型 `MapGenerator<B>` 会逼调用方编译期定 mode，与「单入口 + config 分派」矛盾。trait 对象开销可忽略（每次 generate 仅调一次）。

5. **RNG 编排保持共享**：`generate()` 仍负责 `root_rng.derive("topology"/"layout"/"terrain"/"spawn")` 并传给 backend，派生标签链**不变**（守黄金测试 + 审核 §5.2 跨模式一致性不退化）。

---

## 4. SidePlatformer backend 生成算法

新建部分的核心。四个阶段如何产出「能通关的横版关卡」。简化运动学贯穿全程：**生成时即保证可跳，而非生成后补救**。

### 4.1 Layout（侧视铺排）

- 关键路径沿 **x 轴水平推进**（与俯视角一致）；分支用 **y 轴 = 高度** 错位，错位量受 `max_jump_height` 约束（不再是审核 §3.4 那个「恒大于房间高、垂直不可达」的 `row_spacing`）。
- 房间间连接按相对高度产出 `TraversalKind`：同高 → `Horizontal`；上方且 Δy ≤ `max_jump_height` → `JumpUp{height}`；下方 → `DropDown`。
- 连接点（门锚）落在房间**地面高度**而非中心（修审核 §3.4「中心高度常悬空」）。

### 4.2 Terrain（平台/间隙雕刻）

- 房间内生成「地面 + 悬空平台」剖面：底部铺 `Solid`；上方按 `max_jump_height`/`max_jump_distance` 间距撒 `OneWayPlatform`，确保相邻平台在跳跃能力内。
- 垂直落差大处放 `Ladder`；可选 `Hazard` 作障碍。
- 平台宽度 ≥ `min_platform_width`，保证可落脚。

### 4.3 Spawn（落脚点采样）

- 候选点限定「脚下为 `Solid`/`OneWayPlatform` 的格」（修审核 §3.5「点位悬空/嵌墙」），不再是任意 floor。

### 4.4 Validate（跳跃可达性，进生产路径）

- 简化运动学可达性：从 Start 落脚点出发 BFS/DFS 扩展，邻接判定 = 「水平相邻可走 + 垂直 Δ ≤ `max_jump_height` + 间隙 ≤ `max_jump_distance`」（替代审核 §3.2 那个把 `(x,y+1)` 无条件当可走的 4 邻接）。
- 校验 Start → Boss 跳跃可达；不可达返回 `Err`（不静默放行）。

---

## 5. 测试与确定性策略

- **俯视角零回归**：现有 293 测试 + 黄金测试（seed 42/12345）必须全绿——这是 topdown backend 行为不变的硬证据。
- **platformer 黄金测试**：新增固定 seed 的 platformer 样本，**锁定具体输出值**（顺带落实审核 §5.3「黄金测试要锁定值，不只自比对」）。
- **platformer property test**：核心不变量——「Start→Boss 跳跃可达」「所有 spawn 落脚有效」「相邻必经平台间隙 ≤ 跳跃能力」。这些**进生产校验**，**不留 `#[ignore]`**。
- **确定性**：platformer 复用同一 `StableRng` 派生链（`topology`/`layout`/`terrain`/`spawn`），派生标签不变，守跨平台复现。
- **错误路径测试**：地形策略回退失败返回 `Err`（验证不再静默吞错）；platformer 校验失败返回 `Err`。

---

## 6. 本 spec 纳入的审核修复（与重写区域交叉）

| 审核条目 | 严重度 | 本 spec 如何修 |
|----------|--------|----------------|
| §4.1 校验不进生产路径 | High | `backend.validate(&result)?` 无条件进 `generate()`，两模式各自校验集 |
| §4.2 地形策略失败静默吞错 | High | trait 方法返回 `PcgResult`，回退失败返回 `Err` |
| §3.1 几何无高度维（z 恒 0）| Critical | platformer backend 由 `grid.y` 映射真实高度到 `WorldPoint.z` |
| §3.2 连通性是俯视角 4 邻接 | Critical | platformer validate 用跳跃弧可达性 |
| §3.4 房间拼接垂直不可达 | High | platformer layout 错位量受 `max_jump_height` 约束、门锚落地面 |
| §3.5 spawn 落点悬空 | High | platformer spawn 限定平台顶面 |
| §5.3 黄金测试不锁值 | Medium | platformer 黄金测试锁定具体输出 |

### 不在本 spec 范围（另立清单后续做）

DefaultHasher 跨版本不稳定（§5.1）、CacheKey 漏 constraints（§4.4）、API builder/`#[non_exhaustive]`（§6.1）、死代码清理（ResultCache/dead_end_count/grammar）、RuntimeChunked 重算整层（§6.2）、UE 层 cm 缩放（§7.3）等与模式无关的修复。

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 重构 topdown 进 backend 时引入回归 | 现有 293 测试 + 黄金测试做护栏，先迁移后改造，迁移步零行为变化 |
| trait 边界设计不当导致数据类型被迫分叉 | 数据容器（LayoutOutput/Terrain/SpawnOutput）保持共享，仅算法分叉；先用 topdown 验证 trait 形状再写 platformer |
| platformer 生成可能无法满足跳跃可达（生成失败率高）| 简化运动学下「生成时保证可跳」，校验兜底返回 Err；必要时加重试上限 |
| `TileKind` 扩展破坏现有 match 穷尽性 | 编译期暴露所有 match 点，逐一补 platformer 分支或 `_`；俯视角逻辑不产生新变体 |
| 序列化向后兼容 | `map_kind` 默认 TopDownGrid、`traversal` 为 Option、新 TileKind 变体追加在末尾 |

---

## 8. 验收标准

1. `cargo clippy --all-targets --all-features -- -D warnings` 零警告。
2. 现有 293 单测 + 黄金测试（seed 42/12345）全绿，俯视角对外行为零变化。
3. `MapKind::SidePlatformer` 下 `generate()` 产出的关卡：Start→Boss 跳跃可达、所有 spawn 落脚有效、相邻必经平台间隙 ≤ 配置跳跃能力——且这些由**进生产路径的校验**保证（违反返回 `Err`）。
4. platformer 黄金测试锁定具体输出值，同 seed 跨运行复现。
5. 审核 §4.1/§4.2 修复在两模式都生效：默认路径校验重叠/连通/可达，地形策略失败不再静默吞房间。

---

## 附录：模块结构（拟）

```
src/
  generator.rs          # 编排（mode 无关）+ select_backend
  config.rs             # + MapKind, PlatformerConfig
  model/
    geometry.rs         # GridPoint 语义升级（高度维）
    terrain.rs          # TileKind 扩展
    room.rs             # RoomEdge.traversal
  topology/             # 共享，不动
  backend/
    mod.rs              # trait PipelineBackend + select_backend
    topdown/            # 现有 layout/terrain/spawn/validate 迁入
    platformer/         # 新建 layout/terrain/spawn/validate
```
（`layout/`、`terrain/`、`spawn/`、`validation.rs` 的现有内容迁入 `backend/topdown/`；共享判定如 `Grid2D` 留在 `model/`。）

