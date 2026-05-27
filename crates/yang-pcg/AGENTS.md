# yang-pcg — UE5/Roguelike PCG Library

**Parent:** lib_yang workspace

## OVERVIEW
Deterministic procedural map generator for UE5/Roguelike workflows. Produces topology, room layout, terrain grids, item/enemy spawn points, chunks, exports, debug bundles, and UE-compatible named channels.

## STRUCTURE
```text
yang-pcg/
├── src/
│   ├── lib.rs              # 17 public modules + common re-exports
│   ├── generator.rs        # MapGenerator orchestration entry point
│   ├── config.rs           # GenerationConfig, NormalizedConfig, capability flags
│   ├── error.rs            # PcgError + PcgResult<Box<PcgError>>
│   ├── validation.rs       # reachability/overlap/connectivity/spawn invariant report
│   ├── topology/           # room graph, critical path, branch planning
│   ├── layout/             # room bounds, door anchors, corridors
│   ├── terrain/            # strategy-based room terrain generation
│   ├── spawn/              # item/enemy budgets, sampling, debug tracking
│   ├── constraint/         # anchors, exclusion zones, templates
│   ├── model/              # request/result/room/terrain/spawn/chunk data types
│   ├── ue/                 # UE5 point/channel/streaming adapter
│   ├── export/             # JSON + binary import/export
│   ├── cache/              # in-memory result cache
│   ├── debug/              # DebugBundle, stage stats, spawn debug
│   └── grammar/            # weighted grammar selector, token hooks
├── tests/                  # generation_bench.rs, ignored benchmark-style tests
├── proptest-regressions/   # task27 property regression corpus
└── docs/                   # config/error guides + task summaries
```

## GENERATION PIPELINE
```text
GenerationRequest
  -> validate_request + config.normalize
  -> topology::generate_topology       RNG derive("topology")
  -> layout::solve_layout              RNG derive("layout")
  -> terrain::generate_terrains        RNG derive("terrain")
  -> spawn::generate_spawns            RNG derive("spawn")
  -> ue::streaming::build_chunks
  -> validate_result
  -> debug mode: run_full_validation
```

`RuntimeChunked` delegates to `chunked::generate_chunk`. `HybridPrecompute` uses `generate_topology_only` then `fill_chunk_details`.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Public API | `src/lib.rs` | re-exports `MapGenerator`, config, errors, model types, export helpers |
| Full generation | `src/generator.rs` | stage orchestration, debug timing, trace_id propagation |
| Config defaults/ranges | `src/config.rs` | config normalization and capability flags |
| Deterministic RNG | `src/rng.rs` | stable seed derivation; changing stream names breaks reproducibility |
| Topology | `src/topology/planner.rs` | start/boss path, branches, room type assignment |
| Layout | `src/layout/solver.rs` | row-style room placement; known overlap gaps |
| Terrain | `src/terrain/` | child AGENTS.md covers strategies and fallback |
| Spawn points | `src/spawn/items.rs`, `src/spawn/enemies.rs` | per-room item/enemy point generation and debug variants |
| Constraints | `src/constraint/` | anchors, exclusion zones, template references |
| UE5 adapter | `src/ue/adapter.rs`, `src/ue/channels.rs` | named channels: rooms, doors, corridors, tiles, spawns, debug |
| Export/import | `src/export/`, `src/export/binary/` | schema versioned JSON/binary roundtrip |
| Validation | `src/validation.rs` | structural checks and `ValidationReport` |
| Bench/property tests | `src/tests_task27/`, `tests/generation_bench.rs` | proptest invariants and ignored benchmark cases |
| Requirements/design | `.kiro/specs/ue5-roguelike-map-generator/` | canonical PRD/design/tasks for PCG behavior |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `MapGenerator` | struct | `src/generator.rs` | main API and generation-mode dispatcher |
| `GenerationRequest` | struct | `src/model/request.rs` | seed/config/constraints/runtime context input |
| `GenerationResult` | struct | `src/model/result.rs` | topology, rooms, terrain, spawns, chunks, debug output |
| `GenerationConfig` | struct | `src/config.rs` | high-level generator settings |
| `StableRng` | struct | `src/rng.rs` | deterministic child streams |
| `RoomGraph` / `Room` | structs | `src/model/room.rs` | topology and room metadata |
| `TerrainStrategy` | trait | `src/terrain/strategy.rs` | terrain generation extension point |
| `SpawnPoint` | struct | `src/model/spawn.rs` | item/enemy/interaction point data |
| `NamedChannel` | struct | `src/ue/channels.rs` | UE-compatible channel output |
| `PcgError` | enum | `src/error.rs` | structured error codes/context |

## CONVENTIONS
- Core algorithm must stay independent of UE runtime types; UE mapping belongs under `src/ue/`.
- RNG stream names are part of the determinism contract: `topology`, `layout`, `terrain`, `spawn`, plus per-room item/enemy streams.
- Debug output is side-channel only; enabling `set_debug(true)` must not change gameplay outputs.
- Public docs are Chinese and include requirement mapping comments (`验证需求`).
- `PcgResult<T>` boxes `PcgError`; keep that shape unless addressing enum-size tradeoffs deliberately.
- Config/docs live in both `docs/` and `.kiro/specs/ue5-roguelike-map-generator/`; `.kiro` is the product/design source of truth.

## TESTING
- `cargo test --lib -p yang-pcg` runs unit, model, terrain, export, chunked, task26/task27 tests.
- `src/tests_task27/property_tests.rs` contains proptest invariants; three are `#[ignore]` for known algorithm gaps.
- `tests/generation_bench.rs` contains ignored benchmark-style tests, not criterion benches.
- `proptest-regressions/tests_task27/property_tests.txt` is intentional and should remain.

## ANTI-PATTERNS
- Do not weaken ignored properties for overlap/connectivity/spacing; they encode real unsatisfied invariants.
- Do not change RNG derivation labels casually; this breaks seed reproducibility and golden/debug expectations.
- Do not mix UE-specific concepts into generator/topology/layout/terrain/spawn core modules.
- Do not treat `cache/` as persistent cache; it is currently in-memory only, no TTL/LRU/disk.
- Do not assume Grammar is complete; selector and grammar fields exist, but external Shape Grammar integration is not implemented here.

## KNOWN GAPS
- Layout can produce overlapping room bounds in dense configurations.
- Terrain connectivity can fail for some obstacle densities/strategy outputs.
- Item/enemy spacing is generated separately; cross-type spacing is only caught by validation.
- `INSTALL.md.md` is a double-extension documentation artifact.
- `docs/task_4_summary.md` and `TASK_3_SUMMARY.md` are historical summaries, not source-of-truth specs.
