# yang-pcg → UE5 集成全流程教程

本教程讲清楚一件事：**怎么把 yang-pcg 生成的地图数据用到 UE5 里。**

## 0. 先理解集成模型（重要，别走弯路）

`yang-pcg` 是一个**纯 Rust 算法库**，它的产物是**确定性的地图数据**（房间、走廊、地形网格、刷怪点），不是 UE5 插件、不含任何 UE 类型、也**不通过 FFI/动态库**直接被引擎调用。

集成的本质是一条**数据通路**：

```text
┌─────────────────┐   导出      ┌──────────────┐   读取/解析   ┌─────────────────┐
│   yang-pcg      │  JSON /     │  .json /     │  C++ / 蓝图 / │     UE5         │
│ (Rust 生成地图) │ ──binary──> │  .ypcg 文件   │ ──Python──>  │ (生成关卡 Actor) │
└─────────────────┘             └──────────────┘              └─────────────────┘
```

yang-pcg 负责"地图长什么样"，UE5 负责"把数据变成可玩的关卡"。两者通过序列化数据解耦。这样做的好处：

- **确定性可复现**：同一 `seed` + `config` 永远产出同一份数据，存档只需存 seed。
- **离线/运行时都能用**：可以在打包前烘焙成资产，也可以在游戏运行时调用。
- **引擎无关**：同一份数据理论上能喂给 UE5、Unity 或自研引擎。

> ⚠️ **确定性的边界**：相同 seed+config 的复现性保证是**同一生成模式内**成立的。`OfflineFullFloor`、`RuntimeChunked`、`HybridPrecompute` 三种模式因为 RNG 派生路径不同，**同一 seed 在不同模式下会产出不同地图**。选定一种模式后保持一致。若 `seed` 留空（`None`），库会**从 config 派生确定性种子**——相同 config 仍复现同一地图；想要不同结果时显式提供 seed 或修改 config。

下面给出三条集成路线，从简单到进阶，**绝大多数项目用路线 A 就够了**。

| 路线 | 适用场景 | UE5 侧成本 | 运行时生成 |
|------|----------|-----------|-----------|
| **A. 离线烘焙 JSON** | 关卡在打包前确定，或后台批量生成 | 低（读 JSON） | 否 |
| **B. 独立 CLI 工具 + 运行时调用** | 游戏运行时按 seed 生成新楼层 | 中（调进程/读文件） | 是 |
| **C. 编译为 C 动态库 + FFI** | 需要引擎内同步调用、零文件 IO | 高（写 FFI + C++ 封装） | 是 |

---

## 1. 路线 A：离线烘焙 JSON（推荐起步）

### 1.1 在 Rust 侧生成并导出

把下面这段作为一个 example 或独立 bin（参考仓库 `crates/yang-pcg/examples/basic_generation.rs`）：

```rust
use yang_pcg::{export_json, GenerationConfig, GenerationRequest, MapGenerator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = GenerationRequest {
        seed: Some(20260610),                 // 生产中务必显式给定
        config: GenerationConfig::default(),  // 或自定义，见 §4
        constraints: vec![],
        runtime_context: None,
        trace_id: Some("floor-1".to_string()),
    };

    let result = MapGenerator::new().generate(request)?;

    // 导出为人类可读 JSON（调试期）
    let json = export_json(&result)?;
    std::fs::write("floor-1.json", &json)?;

    // 或导出紧凑 JSON（体积更小，适合随包分发）
    // let json = yang_pcg::export_json_compact(&result)?;

    println!("已生成 {} 个房间，{} 个敌人点位",
             result.rooms.len(), result.enemy_spawns.len());
    Ok(())
}
```

运行：

```bash
cargo run --release --example basic_generation -p yang-pcg
# 或你自己的 bin
```

把产出的 `floor-1.json` 放进 UE 工程的 `Content/Maps/` 或 `<Project>/GeneratedMaps/`。

### 1.2 导出数据的结构

`export_json` 序列化的是整个 `GenerationResult`。真实结构骨架（seed=42 默认配置）：

```jsonc
{
  "metadata": {
    "seed": 42,
    "config_digest": "5b5dbe24bf19a6b4",  // 配置指纹，可用于校验
    "schema_version": "1.0.0",            // 数据格式版本
    "algorithm_version": "0.1.0",
    "trace_id": "floor-1"
  },
  "rooms": [
    {
      "id": "room-000",
      "room_type": "Start",               // Start/Combat/Treasure/Shop/Elite/Puzzle/Safe/Boss/Event/Secret
      "depth_from_start": 0,
      "difficulty": 0,
      "branch_id": null,
      "theme_tags": ["default"],
      "bounds": { "min": {"x":0,"y":-5}, "max": {"x":13,"y":5} }  // 逻辑网格坐标
    }
  ],
  "door_anchors": [
    { "id":"anchor-000-from", "room_id":"room-000", "edge_id":"edge-000",
      "facing":"East", "grid_pos":{"x":12,"y":0}, "width_tiles":2 }
  ],
  "corridors": [
    { "id":"corridor-000", "from_room":"room-000", "to_room":"room-001",
      "width_tiles":2,
      "path": { "Orthogonal": [ {"x":12,"y":0}, {"x":21,"y":0} ] } }
  ],
  "terrains": [
    { "room_id":"room-000",
      "grid_size": {"width":13, "height":10},
      "tiles": {
        "width":13, "height":10,
        "data": ["Wall","Wall","Floor", "...行优先一维数组..."]
      }
    }
  ],
  "item_spawns":  [ /* SpawnPoint */ ],
  "enemy_spawns": [
    { "id":"enemy-room-001-000", "room_id":"room-001", "kind":"Enemy",
      "grid_pos":{"x":25,"y":2},
      "world_transform":{ "position":{"x":25.0,"y":2.0,"z":0.0},
                          "rotation":[0,0,0], "scale":[1,1,1] },
      "metadata":{ "spawn_tag":"enemy_spawn", "enemy_pool_tag":"combat",
                   "encounter_id":"encounter-room-001", "wave_id":"wave-00",
                   "difficulty":110, "seed":15996169360174610609 } }
  ],
  "chunks": [
    { "id":"chunk-0-0", "bounds":{...}, "room_ids":["room-000", "..."], "dependencies":[] }
  ]
}
```

**关键字段语义：**

- `tiles.data` 是**行优先**的一维数组，索引 `i = y * width + x`。每个元素是字符串枚举：`"Empty"` / `"Floor"` / `"Wall"` / `"Obstacle"` / `"Reserved"` / `"Doorway"`。`Floor`/`Doorway`/`Reserved` 可通行，`Wall`/`Obstacle` 不可通行。
- `bounds`、`grid_pos`、`path` 里的坐标都是**逻辑网格坐标**（整数 tile），不是 UE 世界坐标。坐标系映射见 §3。
- `enemy_spawns[].world_transform` 已经是 yang-pcg 计算好的世界变换，但 `position` 仍是「网格单位」量级（如 `25.0`），**不是 UE 的厘米**。

---

## 2. UE5 侧消费数据

UE5 读 JSON 有三种方式，按集成深度选：

### 2.1 蓝图方式（最快验证，适合策划）

1. 安装/启用 **JsonBlueprintUtilities** 插件（UE5 自带，Edit → Plugins 搜 "Json Blueprint"）。
2. 用 `Load File to String` 读 `floor-1.json` → `From String`（解析为 JsonObject）。
3. 用 `Get Json Object Array`（取 `rooms`）→ 遍历，对每个房间 `Get Json Object Field` 取 `bounds`、`room_type`，在对应世界坐标 `Spawn Actor` 放置房间蓝图。

适合快速跑通，但大地图遍历性能一般，正式项目建议走 C++。

### 2.2 C++ 方式（推荐，正式集成）

**Step 1 — 定义与 JSON 对应的 USTRUCT。** 字段名用 `UPROPERTY` 配合 `FJsonObjectConverter` 自动匹配（注意 UE 默认把字段名首字母大写做匹配，用 `FJsonObjectConverter::JsonObjectStringToUStruct` 时设置 `CheckFlags=0, SkipFlags=0` 并保持字段名小写一致）：

```cpp
// PcgMapData.h
#pragma once
#include "CoreMinimal.h"
#include "PcgMapData.generated.h"

USTRUCT()
struct FPcgGridPoint
{
    GENERATED_BODY()
    UPROPERTY() int32 x = 0;
    UPROPERTY() int32 y = 0;
};

USTRUCT()
struct FPcgBounds
{
    GENERATED_BODY()
    UPROPERTY() FPcgGridPoint min;
    UPROPERTY() FPcgGridPoint max;
};

USTRUCT()
struct FPcgRoom
{
    GENERATED_BODY()
    UPROPERTY() FString id;
    UPROPERTY() FString room_type;
    UPROPERTY() int32 difficulty = 0;
    UPROPERTY() int32 depth_from_start = 0;
    UPROPERTY() FPcgBounds bounds;
};

USTRUCT()
struct FPcgGrid
{
    GENERATED_BODY()
    UPROPERTY() int32 width = 0;
    UPROPERTY() int32 height = 0;
    UPROPERTY() TArray<FString> data;   // 行优先 tile 字符串
};

USTRUCT()
struct FPcgTerrain
{
    GENERATED_BODY()
    UPROPERTY() FString room_id;
    UPROPERTY() FPcgGrid tiles;
};

USTRUCT()
struct FPcgSpawn
{
    GENERATED_BODY()
    UPROPERTY() FString id;
    UPROPERTY() FString room_id;
    UPROPERTY() FString kind;
    UPROPERTY() FPcgGridPoint grid_pos;
};

USTRUCT()
struct FPcgMap
{
    GENERATED_BODY()
    UPROPERTY() TArray<FPcgRoom> rooms;
    UPROPERTY() TArray<FPcgTerrain> terrains;
    UPROPERTY() TArray<FPcgSpawn> item_spawns;
    UPROPERTY() TArray<FPcgSpawn> enemy_spawns;
};
```

**Step 2 — 加载并反序列化：**

```cpp
#include "JsonObjectConverter.h"
#include "Misc/FileHelper.h"

bool ULevelBuilder::LoadPcgMap(const FString& Path, FPcgMap& OutMap)
{
    FString Raw;
    if (!FFileHelper::LoadFileToString(Raw, *Path))
    {
        UE_LOG(LogTemp, Error, TEXT("无法读取地图文件: %s"), *Path);
        return false;
    }
    if (!FJsonObjectConverter::JsonObjectStringToUStruct(Raw, &OutMap, 0, 0))
    {
        UE_LOG(LogTemp, Error, TEXT("地图 JSON 解析失败"));
        return false;
    }
    return true;
}
```

**Step 3 — 把数据变成关卡。** 遍历 tiles 放置地块、遍历 spawns 放置敌人：

```cpp
void ULevelBuilder::BuildLevel(const FPcgMap& Map, UWorld* World)
{
    const float TileSize = 100.f;       // 1 tile = 100 cm（1 米），见 §3

    // 1) 地形：逐房间逐 tile 放地板/墙
    for (const FPcgTerrain& Terrain : Map.terrains)
    {
        // 找到该房间的 bounds.min 作为世界原点偏移
        const FPcgRoom* Room = Map.rooms.FindByPredicate(
            [&](const FPcgRoom& R){ return R.id == Terrain.room_id; });
        if (!Room) continue;
        const FPcgGridPoint Origin = Room->bounds.min;

        const FPcgGrid& G = Terrain.tiles;
        for (int32 ty = 0; ty < G.height; ++ty)
        for (int32 tx = 0; tx < G.width; ++tx)
        {
            const FString& Kind = G.data[ty * G.width + tx];  // 行优先索引
            const int32 wx = Origin.x + tx;                   // 世界网格坐标
            const int32 wy = Origin.y + ty;
            const FVector Loc(wx * TileSize, wy * TileSize, 0.f);

            if (Kind == TEXT("Wall") || Kind == TEXT("Obstacle"))
                SpawnTile(WallClass, Loc, World);
            else if (Kind == TEXT("Floor") || Kind == TEXT("Doorway"))
                SpawnTile(FloorClass, Loc, World);
        }
    }

    // 2) 敌人点位
    for (const FPcgSpawn& S : Map.enemy_spawns)
    {
        const FVector Loc(S.grid_pos.x * TileSize, S.grid_pos.y * TileSize, 0.f);
        World->SpawnActor<AActor>(EnemyClass, Loc, FRotator::ZeroRotator);
    }
}
```

> 提示：`tiles` 的坐标是房间**局部**坐标（从 `0,0` 到 `width-1,height-1`），要加上房间 `bounds.min` 才是全图世界网格坐标。而 `spawns[].grid_pos` 已经是全图世界网格坐标，直接乘 TileSize 即可——别重复加偏移。

---

## 3. 坐标系映射（最容易踩的坑）

yang-pcg 内部用**逻辑网格坐标**（整数，1 单位 = 1 tile），导出数据里所有 `x/y` 都是这个量级。UE5 用**厘米**做世界单位。映射全靠你在 UE 侧选定的 `TileSize`：

```text
UE 世界坐标 (cm) = 网格坐标 × TileSize
```

- 选 `TileSize = 100`：1 tile = 1 米，一个 13×10 的房间 = 13m × 10m。
- 选 `TileSize = 400`：1 tile = 4 米，适合大开间。

**注意 yang-pcg 的 `ue` 适配层**（`ue::adapter::export_named_channels`、`PcgPoint.transform`）做的是 **1:1 直通映射**（网格坐标直接当 `WorldPoint`，z=0，无缩放）。它产出的 `transform.position` 数值仍是网格量级，**不是厘米**。所以无论你用 JSON 还是具名通道，缩放都必须在 UE 侧补上，别假设库已经转成厘米了。

> ℹ️ **关于 `ue` 适配层的现状**：`NamedChannel`/`PcgPoint`/`PropertyValue`/`ChannelKind` 现已全部实现 `serde::Serialize + Deserialize`，可通过 `export_named_channels_json()` 序列化。不过大图（数千通道以上）仍建议走 `export_json`/`export_binary` 主通路作用于完整 `GenerationResult`（性能更好且格式更紧凑），本教程全程采用这条通路。


---

## 4. 自定义生成配置

`GenerationConfig::default()` 生成 10-20 个房间的中等楼层。常调的字段：

```rust
use yang_pcg::config::*;

let mut config = GenerationConfig::default();
config.room_count = RangeU16 { min: 15, max: 25 };          // 房间数
config.critical_path_length = RangeU16 { min: 6, max: 12 }; // 主线长度
config.branch_count = RangeU16 { min: 2, max: 4 };          // 分支数
config.room_size = RoomSizeConfig { min_width: 10, max_width: 16,
                                    min_height: 10, max_height: 16 };
config.terrain.obstacle_density = 0.2;   // 障碍密度 0.0~1.0
config.theme_tags = vec!["ice_cave".to_string()];
```

配置非法（如 `room_count.min < 2`、`min > max`）会在 `generate()` 时返回 `PcgError::Config`，**不会 panic**。归一化逻辑见 `config.rs` 的 `normalize()`。

### 4.1 约束（指定房间类型、排除区）

```rust
use yang_pcg::model::request::{Constraint, AnchorConstraint, ExclusionZoneConstraint};
use yang_pcg::model::room::RoomType;
use yang_pcg::model::geometry::GridPoint;

let constraints = vec![
    // 要求出现一个宝藏房
    Constraint::Anchor(AnchorConstraint {
        label: "want-treasure".into(),
        room_id: None,
        room_type: Some(RoomType::Treasure),
        target_grid_pos: None,
    }),
    // 在该矩形区域内不放任何刷怪点
    Constraint::ExclusionZone(ExclusionZoneConstraint {
        label: "safe-zone".into(),
        min: GridPoint { x: 0, y: 0 },
        max: GridPoint { x: 10, y: 10 },
        exclude_rooms: false,
        exclude_spawns: true,
    }),
];
```

参考 `examples/constrained_generation.rs`。

---

## 5. 路线 B：独立 CLI 工具 + 运行时生成

游戏运行时要按玩家进度生成新楼层时，用 yang-pcg 自带的 `pcg_cli`：UE5 启动它生成地图文件，再读回构建关卡。**该 CLI 已随 crate 提供**（`src/bin/pcg_cli.rs`），无需自己写。

### 5.1 构建 CLI

```bash
cargo build --release --bin pcg_cli -p yang-pcg
# 产物：target/release/pcg_cli(.exe)
```

### 5.2 CLI 用法

```bash
pcg_cli --out floor.json                          # 默认配置 + JSON
pcg_cli --seed 12345 --out floor.json             # 指定种子
pcg_cli --seed 12345 --format binary --out floor.ypcg   # 紧凑二进制 + CRC32
pcg_cli --config dungeon.json --out floor.json    # 从 JSON 文件加载配置
pcg_cli --help                                    # 完整帮助
```

| 选项 | 说明 |
|------|------|
| `--seed <u64>` | 随机种子。省略时**从配置派生确定性种子**（相同配置复现同图）。 |
| `--config <path>` | 配置 JSON 文件路径。省略用默认配置。 |
| `--out <path>` | 输出文件路径（必填）。 |
| `--format <fmt>` | `json`（默认）/ `compact` / `binary`。 |
| `--trace-id <str>` | 追踪标识，写入结果元数据。 |

**退出码**（UE5 侧据此判断成败）：`0` 成功 / `1` 参数错误 / `2` 配置读取失败 / `3` 生成失败（含硬校验）/ `4` 写入失败。

成功时 stdout 打印一行可解析的摘要：
```text
ok seed=12345 rooms=14 items=24 enemies=29 bytes=102402 out=floor.json
```

`--config` 接受一份完整的 `GenerationConfig` JSON（字段必须齐全，缺字段会以 exit 2 报出具体缺哪个）。以下是经验证可直接使用的默认配置样例，按需改动：

```json
{
  "room_count": { "min": 10, "max": 20 },
  "critical_path_length": { "min": 5, "max": 10 },
  "branch_count": { "min": 1, "max": 3 },
  "dead_end_count": { "min": 0, "max": 2 },
  "room_size": { "min_width": 8, "max_width": 16, "min_height": 8, "max_height": 16 },
  "corridor": { "width": 2, "max_turns": 3, "connection_strategy": "Orthogonal" },
  "terrain": { "obstacle_density": 0.2, "min_walkable_ratio": 0.6 },
  "item_spawns": {
    "count_per_room": { "min": 1, "max": 3 },
    "min_spacing": 2,
    "rarity_weights": [0.6, 0.3, 0.1]
  },
  "enemy_spawns": {
    "count_per_room": { "min": 2, "max": 5 },
    "min_spacing": 3,
    "min_distance_from_entrance": 4,
    "base_difficulty_budget": 100
  },
  "chunking": { "chunk_size": 32, "enabled": false },
  "theme_tags": ["default"],
  "generation_mode": "OfflineFullFloor",
  "capability_flags": {
    "runtime_chunked": false,
    "hybrid_precompute": false,
    "grammar_support": false,
    "debug_output": false
  }
}
```

### 5.3 UE5 侧调用 CLI（C++）

用 `FPlatformProcess` 同步启动 CLI、等待退出、检查退出码，然后按 §2.2 解析产出的文件。

```cpp
// PcgRuntimeGenerator.cpp
#include "HAL/PlatformProcess.h"
#include "Misc/Paths.h"
#include "Misc/FileHelper.h"

// 返回 true 表示生成成功，OutFilePath 为产出的地图文件
bool URuntimeGenerator::GenerateFloor(int64 Seed, FString& OutFilePath)
{
    // 1) CLI 可执行路径（随包放在 Binaries/ThirdParty/）
    const FString ExePath = FPaths::Combine(
        FPaths::ProjectDir(), TEXT("Binaries/ThirdParty/pcg_cli.exe"));

    // 2) 输出到工程的可写目录（打包后 Content 只读，用 ProjectSavedDir）
    OutFilePath = FPaths::Combine(
        FPaths::ProjectSavedDir(), FString::Printf(TEXT("floor-%lld.json"), Seed));

    // 3) 组装参数。路径含空格时务必加引号
    const FString Params = FString::Printf(
        TEXT("--seed %lld --out \"%s\""), Seed, *OutFilePath);

    // 4) 同步执行：创建管道捕获 stdout，等待退出，读退出码
    int32 ReturnCode = -1;
    FString StdOut, StdErr;
    const bool bLaunched = FPlatformProcess::ExecProcess(
        *ExePath, *Params, &ReturnCode, &StdOut, &StdErr);

    if (!bLaunched)
    {
        UE_LOG(LogTemp, Error, TEXT("无法启动 pcg_cli: %s"), *ExePath);
        return false;
    }
    if (ReturnCode != 0)
    {
        UE_LOG(LogTemp, Error, TEXT("pcg_cli 失败 (code=%d): %s"), ReturnCode, *StdErr);
        return false;   // 退出码 1-4 对应参数/配置/生成/写入错误
    }

    UE_LOG(LogTemp, Log, TEXT("生成成功: %s"), *StdOut);
    return true;   // 之后用 §2.2 的 LoadPcgMap(OutFilePath, ...) 解析
}
```

> `ExecProcess` 是**阻塞**调用——会卡住调用线程直到 CLI 退出。生成默认配置约 5-10ms，过场加载时可接受；若不想卡主线程，放到 `AsyncTask(ENamedThreads::AnyBackgroundThreadNormalTask, ...)` 里跑，完成后回到游戏线程建关卡。**别在每帧/高频路径上同步调用。**

### 5.4 读取 `.ypcg` 二进制（可选）

如果用 `--format binary`，UE5 侧要先解析 16 字节头再取出 JSON body。格式布局：

```text
0-3:    Magic "YPCG"
4-5:    schema 主版本 (u16 LE)
6-7:    schema 次版本 (u16 LE)
8-11:   保留 (0)
12-15:  body 长度 (u32 LE)
16..:   紧凑 JSON body
末尾4:  CRC32 校验和（覆盖前面全部字节）
```

```cpp
// 从 .ypcg 字节中取出 JSON body 字符串
bool ExtractYpcgBody(const TArray<uint8>& Bytes, FString& OutJson)
{
    constexpr int32 HeaderSize = 16;
    constexpr int32 Crc32Size = 4;
    if (Bytes.Num() < HeaderSize + 1 + Crc32Size) return false;

    // 校验 magic "YPCG"
    if (Bytes[0] != 'Y' || Bytes[1] != 'P' || Bytes[2] != 'C' || Bytes[3] != 'G')
        return false;

    // body 长度在 12-15（小端）
    const uint32 BodyLen =
        (uint32)Bytes[12] | ((uint32)Bytes[13] << 8) |
        ((uint32)Bytes[14] << 16) | ((uint32)Bytes[15] << 24);

    if (HeaderSize + (int32)BodyLen + Crc32Size > Bytes.Num()) return false;

    // 取出 [16, 16+BodyLen) 的 UTF-8 JSON
    OutJson = FString(BodyLen, (const ANSICHAR*)(Bytes.GetData() + HeaderSize));
    return true;   // 之后交给 §2.2 的 JsonObjectStringToUStruct 解析
}
```

> CRC32 校验为可选项：取末 4 字节小端为期望值，对前 `len-4` 字节算 CRC32 比对即可。引擎自带 `FCrc::MemCrc32` 的算法与 `crc32fast` 不同，要严格校验需自行实现标准 CRC-32（IEEE）。多数场景跳过校验、靠文件系统完整性即可。

**JSON 还是二进制怎么选**：JSON 调试友好、UE 直接 `JsonObjectStringToUStruct`，起步首选；二进制体积约小一半、带版本头，适合大量地图或在意包体。


---

## 6. 路线 C：编译为 C 动态库 + FFI（进阶，可选）

如果你坚持要在 UE5 进程内同步调用、零文件 IO，需要**自己写一层 FFI**（库本身不含 `cdylib`/`extern "C"`，需新增）。骨架：

```toml
# 新建 crate yang-pcg-ffi/Cargo.toml
[lib]
crate-type = ["cdylib"]
[dependencies]
yang-pcg = { path = "../yang-pcg" }
```

```rust
// 返回 JSON 字符串指针，UE 侧用完调 free
#[no_mangle]
pub extern "C" fn ypcg_generate_json(seed: u64) -> *mut std::os::raw::c_char {
    let result = yang_pcg::MapGenerator::new().generate(yang_pcg::GenerationRequest {
        seed: Some(seed),
        config: yang_pcg::GenerationConfig::default(),
        constraints: vec![], runtime_context: None, trace_id: None,
    });
    let json = result.and_then(|r| yang_pcg::export_json(&r)).unwrap_or_default();
    std::ffi::CString::new(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn ypcg_free(ptr: *mut std::os::raw::c_char) {
    if !ptr.is_null() { unsafe { drop(std::ffi::CString::from_raw(ptr)); } }
}
```

```bash
cargo build --release -p yang-pcg-ffi
# 产物 .dll(Win)/.so(Linux)/.dylib(Mac)，放进 <Project>/Binaries/ThirdParty/
```

UE5 侧在模块里 `FPlatformProcess::GetDllHandle` 加载，`GetDllExport` 取函数指针调用。**这条路线维护成本最高**（FFI 边界、内存管理、跨平台库分发），只在确有同步调用需求时采用。

---

## 7. 打包与分发清单

| 产物 | 来源命令 | 放到 UE 工程哪里 |
|------|----------|-----------------|
| `*.json` 烘焙地图（路线 A） | `cargo run --release --example basic_generation` | `Content/GeneratedMaps/`（作为外部数据），或转成 DataAsset |
| `pcg_cli` 可执行（路线 B） | `cargo build --release --bin pcg_cli` | `<Project>/Binaries/ThirdParty/` |
| `.dll/.so/.dylib`（路线 C） | `cargo build --release -p yang-pcg-ffi` | `<Project>/Binaries/ThirdParty/`，并在 `.Build.cs` 里 `RuntimeDependencies.Add(...)` |

**跨平台**：Rust 交叉编译目标——Windows `x86_64-pc-windows-msvc`、Android `aarch64-linux-android`、iOS `aarch64-apple-ios`。每个目标分别 `cargo build --release --target <triple>`，产物按平台放入对应 `Binaries` 子目录。

**打包进 UE 包体**：JSON/`.ypcg` 作为 `Content` 下的非 `.uasset` 文件，需在 Project Settings → Packaging → "Additional Non-Asset Directories to Package" 里登记，否则打包后丢失。

---

## 8. 确定性与存档

- **存档只存 `seed` + `config`**（或 `config_digest`），不用存整张地图——重新 `generate` 即可逐字节复现。
- `metadata.config_digest` 是配置指纹，存档回读时比对它能发现"配置变了导致地图对不上"。
- **务必固定生成模式**：本教程全程用默认的 `OfflineFullFloor`。若用 `RuntimeChunked`/`HybridPrecompute`，同 seed 会产出不同地图，存档需记录用了哪种模式。
- **`seed` 语义**：`seed: None` 会从 config 派生确定性种子（相同 config 复现同图）；想要每次不同，显式提供不同 seed。存档存 seed+config 即可复现。

---

## 9. 常见问题排查

| 现象 | 原因 | 解法 |
|------|------|------|
| UE 解析后字段全是默认值 | `FJsonObjectConverter` 字段名大小写不匹配 | USTRUCT 字段名保持与 JSON 一致的小写，`JsonObjectStringToUStruct(..., 0, 0)` |
| 地块位置错乱/重叠 | tiles 局部坐标没加 `bounds.min` 偏移 | tiles 坐标 + 房间 `bounds.min`；spawns 的 `grid_pos` 不要再加偏移 |
| 关卡尺寸太小/太大 | `TileSize` 选错 | 统一在 UE 侧定 `TileSize`，库导出的都是网格量级 |
| 同 seed 两次地图不一样 | 换了生成模式（跨模式不保证） | 固定生成模式 |
| 想序列化 `export_named_channels` 失败 | 确认是否使用 `export_named_channels_json()`（具名通道现已支持 Serialize） | `export_named_channels_json()` 序列化；大图仍建议 `export_json`/`export_binary` 主通路 |
| `.ypcg` 读出来乱码 | 没跳过 16 字节头 | 按 §5 布局先读头再取 body |

---

## 10. 参考

- 可运行示例：`crates/yang-pcg/examples/`（`basic_generation` / `constrained_generation` / `config_normalization` / `ue5_export`）
- 公共 API：`crates/yang-pcg/src/lib.rs` 的 re-export
- 数据模型：`src/model/`（`result.rs` / `room.rs` / `terrain.rs` / `spawn.rs` / `geometry.rs`）
- 导出格式：`src/export/`（JSON）、`src/export/binary/`（二进制 + CRC32）
