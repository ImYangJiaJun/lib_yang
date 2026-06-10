# yang-pcg

YANG 程序化内容生成库 —— 面向 UE5 / Roguelike 的确定性地图生成。
与 `yang-db` / `yang-base` 解耦，不依赖数据库或后端原语。

## 功能特性

- **确定性生成**：同一 `seed` 必产出同一地图，是库的核心契约（黄金测试守护）。
- **完整生成管线**：拓扑（topology）→ 布局（layout）→ 地形（terrain）→ 刷怪点（spawn）→ 分块（chunks）。
- **多种生成模式**：一次性生成、`RuntimeChunked`（按需出块）、`HybridPrecompute`（先拓扑后填充）。
- **UE5 适配层**：UE 相关概念集中在 `src/ue/`，与 core 模块严格隔离。具名通道（`NamedChannel`）可序列化落盘。
- **多种导出格式**：`export_json` / `export_json_compact` / `export_binary`（`.ypcg`，带版本头 + CRC32）。
- **命令行工具**：`pcg_cli` 支持运行时按种子/配置生成地图文件，供 UE5 等宿主调用（路线 B）。
- **可校验**：debug 模式下运行 `run_full_validation`（可达性、重叠、连通性、刷怪间距），且不改变 gameplay 输出。

## 生成管线

```text
GenerationRequest
  -> validate_request + config.normalize
  -> topology::generate_topology     RNG.derive("topology")
  -> layout::solve_layout            RNG.derive("layout")
  -> terrain::generate_terrains      RNG.derive("terrain")
  -> spawn::generate_spawns          RNG.derive("spawn")
  -> ue::streaming::build_chunks
  -> validate_result
```

RNG 派生标签（`topology` / `layout` / `terrain` / `spawn` 及每个房间的 item/enemy 流名）
是确定性契约的一部分，改名等于破坏 seed 复现性。

## 核心类型

- `MapGenerator` —— 生成编排入口。
- `GenerationConfig` —— 配置归一化与默认值。
- `PcgError` —— 结构化错误码与上下文。

## UE5 集成与打包

yang-pcg 是**纯 Rust 算法库**，不是 UE5 插件、不走 FFI。它产出**确定性地图数据**，
由 UE5 侧读取并构建关卡——数据通路是：

```text
yang-pcg (Rust 生成)  ──export_json / export_binary──>  .json / .ypcg 文件  ──C++/蓝图解析──>  UE5 关卡
```

完整的打包与使用教程（含 UE5 侧 C++ USTRUCT 解析、坐标系映射、三条集成路线、
打包清单、排查表）见 **[UE5_INTEGRATION.md](UE5_INTEGRATION.md)**。

最小导出示例：

```rust
use yang_pcg::{export_json, GenerationConfig, GenerationRequest, MapGenerator};

let result = MapGenerator::new().generate(GenerationRequest {
    seed: Some(20260610),                 // None 时从 config 派生确定性种子
    config: GenerationConfig::default(),
    constraints: vec![],
    runtime_context: None,
    trace_id: None,
})?;
std::fs::write("floor.json", export_json(&result)?)?;
```

> ⚠️ 确定性保证是**同一生成模式内**成立的；三种模式（`OfflineFullFloor` /
> `RuntimeChunked` / `HybridPrecompute`）下同一 seed 会产出不同地图。

### 命令行工具 `pcg_cli`（运行时生成 / 路线 B）

crate 自带 `pcg_cli`，供 UE5 等宿主在运行时调用生成地图文件：

```bash
cargo build --release --bin pcg_cli -p yang-pcg
pcg_cli --seed 12345 --out floor.json              # JSON
pcg_cli --seed 12345 --format binary --out floor.ypcg   # 二进制 + CRC32
pcg_cli --config dungeon.json --out floor.json     # 从文件加载配置
pcg_cli --help
```

退出码：`0` 成功 / `1` 参数 / `2` 配置 / `3` 生成 / `4` 写入。详见 [UE5_INTEGRATION.md](UE5_INTEGRATION.md) §5。

## 测试

```bash
cargo test --lib -p yang-pcg
```

## 许可证

MIT OR Apache-2.0
