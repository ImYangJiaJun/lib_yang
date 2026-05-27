# yang-pcg/terrain — Terrain Generation

**Parent:** `crates/yang-pcg/AGENTS.md`

## OVERVIEW
Strategy-based room terrain generation. Converts laid-out rooms and door anchors into `Terrain { Grid2D<TileKind>, reserved_zones, connectivity_summary }`.

## STRUCTURE
```text
terrain/
├── mod.rs                  # generate_terrains orchestration
├── strategy.rs             # TerrainStrategy trait
├── selector.rs             # choose strategy by room type/theme/config
├── default_strategy.rs     # DefaultCarveStrategy wrapper
├── carve.rs                # wall/floor/obstacle/reserved carving helpers
├── connectivity.rs         # connectivity summary helpers
├── grid.rs                 # local/world grid helpers
├── open_arena.rs           # Boss/open center arena
├── pillar.rs               # pillar obstacle strategy
├── maze.rs                 # recursive-backtracking maze strategy
├── organic.rs              # organic/randomized obstacle strategy
└── __tests__/              # strategy tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Terrain entry point | `mod.rs` | `generate_terrains` iterates rooms and selects strategy |
| Strategy contract | `strategy.rs` | `TerrainStrategy::name`, `generate` |
| Strategy choice | `selector.rs` | selection priority and fallback |
| Default room carving | `carve.rs`, `default_strategy.rs` | wall borders, doors, reserved zones, obstacles |
| Boss rooms | `open_arena.rs` | open center, sparse edge obstacles, connectivity cleanup |
| Maze rooms | `maze.rs` | recursive backtracking, doorway connection forcing |
| Pillar/organic variants | `pillar.rs`, `organic.rs` | themed room layouts |
| Connectivity | `connectivity.rs` | walkability and door reachability summary |
| Grid coords | `grid.rs` | local coordinate conversion and tile helpers |
| Tests | `__tests__/strategy_test.rs` | strategy behavior and known edge cases |

## STRATEGY FLOW
```text
for each Room:
  collect DoorAnchor for room
  select TerrainStrategy by RoomType/theme/config
  strategy.generate(room, anchors, config, rng)
  if strategy fails where fallback is allowed -> DefaultCarveStrategy
  summarize connectivity
```

Typical priority: Boss/open arena first, then explicit maze/pillar/organic themes, then default carve.

## TILE MODEL
`TileKind` lives in `model/terrain.rs` and includes `Empty`, `Floor`, `Wall`, `Obstacle`, `Reserved`, `Doorway`. Only floor/doorway/reserved are walkable in validation.

## CONVENTIONS
- Door anchors must become `Doorway` tiles before obstacles are placed.
- Terrain coordinates are room-local; convert from room bounds with helpers in `grid.rs`.
- Keep strategy-specific randomness on the terrain RNG stream supplied by `generator.rs`.
- Boss/open arena should preserve central playable space and use reserved zones deliberately.
- Connectivity summaries are diagnostics; full invariants are checked in `validation.rs`.

## KNOWN GAPS
- Some terrain outputs still fail entrance-to-exit connectivity under property tests; `prop_terrain_connectivity` is ignored for this reason.
- Obstacle placement is heuristic, not a full constraint solver.
- Maze strategy forces doorway connectivity after carving, but not every strategy has equally strong repair logic.
- Fallback to default strategy improves robustness but can hide a strategy-specific failure if not logged/tested.

## ANTI-PATTERNS
- Do not weaken connectivity property tests; fix generation/repair instead.
- Do not place obstacles on `Doorway` or required reserved tiles.
- Do not assume `bounds` exists; strategies must return a structured `PcgError` for missing/zero bounds.
- Do not add a new strategy without updating `selector.rs`, tests, and this file.
