# yang-pcg 优化对照指南

> 范围：仅 `crates/yang-pcg/`。本指南由 7 路代码评审（代码质量 / 逻辑正确性 / 鲁棒性安全 / 性能 / 架构设计 / API-SemVer / 测试确定性）的发现，经独立对抗式核验后整合而成。**只收录核验通过的项**；核验判为 INVALID 或"更优解会引入回归/破坏确定性"的项已被剔除或降级处理。
>
> 生成日期：2026-06-28　保留条目：**83**

---

## 0. 阅读与使用说明

1. **配合三份现有文档使用**：
   - `docs/BACKLOG.md` —— NEW-20~34 的状态表是"当前真相"。本指南中标注 `已跟踪 NEW-xx` 的条目，**不要**当作新阻断项重新登记，按这里给的"更优重构方案"落地即可。
   - `PRODUCTION_AUDIT_2026-06-24.md` —— 历史审计基线，部分位置引用以它为锚。
   - 各模块 `AGENTS.md`（`yang-pcg/src/terrain/AGENTS.md` 等）—— 改对应模块前先读。
2. **确定性契约是第一约束**。`crates/yang-pcg/CLAUDE.md` 已声明：RNG 派生标签（`topology`/`layout`/`terrain`/`spawn` 及分块路径 `terrain:chunk:{c}:{r}`、逐房间 `items:{id}` 等）是确定性契约的一部分，**改名/改派生顺序/改 RNG 消耗量 = 破坏 seed 复现性和黄金测试**。本指南每条都标了 `breaksDeterminism`：
   - `否` —— 安全，可直接落地。
   - `是（破坏性）` —— 会改变 seed→地图输出，**必须**走新 major + 迁移说明 + 同步更新黄金测试，禁止在小版本顺手做。
3. **核验已纠正若干原始发现的错误**，落地前请读每条的"现状/更优解"，已写入纠偏结论。典型：
   - QC-05 原称"删 300 行内部连通修复函数"——核验证明那会改变 tile 集合进而改 spawn 位置，**只能删 4 处冗余 `summarize_connectivity` 调用**（见 OPT-Q-08）。
   - PCG-L-02 / ROB-11 障碍放置死循环——修复用 `max_attempts` 上限（方案 B），**不要**用 Fisher-Yates（方案 A 改变跨房间共享 RNG 的消耗量，破坏确定性）。
4. 每条末尾有 `验收` 勾选项，落地后逐项打勾，并跑：
   ```bash
   cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
   cargo test --lib -p yang-pcg            # 期望 309 passed / 0 ignored
   ```

---

## 1. 总览

### 1.1 主题分布

| 主题 | 条目数 |
|---|---|
| 确定性契约与 RNG | 4 |
| 逻辑与几何正确性 | 5 |
| 鲁棒性与 panic 安全 | 12 |
| 性能与分配 | 16 |
| 架构与 API 暴露面 | 9 |
| SemVer 与发布卫生 | 12 |
| 测试与确定性回归 | 14 |
| 代码质量 | 11 |
| **合计** | **83** |

### 1.2 已剔除（核验未通过，留档备查）

| 原 id | 标题 | 剔除原因 |
|---|---|---|
| ROB-02 | distance_sq 返回 i32 大坐标溢出 | 核验 INVALID：distance_sq 入参是单房间局部瓷砖索引（≤ terrain 宽），非全局世界坐标，默认配置 dx≤13，任何合理配置不可触发。其 i64 化仍随 OPT-R-01 一起做（编译需要），但不作为独立 bug。 |
| ARCH-01 | critical_path_length.max 未与 room_count.max 比对 | 核验 INVALID：`topology/planner.rs:30-33` 采样后立即 `.clamp(2, room_count)`，不会运行时失败；新增校验反而把当前能正常生成的配置变成 config error。 |
| QC-05（原方案） | 删除策略内部 BFS 连通修复 ~300 行 | 核验降级：内部修复与兜底 `connect_all_doorways` 算法不同（BFS 最短路 vs L 形），删除会改变 Floor 瓦片集合→改 spawn 位置→破坏黄金测试。仅保留"删 4 处冗余 summarize_connectivity"，见 OPT-Q-08。 |

### 1.3 优先级 × effort 速览（高杠杆 Top 7）

| id | 标题 | 严重度 | effort | breaking | breaksDeterminism |
|---|---|---|---|---|---|
| OPT-R-01 | 间距平方阈值 i32 溢出致间距校验失效 | HIGH | S | 否 | 否（修 bug） |
| OPT-R-02 | room_size.max 无上界 → Grid2D OOM DoS | HIGH | S | 是 | 否 |
| OPT-R-03 | 障碍放置 while 无上限，小房间死循环 | HIGH | S | 否 | 否（方案 B） |
| OPT-Q-01 | 删除孤儿文件 level_graph.rs | HIGH | S | 否 | 否 |
| OPT-D-01 | HybridPrecompute 经 generate() 静默降级 | HIGH | S | 否 | 否（修 bug） |
| OPT-T-01 | 黄金测试无硬编码期望值，无法防跨版本漂移 | HIGH | S | 否 | 否 |
| OPT-T-03 | 属性测试仅覆盖 OfflineFullFloor 单模式 | HIGH | M | 否 | 否 |

---

## 2. 确定性契约与 RNG

### OPT-D-01　HybridPrecompute 经 generate() 静默降级为 OfflineFullFloor
- 严重度 **HIGH**（原 CRITICAL，核验降级）| 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否（修复消除非法路径，不改合法输出）
- 合并自：PCG-L-01 + ARCH-04　|　已跟踪 NEW-28（三模式 RNG 标签）
- 位置：`generator.rs:47-49`（`MapGenerator::generate`）
- 现状：`generate()` 只对 `RuntimeChunked` 做分支早返回；`HybridPrecompute` 无分支，落穿进 `OfflineFullFloor` 流程（用 `derive("terrain")`）。而合法两阶段 API（`generate_topology_only` + `fill_chunk_details`）用 `derive("terrain:chunk:{c}:{r}")`，**同一 seed 两条路径产出不同地图，全程无报错无警告**。`validate_request` 也不校验该模式的路由需求。该分支零测试覆盖。
- 更优解：在 RuntimeChunked 分支后立即对 `HybridPrecompute` 返回显式 `Err(PcgError::config("HybridPrecompute 须经 generate_topology_only()+fill_chunk_details() 两阶段调用"))`；或在 `generate()` doc 明确标注其等价 OfflineFullFloor 且 seed→map 与两阶段不同。补一条 Err 路径测试。
- 验收：
  - [ ] `generate()` 对 `HybridPrecompute` 不再静默落入 OfflineFullFloor
  - [ ] 新增测试断言该模式经 `generate()` 返回 Err（或文档已明确行为）
  - [ ] OfflineFullFloor / RuntimeChunked 输出不变（黄金测试通过）

### OPT-D-02　from_seed_bytes 的 inner（PCG128）与 seed 字段（FNV64）来源不一致
- 严重度 **LOW**（原 MEDIUM，核验降级）| 优先级 P3 | effort M | breaking 是 | breaksDeterminism 否（生产路径未调用）
- 来源：PCG-L-05
- 位置：`rng.rs:183-191`
- 现状：`from_seed_bytes` 用 `Pcg64::from_seed(32B)` 初始化 `inner`，但 `self.seed` 存 `fnv1a_64(&seed)`。`derive()` 走 `self.seed`、直接生成走 `inner`，两者有效根不同。**核验确认全生产管线只用 `from_seed(u64)`（generator.rs:56、chunked.rs:85/163/297），`from_seed_bytes` 仅出现在单测**，故不破坏现有确定性，纯 API 内部语义不一致。
- 更优解：统一为 `let s = fnv1a_64(&seed); Self::from_seed(s)`，丢弃 128-bit PCG 初始化，使直接生成与 derive 链同根。
- 验收：
  - [ ] `from_seed_bytes` 内 `inner` 与 `seed` 同源
  - [ ] 单测 `test_from_seed_bytes` 已更新且通过

### OPT-D-03　validate_reachability 错误消息中不可达房间列表顺序不确定
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否（仅错误文案）
- 来源：PCG-L-09
- 位置：`validation.rs:106-110`
- 现状：`all_room_ids.difference(&visited).copied().collect()` 用 `HashSet::difference`，迭代顺序随机，同一 bug 不同运行产出不同错误串，难以日志比对/测试断言。
- 更优解：collect 后 `unreachable.sort();`（或改 `BTreeSet`）。
- 验收：
  - [ ] 不可达列表有确定顺序
  - [ ] 可对错误消息做精确断言

### OPT-D-04　rng.sample() 全量 shuffle，n≪len 时浪费（仅可改为部分 Fisher-Yates）
- 严重度 LOW | 优先级 P3 | effort M | breaking 否 | **breaksDeterminism 是（破坏性）**
- 来源：PERF-13
- 位置：`rng.rs:448-456`
- 现状：`sample(slice, n)` 先建全 `Vec<usize>` 再整体 shuffle，只取前 n。n=3、len=500 时浪费 499 次 swap。
- 更优解（**破坏性，需新 major + 迁移**）：改为只走前 n 步的部分 Fisher-Yates，复杂度 O(n)。**但部分 shuffle 与全 shuffle 对同一 seed 输出不同序列**。落地前置条件：grep 确认无生产管线路径调用 `sample`（当前仅 rng 自测调用）；若确认无生产调用，可视为不影响 seed→地图契约，否则**禁止**。同步更新所有 `sample` 相关黄金/断言。
- 验收：
  - [ ] 已确认 `sample` 未出现在 topology/layout/terrain/spawn 生产路径
  - [ ] 若改动，已在新 major 中并更新黄金测试常量
  - [ ] 若有生产调用 → 放弃此项

---

## 3. 逻辑与几何正确性

### OPT-L-01　validate_result corridors 计数守卫多余，放行 edges=0/corridors>0
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PCG-L-03
- 位置：`validation.rs:498-501`
- 现状：`if !edges.is_empty() && corridors.len() != edges.len() { Err }`。前置 `!edges.is_empty()` 使 `edges=0 且 corridors=N>0`（如导入损坏 JSON）被静默放行。`validate_result` 只在 `ValidationScope::FullFloor` 调用，不应保留该容错。
- 更优解：FullFloor 下无条件 `if corridors.len() != edges.len() { Err }`；若担心 Chunk 复用，拆 `validate_result_full` / `validate_result_chunk` 或加 scope 参数。
- 验收：
  - [ ] FullFloor 下 edges=0/corridors>0 被拒绝
  - [ ] Chunk 部分结果路径不受影响

### OPT-L-02　choose_weighted 不拒绝负权重 + config 不校验负 rarity_weights
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-22（NaN 权重）
- 合并自：PCG-L-04 + ROB-08
- 位置：`rng.rs:483-507`（choose_weighted）、`config.rs:484-495`（ItemSpawnConfig::validate）
- 现状：`choose_weighted` 只查 `total_weight.is_finite() && >0`，不查单个权重非负。`weights=[3.0,-2.0,1.0]`（sum=2，过检）→ index0 被 100% 选中，1/2 永不被选。config 只拒 NaN 不拒负值，`rarity_weights=[-0.5,1.5]` 通过校验后触发此路径。
- 更优解：`choose_weighted` 在 total 检查前加 `if weights.iter().any(|&w| w < 0.0) { return None; }`；`ItemSpawnConfig::validate` 加 `if rarity_weights.iter().any(|w| *w<0.0) { Err("稀有度权重不能为负") }`。
- 验收：
  - [ ] 负权重个体使 choose_weighted 返回 None
  - [ ] config 在边界拒绝负 rarity_weights
  - [ ] 新增拒绝测试（与 OPT-T-07 合并落地）

### OPT-L-03　solver 分支父房间缺失时静默回退退化边界
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PCG-L-07
- 位置：`layout/solver.rs:68-74`（solve_room_bounds）
- 现状：`branch.start_room` 不在 `bounds_map` 时静默回退 `{(0,0),(12,12)}`，与关键路径首房间重叠，产出退化布局，无 Err/无 log，仅靠事后 `validate_no_overlap` 兜底。
- 更优解：改 `solve_room_bounds` 返回 `PcgResult<...>`，缺失父房间 `ok_or_else(|| PcgError::layout(...))?`；调用方透传。不改签名则至少 `debug_assert!` + `tracing::warn!`。
- 验收：
  - [ ] 缺失父房间显式报错或告警
  - [ ] 正常拓扑布局输出不变

### OPT-L-04　sample_rarity_tier 硬编码 3 tier，rarity_weights.len()≠3 时静默全 tier-0
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-09
- 位置：`spawn/items.rs:157-163`、`config.rs` ItemSpawnConfig::validate
- 现状：`tiers=[0,1,2]` 固定 3 元素；`choose_weighted` 对 `len` 不等返回 None → `unwrap_or(0)`。`rarity_weights:[1.0]`（合法 sum=1）使全部物品静默变 rarity 0，无任何提示。
- 更优解：`ItemSpawnConfig::validate` 加 `if rarity_weights.len()!=3 { Err("稀有度权重须恰好 3 个") }`；若将来需可变 tier，items.rs 用 `(0..weights.len() as u8).collect()` 动态生成 tiers。
- 验收：
  - [ ] config 拒绝 len≠3 的 rarity_weights
  - [ ] 不再出现静默全 tier-0 降级

### OPT-L-05　GrammarRule.base_weight=NaN：绕过 weight≤0 短路 + 误导错误消息
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-22
- 合并自：QC-18 + ARCH-12
- 位置：`grammar/selector.rs:130-174`（compute_adjusted_weight / select）
- 现状：`NaN <= 0.0` 为 false，NaN base_weight 不被短路，乘进 total → `!is_finite()` 触发，但错误文案是"所有规则权重为零"（与 sum≤0 共用），对 NaN 配置无法定位。NaN 本身被拦截（非静默绕过），问题在诊断质量。
- 更优解：`compute_adjusted_weight` 改 `if !weight.is_finite() || weight <= 0.0 { return 0.0; }`；`select` 入口或 total 检查处区分 `total.is_nan()` 与 `total<=0.0`，分别给"含 NaN 权重"/"权重全零"消息，最好带规则名。
- 验收：
  - [ ] NaN base_weight 短路为 0
  - [ ] NaN 与全零产生不同、可定位的错误消息

---

## 4. 鲁棒性与 panic 安全

### OPT-R-01　间距平方阈值 i32::pow(2) 溢出，min_spacing≥46341 致间距校验失效
- 严重度 **HIGH** | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否（对所有合理输入结果一致，仅修复溢出区间）
- 合并自：ROB-01 + PCG-L-06（含 ROB-02 的 distance_sq i64 化——编译依赖，非独立 bug）
- 位置：`spawn/sampling.rs:96-97`（min_distance_sq / occupied_distance_sq）、`distance_sq` L128-132
- 现状：`i32::from(min_spacing).pow(2)`，min_spacing 为 u16（≤65535）。`46341² > i32::MAX`，release 静默绕为负 → `distance_sq >= 负数` 恒真，间距约束完全失效，`validate_spawn_spacing` 同被绕过；debug 直接 panic。config 只校验 `min_spacing >= 1` 无上界。
- 更优解：`min_distance_sq` / `occupied_distance_sq` 改 i64：`i64::from(min_spacing) * i64::from(min_spacing)`；`distance_sq` 返回 i64（`i64::from(a.x)-i64::from(b.x)` 等），调用处比较类型对齐。**额外**在 `ItemSpawnConfig::validate` / `EnemySpawnConfig::validate` 给 min_spacing 加合理上界（如 ≤128）双保险。
- 验收：
  - [ ] min_distance_sq / distance_sq 均为 i64，无溢出
  - [ ] config 对 min_spacing 设上界
  - [ ] 默认配置（min_spacing 2/3）spawn 结果与之前逐位一致

### OPT-R-02　room_size.max_width/max_height 无上界 → Grid2D OOM DoS
- 严重度 **HIGH** | 优先级 P1 | effort S | breaking 是 | breaksDeterminism 否
- 来源：ROB-03
- 位置：`config.rs:306-334`（RoomSizeConfig::validate），攻击链经 `pcg_cli --config`
- 现状：只校验 `min >= 4`，无最大值。`max_width=max_height=65535`（u16 合法）→ 每房间 `Grid2D::new` 分配 ~4GB；MazeStrategy 额外分配等大 `Grid2D<bool>`（~8GB/房间）。`room_count.max` 同样无上界。pcg_cli 经 `--config` 加载任意 JSON（UE5 经 `FPlatformProcess::CreateProc` 调用），单文件即可 OOM 崩溃，无需认证。
- 更优解：`RoomSizeConfig::validate` 加 `if max_width > 512 || max_height > 512 { Err("房间尺寸上限 512x512") }`（512 覆盖 UE5 实际需求）；`normalize()` 对 `room_count.max` 加上界（如 ≤4096）。**breaking**：拒绝先前合法的巨型配置，对所有合法配置行为不变，不触碰 RNG。
- 验收：
  - [ ] 超大 max_width/max_height 在 validate 阶段被拒绝
  - [ ] room_count.max 有上界
  - [ ] 合法配置生成输出不变

### OPT-R-03　place_obstacles_with_config while 无上限，小房间死循环
- 严重度 **HIGH** | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否（方案 B）
- 合并自：QC-09 + PCG-L-02 + ROB-11
- 位置：`terrain/carve.rs:160-168`
- 现状：4×4 非排除房间、默认 `obstacle_density=0.2` 时，`target=3`，`random_range(1,2)` 只能返回 1 → 仅能采样 (1,1)；放置 1 个后 (1,1) 变 Obstacle，后续永不命中 Floor，**确定性死循环**（placed 停在 1，无错误返回、无超时）。核验纠偏：触发不需要 Boss/Reserved 或角落 Doorway（Boss 房提前 return、门锚点恒在边界格），任意 4×4 非排除房间即触发。config 显式允许 `min_width=4`。
- 更优解（**采用方案 B**）：加 `attempts < max_attempts` 上限：`let max_attempts = target.saturating_mul(10).max(40); ... while placed<target && attempts<max_attempts && max_x>1 && max_y>1 { attempts+=1; ... }`。**不要用方案 A（Fisher-Yates 预收集 + shuffle）**——它改变跨房间共享 `&mut StableRng` 的消耗量（attempt 消耗 2 次 vs shuffle 消耗 floor_count-1 次），破坏存量 seed 的地图。方案 B 对正常房间不触发上限、RNG 消耗不变，仅终结病态路径。
- 验收：
  - [ ] 4×4 房间 + obstacle_density>0 不再死循环
  - [ ] 正常房间 RNG 消耗与之前一致（黄金测试通过）
  - [ ] 未使用预收集/shuffle 方案

### OPT-R-04　row_spacing 用 u16 算术，max_height+corridor.width+8 可 wrap
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-04
- 位置：`layout/solver.rs:35`
- 现状：`i32::from(max_height + corridor.width + 8)` 中加法是 u16 运算，`max_height=65530,width=10` → wrap 为 12（应为 65548），debug panic / release 布局错误。
- 更优解：各自先转 i32 再加：`i32::from(max_height) + i32::from(corridor.width) + 8`。
- 验收：
  - [ ] row_spacing 在大尺寸下不 wrap

### OPT-R-05　difficulty / budget 的 u16 算术不加保护
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-05
- 位置：`topology/planner.rs:56`（`(index as u16)*10`）、`spawn/budget.rs:12`（`base + room.difficulty`）
- 现状：room_count.max 可达 u16::MAX，index≥6554 时 `*10` 溢出、index≥65536 时 `as u16` 截断；`base+difficulty` 两 u16 相加 debug panic/release 饱和错误。
- 更优解：planner `((index as u64)*10).min(u16::MAX as u64) as u16`；budget `base.saturating_add(room.difficulty)`。（配合 OPT-R-02 的 room_count 上界从根上收敛。）
- 验收：
  - [ ] 高 room_count 下 difficulty/budget 不溢出

### OPT-R-06　critical_cursor_x i32 累加可溢出，大 room_count×room_size 布局错乱
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 否 | breaksDeterminism 否
- 来源：ROB-06
- 位置：`layout/solver.rs:62`
- 现状：逐房间 `cursor_x = bounds.max.x + corridor.width + 6` 累加，room_count=1100、max_width=2000 时超 i32::MAX，wrap 为负致坐标错乱；release 静默产出无效布局。
- 更优解：内部用 `i64` 累加，转 RoomBounds 时 clamp 到 i32 并对越界返回 Err；或经 OPT-R-02 的 room_count 上界作为根本修复。
- 验收：
  - [ ] 大规模布局坐标不溢出或显式报错

### OPT-R-07　import_binary / import_json 反序列化后无 validate_result 后校验
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-07
- 位置：`export/binary/mod.rs:229-236`、`export/mod.rs:111-139`
- 现状：过 magic/version/CRC 后直接返回 `GenerationResult`，不调 `validate_result()`。攻击者可构造过 CRC 的畸形文件（rooms.len≠nodes.len、空 schema_version、corridors 超 anchors），UE5 适配层索引时 panic/产错关卡。
- 更优解：两函数 `Ok(result)` 前插 `validate_result(&result)?;`；可进一步遍历 SpawnPoint.grid_pos 确认在房间 bounds 内。
- 验收：
  - [ ] 导入后强制 validate_result
  - [ ] 畸形文件被拒绝（新增测试）

### OPT-R-08　manhattan_distance i32 减法，极端坐标溢出致间距验证漏报
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-10（与 OPT-R-07 联动：经 import 的对抗坐标触发）
- 位置：`validation.rs:338-340`
- 现状：`(a.x-b.x).abs()+(a.y-b.y).abs()` 全 i32。`a.x=i32::MAX,b.x=i32::MIN` → 溢出 wrap 为 -1，abs=1，间距违规漏报。
- 更优解：改 i64 计算，调用处 spacing 比较对齐 i64。
- 验收：
  - [ ] manhattan_distance 用 i64，无溢出

### OPT-R-09　rarity_weights Vec 反序列化无长度上界，超大 config 致 OOM
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-12
- 位置：`config.rs` ItemSpawnConfig（serde Deserialize）、`load_config`
- 现状：`rarity_weights: Vec<f32>` 由 serde_json 直填无限制，`load_config` 读文件无大小检查，10⁹ 元素 JSON 致 OOM。
- 更优解：`load_config` 先 `std::fs::metadata(path)` 检查文件 ≤1MB；或结合 OPT-L-04 的 `len()==3` 约束 + serde `deserialize_with` 提前截断。
- 验收：
  - [ ] 配置文件大小/rarity_weights 长度有上界

### OPT-R-10　Circle reserved zone radius 转 i32 后 pow(2) 对反序列化数据溢出
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-13（生产路径安全，仅 import 路径触发，与 OPT-R-07 联动）
- 位置：`terrain/carve.rs:123`（mark_reserved_zones）
- 现状：`(*radius as i32).pow(2)`，`radius: u32` 可序列化。`radius=2^31` → `as i32 = i32::MIN`，`pow(2)` debug panic / release wrap 为 0。
- 更优解：`i64::from(*radius) * i64::from(*radius)`，比较式 dx/dy 也转 i64。
- 验收：
  - [ ] radius_sq 用 i64，无溢出

### OPT-R-11　pcg_cli --config / --out 路径无限制，UE5 场景可能路径穿越
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ROB-14（上下文依赖）
- 位置：`bin/pcg_cli.rs:228`（load_config）、`main:145`（write out）
- 现状：`read_to_string(path)` / `write(out_path)` 直用用户路径，无规范化/白名单。若路径来自玩家可控输入（自定义配置、MOD），可 `../../` 越权读写。
- 更优解：若路径由引擎硬编码 → 文档标注即可；若来自外部 → `canonicalize` 后 `starts_with(allowed_base_dir)` 校验，至少限制 out_path 扩展名。
- 验收：
  - [ ] 路径来源已明确（文档或白名单校验二选一）

### OPT-R-12　ConfigDigest::from_config / seed_from_config 用 .expect() 库级 panic
- 严重度 LOW | 优先级 P3 | effort S | breaking 否（infallible 兜底方案） | breaksDeterminism 否　|　已跟踪 NEW-23
- 合并自：PCG-L-11 + API-11
- 位置：`digest.rs:41`、`digest.rs:69`
- 现状：`serde_json::to_string(config).expect(...)`，传入含 NaN f32 的未归一化 config 时 panic 而非 PcgError。两函数为 pub，可在 normalize() 前被调用。
- 更优解（择一）：① 改返回 `PcgResult<...>`，调用方已普遍用 `?`（breaking）；② 保持 infallible，内部对 f32 字段先做 NaN 校验（与 validate() 对齐）或 `unwrap_or_default` 兜底 + 注释；③ 最佳：手写字段级 fnv1a_64 组合替代 serde_json，彻底消除 serde 依赖与 panic（同时给 OPT-P-05 性能收益）。
- 验收：
  - [ ] from_config/seed_from_config 不再对调用者环境 panic

---

## 5. 性能与分配

> 提示：本主题多条经核验从 HIGH 降为 MEDIUM/LOW——PCG 是单次重计算、房间网格典型 ≤60×60，绝对开销有限。值得做（多为 S 级零风险），但**不构成紧急热点**，不要为此牺牲确定性。

### OPT-P-01　StableRng::derive() 每次调用做 Vec 堆分配（全管线最高频）
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-01
- 位置：`rng.rs:232-234`；附带 `chunked.rs:192/210/212` 的 format! String
- 现状：`self.seed.to_le_bytes().to_vec(); bytes.extend_from_slice(label.as_bytes())` 每次派生分配临时 Vec，仅用于一次哈希。20 房间楼层 60+ 次分配。
- 更优解：直接对 `self.seed.to_le_bytes().iter().chain(label.as_bytes())` 跑 FNV-1a，零中间 Vec（常数 OFFSET=0xcbf29ce484222325 / PRIME=0x100000001b3，**字节序与输出与现行完全一致**）。可加 `derive2(a:&str,b:&str)` 消灭分块 format! 分配。核验确认输出逐位相同，不破坏确定性。
- 验收：
  - [ ] derive() 无堆分配
  - [ ] 黄金测试通过（派生种子逐位不变）

### OPT-P-02　connect_all_doorways 对每个门口重启完整 BFS
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P1 | effort M | breaking 否 | breaksDeterminism 否
- 来源：PERF-03
- 位置：`terrain/connectivity.rs:104`
- 现状：循环内每门无条件 `reachable_from(grid, first)`（全 BFS + 新 HashSet），即使本次未雕刻也重算，O(D×W×H)。
- 更优解：循环外一次 `reachable_from(first)`，每次 carve 后从新雕刻入口做增量 flood 扩充 reachable。核验确认：雕刻严格单调（Wall→Floor），增量等价于在新 grid 上对 first 重跑 BFS，决策与输出 grid 逐位相同。配合 OPT-P-03 效果更佳。
- 验收：
  - [ ] BFS 总工作量降为 O(W×H)+O(雕刻格数)
  - [ ] 输出 grid 逐位不变

### OPT-P-03　flood_fill / reachable_from 用 HashSet<GridPoint>，应改平坦 Vec<bool> 位图
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P1 | effort M | breaking 否 | breaksDeterminism 否
- 来源：PERF-04
- 位置：`connectivity.rs:16/35-55/114-118`；`maze.rs:201`、`organic.rs:226`（含 maze.rs:202 的 `HashMap<GridPoint,GridPoint>` parent 表）
- 现状：网格访问标记用 HashSet（8 字节 key + 哈希碰撞 + rehash）。可用 `index=y*width+x` 的 Vec<bool> 替代，O(1) 数组访问，单 buffer 跨调用复用。
- 更优解：`flood_fill` 改收 `&mut Vec<bool>`（长度 W×H，`.fill(false)` 复位）；`summarize_connectivity` 复用同 buffer；maze/organic 的本地 HashSet/HashMap 一并换平坦 `Vec`/`Vec<u32>`，BFS 队列用 `VecDeque<u32>`。不访问 RNG，不破坏确定性。
- 验收：
  - [ ] 连通性 BFS 全部使用平坦位图
  - [ ] maze.rs parent 表也已平坦化
  - [ ] 连通性测试通过

### OPT-P-04　generate_spawns 对 terrains 做 O(R²) 线性扫描（String 比较）
- 严重度 LOW（原 HIGH，核验降级）| 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 合并自：PERF-02 + QC-16
- 位置：`spawn/mod.rs:77`、`spawn/mod.rs:129`
- 现状：每房间 `terrains.iter().find(|t| t.room_id==room.id)`（RoomId=String），O(R²) 堆字符串比较。R≤50 实测非瓶颈，但模式重复两处。
- 更优解：循环前建 `HashMap<&str,&Terrain>`，O(1) 查询。for 顺序不变 → RNG 派生顺序不变。
- 验收：
  - [ ] 两处（prod + debug）均改为哈希查找
  - [ ] spawn 输出顺序/结果不变

### OPT-P-05　generate() 对同一 config 连续两次 serde_json 序列化
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 合并自：PERF-05 + ARCH-10 + QC-13
- 位置：`generator.rs:54-57`、`chunked.rs:84-86`；`digest.rs:38-71`
- 现状：`seed_from_config` 与 `from_config` 各做一次 `serde_json::to_string`，两次字节相同。`matches()` 又序列化一次。两函数实现几乎完全重复。
- 更优解：加 `pub fn seed_and_digest_from_config(config) -> (u64, String)`（或 `from_config_both`），一次序列化返回二者；入口改 `let (s, digest)=...; let seed=request.seed.unwrap_or(s);`。`from_config` 内部直接调 `seed_from_config` 去重。（若采纳 OPT-R-12 方案③字段级哈希，可一并消除 serde 路径。）
- 验收：
  - [ ] 单次入口只序列化一次
  - [ ] from_config 复用 seed_from_config

### OPT-P-06　fill_chunk_details 用 Vec<String>::contains 过滤房间/锚点
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-06
- 位置：`chunked.rs:149`、`chunked.rs:157`
- 现状：`chunk.room_ids.contains(&room.id)`（Vec<String> 线性查找），O(M×P) 字符串比较。
- 更优解：filter 前建 `HashSet<&str>`，O(M+P)。
- 验收：
  - [ ] chunk 过滤改哈希集合

### OPT-P-07　compute_adjusted_weight 每次对每条规则 to_lowercase() 分配
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 是（GrammarRule 加字段/构造函数） | breaksDeterminism 否
- 来源：PERF-07
- 位置：`grammar/selector.rs:138`、`:150`
- 现状：每条规则每次 `rule.name.to_lowercase()`，主题循环内每 tag 再 `tag.to_lowercase()`，select 频繁调用时大量 String 分配。
- 更优解：`GrammarRule` 构造时预存 `name_lower`，`GrammarContext.theme_tags` 保持小写，比较时直接用预存字段。
- 验收：
  - [ ] select 路径零 to_lowercase 运行时分配

### OPT-P-08　maze get_unvisited_neighbors 每格分配 Vec<GridPoint>
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-08
- 位置：`terrain/maze.rs:146-170`（调用 `:123`）
- 现状：迷宫主循环每格调用一次，分配最多 4 元素 Vec 随即丢弃，32×32 房间 ~256 次。
- 更优解：返回 `([GridPoint;4], usize)` 栈数组 + 长度；调用方用 `random_range(0,cnt)` 取索引。RNG 调用次序不变。
- 验收：
  - [ ] get_unvisited_neighbors 零堆分配
  - [ ] 迷宫输出不变

### OPT-P-09　maze/organic 连通修复每门各新建 VecDeque+HashSet+HashMap
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-09
- 位置：`maze.rs:200-253`（connect_doorways_to_maze）、`organic.rs:299-350`（carve_path_to_reachable）
- 现状：每未连通门口新建三套集合做 BFS，D 门 = D 套分配。
- 更优解：三套集合提升到循环外，每轮 `.clear()` 复用；配合 OPT-P-03 将 HashSet 换平坦 bool vec。
- 验收：
  - [ ] BFS buffer 跨门口复用

### OPT-P-10　select_spaced_points（非 debug 路径）仍构建 rejection 字符串
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 否 | breaksDeterminism 否
- 来源：PERF-10
- 位置：`spawn/sampling.rs:91-125`
- 现状：生产路径最终走 `_tracked_excluding`，始终对每个被拒点 `format!` 一个 String 再整体丢弃，50×50 房间数百次无用分配。
- 更优解：拆两个内部实现——`_no_track`（生产，else 分支空）与 `_tracked`（debug/tracked）；生产路径零 rejection String。需测两路输出一致。
- 验收：
  - [ ] 生产路径不构建 rejection String
  - [ ] tracked/debug 路径行为不变

### OPT-P-11　sample_rarity_tier 每个 SpawnPoint 分配临时 Vec<f64>
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 合并自：PERF-11 + QC-15
- 位置：`spawn/items.rs:157-163`
- 现状：每次 `weights.iter().map(f64::from).collect::<Vec<f64>>()`（固定 3 元素），每 SpawnPoint 一次。
- 更优解：用栈数组 `let w=[f64::from(weights[0]),...]`；或把 `rarity_weights` 类型从 `Vec<f32>` 改 `[f64;3]` 从源头消除（配合 OPT-L-04 的 len==3 约束）。
- 验收：
  - [ ] sample_rarity_tier 无堆分配

### OPT-P-12　organic grid_b = grid_a.clone()，首次 CA step 立即全覆盖
- 严重度 LOW | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-12
- 位置：`terrain/organic.rs:71-78`
- 现状：`grid_b` 克隆内容在第一次 `apply_ca_step_into` 即被全覆盖，clone 无用。
- 更优解：改 `Grid2D::new(width,height,TileKind::Wall)`（任意默认值），免 clone。
- 验收：
  - [ ] grid_b 不再 clone
  - [ ] CA 输出不变

### OPT-P-13　多处地形策略两遍循环初始化网格（先全 Floor 再设边框 Wall）
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：PERF-15
- 位置：`carve.rs:58-65`、`pillar.rs:55-64`、`organic.rs:146-152`
- 现状：先 W×H 全写 Floor，再 W×H 遍历只改边框，两次扫描。
- 更优解：`Grid2D::new(w,h,Wall)` 后只对内部 `(1..h-1)×(1..w-1)` 写 Floor，一次迭代。可与 OPT-Q-03（提取 `init_room_grid`）合并落地。
- 验收：
  - [ ] 网格初始化单遍完成
  - [ ] 三处文件输出不变

### OPT-P-14　build_spawn_points 对同一 local_point 调用 world_grid_point 两次
- 严重度 NIT | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 合并自：PERF-14 + QC-10
- 位置：`spawn/items.rs:115-116`
- 现状：`grid_pos: world_grid_point(...)` 与 `world_transform: Some(grid_point_to_transform(world_grid_point(...)))` 重复计算。
- 更优解：提取 `let gp = world_grid_point(room, local_point);` 复用。（QC-06 合并后尤为突出。）
- 验收：
  - [ ] world_grid_point 每点只算一次

### OPT-P-15　ResultCache 每次查找 as_string() 分配 String，而 CacheKey 已 Hash+Eq
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ARCH-09
- 位置：`cache/store.rs`（get/contains/insert 均调 `key.as_string()`）
- 现状：内部 `HashMap<String,...>`，每次操作格式化 5 字段 String，而 CacheKey 已 derive(Hash,Eq)。
- 更优解：内部改 `HashMap<CacheKey,GenerationResult>`，直接用 CacheKey 为键；`as_string()` 仅留作日志/序列化。公共方法签名不变。
- 验收：
  - [ ] 缓存查找路径零 String 分配

### OPT-P-16　layout solver nudge_clear O(n²)（当前规模可接受）
- 严重度 LOW | 优先级 P3 | effort S（短期）/ L（长期）| breaking 否 | breaksDeterminism 否　|　**已跟踪 NEW-24**
- 来源：PERF-16
- 位置：`solver.rs:132-161`（nudge_clear）、`:164-176`（overlaps_any）
- 现状：`nudge_clear` 最多 placed.len()+4 次，每次 `overlaps_any` 遍历全部已放置房间 O(N²)；`inflated` 的 x 方向在 nudge 过程不变却每轮重建。N≤50 时最坏 2500 次 AABB 比较，**实测非瓶颈**。
- 更优解：短期——调用侧预计算 inflated x，循环内只更新 y；长期（N>100 经基准确认后）——interval tree / 排序 + sweep 降到 O(N log N)。
- 验收：
  - [ ] 短期：x 预计算落地（或确认当前规模无需动）
  - [ ] 长期重构以基准（N>100）为触发条件

---

## 6. 架构与 API 暴露面

> 多条命中 NEW-26（收窄暴露面）/ NEW-27（内部类型泄露）/ NEW-32（Box<dyn>→enum）/ NEW-34（Send+Sync）。这些是"已跟踪主题"的补充覆盖，本处给具体边界，不重复登记为新阻断。yang-pcg 是 workspace 内部 crate（非发布库），暴露面问题多为"未来重构会变 breaking"的设计债，严重度普遍 MEDIUM。

### OPT-A-01　pub mod layout 全量对外暴露（LayoutOutput / solve_layout / 三子模块）
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P1 | effort M | breaking 是 | breaksDeterminism 否　|　已跟踪 NEW-26/27（layout 漏网）
- 来源：API-01
- 位置：`lib.rs:70 pub mod layout`；`layout/mod.rs:4-6/15-16/23`
- 现状：layout 实现细节（房间边界、走廊算法）成为公开契约，重构即 breaking。所有消费者均为 crate 内 `crate::layout::...`，外部所需数据已经 GenerationResult 暴露。`#[non_exhaustive]` 已部分缓解。
- 更优解：`pub mod layout` → `pub(crate) mod layout`，`LayoutOutput`/`solve_layout`/三子模块同步降 `pub(crate)`。纯可见性变更，不触碰生成逻辑/RNG。
- 验收：
  - [ ] layout 模块对外不可见
  - [ ] crate 内编译通过，输出不变

### OPT-A-02　pub mod validation 泄露 7 个内部 pub fn + SpacingViolation/ExclusionViolation
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P1 | effort M | breaking 是 | breaksDeterminism 否　|　已跟踪 NEW-26/27
- 来源：API-02
- 位置：`lib.rs:66 pub mod validation`；`validation.rs` 的 validate_request(22)/validate_reachability(54)/validate_no_overlap(146)/validate_terrain_connectivity(212)/validate_spawn_spacing(369)/validate_result(491)/run_full_validation(615)；SpacingViolation(310)/ExclusionViolation(326)
- 现状：7 个管线内部函数 + 2 个聚合结构体对外可见，外部直接调用会得到与 generate() 不一致的验证语义。这些均为纯函数，误调不污染状态，主要风险是把内部承诺为契约。
- 更优解：`pub mod validation` → `pub(crate)`；7 函数与 2 violation 结构降 `pub(crate)`；**注意** `ValidationReport`/`ValidationItem` 是 `DebugBundle.validation_report` 字段类型，须在 lib.rs 单独 `pub use validation::{ValidationReport, ValidationItem}`（或移入 debug 模块），否则触发 "private type in public interface" lint（见 OPT-S-02）。
- 验收：
  - [ ] validation 内部函数/violation 不再对外
  - [ ] ValidationReport/ValidationItem 仍可达（随 DebugBundle）
  - [ ] 无 private-in-public lint

### OPT-A-03　SpawnOutput / SpawnOutputWithDebug / generate_spawns / min_cross_type_spacing 经 pub mod spawn 泄露
- 严重度 MEDIUM | 优先级 P1 | effort M | breaking 是 | breaksDeterminism 否　|　已跟踪 NEW-27（补 min_cross_type_spacing 遗漏项）
- 来源：API-14
- 位置：`spawn/mod.rs:20/57/65/114/30`
- 现状：SpawnOutput/WithDebug（NEW-27 已列），外加 `min_cross_type_spacing`（配置内部辅助）被 pub，`generate_spawns(_with_debug)` 与 PipelineBackend trait 方法形成重复入口，诱导绕过 backend 抽象。
- 更优解：`pub mod spawn` → `pub(crate)`；上述类型/函数全降 `pub(crate)`，外部只经 `MapGenerator::generate()` 取点位。
- 验收：
  - [ ] spawn 模块对外关闭，min_cross_type_spacing 收回

### OPT-A-04　TopologyResult::normalized 公开暴露内部类型 NormalizedConfig
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 是 | breaksDeterminism 否　|　已跟踪 NEW-27（chunked 未覆盖）
- 来源：ARCH-08
- 位置：`chunked.rs:32-49`（TopologyResult，pub use 至 lib.rs）
- 现状：`pub normalized: NormalizedConfig` 暴露内部配置类型（含 time_budget_ms / iteration_budget 内部预算字段），外部无理由读写。
- 更优解：字段降 `pub(crate)`，提供 `pub fn config(&self) -> &GenerationConfig`；或 TopologyResult 直接存 GenerationConfig，预算作为 `fill_chunk_details` 独立参数。
- 验收：
  - [ ] NormalizedConfig 不再经 TopologyResult 泄露

### OPT-A-05　ResultCache 与 MapGenerator 完全解耦，公共 API 形同虚设
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 否 | breaksDeterminism 否
- 来源：ARCH-03
- 位置：`cache/store.rs`（与 generator.rs 无调用关系）
- 现状：`pub mod cache` 暴露 ResultCache 但 generate() 不读写缓存，调用方需自行管理整个 cache 流程，造成"有缓存为何不用"的困惑。
- 更优解（择一）：① 为 MapGenerator 加可选 `cache: Option<ResultCache>` + `with_cache()`，generate() 内查/写（key 由 seed+config_digest 构成）；② 最小：doc 明确"调用方自管工具类，generate 不自动使用"；若定位为内部中间件则降 `pub(crate)`（呼应 NEW-26）。
- 验收：
  - [ ] cache 与 generator 的关系已明确（集成或文档或收窄）

### OPT-A-06　import_json 版本不兼容返回 PcgError::Export 而非 CorruptedData
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：ARCH-05
- 位置：`export/mod.rs:122-137`
- 现状：schema 主版本不匹配返回 `Export`(PCG-EXPORT-001)，语义应为 `CorruptedData`(PCG-CORRUPTED-001，且其 expected/actual_version 字段专为此设计未被用)。下游按错误码匹配会误判为序列化失败。
- 更优解：改用 `PcgError::corrupted_data_with_version(...)`，填 CURRENT_SCHEMA_VERSION / 导入版本；补测试断言 `error_code()=="PCG-CORRUPTED-001"`。非 breaking（签名不变）。
- 验收：
  - [ ] 版本不兼容返回 CorruptedData 变体

### OPT-A-07　PipelineBackend trait 缺失 Send + Sync
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否　|　关联 NEW-34（TerrainStrategy 已加，PipelineBackend 漏）
- 来源：ARCH-07
- 位置：`backend/mod.rs:42`、`select_backend` 返回 `Box<dyn PipelineBackend>`（:96）
- 现状：无 Send+Sync 约束，UE5 多线程流式加载下 `Box<dyn PipelineBackend>` 不能跨线程。TerrainStrategy 已加（strategy.rs:74），应对齐。TopDownBackend 是 ZST 天然满足，加约束零负担。
- 更优解：`pub trait PipelineBackend: Send + Sync`，可选地把 select_backend 返回 `Box<dyn PipelineBackend + Send + Sync>`。
- 验收：
  - [ ] PipelineBackend: Send + Sync
  - [ ] Box<dyn PipelineBackend> 可跨线程

### OPT-A-08　GenerationResult 双份 Room（topology.nodes 无 bounds / rooms 有 bounds）
- 严重度 MEDIUM（ARCH-02 核验 MEDIUM；API-10 核验 LOW）| 优先级 P2 | effort L | breaking 是 | breaksDeterminism 否
- 合并自：ARCH-02 + API-10
- 位置：`model/result.rs:14-35`；`solver.rs:179-188`（apply_room_bounds 克隆 nodes 写 bounds）；`validation.rs:492`（断言两者等长）
- 现状：`topology.nodes`（bounds=None）与 `rooms`（bounds=Some）存同批房间不同状态，`Room.bounds: Option` 被当生命周期标志。消费方误用 `topology.nodes` 查 bounds 得 None 无编译保护；`validate_no_overlap` 对 bounds=None 静默 continue（误传 topology.nodes 会"什么都没校验却 Ok"）。**核验关键纠偏**：分块模式下 `topology.nodes`=整层全部房间、`rooms`=当前 chunk 子集（`chunked_tests.rs:180` 断言 `rooms.len() < topology.nodes.len()`），二者承载不可折叠的不同职责。
- 更优解：**优先方案 B**——加 `pub fn room_by_id(&self, id)->Option<&Room>` 封装正确查找路径，并在 `RoomGraph.nodes` / `result.rooms` 字段加交叉引用 doc（说明全层 vs 分块模式语义、bounds 差异）。**方案 A（把 nodes 改纯 ID/精简结构体）核验判为不成立**——分块模式 topology.nodes 需保留完整 room_type/depth/theme_tags 等元数据，改纯 ID 会迫使全部消费者二次查找。任何结构性拆分按 breaking 处理。
- 验收：
  - [ ] 提供 room_by_id 访问器
  - [ ] 两字段 doc 说明全层/分块语义差异
  - [ ] 不采用"nodes 改纯 ID 列表"方案

### OPT-A-09　select_backend 每次为 ZST 创建 Box<dyn>，引入不必要 vtable
- 严重度 LOW | 优先级 P3 | effort M | breaking 否（Backend 类型可不导出）| breaksDeterminism 否　|　关联 NEW-32（strategy 已 enum 化，backend 漏）
- 来源：ARCH-11
- 位置：`backend/mod.rs:96-98`（被 generator.rs:62、chunked.rs:90 调用）
- 现状：唯一 backend 仍包成 `Box<dyn PipelineBackend>`，每次方法调用走 vtable 间接。`_config` 带下划线说明无分支逻辑。PCG 单次重计算，开销极小，但属提前引入的复杂度。
- 更优解：引入 `enum Backend { TopDown(TopDownBackend) }` 静态派发，`impl PipelineBackend for Backend { match ... }`，待第二个 backend 需要 object safety 再回 Box<dyn>。
- 验收：
  - [ ] backend 走静态派发（或确认当前开销可忽略而暂缓）

---

## 7. SemVer 与发布卫生

### OPT-S-01　WeightedRuleSelector 在根 re-export 但其 select() 必需的 StableRng 未 re-export
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：API-03
- 位置：`lib.rs:91`、`rng.rs:148`、`selector.rs:89`
- 现状：根重导出了 WeightedRuleSelector/GrammarContext/GrammarRule，却没导出 `StableRng`，外部须自行 `use yang_pcg::rng::StableRng`，无导航提示。
- 更优解：`lib.rs` 加 `pub use rng::StableRng;`。
- 验收：
  - [ ] StableRng 在根可达

### OPT-S-02　ValidationReport / ValidationItem 出现在公开 DebugBundle 却不在根 re-export
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：API-04（与 OPT-A-02 联动）
- 位置：`lib.rs:87`（DebugBundle）；`debug/report.rs:103`；`validation.rs:569`
- 现状：DebugBundle.validation_report: Option<ValidationReport>，但类型仅 `validation::` 路径可达，导入不对称。
- 更优解：`pub use validation::{ValidationReport, ValidationItem}`（或移入 debug 模块随 DebugBundle 一起导出）。与 OPT-A-02 一并落地。
- 验收：
  - [ ] ValidationReport/ValidationItem 与 DebugBundle 同前缀可达

### OPT-S-03　约束 API 所需 GridPoint / WorldPoint / CardinalDir 未在根 re-export
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：API-05
- 位置：`lib.rs`（无 model::geometry 重导出）；`model/request.rs:110/157`、`model/geometry.rs:7/22`
- 现状：`AnchorConstraint::with_target_grid_pos(GridPoint)`、`ExclusionZoneConstraint::new(GridPoint,GridPoint)`、`RuntimeContext::focus_position(WorldPoint)` 的参数类型只能从 `model::geometry::` 导入。
- 更优解：`pub use model::geometry::{GridPoint, WorldPoint, CardinalDir};`（CardinalDir 因 GrammarContext::facing 用到）。
- 验收：
  - [ ] 三个几何类型在根可达

### OPT-S-04　WeightedRuleSelector 标 #[non_exhaustive] 却无 Default/new，外部无法构造
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：API-06
- 位置：`grammar/selector.rs:70-71`（ZST + non_exhaustive），lib.rs:91 重导出
- 现状：non_exhaustive ZST 禁止外部 `WeightedRuleSelector {}` 字面量构造，又无 Default/new，重导出却无法实例化。
- 更优解：`impl Default`（最简）或 `pub fn new() -> Self`；或彻底改 `select` 为自由函数消除实例化需要。
- 验收：
  - [ ] 外部可构造 WeightedRuleSelector

### OPT-S-05　Branch.purpose: String 字符串化，文档列了 4 值但无编译期保证
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 是 | breaksDeterminism 否
- 来源：API-07
- 位置：`model/room.rs:115`
- 现状：`pub purpose: String`（reward/shop/event/shortcut），外部匹配靠手写字符串，拼错静默通过，扩展无 match exhaustiveness 保护。
- 更优解：`#[non_exhaustive] pub enum BranchPurpose { Reward, Shop, Event, Shortcut }` + serde `rename_all="snake_case"` 保持 JSON 兼容。model 层变更，breaking。
- 验收：
  - [ ] purpose 改枚举，JSON schema 不变

### OPT-S-06　ChunkId 类型别名在 chunk.rs 和 request.rs 各自独立定义
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：API-08
- 位置：`model/chunk.rs:9`、`model/request.rs:85`
- 现状：两处各 `pub type ChunkId = String`，值兼容但是两个不同路径项，IDE 补全混乱、未来各自演化有不一致风险。
- 更优解：集中在 `model/mod.rs` 定义一处，另一处 `use` 引用。
- 验收：
  - [ ] ChunkId 单一定义来源

### OPT-S-07　ResultMetadata.config_digest: String 与 ConfigDigest 类型不一致，丢失 matches() 能力
- 严重度 MEDIUM | 优先级 P2 | effort M | breaking 是 | breaksDeterminism 否
- 来源：API-09
- 位置：`model/result.rs:44`；`generator.rs:57`；`digest.rs:103`
- 现状：crate 导出 ConfigDigest newtype（带 matches()），但结果里存原始 String，用户想 `digest.matches(config)` 须手动包裹。
- 更优解：字段类型改 `ConfigDigest`（实现 Serialize/Deserialize 保持十六进制串 JSON 不变）；或保留 String 另加 `config_digest_typed()` 访问器。
- 验收：
  - [ ] 可直接对结果调用 matches（或提供访问器），JSON 不变

### OPT-S-08　GenerationConfig::merge() 名实不符（实为全量覆盖）
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否（重命名 + 保留 deprecated alias）| breaksDeterminism 否
- 合并自：ARCH-06 + API-12
- 位置：`config.rs:184-218`
- 现状：doc 说"层级合并"，实现是无条件全量覆盖 other 全部字段（仅 other.theme_tags 为空时保留 self），等价 `other.clone()` + 特例。调用方期望增量合并会丢失未在 other 修改的配置。
- 更优解：重命名 `override_with()` / `apply_override()`，doc 明确"全量覆盖"，保留 `merge` 为 deprecated alias；若需真正字段级合并，引入 `Option` 包裹的 `ConfigPatch` + serde default（更大工作量）。
- 验收：
  - [ ] 方法名/文档反映"全量覆盖"语义
  - [ ] 旧 merge 保留为 deprecated 平滑迁移

### OPT-S-09　RangeU16::validate 是 pub 但接受内部字段路径字符串
- 严重度 LOW | 优先级 P3 | effort S | breaking 是 | breaksDeterminism 否
- 来源：API-13
- 位置：`config.rs:269`
- 现状：`pub fn validate(&self, field_name: &str)` 的 field_name 用于内部错误路径，对外部调用者无意义（随手传串）。
- 更优解：降 `pub(crate)`；外部需要则提供无参 `is_valid()` / `check() -> Result<(),String>`。
- 验收：
  - [ ] RangeU16::validate 不再泄露 field_name 概念

### OPT-S-10　import 仅校验 schema 主版本，不检查 algorithm_version
- 严重度 LOW | 优先级 P3 | effort M | breaking 否 | breaksDeterminism 否
- 来源：API-15
- 位置：`export/mod.rs:111-140`、`export/binary/mod.rs:146-238`；`result.rs:48`、`generator.rs:175`
- 现状：只查 schema_version 主版本，不查 algorithm_version。算法修复后旧缓存结果可被新版本无警告导入当作最新，破坏 seed 复现预期。
- 更优解：import 增加 algorithm_version 检查，不匹配则返回警告（可 `Result<(Result, Option<Warning>),_>` 或 PcgError::Warning 变体）；文档给出 schema_version（wire 格式）vs algorithm_version（相同 seed 可能不同结果）的兼容矩阵。
- 验收：
  - [ ] import 对 algorithm_version 不匹配至少给出提示
  - [ ] 文档说明两类版本语义

### OPT-S-11　StableRng impl 公开 rand::TryRng，与上游版本强耦合
- 严重度 LOW | 优先级 P3 | effort M | breaking 是 | breaksDeterminism 否
- 来源：API-16
- 位置：`rng.rs:510-524`；`Cargo.toml:43`（rand = "0.10.1"）
- 现状：`impl TryRng for StableRng` 使外部可把 StableRng 当通用 rand RNG，绕过 derive 设计，且 rand 0.11 接口变动会破坏 yang-pcg 公开接口。
- 更优解：删除 TryRng 公开实现，或经内部包装类型仅在 `pub(crate)` 范围适配 rand 生态；StableRng 公开 API 只保留自有方法（from_seed/derive/random_range/choose/shuffle 等）。
- 验收：
  - [ ] StableRng 不再对外透传 rand trait

### OPT-S-12　generate_spawns 接受 &mut StableRng 但只调 &self 的 derive()，签名误导
- 严重度 NIT | 优先级 P3 | effort S | breaking 是（trait 方法签名）| breaksDeterminism 否
- 来源：PCG-L-10
- 位置：`spawn/mod.rs:65-104/114-173`；PipelineBackend::generate_spawns 同
- 现状：签名 `rng: &mut StableRng` 但函数体只调 `rng.derive(...)`（取 &self），父 RNG 返回后状态不变，调用方无法从签名得知。
- 更优解：改 `rng: &StableRng`，同步改 PipelineBackend trait 与 TopDownBackend 实现；暂不改则 doc 注明父 RNG 不被消耗。下个 minor 做。
- 验收：
  - [ ] generate_spawns 签名为不可变引用（或文档已注明）

---

## 8. 测试与确定性回归

> 命中 NEW-20（DefaultHasher 已修，补稳定性锚点）/ NEW-22（NaN 权重）/ NEW-28（三模式 RNG 标签无回归测试）。当前测试在"同进程确定性"覆盖充分，缺口在**跨版本黄金回归**、**三模式覆盖**、**边界拒绝**、**CLI**。

### OPT-T-01　黄金样本测试无硬编码期望值，无法检测跨 commit 算法漂移
- 严重度 **HIGH** | 优先级 P0 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-28
- 合并自：T-01 + PCG-L-08
- 位置：`tests_task26/golden_sample_tests.rs`（test_golden_sample_seed_42 / _12345，48-167 行）
- 现状：只验证"同进程两次调用一致"（intra-session），未固化哈希常量（行 66-67 注释写"记录哈希后续对比"但从未落地）。任何改输出但自洽的提交（RNG 标签重命名、阶段调序、依赖升级）都通过测试，无跨版本警报。
- 更优解：增加 `const GOLDEN_SEED42_ROOM_HASH: u64 = 0x...;` 等常量（首次运行记录、人工审阅后固化），断言对象从"第二次运行"改为常量，保留两次互比作为 intra-session 验证。三模式各一组（配合 OPT-T-02）。
- 验收：
  - [ ] 至少 OfflineFullFloor seed-42/12345 固化期望哈希
  - [ ] 改变输出的提交会使测试 fail 并提示更新常量

### OPT-T-02　黄金测试仅覆盖 OfflineFullFloor，RuntimeChunked / HybridPrecompute 无固定回归
- 严重度 MEDIUM（原 HIGH，核验降级）| 优先级 P0 | effort M | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-28
- 来源：T-02
- 位置：`golden_sample_tests.rs` 全用 `GenerationConfig::default()`（OfflineFullFloor）
- 现状：分块/混合路径有独立 RNG 标签体系（rng.rs:50-87），但 golden_sample_tests.rs 零覆盖。核验纠偏：`chunked_tests.rs` 已有功能等价的 intra-session 确定性测试（test_runtime_chunked_determinism 等），故三模式当前防护强度一致（都缺固化常量），非"某模式完全无测试"。
- 更优解：新增 `test_golden_runtime_chunked_seed_42()` / `test_golden_hybrid_seed_42()`，用对应 config 固化 room_hash/terrain_count/item_spawn_hash；HybridPrecompute 跨两阶段哈希（先 topo layout.rooms，再合并各 chunk fill 的 terrains）。
- 验收：
  - [ ] 三模式各有固化期望哈希的黄金测试

### OPT-T-03　属性测试 arb_generation_config 硬编码 OfflineFullFloor，两分块模式无 proptest
- 严重度 **HIGH** | 优先级 P1 | effort M | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-28
- 来源：T-03
- 位置：`tests_task27/property_tests.rs:77`（hardcode OfflineFullFloor）
- 现状：6 个 proptest 全走 OfflineFullFloor。RuntimeChunked 有独立 early-return + `ValidationScope::Chunk` + 预算 partial-result 逻辑；HybridPrecompute 两阶段 API 均未被属性测试覆盖。**核验前置条件**：RuntimeChunked 须 `capability_flags.runtime_chunked=true` + `runtime_context.is_some()`，HybridPrecompute 须 `hybrid_precompute=true`，否则在 validate_request 阶段就 Err，覆盖不到目标路径。
- 更优解：加 `arb_generation_mode()`（OfflineFullFloor 0.6 / RuntimeChunked 0.2 / HybridPrecompute 0.2），按模式自动填 capability_flags / runtime_context；RuntimeChunked 封装 `generate_chunk()`，HybridPrecompute 封装 `generate_topology_only` + 逐 chunk `fill_chunk_details` 再验不变量。cases 保持 50。
- 验收：
  - [ ] proptest 覆盖三种模式且正确填充 capability/runtime_context
  - [ ] 三模式的可达性/无重叠/连通/间距不变量均被属性测试守护

### OPT-T-04　seed=None（config 派生种子）无集成级确定性测试
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-04
- 位置：`golden_sample_tests.rs`；`generator.rs:53-55`（unwrap_or_else seed_from_config）
- 现状：所有测试显式传 `seed: Some(x)`，None 分支端到端不可见。若 seed_from_config 或 serde 字段顺序变化，seed=None 结果静默漂移。
- 更优解：加 `test_golden_seed_none_stability()`（两次 None+同 config 一致 + 固化哈希）；加 `test_seed_none_equals_seed_from_config()`（None 与 `Some(seed_from_config(config))` 结果相同）。
- 验收：
  - [ ] seed=None 路径有黄金 + 等价性测试

### OPT-T-05　debug 隔离测试未比较地形网格 tiles.data（最高风险盲区）
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-05
- 位置：`tests_task26/debug_isolation_tests.rs`（11-271 行）
- 现状：四个隔离测试覆盖 rooms/corridors/spawns/topology，**唯独不比较 terrains（tiles.data/door_anchors）**。地形是最复杂阶段，debug 走 `generate_spawns_with_debug`（generator.rs:92-102），若该路径 RNG 消耗与非 debug 不同会影响 spawn；现有 spawn 测试只比数量+位置，不比 rarity/metadata。
- 更优解：加 `test_debug_toggle_terrains_identical()`（比较 tiles.data/grid_size/connectivity_summary）；spawn 补 `metadata.spawn_tag`、corridor 补 path_points 比较。
- 验收：
  - [ ] debug on/off 下 terrains tiles.data 逐格一致
  - [ ] spawn metadata 也纳入比较

### OPT-T-06　pcg_cli 无单元测试，parse_args / load_config 无覆盖
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-06
- 位置：`bin/pcg_cli.rs:164-213`（parse_args）、`:224-233`（load_config）
- 现状：UE5 集成关键边界，无效 --seed、缺 --out、未知参数、--format 非法、--config 不存在/非法 JSON 的退出码与错误均无断言。
- 更优解：在 bin 内加 `#[cfg(test)] mod tests` 直接调函数（无需 Command）：正常 3 路 + 5 错误路径；load_config 测 None→Default、不存在路径→Err、临时文件 invalid JSON。无 I/O 副作用，可作 lib test。
- 验收：
  - [ ] parse_args/load_config 正常 + 错误路径均有测试

### OPT-T-07　边界/非法 config 拒绝测试缺失（NaN density / 空 rarity_weights / 越界 ratio）
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-22
- 来源：T-07（与 OPT-L-02 / OPT-L-04 联动）
- 位置：`config.rs:120-182`（TerrainConfig::validate / ItemSpawnConfig::validate）
- 现状：无测试验证子校验拒绝：NaN/Inf obstacle_density、越界 min_walkable_ratio、空/全零/负 rarity_weights、count min>max。这些值可经 deserialize 进管线在加权选择处 NaN 传播。
- 更优解：在 config 测试模块加参数化拒绝测试 `assert!(bad_config.normalize().is_err())`；若 validate 当前不检查则同步补校验（与 OPT-L-02/OPT-L-04 一起）。
- 验收：
  - [ ] NaN/越界/空/负权重 config 在边界被拒绝（有测试）

### OPT-T-08　chunked_tests 确定性验证仅比数量+坐标，未做 JSON 全量等价
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-08
- 位置：`chunked_tests.rs:453-492`、`:379-450`
- 现状：只比 rooms.len/terrains.len/item_spawns 的 grid_pos+room_id，遗漏 tiles.data/room.bounds/corridor path；回退路径（terrain:fallback:{id}）输出变化无法被数量比较检测。OfflineFullFloor 的 `prop_deterministic_generation` 已用 `serde_json` 全量等价（149-152）。
- 更优解：两测试末尾加 `assert_eq!(serde_json::to_string(&r1)?, serde_json::to_string(&r2)?)`；HybridPrecompute 对每个 ChunkDetailResult 同样做 JSON 等价。
- 验收：
  - [ ] 分块确定性测试做 JSON 全量等价断言

### OPT-T-09　StableRng 无固定输出锚点测试，rand_pcg 升级会静默破坏确定性
- 严重度 MEDIUM | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-20（DefaultHasher 已修，补锚点）
- 来源：T-09
- 位置：`rng.rs` 的 `#[cfg(test)] mod tests`（527-793）
- 现状：只验证"同 seed 两次一致"，无固定期望值。`fnv1a_64(b"topology")`、`from_seed(42).random::<u32>()`、`derive("topology")` 子种子均无锚点，跨依赖版本漂移不可感知。
- 更优解：加 `test_fnv1a_stability`（`assert_eq!(fnv1a_64(b"topology"), 0x...)`）、`test_seed_from_u64_stability`、`test_derive_stability`，首次运行固化常量。
- 验收：
  - [ ] FNV/from_seed/derive 均有固定期望值锚点测试

### OPT-T-10　WeightedRuleSelector 无 NaN base_weight 测试与确定性固定测试
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否　|　已跟踪 NEW-22
- 来源：T-10
- 位置：`grammar/selector.rs:89-127`（select）、`:132-174`
- 现状：有 intra-session test_select_deterministic 但无固化期望索引；无 NaN/0.0/-1.0 base_weight 行为测试；无多规则权重分布 proptest。
- 更优解：加 `test_select_nan_weight_returns_err`、`test_select_negative_weight_treated_as_zero`、`prop_select_deterministic`（arb rules + 固定 seed 两次一致）。与 OPT-L-05 联动。
- 验收：
  - [ ] NaN/负权重行为有测试
  - [ ] select 有 proptest 确定性覆盖

### OPT-T-11　geometry 不变量测试缺溢出/退化（min==max）场景
- 严重度 LOW | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-11
- 位置：`model/__tests__/geometry_test.rs:28-48`
- 现状：只测正常尺寸（10×20）。缺零面积 `RoomBounds{min:(0,0),max:(0,0)}`（width()=0）、i32::MAX 坐标溢出、奇数宽 center() 舍入锚定。solver/ue adapter 直接基于 bounds.width()。
- 更优解：加 `test_room_bounds_zero_area`、`test_room_bounds_center_odd_size`（固化期望）、`test_room_bounds_large_coords`；可加 GridPoint 算术溢出属性测试。
- 验收：
  - [ ] geometry 退化/溢出场景有测试

### OPT-T-12　export 往返测试无跨 schema 版本兼容性 proptest
- 严重度 LOW | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-12
- 位置：`export/__tests__/mod.rs:279-288`
- 现状：只测硬编码 "2.0.0" 不兼容 / "1.2.3" 兼容，无任意 semver proptest、无格式错误版本（"abc"/"1.2"/"1.2.3.4"）。binary 侧同。
- 更优解：`prop_schema_version_compat`（随机主/次版本，主版本相同兼容、不同拒绝）+ `test_import_json_malformed_version`；binary 对应。
- 验收：
  - [ ] schema 版本兼容性有 proptest + 畸形版本测试

### OPT-T-13　proptest cases=50 对种子空间覆盖低，room_count 上限仅 12
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-13
- 位置：`property_tests.rs` 各 `ProptestConfig::with_cases(50)`（94/161/196/232/268/315）
- 现状：50 cases 适合开发，但 arb_generation_config 的 room_count 上限 12，生产可达 50+，大图下 solver O(n²) 与 terrain 回退路径未被属性测试覆盖。
- 更优解：用 `PROPTEST_CASES` 环境变量（proptest 原生支持）在 CI 慢测试设 500，arb_generation_config room_count 上限扩到 50；日常保持 50 无需改码。
- 验收：
  - [ ] CI 慢测试矩阵覆盖大 room_count + 更多 cases

### OPT-T-14　golden_sample_json_roundtrip 未验证 terrains / enemy_spawns 坐标，弱于 export 一致性测试
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：T-14
- 位置：`golden_sample_tests.rs:169-207`
- 现状：只比 len + 逐房间 id/room_type，未比 terrains tiles.data、item/enemy_spawns 的 grid_pos/kind，而 export 的 test_consistency_terrains_roundtrip 已验地形数据。golden 应更强而非更弱。
- 更优解：补 terrains tiles.data zip 比较 + spawn grid_pos 比较，与 export consistency 覆盖范围对齐。
- 验收：
  - [ ] golden roundtrip 覆盖范围 ≥ export consistency

---

## 9. 代码质量（死代码 / 重复 / 命名）

### OPT-Q-01　level_graph.rs 是孤儿文件，完全不在模块树中
- 严重度 **HIGH** | 优先级 P1 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-01（核验：lib.rs 无 `pub mod level_graph`，grep 零命中，codegraph 无调用方）
- 位置：`crates/yang-pcg/src/level_graph.rs`（整文件 1-47）
- 现状：4 个类型（RoomType/RoomNode/Edge/LevelGraph）对编译器不可见，其 RoomType（7 变体、仅 `#[derive(Clone, Copy, Debug)]`）是 `model/room.rs:50`（10 变体、完整 derive：Debug/Clone/Copy/PartialEq/Eq/Hash/Serialize/Deserialize，已 re-export）的过时副本，持续造成混淆。
- 更优解：直接删除该文件，无副作用。
- 验收：
  - [ ] level_graph.rs 已删除
  - [ ] 编译/测试全量通过

### OPT-Q-02　地形策略 bounds 提取样板在 4 个文件完全重复
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-02
- 位置：`open_arena.rs:42-52`、`pillar.rs:42-52`、`organic.rs:48-58`、`maze.rs:41-51`
- 现状：10 行 bounds 提取 + 零尺寸校验在 4 策略逐字重复，新策略须再手写。
- 更优解：`terrain/carve.rs` 加 `pub(crate) fn extract_room_bounds(room)->PcgResult<(RoomBounds,u32,u32)>`，各策略调用。
- 验收：
  - [ ] 4 策略复用 extract_room_bounds

### OPT-Q-03　墙体边框绘制循环在 3+ 策略重复
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-03（与 OPT-P-13 同区域，可合并落地）
- 位置：`carve.rs:59-65`、`open_arena.rs:58-64`、`pillar.rs:57-64`、`organic.rs:82-88`
- 现状：边框初始化为 Wall 的嵌套循环多处内联重复。
- 更优解：`terrain/grid.rs` 加 `pub(crate) fn init_room_grid(width,height)->Grid2D<TileKind>`（按 OPT-P-13 单遍：全 Wall + 内部 Floor），替换各处组合。
- 验收：
  - [ ] 各策略复用 init_room_grid（输出不变）

### OPT-Q-04　门锚点过滤与 Doorway 标记在 3 个文件重复
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-04
- 位置：`open_arena.rs:67-74`、`pillar.rs:66-74`、`maze.rs:56-68`
- 现状：anchor 过滤 + to_local + set Doorway 模式逐字重复，maze 还带额外 `Vec<GridPoint>` 收集。
- 更优解：`terrain/grid.rs` 加 `pub(crate) fn mark_doorways(tiles, room, anchors, origin)->Vec<GridPoint>`（返回局部坐标供 maze/organic 用）。
- 验收：
  - [ ] 各策略复用 mark_doorways（输出不变）

### OPT-Q-05　world_grid_point / grid_point_to_transform 在 items.rs 与 enemies.rs 完全重复
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-06
- 位置：`spawn/items.rs:174-196`、`spawn/enemies.rs:262-284`
- 现状：两份完全相同私有函数（注释自述"与 items.rs 一致"）。
- 更优解：移至 `spawn/mod.rs`（或新建 `spawn/utils.rs`）声明 `pub(crate)`，两处调用。落地后顺带处理 OPT-P-14。
- 验收：
  - [ ] 两函数单一来源

### OPT-Q-06　carve_room_terrain（NormalizedConfig 变体）是死代码且静默丢弃错误
- 严重度 MEDIUM | 优先级 P2 | effort S | breaking 是 | breaksDeterminism 否
- 来源：QC-07
- 位置：`terrain/carve.rs:14-21`
- 现状：`pub fn carve_room_terrain` 无任何调用方，且把 `PcgResult<Terrain>` 转 `Option` 静默丢错。活跃的是 `carve_room_terrain_with_config`。
- 更优解：删除该函数；若将来需 NormalizedConfig 公共变体应返回 `PcgResult` 而非 `Option`。
- 验收：
  - [ ] 死函数已删除

### OPT-Q-07　enemies.rs 两个未被调用的公共包装函数
- 严重度 LOW | 优先级 P3 | effort S | breaking 是 | breaksDeterminism 否
- 来源：QC-08
- 位置：`spawn/enemies.rs:28-35`、`:87-94`
- 现状：`generate_enemy_spawns_for_room(_tracked)` 仅委托 `_excluding`（occupied=[], spacing=0），grep 确认无调用方，属"对称性存根"制造 API 噪声。
- 更优解：删除两者；如需简便 API 用文档说明"空 occupied 等价于不传"。
- 验收：
  - [ ] 未调用的包装函数已删除

### OPT-Q-08　策略内部冗余 summarize_connectivity 调用（4 处，结果被兜底覆盖）
- 严重度 LOW（原 QC-05 HIGH，核验大幅降级）| 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 合并自：QC-05（缩减范围）+ QC-11
- 位置：`open_arena.rs:112`、`organic.rs:111`、`maze.rs:77`、`maze.rs:84`
- 现状：`terrain/mod.rs:65` 在每策略返回后无条件 `repair_terrain_connectivity`（含一次 summarize），故上述 4 处 `summarize_connectivity` 结果必被覆盖，是纯计算浪费。**核验关键纠偏**：原方案"删除 3 个策略内部 BFS 修复函数（~300 行）"**不可执行**——内部修复（BFS 最短路 / 接入迷宫通道）与兜底 `connect_all_doorways`（L 形）算法不同，删除会改变 Floor 瓦片集合 → 改 spawn 位置 → 破坏黄金测试。它们是质量优化（更短/更自然路径），兜底是保险，二者**协作而非替代**，**保留**。
- 更优解：仅删除上述 4 处冗余 `summarize_connectivity` 调用（~4 行）；maze.rs 统一在返回前调用一次。**不动**内部修复函数本身。
- 验收：
  - [ ] 4 处冗余 summarize_connectivity 已清理
  - [ ] 内部 BFS 修复函数保留，tile/spawn 输出逐位不变
  - [ ] 黄金测试通过

### OPT-Q-09　ConnectivitySummary::all_doors_connected 字段名语义与实现不符
- 严重度 LOW | 优先级 P2 | effort M | breaking 是 | breaksDeterminism 否
- 来源：QC-12
- 位置：`terrain/connectivity.rs:28`
- 现状：`all_doors_connected: connected_region_count <= 1` 实际含义是"所有可通行瓦片同属一连通分量"，与"所有 Doorway 互相可达"不等价（孤立 Floor 岛会误报）。
- 更优解：重命名 `is_fully_connected`（或 `single_walkable_region`），更新文档；如需真正"门口互达"另加 `all_doorways_connected`（对所有 Doorway 做可达性检查，复用 reachable_from）。pub struct 字段重命名 = breaking。
- 验收：
  - [ ] 字段名反映真实语义
  - [ ] 如需"门口互达"语义则单列新字段

### OPT-Q-10　config.rs normalize 中 min>max 校验模式重复 4 次
- 严重度 LOW | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-14
- 位置：`config.rs:73-138`（room_count / critical_path_length / branch_count / dead_end_count）
- 现状：结构相同的 min>max 校验逐字重复 4 次，每加 RangeU16 字段都要手写。
- 更优解：加私有 `fn validate_range(range, field, label)->PcgResult<()>`，normalize 改为 `validate_range(self.room_count, "room_count", "房间数量")?;`。
- 验收：
  - [ ] 4 处校验复用 validate_range

### OPT-Q-11　rng.rs derive() 文档中的派生标签示例已过期
- 严重度 NIT | 优先级 P3 | effort S | breaking 否 | breaksDeterminism 否
- 来源：QC-17
- 位置：`rng.rs:207-213`
- 现状：docstring 示例写 `"spawn:items"` / `"spawn:enemies"`，实际调用点（spawn/mod.rs:81-82）用 `"items:{id}"` / `"enemies:{id}"`，与文件顶部标签契约（rng.rs:44-47）矛盾。
- 更优解：docstring 改为 `"items:{room_id}"` / `"enemies:{room_id}"`，注明带房间 ID 后缀。
- 验收：
  - [ ] derive 文档示例与实际调用点一致

---

## 10. 建议落地顺序

> 原则：先消除"会崩/会失效/会被攻击"的真实风险（不改确定性），再做零风险清理，最后做需要新 major 的破坏性结构调整。

**第 1 批 — 安全/正确性急修（HIGH，S，不破坏确定性）**
1. OPT-R-01 间距 i32 溢出 → i64 + config 上界
2. OPT-R-02 房间尺寸/room_count 上界（OOM 防护，breaking 但仅拒绝病态配置）
3. OPT-R-03 障碍放置 max_attempts 上限（方案 B）
4. OPT-D-01 HybridPrecompute 经 generate() 显式 Err / 文档
5. OPT-Q-01 删除 level_graph.rs

**第 2 批 — 黄金回归与测试网（守住确定性契约的护栏，先于任何重构）**
6. OPT-T-01 / OPT-T-09 固化 OfflineFullFloor + RNG 锚点期望哈希
7. OPT-T-02 / OPT-T-03 三模式黄金 + 属性测试
8. OPT-T-05 / OPT-T-07 / OPT-T-06 debug-terrain 隔离、config 拒绝、CLI 测试
   （此批完成后，后续任何性能/重构改动都有黄金测试兜底）

**第 3 批 — 其余鲁棒性与逻辑修复（MEDIUM/LOW，多 S）**
9. OPT-R-04~12（溢出/OOM/import 校验/路径）
10. OPT-L-01~05（corridors 守卫、负权重、分支回退、tier 降级、NaN 诊断）
11. OPT-Q-08 清理 4 处冗余 summarize_connectivity（**严禁**删内部修复函数）

**第 4 批 — 性能与分配（确认黄金测试通过后逐条做，多为逐位等价）**
12. OPT-P-01/02/03（derive 零分配、BFS 增量、平坦位图）—— 注意每条做完跑黄金
13. OPT-P-04~16 其余分配/复用优化

**第 5 批 — 代码质量去重（零风险清理）**
14. OPT-Q-02~07、OPT-Q-10/11（提取共享辅助、删死代码、去重）

**第 6 批 — API 收窄与命名（多为 breaking，集中到一个 major 版本统一发布）**
15. OPT-A-01~04（pub(crate) 收窄 layout/validation/spawn/TopologyResult）
16. OPT-A-05~09、OPT-S-01~12（缓存集成、错误变体、Send+Sync、根重导出、枚举化、merge 重命名、TryRng 收回）
17. OPT-Q-09（ConnectivitySummary 字段重命名）、OPT-A-08（双份 Room：优先 room_by_id + 文档，慎用类型拆分）

**第 7 批 — 确定性破坏性变更（独立 major，需迁移说明 + 全量黄金更新；非必要不做）**
18. OPT-D-04（rng.sample 部分 shuffle）—— 仅在确认无生产路径调用且接受黄金更新时
19. OPT-D-02（from_seed_bytes 统一）—— 因生产未调用，可随便利窗口处理

> 每批结束统一跑 `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test --lib -p yang-pcg`，确保 309 passed / 0 ignored、clippy 干净后再进入下一批。
