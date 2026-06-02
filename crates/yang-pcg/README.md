# yang-pcg

YANG 程序化内容生成库 —— 面向 UE5 / Roguelike 的确定性地图生成。
与 `yang-db` / `yang-base` 解耦，不依赖数据库或后端原语。

## 功能特性

- **确定性生成**：同一 `seed` 必产出同一地图，是库的核心契约（黄金测试守护）。
- **完整生成管线**：拓扑（topology）→ 布局（layout）→ 地形（terrain）→ 刷怪点（spawn）→ 分块（chunks）。
- **多种生成模式**：一次性生成、`RuntimeChunked`（按需出块）、`HybridPrecompute`（先拓扑后填充）。
- **UE5 适配层**：UE 相关概念集中在 `src/ue/`，与 core 模块严格隔离。
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

## 测试

```bash
cargo test --lib -p yang-pcg
```

## 许可证

MIT OR Apache-2.0
