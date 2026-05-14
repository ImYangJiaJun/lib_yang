# 需求文档：yang-pcg UE5 Roguelike 地图生成算法库

## 介绍

本文档定义了 `yang-pcg`（YANG Procedural Content Generation）地图生成算法库的需求。该库面向 UE5 Roguelike 游戏项目，目标不是直接替代 Unreal Engine 的 PCG Graph，而是提供一个可复现、可测试、可离线构建、可运行时分块的地图生成核心，并定义与 UE5 PCG 工作流对接的数据契约。

本次修订在原有“随机种子、房间路径、房间地形、道具与敌人点位”的基础上，补充了以下关键能力：

1. 将“纯算法核心”和“UE5 集成适配层”分层定义，避免把引擎行为和算法行为混在一起。
2. 将“拓扑路径”扩展为“房间边界、门锚点、走廊中心线、可落地的物理连通关系”。
3. 增加与 UE5 PCG 对齐的数据模型，包括 Point/Metadata/Named Channel/Chunk 信息。
4. 增加运行时生成、World Partition、Data Layer、External Data Layer、HLOD、离线烘焙缓存等需求。
5. 增加手工干预、保留区、锚点、Grammar 模块化拼装、调试输出与可观察性需求。

## 当前文档分析

### 已有优势

1. 已覆盖 Roguelike 地图生成的主流程：随机种子、房间拓扑、房间地形、道具、敌人、导出、错误处理、性能、测试。
2. 需求写法已经接近可验证的验收标准格式，便于后续拆分任务。
3. 需求重心放在“确定性”和“配置驱动”，这与程序化生成库的核心目标一致。

### 主要缺口

1. 当前文档只定义了“房间路径”，没有定义房间的物理边界、门口、走廊和世界空间映射，无法直接落地到 UE5 关卡内容生成。
2. 当前文档没有区分算法库与 UE5 适配层，导致“导出 JSON”与“在 UE5 中加载和使用”之间缺少明确契约。
3. 当前文档没有定义 UE5 PCG 的关键输入输出模型，例如 Point、Bounds、Density、Seed、用户自定义属性、Named Channels。
4. 当前文档没有覆盖运行时生成、分块、Generation Source、World Partition、Data Layer/HLOD 等现代 UE5 PCG 工作流。
5. 当前文档没有提供手工干预能力，例如锚点、禁布区、保留房间、模板房间引用，这会让设计师很难把程序化与手工关卡结合。
6. 当前文档的性能需求缺少测试基线和硬件上下文，`20 个房间 100ms` 的要求过于孤立，不足以指导长期回归。
7. 当前文档没有为调试、可视化、缓存、版本兼容、导出重建和中间结果检查定义要求，后期维护成本会很高。

### 修订原则

1. 保留原文档中“随机种子 + 拓扑 + 地形 + 点位”的主链路。
2. 把新增内容限制在 UE5 Roguelike 项目真正会用到的契约层，不把整套引擎工具需求全部塞进算法库。
3. 以“能测试、能导出、能调试、能集成”为目标定义需求，避免仅写概念性描述。

## 范围与非目标

### 范围

1. 楼层级地图拓扑生成。
2. 房间、门口、走廊、网格地形和玩法点位生成。
3. 与 UE5 PCG 对齐的数据导出契约。
4. 运行时分块、离线构建和调试输出所需元数据。
5. 手工锚点、排除区、模板引用和模块化 Grammar 兼容接口。

### 非目标

1. 本库不直接负责在 UE5 中生成最终 Static Mesh、Material、Niagara 或 NavMesh。
2. 本库不负责敌人 AI 行为树、掉落表、战斗逻辑、任务逻辑等玩法系统实现。
3. 本库不强制依赖 UE Runtime 类型；若存在 UE 集成层，应作为独立适配边界。
4. 本库不负责美术资源生产，仅输出可驱动美术生成的数据。
5. 本版本不将 2D 平台跳跃专用可达性模型（跳高、落差、墙跳、轨迹避让等）纳入核心需求。

## UE5 对齐假设

本需求默认对齐 UE5.5 及以上版本，并参考 UE5.6 文档中的当前 PCG 能力边界：

1. UE5 PCG 以 `Point`、`Spline`、`Metadata` 和 `PCG Component/Graph` 为核心数据流，Point 天然携带 `transform`、`bounds`、`density`、`seed` 等信息，并支持用户自定义属性。
2. UE5 PCG 支持 `Graph Parameters` 与 `Graph Instances`，适合把本库的配置映射成图参数和实例覆盖。
3. UE5 PCG 支持 `Runtime Generation`、`Hierarchical Generation` 和 `Generation Source`，适合将本库的结果按块加载或按玩家邻近区域生成。
4. UE5 PCG 与 `World Partition`、`Data Layer`、`External Data Layer`、`HLOD` 具备集成能力，因此算法输出应能携带这些元信息。
5. UE5.5 引入了 `Shape Grammar`、`Save to PCG Data Assets`、离线路径/几何处理等能力，UE5.6 继续增强了执行效率、调试树和数据视口能力。
6. WHEN 项目目标版本低于 UE5.5 时，THE 系统 SHALL 允许通过能力开关禁用 `Shape Grammar`、离线数据资产和增强运行时生成相关需求。

参考资料：

1. [Procedural Content Generation Overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/procedural-content-generation-overview?application_version=5.6)
2. [PCG Development Guides](https://dev.epicgames.com/documentation/en-us/unreal-engine/pcg-development-guides)
3. [Using PCG Generation Modes](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-pcg-generation-modes)
4. [Unreal Engine 5.5 Release Notes](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-5-release-notes)
5. [Unreal Engine 5.6 Release Notes](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-6-release-notes)

## 术语表

- **Map_Generator**: 地图生成核心，负责执行确定性地图生成逻辑
- **UE_Adapter**: UE5 适配层，负责将生成结果映射为 UE5 PCG 兼容数据
- **Generation_Request**: 单次地图生成请求，包含 Seed、Config、约束和上下文
- **Generation_Result**: 单次地图生成结果，包含拓扑、几何、点位、元数据和调试信息
- **Random_Seed**: 随机种子，用于初始化随机数生成器的 `u64` 整数值
- **Room**: 地图中的基本空间单元
- **Room_Type**: 房间类型，如 Start、Combat、Treasure、Shop、Elite、Puzzle、Safe、Boss
- **Corridor**: 连接两个房间的物理通路，可表现为网格路径、样条或分段折线
- **Door_Anchor**: 房间边界上的门口锚点，表示连通点和朝向
- **Terrain**: 房间内部的地形/网格布局，包括地板、墙壁、障碍物、保留区等
- **PCG_Point**: 对齐 UE5 PCG 的点数据，至少包含 Transform、Bounds、Density、Seed 和自定义属性
- **Metadata_Attribute**: 附着在 Point 或数据集合上的元数据属性
- **Named_Channel**: 具名输出通道，如 `rooms`、`corridors`、`spawn_items`
- **Chunk**: 用于运行时生成或流式加载的空间分块
- **Generation_Mode**: 生成模式，如整层离线生成、运行时分块生成、混合模式
- **Anchor**: 由设计师指定的强约束点，如起点、Boss 房、商店、特定主题房
- **Exclusion_Zone**: 禁止生成或限制生成的区域
- **Grammar_Token**: 用于模块化拼装或 Shape Grammar 的语法单元
- **Config_Digest**: 生成配置的稳定哈希，用于缓存和回归验证

## 需求

### 需求 1：系统分层与对外接口

**用户故事：** 作为系统开发者，我希望算法核心与 UE5 集成逻辑清晰分层，以便保持 `yang-pcg` 可测试、可复用，并降低后续引擎适配成本。

#### 验收标准

1. THE Map_Generator SHALL 提供不依赖 `UObject`、`AActor`、`UPCGComponent` 等 UE Runtime 类型的核心 API
2. THE UE_Adapter SHALL 作为独立边界，将 `Generation_Request` 和 `Generation_Result` 映射到 UE5 兼容数据结构
3. THE Generation_Result SHALL 至少包含 `topology`、`rooms`、`corridors`、`terrain`、`spawn_items`、`spawn_enemies` 六类具名输出通道
4. WHEN 某项能力依赖特定 UE 版本时，THE 系统 SHALL 显式暴露能力开关或版本约束，而不是静默退化
5. THE Map_Generator SHALL 保证同一结果可无语义损失地导出为 JSON、二进制和 UE_Adapter 可消费的中间格式

### 需求 2：随机种子与确定性

**用户故事：** 作为游戏开发者，我希望使用随机种子稳定复现楼层生成结果，以便支持存档、回放、问题复现和种子分享。

#### 验收标准

1. THE Map_Generator SHALL 接受 `u64` 类型的 `Random_Seed`
2. WHEN 使用相同的 `Random_Seed`、相同的 `Generation_Config` 和相同的算法版本时，THE Map_Generator SHALL 生成语义完全一致的 `Generation_Result`
3. THE Map_Generator SHALL 为拓扑、房间、走廊、地形和点位等子流程派生确定性的子随机流
4. WHEN `Random_Seed` 未提供时，THE Map_Generator SHALL 生成默认种子并将其记录到结果元数据中
5. THE Generation_Result SHALL 包含 `Random_Seed`、`Config_Digest` 和算法版本标识
6. WHEN 仅开启或关闭调试输出时，THE 游戏玩法相关输出通道 SHALL 保持不变

### 需求 3：楼层拓扑与进度曲线

**用户故事：** 作为关卡设计者，我希望生成带有节奏控制的房间拓扑，以便构建可玩、可重复且符合 Roguelike 进程曲线的楼层结构。

#### 验收标准

1. THE Map_Generator SHALL 生成至少包含一个 Start 房间和一个 Boss 或终点房间的楼层拓扑
2. THE Map_Generator SHALL 确保所有 Room 从 Start 房间可达
3. THE Map_Generator SHALL 支持配置房间总数、关键路径长度、分支数量和死路数量范围
4. THE Map_Generator SHALL 为每个 Room 分配一个 `Room_Type`
5. THE Map_Generator SHALL 支持配置各 `Room_Type` 的目标数量、最小数量和权重
6. THE Map_Generator SHALL 确保 Boss 房间仅出现在关键路径终点或设计上允许的终端分支
7. THE Map_Generator SHALL 支持沿关键路径输出难度递增或波动受控的难度曲线信息
8. WHEN 楼层包含分支时，THE Map_Generator SHALL 支持为每个分支指定其设计目的，如奖励、商店、事件或捷径

### 需求 4：房间边界、门锚点与走廊生成

**用户故事：** 作为关卡开发者，我希望拓扑连接能落地为可放置内容的房间边界和走廊，以便后续在 UE5 中实例化场景。

#### 验收标准

1. THE Map_Generator SHALL 为每个 Room 生成明确的房间边界信息
2. THE Map_Generator SHALL 为每条拓扑边生成对应的 `Door_Anchor` 和 `Corridor`
3. THE Map_Generator SHALL 确保每个 `Door_Anchor` 具有位置、朝向和所属房间信息
4. THE Map_Generator SHALL 支持配置走廊宽度、最大转折次数和连线策略
5. WHEN 两个房间被判定为物理相邻时，THE Map_Generator SHALL 支持使用共享边开口代替独立走廊
6. THE Map_Generator SHALL 输出走廊中心线或分段折线数据，以便 UE5 适配层转换为样条或 Grammar 输入
7. THE Map_Generator SHALL 避免房间与走廊发生未声明的重叠；若允许重叠，THE 结果 SHALL 显式标注重叠策略

### 需求 5：房间内部网格地形生成

**用户故事：** 作为游戏开发者，我希望每个房间具备可通行、可放置玩法元素的网格地形，以便形成稳定可玩的战斗与探索空间。

#### 验收标准

1. THE Map_Generator SHALL 为每个 Room 生成一个 `Terrain`
2. THE Terrain SHALL 至少区分地板、墙体、障碍物、保留区和门口连通区
3. THE Map_Generator SHALL 根据 `Room_Type`、主题标签或模板引用选择不同的地形生成策略
4. THE Map_Generator SHALL 确保每个房间从所有入口到所有必达出口之间存在可通行路径
5. THE Map_Generator SHALL 支持配置房间尺寸范围、障碍物密度和最小可通行面积比例
6. WHEN `Room_Type` 为 Boss 房间时，THE Map_Generator SHALL 生成更大的开放区域和更清晰的中心战斗空间
7. THE Terrain SHALL 使用逻辑网格坐标表示，并支持可选的世界空间转换信息
8. WHEN 房间地形无法满足连通性或面积约束时，THE Map_Generator SHALL 触发重试或返回约束不满足错误

### 需求 6：手工约束、锚点与排除区

**用户故事：** 作为设计师，我希望在程序化生成中保留手工控制能力，以便把关键玩法节点、剧情房间和禁布区域稳定地固定下来。

#### 验收标准

1. THE Generation_Request SHALL 支持输入 `Anchor`、`Exclusion_Zone` 和保留房间约束
2. THE Anchor SHALL 至少支持 Start、Boss、Shop、Treasure、Safe、Secret 和 ThemeAnchor 类型
3. THE Map_Generator SHALL 支持将指定 `Room_Type` 固定到指定锚点或指定区域
4. THE Map_Generator SHALL 支持通过 `template_id` 或 `room_preset_id` 引用外部设计模板
5. THE Map_Generator SHALL 避免在 `Exclusion_Zone` 中生成被禁止的房间、走廊或点位
6. WHEN 输入约束彼此冲突时，THE Map_Generator SHALL 返回包含冲突详情的描述性错误

### 需求 7：交互物点位生成

**用户故事：** 作为游戏开发者，我希望为房间生成可复现的交互物点位，以便放置宝箱、祭坛、商店、拾取物和其他玩法对象。

#### 验收标准

1. THE Map_Generator SHALL 为指定 Room 生成交互物点位集合
2. THE Map_Generator SHALL 确保所有交互物点位位于可通行地板或显式允许的保留区域
3. THE Map_Generator SHALL 支持配置交互物数量范围、稀有度权重和与房间类型相关的生成密度
4. THE Map_Generator SHALL 确保交互物之间保持最小间距
5. THE Map_Generator SHALL 避免在入口、出口、Boss 核心战斗区和关键走廊上生成不应阻塞移动的交互物
6. WHEN `Room_Type` 为 Treasure 或 Shop 房间时，THE Map_Generator SHALL 支持使用独立的交互物生成配置
7. EACH 交互物点位 SHALL 至少携带 `room_id`、`room_type`、`spawn_tag`、`rarity_tier` 和 `seed` 元数据

### 需求 8：敌人点位与战斗预算生成

**用户故事：** 作为战斗设计者，我希望房间中的敌人点位与战斗强度可以由规则驱动生成，以便形成稳定的难度曲线和复玩价值。

#### 验收标准

1. THE Map_Generator SHALL 为指定 Room 生成敌人点位或敌人小组点位
2. THE Map_Generator SHALL 确保所有敌人点位位于可通行区域
3. THE Map_Generator SHALL 确保敌人点位与入口、交互物点位和关键通道之间保持最小安全间距
4. THE Map_Generator SHALL 支持按房间类型、房间难度和关键路径深度配置敌人预算
5. WHEN `Room_Type` 为 Boss 房间时，THE Map_Generator SHALL 生成 Boss 主点位和可选的辅助刷怪区域信息
6. WHEN `Room_Type` 为 Safe 或 Shop 房间时，THE Map_Generator SHALL 不生成普通敌人点位
7. EACH 敌人点位 SHALL 至少携带 `room_id`、`encounter_id`、`wave_id`、`enemy_pool_tag`、`difficulty` 和 `seed` 元数据

### 需求 9：UE5 PCG 数据契约

**用户故事：** 作为 UE5 集成开发者，我希望生成结果能自然映射到 PCG 数据流，以便在 PCG Graph、Blueprint 和离线构建流程中复用。

#### 验收标准

1. THE UE_Adapter SHALL 能将 `Generation_Result` 映射为 UE5 兼容的 Point、Spline 和数据集合
2. THE UE_Adapter SHALL 至少导出 `rooms`、`doors`、`corridors`、`floor_tiles`、`wall_tiles`、`spawn_items`、`spawn_enemies` 和 `debug` 八类 `Named_Channel`
3. EACH `PCG_Point` SHALL 至少包含 Transform、Bounds、Density、Seed 和用户自定义属性
4. THE UE_Adapter SHALL 区分全局级属性和逐点属性，以便分别映射为数据级元数据和点级元数据
5. THE UE_Adapter SHALL 使用稳定、可文档化的属性命名，并避免产生不兼容 UE 属性系统的非法名称
6. THE UE_Adapter SHALL 支持将 `room_id`、`room_type`、`chunk_id`、`theme_tag`、`difficulty` 等核心属性传递到下游 PCG 节点
7. WHEN 某通道为空时，THE UE_Adapter SHALL 仍保留明确的通道语义，而不是把不同类型数据混入单一集合

### 需求 10：配置管理与图参数映射

**用户故事：** 作为工具开发者，我希望将生成配置稳定映射到 UE5 的图参数和运行时覆盖，以便在编辑器、蓝图和运行时统一调参。

#### 验收标准

1. THE Generation_Config SHALL 支持序列化、反序列化和默认值填充
2. THE Generation_Config SHALL 至少包含房间数量、关键路径长度、房间尺寸、走廊规则、障碍密度、交互物密度、敌人预算、主题标签和生成模式
3. THE 系统 SHALL 支持默认配置、预设配置、图实例覆盖和运行时局部覆盖的层级合并
4. THE Map_Generator SHALL 验证配置中的数值范围、枚举兼容性和互斥约束
5. WHEN 配置非法时，THE Map_Generator SHALL 返回带有字段路径的描述性错误信息
6. THE UE_Adapter SHALL 支持将可覆盖配置映射为 UE5 `Graph Parameters` 或等价参数字典
7. THE 系统 SHALL 生成稳定的 `Config_Digest` 用于缓存、回归验证和导出签名

### 需求 11：运行时生成模式与分块

**用户故事：** 作为技术美术或关卡程序员，我希望支持整层预生成和运行时邻近区域生成两种模式，以便兼顾 Roguelike 楼层构建和大地图流式加载。

#### 验收标准

1. THE Map_Generator SHALL 支持 `OfflineFullFloor`、`RuntimeChunked` 和 `HybridPrecompute` 三种 `Generation_Mode`
2. WHEN 使用 `RuntimeChunked` 模式时，THE Generation_Result SHALL 按 `Chunk` 输出空间边界、标识和依赖信息
3. THE Generation_Request SHALL 支持附带运行时上下文，如关注位置、兴趣半径、优先级或来源标签
4. THE Map_Generator SHALL 支持在分块模式下仅生成请求范围内的房间细节和点位细节
5. THE Map_Generator SHALL 支持可配置的时间预算或迭代预算，以便运行时逐步推进生成
6. WHEN 相同 Chunk 在相同输入上下文中被重复请求时，THE Map_Generator SHALL 返回语义一致的局部结果
7. THE HybridPrecompute 模式 SHALL 支持先生成楼层拓扑，再按需补全房间内部细节

### 需求 12：World Partition、Data Layer 与流式元数据

**用户故事：** 作为 UE5 世界构建开发者，我希望程序化结果可以带着流式世界元数据进入 UE 工作流，以便与 World Partition 和 Data Layer 体系协同工作。

#### 验收标准

1. EACH `Chunk` SHALL 支持携带 `data_layer`、`external_data_layer`、`hlod_layer` 和 `streaming_priority` 等可选元数据
2. WHEN 调用方提供默认的 Data Layer 或 External Data Layer 上下文时，THE UE_Adapter SHALL 将这些信息传递到生成产物
3. THE 系统 SHALL 生成稳定的 `chunk_id`，以便在相同 Seed 和 Config 下复用缓存和离线构建结果
4. THE UE_Adapter SHALL 支持为离线构建输出适合 World Partition Builder 或等价构建流程消费的中间数据
5. WHEN 未指定流式元数据时，THE 系统 SHALL 保留空值或默认值，而不是伪造层级信息

### 需求 13：Grammar 与模块化拼装兼容

**用户故事：** 作为技术关卡设计者，我希望算法输出能够驱动模块化拼装或 Shape Grammar，以便把抽象房间/走廊结果映射到具体的模块资产布局。

#### 验收标准

1. THE Map_Generator SHALL 在不启用 Grammar 的情况下仍能输出基础的房间边界、门口和走廊数据
2. WHEN 启用 Grammar 兼容模式时，THE Generation_Result SHALL 支持输出 `Grammar_Token`、模块槽位或走廊分段标记
3. THE Grammar 输出 SHALL 支持结合朝向、房间主题、走廊长度和房间类型进行规则选择
4. THE Grammar 输出 SHALL 支持确定性的权重选择，确保同种子下结果可复现
5. WHEN 外部 Grammar 规则或模块引用无效时，THE 系统 SHALL 返回能力或映射错误，而不是生成不完整静默结果

### 需求 14：数据导出、缓存与重建

**用户故事：** 作为工具链维护者，我希望将生成结果导出、缓存并重建，以便支持离线构建、调试回放和跨工具消费。

#### 验收标准

1. THE Map_Generator SHALL 支持将 `Generation_Result` 导出为 JSON 和二进制格式
2. THE 导出格式 SHALL 包含 `schema_version`、算法版本、`Random_Seed`、`Config_Digest`、目标引擎版本和具名通道信息
3. THE Map_Generator SHALL 提供从导出数据重建 `Generation_Result` 的能力
4. THE 系统 SHALL 支持基于 `Random_Seed` 与 `Config_Digest` 的缓存键
5. WHEN 输入未变化且缓存命中时，THE 系统 SHALL 支持跳过完整重算并返回缓存结果
6. THE UE_Adapter SHALL 支持输出适合进入 UE 离线数据资产流程的中间结果

### 需求 15：调试、诊断与分析输出

**用户故事：** 作为开发者，我希望能够检查生成过程中的中间状态和失败原因，以便快速定位地图问题并调优参数。

#### 验收标准

1. THE Map_Generator SHALL 支持输出调试通道，包括至少房间中心、门口、走廊中心线、被拒绝点位和关键路径信息
2. THE Map_Generator SHALL 为拓扑生成、房间布局、地形生成、交互物生成和敌人生成输出阶段性计数与耗时
3. THE Map_Generator SHALL 输出约束验证报告，说明哪些不变量被满足或被拒绝
4. THE 调试输出 SHALL 可独立开关，并且不得改变游戏玩法相关通道的生成结果
5. WHEN 生成失败时，THE 系统 SHALL 尽可能输出失败阶段、失败约束和可选的部分调试上下文
6. THE 系统 SHALL 支持为单次生成附加追踪标识，以便串联日志、缓存与导出结果

### 需求 16：错误处理

**用户故事：** 作为库使用者，我希望库能够以结构化的方式表达错误，以便在编辑器、工具链和运行时环境中一致处理。

#### 验收标准

1. THE Map_Generator SHALL 使用 `Result` 类型返回可能失败的操作
2. THE 错误类型 SHALL 至少区分配置错误、约束不满足、能力不可用、预算耗尽、序列化错误和数据损坏错误
3. THE 错误类型 SHALL 提供中文描述信息和稳定的机器可读错误码
4. THE 错误信息 SHALL 包含足够的上下文，如阶段名称、`Random_Seed`、相关房间或相关字段路径
5. WHEN 安全且有用时，THE 错误结果 SHALL 支持附带部分调试上下文
6. THE 错误类型 SHALL 实现 `std::error::Error` trait

### 需求 17：性能、并发与资源使用

**用户故事：** 作为技术负责人，我希望地图生成在离线和运行时都具备可预测性能，以便支持快速迭代并降低帧时间风险。

#### 验收标准

1. THE Map_Generator SHALL 支持并发生成多个彼此独立的地图请求
2. THE Map_Generator SHALL 避免不必要的内存分配，并支持容量预估或预分配接口
3. THE 仓库 SHALL 提供至少 `small`、`medium`、`large` 三档性能基准配置，分别覆盖小型、中型和大型楼层
4. THE 性能基准 SHALL 至少记录总耗时、阶段耗时、房间数量、Chunk 数量和峰值分配统计
5. WHEN 采用运行时分块模式时，THE Map_Generator SHALL 支持可中断、可续跑或按预算分步推进的执行方式
6. THE 性能目标 SHALL 以基准配置和参考环境定义，而不是仅以单一绝对时间要求表达

### 需求 18：测试与验收支持

**用户故事：** 作为库维护者，我希望通过自动化测试覆盖核心不变量和导出契约，以便在迭代中稳定扩展功能。

#### 验收标准

1. THE 库 SHALL 为所有公开 API 提供单元测试
2. THE 测试 SHALL 验证相同 Seed 与相同 Config 生成相同结果哈希或等价结构
3. THE 测试 SHALL 验证所有拓扑、连通性、房间面积、点位间距和保留区约束等核心不变量
4. THE 测试 SHALL 包含基于属性的测试，以验证随机输入下的鲁棒性
5. THE 测试 SHALL 包含导出/导入一致性测试、缓存命中测试和 UE 适配层数据契约测试
6. THE 测试 SHALL 覆盖分块模式与整层模式的一致性、Grammar 模式、Anchor 模式和 Exclusion_Zone 模式
7. THE 测试代码 SHALL 遵循项目约定放置在 `__tests__` 或 `tests` 目录中，并用中文注释标明验证的需求编号
