# 实现计划：yang-pcg UE5 Roguelike 地图生成算法库

## 概述

本文档定义 `yang-pcg` 的实现任务列表，用于把 [requirements.md](requirements.md) 和 [design.md](design.md) 落地为可交付代码。

本版任务文档基于当前实现状态重新整理，反映已完成的基础架构和 MVP 主链路，以及后续待完善的工作。

## 里程碑

### M1：离线整层生成 MVP（已完成）

已具备：

1. 给定 `seed + config` 可以生成完整楼层。
2. 输出房间拓扑、房间边界、门锚点、走廊、地形、交互物点位和敌人点位。
3. 支持 JSON 导出和基本调试输出。
4. 支持 UE5 具名通道导出。
5. 支持约束求解（锚点、排除区、模板引用）。
6. 支持分块元数据和内存缓存。

### M2：质量加固与深度验证

完成后应具备：

1. 完善的不变量验证（可达性、边界不重叠、连通性、间距）。
2. 黄金样本测试、属性测试和基准测试。
3. JSON 导出/导入一致性。
4. 调试输出增强（阶段耗时、约束报告、被拒绝点位）。

### M3：UE5 深度适配与运行时分块

完成后应具备：

1. 支持 `RuntimeChunked` 和 `HybridPrecompute` 模式的实际生成逻辑。
2. Chunk 元数据、World Partition 相关字段完善。
3. 图参数映射增强。

### M4：增强功能

完成后可补充：

1. Grammar 兼容输出。
2. 二进制导出与缓存。
3. 更细粒度性能分析。
4. 多地形策略扩展。

## 任务列表

- [x] 1. 修正 crate 基础配置并建立模块骨架
  - 将 `crates/yang-pcg/Cargo.toml` 的 `edition` 调整为 `2021`
  - 增加首批依赖：`serde`、`serde_json`、`thiserror`、`rand`、`rand_pcg`
  - 建立 `src/` 模块骨架：`config`、`generator`、`rng`、`topology`、`layout`、`terrain`、`spawn`、`constraint`、`model`、`ue`、`debug`、`cache`、`validation`、`error`、`digest`
  - 更新 `lib.rs` 导出入口和模块声明
  - _需求映射：1, 10, 16, 18_

- [x] 2. 定义基础错误类型与通用结果类型
  - 实现 `PcgError` 枚举，覆盖配置、约束、拓扑、布局、地形、点位、导出、预算耗尽、迭代超限、数据损坏、能力不可用
  - 统一错误上下文字段：`stage`、`seed`、`trace_id`、`field_path`、`room_id`、`chunk_id`
  - 实现 `PcgResult<T>` 类型别名
  - 提供便捷构造方法和错误码查询
  - 补充中文错误描述
  - _需求映射：16.1-16.6_

- [x] 3. 定义核心数据模型
  - 定义 `GenerationRequest`、`GenerationConfig`、`GenerationMode`、`RuntimeContext`
  - 定义 `GenerationResult`、`ResultMetadata`
  - 定义 `Room`、`RoomGraph`、`RoomType`、`RoomEdge`、`Branch`
  - 定义 `DoorAnchor`、`Corridor`、`CorridorPath`
  - 定义 `Terrain`、`TileKind`、`Grid2D`、`GridSize`
  - 定义 `SpawnPoint`、`SpawnKind`、`SpawnMetadata`
  - 定义 `Chunk`、`StreamingMetadata`
  - 定义 `GridPoint`、`RoomBounds`、`WorldPoint`、`Transform3`、`Bounds3`
  - _需求映射：1, 3, 4, 5, 7, 8, 11, 12, 14_

- [x] 4. 实现配置归一化、默认值与摘要
  - 为 `GenerationConfig` 提供完整默认值
  - 实现配置校验与归一化（`normalize()` 方法）
  - 实现 `ConfigDigest`（基于 JSON 序列化的稳定哈希）
  - 支持能力开关与生成模式兼容性验证
  - _需求映射：2.5, 10.1-10.7, 14.4_

- [x] 5. 实现确定性 RNG 与子随机流
  - 封装 `StableRng`（基于 `rand_pcg::Pcg64`）
  - 支持从根种子派生阶段级、房间级、走廊级子随机流（`derive()` 方法）
  - 实现 `random_range`、`gen_bool`、`gen_bool_with_probability`、`gen_f64`、`gen_f32`
  - 实现 `choose`、`choose_mut`、`shuffle`、`sample`、`choose_weighted`
  - 保证调试流不影响玩法输出
  - _需求映射：2.1-2.6_

- [x] 6. 实现拓扑生成模块
  - 实现 `TopologyGenerator`，采用"关键路径 + 分支"启发式构造
  - 生成房间数、连通图骨架、关键路径和分支
  - 分配房间类型（Start、Boss、Combat、Treasure、Shop 等）和路径深度
  - 生成难度曲线（基于深度递增）
  - 支持分支目的标签（reward、shop、event）
  - _需求映射：3.1-3.8_

- [x] 7. 实现空间布局模块
  - 实现 `solve_room_bounds` 计算房间边界
  - 实现 `generate_door_anchors` 生成门锚点
  - 实现 `generate_corridors` 生成正交走廊路径
  - 支持共享边开口与独立走廊两种连接策略
  - _需求映射：4.1-4.7_

- [x] 8. 实现地形生成模块
  - 实现 `carve_room_terrain` 为每个房间生成逻辑网格
  - 支持地板、墙体、障碍物、保留区和门口通行区
  - 实现连通性验证模块
  - _需求映射：5.1-5.8_

- [x] 9. 实现交互物与敌人点位生成
  - 实现 `generate_item_spawns_for_room` 交互物点位生成
  - 实现 `generate_enemy_spawns_for_room` 敌人点位生成
  - 实现最小间距采样（`sampling` 模块）
  - 实现战斗预算计算（`budget` 模块）
  - _需求映射：7.1-7.7, 8.1-8.7_

- [x] 10. 实现约束求解模块
  - 实现锚点约束验证与应用（`anchors` 模块）
  - 实现排除区约束验证与点位过滤（`exclusion` 模块）
  - 实现模板引用约束（`templates` 模块）
  - 支持约束冲突预检查
  - 在拓扑和点位阶段分层注入约束
  - _需求映射：6.1-6.6_

- [x] 11. 实现核心编排器 `MapGenerator`
  - 建立完整生成流水线：配置验证 → RNG 派生 → 约束预检 → 拓扑 → 布局 → 地形 → 点位 → 验证 → 结果组装
  - 支持 `OfflineFullFloor` 模式
  - 提供 `set_debug` 开关
  - 支持 `trace_id` 追踪
  - _需求映射：1, 2, 10, 15, 16_

- [x] 12. 实现基础结果验证
  - 验证 rooms 数量与 topology.nodes 一致
  - 验证 corridors 数量与 topology.edges 一致
  - 验证门锚点数量不少于走廊数量
  - 验证元数据完整性（schema_version、algorithm_version）
  - _需求映射：15.3, 15.5, 18.3_

- [x] 13. 实现 UE5 基础适配层
  - 定义 `PcgPoint`、`NamedChannel`、`ChannelKind`、`PropertyValue`
  - 实现 `export_named_channels` 导出 rooms、doors、corridors、floor_tiles、wall_tiles、spawn_items、spawn_enemies 七类通道
  - 稳定输出核心元数据：`room_id`、`room_type`、`difficulty`、`facing`、`spawn_tag`
  - _需求映射：9.1-9.7_

- [x] 14. 实现图参数映射
  - 将可覆盖配置字段映射为稳定参数键（`map_config_to_graph_params`）
  - 输出 room_count、critical_path_length、corridor.width、generation_mode、theme.primary
  - _需求映射：10.3, 10.6, 10.7_

- [x] 15. 实现分块与流式元数据基础
  - 实现 `build_chunks` 根据房间边界生成分块信息
  - 支持分块启用/禁用配置
  - 定义 `StreamingMetadata`（data_layer、external_data_layer、hlod_layer、streaming_priority）
  - 生成稳定 `chunk_id`
  - _需求映射：11.1-11.7, 12.1-12.5_

- [x] 16. 实现缓存键与基础缓存机制
  - 定义 `CacheKey`：schema_version + algorithm_version + seed + config_digest + scope
  - 定义 `CacheScope`：FullFloor、Chunk、Export
  - 实现 `ResultCache` 内存缓存
  - _需求映射：14.4-14.5, 17.2_

- [x] 17. 实现基础调试输出
  - 定义 `DebugBundle`、`StageStat`
  - 在生成流程中记录各阶段产出数量
  - 调试开关不改变玩法输出
  - _需求映射：15.1, 15.4_

- [ ] 18. 增强结果验证与不变量检查
  - [x] 18.1 实现房间可达性验证（BFS/DFS 从 Start 房间出发）
    - 在 `validation.rs` 中新增 `validate_reachability` 函数
    - 验证所有房间从 Start 可达，不可达时返回包含不可达房间 ID 的错误
    - _需求映射：3.2, 18.3_
  - [x] 18.2 实现房间边界不重叠验证
    - 在 `validation.rs` 中新增 `validate_no_overlap` 函数
    - 检测所有房间对的 AABB 碰撞，重叠时返回冲突房间对信息
    - _需求映射：4.7, 18.3_
  - [x] 18.3 实现地形连通性验证
    - 在 `validation.rs` 中新增 `validate_terrain_connectivity` 函数
    - 对每个房间验证从所有入口到所有出口存在可通行路径（BFS on grid）
    - _需求映射：5.4, 18.3_
  - [x] 18.4 实现点位间距和禁布规则验证
    - 在 `validation.rs` 中新增 `validate_spawn_spacing` 函数
    - 验证交互物和敌人点位满足最小间距约束
    - 验证点位不在排除区域内
    - _需求映射：7.4, 8.3, 18.3_
  - [x] 18.5 生成结构化约束验证报告
    - 定义 `ValidationReport` 结构，包含各不变量的通过/失败状态
    - 将验证报告附加到 `DebugBundle` 中
    - _需求映射：6.6, 15.3_
  - [x] 18.6 编写不变量验证单元测试
    - 为每个验证函数编写正向和反向测试用例
    - _需求映射：18.3_

- [ ] 19. 增强调试与分析输出
  - [x] 19.1 记录阶段耗时（使用 `std::time::Instant`）
    - 在 `MapGenerator::generate` 中为拓扑、布局、地形、点位各阶段记录开始/结束时间
    - 将实际 `duration_ms` 写入 `StageStat`
    - _需求映射：15.2, 17.4_
  - [x] 19.2 扩展 `DebugBundle` 包含调试通道
    - 新增 `debug_channels` 字段，输出关键路径节点、门锚点坐标、走廊中心线、被拒绝房间
    - _需求映射：15.1_
  - [x] 19.3 输出被拒绝点位和约束报告
    - 在点位生成阶段记录候选点位数、拒绝数、拒绝原因
    - 将约束验证报告写入 `DebugBundle`
    - _需求映射：15.3, 15.5_
  - [x] 19.4 支持追踪标识串联
    - 确保 `trace_id` 贯穿日志、缓存键与导出元数据
    - _需求映射：15.6_

- [ ] 20. 实现 JSON 导出与导入重建
  - [x] 20.1 为核心结构补齐 `Serialize` / `Deserialize` derive
    - 确保 `GenerationResult` 及其所有子结构（`RoomGraph`、`Terrain`、`SpawnPoint` 等）可完整序列化
    - 处理 `Grid2D` 等自定义容器的序列化
    - _需求映射：14.1_
  - [x] 20.2 实现 `export_json` 函数导出包含完整元数据的 JSON
    - 包含 `schema_version`、`algorithm_version`、`seed`、`config_digest`、`target_engine_version`
    - _需求映射：14.2_
  - [x] 20.3 实现 `import_json` 函数从 JSON 重建 `GenerationResult`
    - 支持反序列化和基本完整性校验（schema_version 兼容性检查）
    - _需求映射：14.3_
  - [x] 20.4 编写导出/导入一致性测试
    - 验证 serialize → deserialize 后结果语义一致
    - _需求映射：14.3, 18.5_

- [x] 21. Checkpoint - 确保所有测试通过
  - 确保所有测试通过，ask the user if questions arise.

- [ ] 22. 扩展地形生成策略
  - [x] 22.1 定义 `TerrainStrategy` trait
    - 定义统一接口：`fn generate(&self, room: &Room, anchors: &[DoorAnchor], config: &TerrainConfig, rng: &mut StableRng) -> PcgResult<Terrain>`
    - _需求映射：5.3_
  - [x] 22.2 实现开放式策略（`OpenArenaStrategy`）
    - Boss 房优先生成开放中心战斗区，障碍物稀疏分布在边缘
    - _需求映射：5.6_
  - [x] 22.3 实现柱状策略（`PillarStrategy`）
    - 在地板上放置规则柱状障碍物，提供掩体
    - _需求映射：5.3_
  - [x] 22.4 实现迷宫式策略（`MazeStrategy`）
    - 使用递归回溯或 Prim 算法生成迷宫式通道布局
    - _需求映射：5.3_
  - [x] 22.5 实现有机式策略（`OrganicStrategy`）
    - 使用随机行走或 cellular automata 生成自然洞穴形态
    - _需求映射：5.3_
  - [x] 22.6 实现策略选择器
    - 根据 `RoomType` + `theme_tags` + `template_ref` 选择合适的地形策略
    - 将选择器集成到 `terrain::generate_terrains` 流程中
    - _需求映射：5.3_
  - [x] 22.7 编写地形策略单元测试
    - 验证各策略生成的地形满足连通性和面积约束
    - _需求映射：5.4, 5.5, 18.3_

- [ ] 23. 实现运行时分块生成逻辑
  - [x] 23.1 实现 `RuntimeChunked` 模式的增量生成
    - 在 `MapGenerator` 中新增 `generate_chunk` 方法
    - 仅生成请求范围内的房间细节和点位
    - 复用已有拓扑结果
    - _需求映射：11.2, 11.4_
  - [x] 23.2 实现 `HybridPrecompute` 模式
    - 新增 `generate_topology_only` 方法先生成楼层拓扑
    - 新增 `fill_chunk_details` 方法按需补全房间内部细节
    - _需求映射：11.7_
  - [x] 23.3 支持时间预算/迭代预算限制
    - 在 `NormalizedConfig` 中增加 `time_budget_ms` 和 `iteration_budget` 字段
    - 在生成循环中检查预算并提前返回部分结果
    - _需求映射：11.5, 17.5_
  - [x] 23.4 验证相同 Chunk 重复请求返回一致结果
    - 确保相同 seed + config + chunk_id 下结果确定性
    - _需求映射：11.6_
  - [x] 23.5 编写分块模式与整层模式一致性测试
    - 验证分块结果合并后与整层结果语义一致
    - _需求映射：18.6_

- [x] 24. Checkpoint - 确保所有测试通过
  - 确保所有测试通过，ask the user if questions arise.

- [ ] 25. 实现 Grammar 兼容输出
  - [x] 25.1 为 `Room` 增加 `grammar_token` 可选字段
    - 在 `model/room.rs` 中扩展 `Room` 结构
    - _需求映射：13.2_
  - [x] 25.2 为 `DoorAnchor` 增加 `socket_tag` 可选字段
    - 在 `model/room.rs` 中扩展 `DoorAnchor` 结构
    - _需求映射：13.2_
  - [x] 25.3 为 `Corridor` 增加分段标签
    - 增加 `segment_tags` 字段，标注走廊各段的长度、转折类型和主题
    - _需求映射：13.3_
  - [x] 25.4 实现确定性权重选择器
    - 在 Grammar 模式下根据朝向、房间主题、走廊长度和房间类型进行确定性规则选择
    - _需求映射：13.4_
  - [x] 25.5 为外部规则无效场景提供结构化错误
    - 当 Grammar 规则或模块引用无效时返回 `PcgError::Capability` 错误
    - _需求映射：13.5_

- [ ] 26. 编写单元测试与黄金样本测试
  - [x] 26.1 核心模块单元测试补全
    - 为 `topology`、`layout`、`terrain`、`spawn` 模块补充边界情况测试
    - 覆盖空配置、极端房间数、零走廊等边界场景
    - _需求映射：18.1_
  - [x] 26.2 固定种子的黄金样本测试
    - 使用固定种子生成结果，保存为参考 JSON
    - 后续回归时对比结果哈希或关键字段
    - _需求映射：18.2_
  - [x] 26.3 调试开关不影响玩法结果测试
    - 同种子开启/关闭调试，验证玩法通道（rooms、corridors、spawns）完全一致
    - _需求映射：2.6, 15.4_

- [ ] 27. 编写属性测试
  - [x] 27.1 确定性属性测试
    - 相同 seed + config 生成相同结果哈希
    - 使用 `proptest` 或 `quickcheck` 随机生成配置参数
    - _需求映射：2.2, 18.2_
  - [x] 27.2 拓扑连通性属性测试
    - 任意合法配置下所有房间从 Start 可达
    - _需求映射：3.2, 18.3_
  - [x] 27.3 房间边界不重叠属性测试
    - 任意合法配置下房间 AABB 不重叠
    - _需求映射：4.7, 18.3_
  - [x] 27.4 地形连通性属性测试
    - 任意房间从入口到出口存在通路
    - _需求映射：5.4, 18.3_
  - [x] 27.5 点位最小间距属性测试
    - 任意配置下点位满足最小间距
    - _需求映射：7.4, 8.3, 18.4_
  - [x] 27.6 约束满足属性测试
    - 锚点和排除区约束在结果中被满足
    - _需求映射：6.5, 18.6_

- [ ] 28. 编写性能基准测试
  - [x] 28.1 `small` 基准：10 房间级
    - _需求映射：17.3_
  - [x] 28.2 `medium` 基准：20 房间级
    - _需求映射：17.3_
  - [x] 28.3 `large` 基准：40 房间级
    - _需求映射：17.3_
  - [x] 28.4 记录总耗时、阶段耗时、房间数、Chunk 数、峰值分配
    - 使用 `criterion` 或自定义基准框架
    - _需求映射：17.4_

- [ ] 29. 实现二进制导出与版本兼容
  - [x] 29.1 定义二进制导出格式（magic + version header + body）
    - 设计紧凑的二进制布局，header 包含 schema_version 和 algorithm_version
    - _需求映射：14.1_
  - [x] 29.2 实现二进制序列化与反序列化
    - 可选使用 `bincode` 或手写编解码
    - _需求映射：14.1, 14.3_
  - [x] 29.3 为版本升级预留兼容字段
    - header 中保留扩展位，支持向前兼容读取
    - _需求映射：14.2_
  - [x] 29.4 提供损坏数据检测（CRC32 校验和）
    - 在 footer 或 header 中写入校验和，读取时验证
    - _需求映射：16.2_

- [ ] 30. 完善示例与使用文档
  - [x] 30.1 增加基础楼层生成示例（`examples/basic_generation.rs`）
    - 演示最简单的生成流程和结果访问
    - _需求映射：1_
  - [x] 30.2 增加约束输入示例（`examples/constrained_generation.rs`）
    - 演示锚点、排除区和模板引用的使用
    - _需求映射：6_
  - [x] 30.3 增加 UE5 适配通道导出示例（`examples/ue5_export.rs`）
    - 演示 `export_named_channels` 和 `map_config_to_graph_params` 的使用
    - _需求映射：9, 14_

- [x] 31. Final checkpoint - 确保所有测试通过
  - 确保所有测试通过，ask the user if questions arise.

## 建议执行顺序

### 第一阶段：质量加固（M2）

按顺序完成：

1. `18`（增强验证）
2. `19`（增强调试）
3. `20`（JSON 导出/导入）
4. `21`（检查点）
5. `26`（单元测试与黄金样本）

### 第二阶段：功能扩展（M3）

按顺序完成：

1. `22`（地形策略扩展）
2. `23`（运行时分块）
3. `24`（检查点）

### 第三阶段：Grammar 与增强（M4）

按顺序完成：

1. `25`（Grammar 兼容）
2. `29`（二进制导出）
3. `30`（示例与文档）
4. `31`（最终检查点）

### 可选阶段：测试与性能

按顺序完成：

1. `27`（属性测试）
2. `28`（性能基准）

## 完成定义

每个任务在标记完成前，至少满足以下条件：

1. 对应源码已经提交到 crate 中。
2. 对应测试已经补齐并通过。
3. 公开结构和接口已有中文文档注释。
4. 与需求映射的项可以被证明满足，而不是仅凭主观判断。

## 备注

- 标记 `*` 的任务为可选任务，可以跳过以加速 MVP 交付
- 每个任务引用具体需求编号以保证可追溯性
- 检查点任务确保增量验证
- 属性测试验证通用正确性属性
- 单元测试验证具体示例和边界情况
