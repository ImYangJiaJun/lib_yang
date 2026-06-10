# yang-pcg 生产级代码审核报告

> 审核对象：`crates/yang-pcg`（约 11,000 行 Rust，0.1.0）
> 审核日期：2026-06-10
> 目标使用场景：**横版平台跳跃游戏（2D platformer）的地图生成基础库**
> 方法：15 维度并行审核 + 逐条对抗性验证（部分维度因服务端限流改为自校验），辅以审核者亲自核实的第一手代码证据
> 基线：`cargo clippy --all-targets --all-features` 零警告；`cargo test --lib` 293 通过 / 0 失败 / 3 个已知 `#[ignore]`；示例可编译

> **后记（2026-06-10 晚，本报告之后的修复）**：本报告为审核当时的快照，下列条目随后已修复并验证，阅读正文时请对照：
> - **3 处生产路径 expect panic 已消除**（原见「亮点」与 §7.1）：`spawn/items.rs`、`spawn/enemies.rs` 的 `world_grid_point` 改为 `unwrap_or` 局部坐标兜底 + 入口 `bounds` 守卫；`ue/streaming.rs` 改 `let-else continue`。生产路径已无 expect/unwrap panic。
> - **`seed: None` 不再走系统时间**：改为从 config 派生确定性种子（新增 `ConfigDigest::seed_from_config`），相同 config 复现同图；`system_time_seed` 已删除。
> - **UE 具名通道已可序列化**：`NamedChannel`/`PcgPoint`/`PropertyValue`/`ChannelKind` 加 `Serialize+Deserialize`，新增 `ue::export_named_channels_json`。（tile 通道 O(总面积) 内存问题仍在，大图仍建议走 `export_json` 主通路。）
> - **新增 `pcg_cli`**（`src/bin/pcg_cli.rs`）：UE5 运行时集成路线 B 的命令行工具，详见 `../UE5_INTEGRATION.md` §5。
> - 当前基线：`cargo test --lib` **307 通过 / 0 失败 / 0 ignored**；clippy 全目标全特性零警告；LICENSE-MIT/APACHE 已补齐。
>
> 报告正文（含三种 ignored test、3 处 panic 的描述）保留为审核当时的原始记录，不再回改。

---

## 一、执行摘要

**一句话结论：作为"UE5 俯视角 Roguelike 网格地图生成器"，yang-pcg 工程质量扎实、可达到生产可用的水准；但作为"平台跳跃游戏的地图生成基础库"，它在架构层面基本不适用，需要重写核心的几何 / 可达性 / 地形 / 拼接四层，而不是加一层适配。**

这个判断来自两条彼此独立的证据线：

1. **契合度错配（决定性）**：该库的几何模型、连通性定义、瓦片语义、房间拼接方式全部建立在"俯视角网格、四方向自由行走"的假设上。平台跳跃所需的**重力方向、高度维度、跳跃可达性、平台/落脚面语义**在数据模型里既不存在也无法表达。这不是参数能调的，是模型层面的根本错配。

2. **生产健壮性缺口（可修复）**：抛开场景错配，库自身还有一个系统性问题——**多类失败在默认生成路径上被静默吞掉**：房间重叠、地形连通失败、地形策略失败、分块路径丢房间，都不会让 `generate()` 返回错误，调用方拿不到任何信号。已知 gap 本身或许可接受，但"默认不暴露给调用方"显著放大了它们的危害。

如果你的项目确实是平台跳跃，**建议不要在此库上做适配**，理由见第三节。如果项目实际是俯视角网格地图（Roguelike/ARPG/战旗），那么修掉第四节列出的 high 级健壮性问题后，它是一个结构清晰、确定性良好、可投入使用的库。

### 确认问题统计（经验证后去重，共 85 条）

| 严重度 | 数量 | 主要分布 |
|--------|------|----------|
| 🔴 Critical | 2 | 平台跳跃契合度（几何无高度维 / 连通性是俯视角） |
| 🟠 High | 13 | 平台跳跃契合度（8）、失败被静默吞掉（5） |
| 🟡 Medium | 26 | 校验缺口、确定性契约、管线编排、API 设计 |
| 🔵 Low | 36 | 边界 panic、性能异味、死代码、文档不符 |
| ⚪ Info | 8 | 含 2 条"证伪"记录（担忧的 panic 实际不可达） |

### 按问题类别

| 类别 | 数量 | 说明 |
|------|------|------|
| platformer-fit | 16 | **最大类**——目标场景契合度缺陷 |
| correctness | 13 | 正确性 bug / 潜伏 bug |
| error-handling | 11 | 失败被静默吞掉 / 错误信息丢失 |
| panic-safety | 10 | 边界 / 极端配置下的 panic（多为低危） |
| testing | 8 | 核心不变量缺测试守护 |
| api-design | 8 | 误用面、版本兼容、误导性 API |
| code-smell / performance / docs / determinism | 18 | 维护性、性能异味、文档不符、确定性契约 |

---

## 二、亮点（先说做得好的）

审核不是只挑毛病。这个库有几处明显高于"个人项目"水准：

- **确定性架构清晰**：`StableRng` 按 stage 派生子流（`topology`/`layout`/`terrain`/`spawn` + 每房间 item/enemy 流），派生标签链与文档一致，生成路径无并行、无 HashMap 迭代序依赖、无浮点 / 指针 / 时间注入。单进程内同 seed 可复现，golden 测试存在。
- **生产路径 panic 面很干净**：系统排查了全部 132 处 `unwrap/expect/panic!` 命中，**生产路径（`generate()` 的 `#[cfg(test)]` 之前）几乎为零**——绝大多数在测试模块内。`generator.rs` 的 21 个 expect、`grammar/selector.rs` 的 10 个 unwrap、`validation.rs`/`topology` 的命中全部在 `#[cfg(test)]` 之后。剩余 3 处生产 expect（`spawn/items.rs:162`、`enemies.rs:228`、`ue/streaming.rs:34`）均有上游过滤 / 不变量保证，当前不可触发。
- **错误模型成熟**：`PcgError` 是带稳定 `error_code()` 的结构化枚举，可程序化匹配而非字符串，带 stage/seed/trace_id 访问器。
- **配置归一化充分**：`GenerationConfig::normalize()` 对 `room_count`/`critical_path_length`/`obstacle_density`/`chunk_size` 等做了范围校验，挡住了下游多处潜在 panic（planner 的 clamp、organic 的 random_bool、streaming 的 div_euclid）。
- **模块边界遵守约定**：UE 概念基本隔离在 `src/ue/`，core 模块未混入 UE 类型。
- **对抗性验证证伪了两条担忧**：`planner.rs:79` 的整数减法经数学证明不会下溢；topology 不产生孤立节点，因此"`bounds=None` → spawn panic"链在当前实现下不可触发。这些是真·安全，不是侥幸。

---

## 三、核心结论：平台跳跃契合度（🔴 决定性）

这是本次审核最重要的一节。库自我定位（`AGENTS.md`、`lib.rs`）是"UE5 Roguelike 俯视角网格地图"，而你的目标是横版平台跳跃。两者在四个层面根本错配，以下每条都附审核者亲自核实的代码证据。

### 3.1 几何模型没有高度 / 重力维度（🔴 Critical）

`src/model/geometry.rs` 的逻辑坐标是纯 2D 俯视角：

```rust
pub struct GridPoint { pub x: i32, pub y: i32 }          // 只有平面 X/Y，没有 Z
pub struct RoomBounds { pub min: GridPoint, pub max: GridPoint }  // 房间是平面矩形
pub enum CardinalDir { North, South, East, West }        // 四个水平方向，无"上/下跳跃"
```

`WorldPoint` 虽有 `z` 字段，但全库每一处写入都是 `z: 0.0`（`ue/adapter.rs:299`、`spawn/items.rs:174`、`enemies.rs:240`）。平台跳跃必需的**重力方向、平台高低差、跳跃弧线、坠落、单向平台**在这个模型里无处表达。`CardinalDir::North` 注释是"北(上)"——那是地图方位的上，不是垂直的上。

**这是模型层缺陷，不是一行能补的。** `WorldPoint.z` 字段存在只是基础设施，真正缺的是 core（topology/layout/terrain）根本不产出任何高程 / 平台语义数据。

### 3.2 连通性是俯视角 4 邻接洪泛，对平台跳跃不成立（🔴 Critical）

`src/terrain/connectivity.rs:57-76` 和 `src/validation.rs:274-280` 的可达性判定**完全一致**——都是 4 邻接 flood-fill：

```rust
fn neighbors(point: GridPoint) -> [GridPoint; 4] {
    [ (x+1,y), (x-1,y), (x,y+1), (x,y-1) ]   // 上下左右皆可走
}
```

这等价于"角色可以自由向任意方向移动一格，**包括向上**"——纯俯视角行走假设。横版平台跳跃中：

- 角色受**重力**约束，不能随意向上走一格：向上靠跳跃（有最大高度限制），向下是坠落；
- "连通"必须考虑跳跃弧线：两平台水平间距 ≤ 跳跃距离、垂直落差 ≤ 跳跃高度才算连通；
- 此处把 `(x, y+1)` 无条件当作可走邻居，相当于假设角色能垂直飞行。

**后果**：即便库生成的地图通过它自己的连通性校验，放进平台跳跃游戏里玩家也可能**根本跳不上去**——校验的"可达"和平台跳跃的"可达"是两个东西。`validation.rs` 的 `all_doors_connected` 对横版毫无保证意义。

### 3.3 瓦片语义缺平台跳跃必需类型（🟠 High）

`src/model/terrain.rs:24-38` 的 `TileKind`：

```rust
pub enum TileKind { Empty, Floor, Wall, Obstacle, Reserved, Doorway }
```

全是俯视角"可走 / 不可走"二元语义。缺少平台跳跃必需的：**OneWayPlatform（单向平台，可从下方穿过、可站在上面）**、**Ladder（梯子）**、**Hazard（尖刺 / 岩浆等致死区）**。`Wall` 和 `Obstacle` 都只表示"不可通行"，没有"可站立的平台表面"概念——而横版里同一格的语义是剖面的：要"脚下实心 + 头上空气"才能站立。

### 3.4 房间拼接是平面 floorplan，不是带高度的横版关卡（🟠 High）

`src/layout/solver.rs` 把关键路径房间沿 X 轴单向排开，分支按 `branch_index % 2` 在 Y 轴上下错开——Y 是平面坐标不是海拔。门锚点按 N/S/E/W 四向落在房间中心高度（`doors.rs`），走廊是正交折线。关键问题：

- 上下相邻房间用**水平 / 正交走廊**连接，横版角色无法靠走廊"向上走"；
- `row_spacing = max_height + corridor.width + 8`（`solver.rs:19`）使上下行间隔恒大于一个房间高，远超任何跳跃高度——垂直方向**天然不可达**；
- 门锚点取房间中心高度，平台跳跃要求入口落在可达落脚高度，中心高度常常悬空。

### 3.5 其余契合度缺陷（🟠 High / 🟡 Medium）

- **spawn 落点（High）**：`spawn/items.rs`、`enemies.rs` 在任意 `Floor/Reserved` 格取候选点，没有"脚下是实心地面"约束。横版里物品 / 敌人必须站在平台顶面，直接套用会让大量点位悬空或嵌在地里。
- **公共 API 无平台跳跃维度（High）**：`GenerationConfig` 没有 `jump_height` / `jump_distance` / `min_platform_width` / `vertical_gap` 任何字段，无法向生成器表达跳跃能力约束。
- **RoomEdge 无遍历语义（Medium）**：`model/room.rs` 的边只有 `from/to/is_critical`，本质无向。横版可达性受重力支配、天然有向（能下不一定能上），edge 无法表达"这条连接需要 H 高度跳跃 / 是单向坠落口 / 需某能力"。
- **RoomType / 约束锚定（Low/Medium）**：`RoomType`（Treasure/Shop/Boss…）是 roguelike 俯视角房间预设；约束系统的 `target_grid_pos` 坐标锚定未实现，无法把出生房固定底部、Boss 房固定顶部。

### 3.6 总判定

| 维度 | 俯视角网格地图 | 横版平台跳跃 |
|------|:---:|:---:|
| 几何模型 | ✅ 适用 | 🔴 无高度/重力维 |
| 连通性 | ✅ 4 邻接正确 | 🔴 不考虑跳跃可达 |
| 瓦片语义 | ✅ 够用 | 🟠 缺平台/单向/危险 |
| 房间拼接 | ✅ 合理 | 🟠 垂直不可达 |
| 可复用部分 | — | 仅 RoomGraph 拓扑骨架（坐标无关）+ 确定性 RNG 编排 |

**对平台跳跃：基本不适用，需重写核心四层。** 可复用的只剩房间图拓扑（`RoomGraph`/`critical_path`/`branches`，坐标无关）和确定性 RNG 编排骨架。若仍要推进，几何 / 可达性 / 地形 / 拼接需按"侧视剖面 + 重力 + 跳跃运动学"重新建模——工作量接近重写，而非适配。

---

## 四、生产健壮性：失败被静默吞掉（🟠 High，与场景无关）

即使抛开平台跳跃错配，这是库自身最需要修的一簇问题。审核者亲自核实了完整证据链。

### 4.1 默认生成路径不检查任何几何不变量（🟠 High）

`generator.rs:42-197` 的生产路径上，**唯一会返回 `Err` 阻断生成的校验是 `validate_result(&result)?`（`generator.rs:186`）**。而 `validate_result`（`validation.rs:489-515`）只做**结构计数检查**：

```rust
// 它只检查这些：
rooms.len() == topology.nodes.len()      // 数量一致
corridors.len() == edges.len()           // 数量一致
door_anchors.len() >= corridors.len()    // 数量关系
schema_version / algorithm_version 非空
// 它完全不检查：房间重叠、地形连通、可达性、spawn 间距
```

真正的语义校验 `run_full_validation`（含 `validate_no_overlap` / `validate_terrain_connectivity` / `validate_spawn_spacing`）：

1. **只在 `debug.is_some()` 时运行**（`generator.rs:190`）——默认 `set_debug(false)` 下根本不跑；
2. **返回 `ValidationReport` 而非 `Result`**——即便 debug 模式跑出 fail，也只是塞进 `debug_bundle.validation_report`，**永不阻断生成**。

**净效果**：默认（非 debug）路径下，AGENTS.md 记录的三个已知 gap——房间重叠、地形连通失败、spawn 间距违规——**完全不被检测，库静默返回一张坏地图，调用方零信号**。已知 gap 本身可接受，但"默认路径不暴露"把它从"已知限制"放大成了"沉默的正确性事故"。

> 修复方向：把 `validate_no_overlap` / `validate_terrain_connectivity` 提升为 `validate_result` 内的硬校验并返回 `PcgError`；若不愿默认开启，至少提供 `strict` 开关，并在文档明确"默认不检测重叠 / 连通"。

### 4.2 地形策略失败 + 无边界房间被静默丢弃（🟠 High）

`terrain/mod.rs:46-72`（整层）与 `chunked.rs:216-227, 382-393`（分块）：

- 无边界房间被 `continue` 跳过，不报错；
- 主策略 `Err(_)` 时原始错误**被 `_` 直接丢弃**（错因完全丢失），回退 `DefaultCarveStrategy`；
- 回退用 `if let Ok(t) = ... { terrains.push(t) }`，**没有 else**——回退也失败时该房间不产生 terrain，不报错、不记日志、不写 debug note，直接进下一个房间。

结果：`result.rooms` 有该房间，`result.terrains` 缺它，且 `validate_result` 不校验 terrains 与 rooms 的对应关系，缺失被咽下。这正是 `terrain/AGENTS.md` 自己点名的风险（"fallback 可能掩盖策略失败"），但实现层没有兜住。

> 修复方向：回退失败返回 `PcgError`（与整层 `?` 传播对齐）；`validate_result` 增加 terrains 覆盖检查（每个有 bounds 的房间都应有对应 terrain）。

### 4.3 分块路径的多重静默缺失（🟡 Medium）

`chunked.rs` 的 `generate_chunk` / `fill_chunk_details` 整体质量低于整层路径：

- **跳过 `validate_result`**：分块结果组装后直接返回，从不校验。且因它把"整层 topology + 子集 rooms"放进同一 `GenerationResult`，`validate_result` 的第一条 `rooms.len()==nodes.len()` 反而必然失败——topology 与 rooms 字段语义不自洽（一个整层、一个子集），调用方若假定一致会出错。
- **预算截断不标记 partial**：命中预算时仅 `break` 后照常返回，`GenerationResult` 无 `partial` 标志（对比 `ChunkDetailResult` 明确带 `partial: bool`），部分结果与完整结果结构上无法区分。
- **未知 chunk_id 静默返回空**：`generate_chunk` 对不存在的 chunk id 返回 `terrains/rooms` 均空的 `Ok` 结果；而 `fill_chunk_details` 对同样情况明确报 `PcgError`——同库内行为不一致。
- **`set_debug` 被无声忽略**：`RuntimeChunked` 路径丢弃 `self.debug_enabled`，分块模式下 debug 输出永不产生，无提示。

### 4.4 CacheKey 不含 constraints（🟠 High，潜伏正确性 bug）

`cache/key.rs:19-49` 的 `CacheKey` 由 `schema/algorithm/seed/config_digest/scope` 组成，而 `config_digest`（`digest.rs:41`）**只哈希 `GenerationConfig`，不含 `request.constraints`**。但 constraints 确实影响输出（`apply_room_constraints` 改 room_type/template_ref，`apply_spawn_constraints` 按排除区过滤点位）。

因此 **seed 相同 + config 相同 + constraints 不同时，CacheKey 完全相同**，缓存命中会把 A 的结果返回给 B——错误结果。当前因 `ResultCache` 是未接入管线的死代码而未触发，但一旦启用缓存即成真。

> 修复方向：把 constraints 纳入 `ConfigDigest` 哈希，或在 `CacheKey` 增加 `constraints_digest` 字段。

---

## 五、确定性契约（🟡 Medium）

确定性是这个库的核心卖点，整体架构正确，但有两处契约缺陷：

### 5.1 种子派生 / ConfigDigest 依赖 std DefaultHasher（跨版本不稳定）

`rng.rs:128-136` 的 `StableRng::derive` 与 `digest.rs:38-50` 的 `ConfigDigest::from_config` 都用 `std::collections::hash_map::DefaultHasher`（SipHash）。std 官方明确声明**其算法不保证跨 Rust 版本稳定**。而 `digest.rs:18` 注释宣称"摘要格式稳定、不受 Rust 版本影响"——与事实矛盾（`digest.rs:40` 甚至自陈"用 serde_json 避免 Hash 不稳定"，却又把序列化结果喂进不稳定的 DefaultHasher，自相矛盾）。

- **影响**：升级 Rust 工具链若改动 SipHash，所有派生子种子和所有 ConfigDigest 同时变化，同 seed+config 在新工具链上生成不同地图，旧缓存 / 导出签名全部失配。
- **边界**：同一发行二进制内（同 toolchain、小端，UE5 主流目标）完全确定，单次发行内 seed 分享成立；跨版本 / 大端才出问题。故定为 Medium 而非 High。
- **修复**：`derive` 与 `digest` 改用版本无关、字节序固定的稳定哈希（自实现 FNV-1a / xxhash，整数显式 `to_le_bytes`），并修正注释。

### 5.2 Chunked/Hybrid 与整层模式 RNG 派生路径不一致（注释谎称一致）

整层路径：terrain 用 `root.derive("terrain")` 后所有房间**共享一个顺序流**；spawn 路径是 `root→"spawn"→"items:{id}"`。
分块路径（`chunked.rs:346-362`）：每房间直接 `root.derive("terrain:{id}")`、`root.derive("items:{id}")`——**独立流**，且缺中间 `"spawn"` 层。

因 `derive` 是 `hash(parent_seed + label)`，`root→"spawn"→"items:0"` 与 `root→"items:0"` 的最终种子必然不同。**同 seed+config 下，整层 `generate()` 与分块产出不同地图。** 但 `chunked.rs:346` 注释明写"使用与整层一致的 RNG 派生路径，保证确定性"——代码与注释直接矛盾（项目自己的 `chunked_tests.rs:446` 也坦承"由于派生路径不同，只验证结构而非逐字节一致"，与该注释打架）。

对"整层预览 + 运行时分块加载"的典型用法，玩家会在分块边界看到与预览不同的关卡。

> 修复：让整层也改为每房间独立派生（去掉中间 `"spawn"` 层），使 HybridPrecompute 能逐房间复现整层；并补一个跨模式黄金测试断言同 seed 下同房间产出一致。

### 5.3 黄金测试只自比对、不锁定具体值

`tests_task26/golden_sample_tests.rs` 的"黄金样本"实际只在同进程内生成 2~3 次再断言彼此相等，**从不与固化常量比对**（注释说"第一次运行记录哈希后续对比"，但代码里没有任何 `EXPECTED_*` 常量）。因此 5.1 描述的跨版本漂移**完全不会被捕获**——漂移后两次生成仍彼此相等，测试照样 PASS。确定性契约缺乏真正的回归护栏。

---

## 六、API 设计、管线编排与其它（🟡 Medium / 🔵 Low）

### 6.1 API 设计

- **陈旧误导注释（Medium）**：`lib.rs:125` 注释"实际生成功能尚未实现，这里只测试类型创建"——审核者核实 `generate()` 早已完整实现并被大量测试覆盖。这是早期 MVP 遗留的失真文案，会让读源码者误判库不可用。**应删除。**
- **裸 pub 字段 + 无 builder + 无 `#[non_exhaustive]`（Medium）**：`GenerationRequest`（5 字段）、`GenerationConfig`（13 字段）及所有子配置都是裸 pub 字段结构体，无 builder、无 `#[non_exhaustive]`。后果：(1) 库新增任一字段即编译破坏所有不带 `..Default::default()` 的构造点，对发布库是硬 break；(2) 无构造期校验入口，所有约束只能延后到 `normalize()` 运行期。
- **`merge` 文档与实现不符（Medium）**：`config.rs:183-216` 文档称"非默认值覆盖"，实现却是**无条件全量覆盖**（注释自陈"简化实现"）。按文档预期做配置分层会被静默清掉自定义值。
- **`NormalizedConfig` 全 pub（Low）**：类型宣称"已校验配置"，但字段全 pub 可被任意字面量构造，绕过 `normalize()` 的全部校验——这是多处"仅靠 normalize 兜底"的 panic 防线（organic 的 `random_bool`、planner 的 `clamp`、streaming 的 `div_euclid`）能被击穿的根因。建议字段私有、只经 `normalize()` 构造。

### 6.2 管线编排

- **HybridPrecompute 不被 `generate()` 识别（Medium）**：`generator.rs:42-46` 只对 `RuntimeChunked` 委托分发，设 `HybridPrecompute` 调 `generate()` 实际跑的是整层路径——模式标志被静默忽略。AGENTS.md 称 MapGenerator 是 "generation-mode dispatcher"，名不副实。
- **RuntimeChunked 预算是死代码（Medium）**：`normalize()` 把 `time_budget_ms`/`iteration_budget` 硬编码为 `None`，且无任何请求字段能注入，故 `generate_chunk` 内的预算检查在公共路径上永不触发。文档宣传的"分块预算限制"能力实际不可达。
- **RuntimeChunked 每次重算整层（Medium，性能）**：`generate_chunk` 每次请求都重算整层 topology+layout+chunks 后才过滤目标房间，无任何跨调用缓存。`RuntimeChunked` 命名隐含"运行时摊薄"，实际未做（真正能摊薄的是 HybridPrecompute）。默认规模影响有限，长横版关卡放大后成为固定大头。

### 6.3 topology（健康，少量边界问题）

- **difficulty/depth 乘 10 在大 room_count 下 u16 溢出（Medium）**：`planner.rs:55,99,104` 的 `(index as u16)*10`，当 `room_count` 采样 >6553 时溢出（debug panic / release 回绕）。根因是 `room_count.max` 无上限。默认 10-20 不触发。
- **dead_end_count 配置空转（Low）**：该字段被 `normalize` 校验、却从不参与拓扑生成，用户设了无效果，制造"已生效"错觉。

---

## 七、其余维度速览（🔵 Low / ⚪ Info 为主）

### 7.1 panic 安全（生产路径很干净）

生产路径几乎无裸 panic（见第二节）。剩余均为**极端配置 / 不可信输入下的边界 panic，非默认路径**：

- `Grid2D::new` 用 `(width*height) as usize` 无溢出检查，`room_size.max` 无上限 → 病态配置（如 65535×65535）触发约 4GB 分配 OOM。建议 `validate` 加尺寸上限（如 ≤512）+ `checked_mul`。（Low）
- `import_binary` 的 `body_len` 算术在 32 位平台 debug 构建可溢出 panic（64 位 release 安全，需构造 ~4GB 输入）。建议 `checked_add`。（Info）
- `spawn/budget.rs:12`、`enemies.rs` 的 u16 加法在极端配置下可溢出（默认不触发）。（Low/Info）

### 7.2 导出 / 序列化（基本健康）

- `schema_version` 在 `generator.rs:170`、`chunked.rs:402` 各自硬编码 `"1.0.0"`，未引用 `CURRENT_SCHEMA_VERSION` 常量，升级时易漏改漂移。（Low）
- f32 字段为 `NaN`/`Infinity` 时 JSON roundtrip 不保真（serde_json 写 `null`，反序列化失败）。（Low）
- JSON/binary 主版本号校验存在且测试覆盖；binary 次版本号读后丢弃（符合向前兼容设计）。整体往返无损，schema 版本机制可用。

### 7.3 UE 适配层

- **`WorldPoint` 无 cm 缩放（Medium）**：`grid_world_point`（`adapter.rs:295`）直接把网格索引 `as f32` 写入，无 `cell_size→cm` 缩放，三处重复实现。号称"世界坐标"实为格子索引，相邻 tile 间距 1.0，UE 里整图缩成几十厘米。建议引入显式 `cell_world_size` 参数统一缩放。
- `export_named_channels` 不产出 `ChannelKind::Debug`（死变体，调试数据走 side channel 是有意设计，但枚举变体悬空）。（Low）
- 门朝向 `facing` 只进字符串属性，未写入 `Transform.rotation`（信息无损，仅工效学）。（Low）
- `O(terrains×rooms)` 线性查找定位房间（默认规模无感）。（Low）

### 7.4 性能（默认规模可接受）

无 O(n³) 热点。拓扑 / 布局线性，地形雕刻每房间 O(W·H)。可优化点都是 Low/Info：分块循环内重复 clone 门锚点 Vec、`O(R²)` 的 `terrains.find`、organic CA 每步分配新网格（固定 4 次迭代）。`ResultCache` 是从未接入管线的死代码——宣称的缓存能力为空壳。这些在默认 10-20 房间下被每房间网格运算量淹没，长横版关卡放大后才显著。

### 7.5 测试（动作丰满，核心守护缺失）

- **三大几何不变量（重叠/连通/间距）的 property test 全部 `#[ignore]`，而生产路径又不硬校验它们**——核心算法不达标既无运行时拦截、又无测试守护。AGENTS.md 称 ignore 是"文档"，但它文档化的恰是"生产会静默吐出坏地图"。（High）
- **零覆盖平台跳跃场景**：所有"可达性"测试都是房间图 BFS 或网格 4 邻接 flood-fill（俯视角假设），没有一条断言涉及跳跃可达 / 重力 / 垂直通行。（High，与第三节呼应）
- 弱断言"假测试"：`lib.rs` 三个用例只测类型创建 / 枚举长度；`generator` 端到端测试只断言 non-empty。（Medium/Low）
- property test 配置空间被钉死成极窄一片（`obstacle_density` 恒 0.15 等），刻意避开了已知会触发失败的维度。（Medium）

---

## 八、分级行动清单

### 第一步：决定方向（阻塞性，必须先定）

平台跳跃契合度是决定性的。**先确认项目到底是哪种地图**：

- **若确为横版平台跳跃** → 不建议在此库上适配。核心几何 / 可达性 / 地形 / 拼接四层需按"侧视剖面 + 重力 + 跳跃运动学"重写，工作量接近重做。可复用的只有 `RoomGraph` 拓扑骨架和 RNG 编排。建议：要么新建 platformer 专用库（拓扑层可借鉴），要么改用专门的平台跳跃 PCG 方案。
- **若实际是俯视角网格地图** → 此库适用，按下面 P0/P1 修复后可投入生产。

### P0 — 上生产前必修（无论哪种场景）

1. **几何不变量进生产校验**（§4.1）：`validate_result` 内硬校验 `no_overlap`/`terrain_connectivity`，失败返回 `PcgError`；或加 `strict` 开关 + 文档明示默认不检测。
2. **地形策略失败不再静默吞**（§4.2）：回退失败返回 `Err`；`validate_result` 增加 terrains 覆盖检查。
3. **CacheKey 纳入 constraints**（§4.4）：启用 `ResultCache` 前必修，否则缓存返回错误结果。

### P1 — 生产质量必须项

4. 稳定哈希替换 DefaultHasher（§5.1），修正确定性注释。
5. 修正 / 对齐分块路径：补 `validate`、`partial` 标记、未知 chunk 报错、debug 透传（§4.3）；对齐或文档化跨模式 RNG 差异、删除 `chunked.rs:346` 虚假注释（§5.2）。
6. 删除 `lib.rs:125` 陈旧注释（§6.1）；修正 `merge` 文档 / 实现（§6.1）。
7. 黄金测试锁定固化值（§5.3）；把三个 `#[ignore]` property test 在算法修复前改为"断言失败被正确转化为 PcgError"的负路径测试（§7.5）。

### P2 — 健壮性与可维护性

8. 公开输入结构体加 `#[non_exhaustive]` + builder（§6.1）；`NormalizedConfig` 字段私有化。
9. `room_count.max` / `room_size.max` 加上限 + checked 运算，杜绝 §6.3 / §7.1 的溢出 / OOM。
10. `schema_version` 引用常量（§7.2）；HybridPrecompute 在 `generate()` 显式报错或文档化（§6.2）。
11. 死代码处理：`ResultCache`、`dead_end_count`、`grammar::WeightedRuleSelector`、约束的 `exclude_rooms`/`target_grid_pos`——要么实现、要么标 `#[doc(hidden)]`/文档注明未实现，停止"宣称能力却空转"。

### P3 — 性能与打磨

12. UE 层引入 `cell_world_size` 缩放（§7.3）；性能微优化（§7.4，非阻塞）。

---

## 附录：审核方法与可信度

- **覆盖**：15 个维度（平台跳跃契合度、确定性、panic 安全、API、管线、topology/layout/terrain/spawn、validation、序列化、UE、cache/constraint/grammar、性能、测试）。
- **验证**：6 个维度经独立 agent 逐条对抗性验证（标"对抗验证"），裁定 isReal 与校准严重度；其余 8 个维度因服务端限流改为"agent 自带源码核实"（标"自校验"）。审核者另就最关键结论（geometry 无 z、connectivity 4 邻接、validate_result 只查计数、generate() 已实现、topology 无孤立节点、layout 校验链）做了第一手代码核实，与 workflow 结论一致。
- **去重**：跨轮次同一发现合并，共 85 条确认问题。对抗验证下调了若干夸大严重度（如 organic panic high→medium、UE 缩放 high→medium），并证伪 2 条担忧（planner 下溢、bounds=None panic 均不可达）。
- **局限**：未实际编译运行触发 panic，结论基于源码与 rand/serde 语义推理；性能为定性评估，无 profiling 实测。

*报告由 yang-pcg 全量审核 workflow 生成，审核者综合校验。*
