// 管线后端抽象
//
// 把拓扑之后的全部空间语义（布局/地形/点位/校验）抽象为 `PipelineBackend` trait，
// 由具体地图种类多态实现。本迭代只实现俯视角 `TopDownBackend`；
// `select_backend` 暂时无条件返回它，为后续接入横版平台跳跃 backend 预留 seam。
//
// trait 方法是 **阶段级** 的：调用方（generator / chunked）继续在外层派生 RNG
// （`derive("layout")` / `derive("terrain")` / `derive("spawn")` 等），
// trait 本身不引入任何新的派生点，因此不影响确定性派生标签链。

pub mod topdown;

use crate::config::NormalizedConfig;
use crate::error::PcgResult;
use crate::layout::LayoutOutput;
use crate::model::request::Constraint;
use crate::model::result::GenerationResult;
use crate::model::room::{DoorAnchor, Room, RoomGraph};
use crate::model::terrain::Terrain;
use crate::rng::StableRng;
use crate::spawn::{SpawnOutput, SpawnOutputWithDebug};

/// 校验范围。
///
/// 整层结果与分块部分结果对「哪些不变量应被强制」有不同要求：
/// 部分结果（分块切片）天然不满足「rooms 数量 == topology.nodes 数量」之类的整图结构检查，
/// 也不应被当作独立可达图校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationScope {
    /// 整层结果：覆盖完整图的全部不变量（结构一致性 + 可达性 + 重叠 + 连通 + 间距）。
    FullFloor,
    /// 分块部分结果：跳过整图结构计数与整图可达性，仅保留对任意子集成立的局部不变量
    ///（无重叠 + 地形连通 + 点位间距）。
    Chunk,
}

/// 拓扑之后的全部空间语义，按地图种类多态实现。
///
/// 数据容器（`LayoutOutput` / `Terrain` / `SpawnOutput` / `GenerationResult`）跨实现共享，
/// trait 只分叉算法不分叉数据类型——UE 导出、序列化、结果组装因此保持地图种类无关。
pub trait PipelineBackend: Send + Sync {
    /// 布局：`RoomGraph` → 房间边界/门锚/走廊。RNG 已在外层派生为 `"layout"`。
    fn solve_layout(
        &self,
        graph: &RoomGraph,
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<LayoutOutput>;

    /// 地形：为给定房间逐个雕刻网格。RNG 已在外层派生（`"terrain"` 或分块标签）。
    /// 策略回退失败必须返回 `Err`（不再静默丢房间）。
    fn generate_terrains(
        &self,
        rooms: &[Room],
        door_anchors: &[DoorAnchor],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Vec<Terrain>>;

    /// 点位：为所有房间采样交互物/敌人。RNG 已在外层派生为 `"spawn"`；
    /// 每房间 `items:{id}` / `enemies:{id}` 派生在内部完成，标签不变。
    fn generate_spawns(
        &self,
        rooms: &[Room],
        terrains: &[Terrain],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<SpawnOutput>;

    /// 点位的调试变体（带候选/拒绝跟踪），仅在调试模式下使用。
    fn generate_spawns_with_debug(
        &self,
        rooms: &[Room],
        terrains: &[Terrain],
        config: &NormalizedConfig,
        rng: &mut StableRng,
    ) -> PcgResult<SpawnOutputWithDebug>;

    /// 可达性 + 结构校验，进生产路径。`scope` 决定整层校验集还是分块子集。
    /// `config` 提供点位间距阈值（以及后续 backend 的能力参数，如跳跃高度）。
    fn validate(
        &self,
        result: &GenerationResult,
        config: &NormalizedConfig,
        constraints: &[Constraint],
        scope: ValidationScope,
    ) -> PcgResult<()>;
}

/// 按配置选择 backend。
///
/// 本迭代尚未引入 `MapKind`，故无条件返回 `TopDownBackend`；
/// 下一迭代在此处按 `config.map_kind` 增加 `SidePlatformerBackend` 分支即可，
/// 编排代码（generator / chunked）无需改动。
pub fn select_backend(_config: &NormalizedConfig) -> Box<dyn PipelineBackend> {
    Box::new(topdown::TopDownBackend)
}
