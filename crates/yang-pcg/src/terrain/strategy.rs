// 地形生成策略 trait 定义
// 提供统一的地形生成接口，支持多种策略实现

use crate::config::TerrainConfig;
use crate::error::PcgResult;
use crate::model::room::{DoorAnchor, Room};
use crate::model::terrain::Terrain;
use crate::rng::StableRng;

/// 地形策略枚举 —— 静态派发替代 `Box<dyn TerrainStrategy>`
///
/// 每个变体对应一种地形生成策略。通过 [`generate`](TerrainStrategyKind::generate)
/// 方法统一派发到各具体策略结构体的实现，消除动态派发开销。
///
/// # 变体
///
/// | 变体 | 对应结构体 | 触发条件 |
/// |------|-----------|---------|
/// | `DefaultCarve` | [`DefaultCarveStrategy`] | 默认回退 |
/// | `OpenArena` | [`OpenArenaStrategy`] | Boss 房间 |
/// | `Maze` | [`MazeStrategy`] | Puzzle + maze 标签 |
/// | `Organic` | [`OrganicStrategy`] | organic / cave 标签 |
/// | `Pillar` | [`PillarStrategy`] | Combat/Elite + pillar 标签 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainStrategyKind {
    /// 默认雕刻策略
    DefaultCarve,
    /// 开放竞技场策略
    OpenArena,
    /// 迷宫策略
    Maze,
    /// 有机洞穴策略
    Organic,
    /// 柱状掩体策略
    Pillar,
}

/// 地形生成策略 trait
///
/// 定义统一的地形生成接口，所有地形策略（如开放式、柱状、迷宫式、有机式等）
/// 都必须实现此 trait。策略选择由房间类型、主题标签和模板引用共同决定。
///
/// # 设计原则
///
/// 1. **统一接口**：所有策略共享相同的输入输出签名
/// 2. **可组合**：策略可以独立实现和测试
/// 3. **确定性**：相同输入（含 RNG 状态）产生相同输出
/// 4. **可识别**：每个策略有唯一名称，便于调试和日志
///
/// # 示例
///
/// ```rust,ignore
/// use yang_pcg::terrain::strategy::TerrainStrategy;
///
/// struct MyCustomStrategy;
///
/// impl TerrainStrategy for MyCustomStrategy {
///     fn name(&self) -> &str {
///         "custom"
///     }
///
///     fn generate(
///         &self,
///         room: &Room,
///         anchors: &[DoorAnchor],
///         config: &TerrainConfig,
///         rng: &mut StableRng,
///     ) -> PcgResult<Terrain> {
///         // 自定义地形生成逻辑
///         todo!()
///     }
/// }
/// ```
pub trait TerrainStrategy: Send + Sync {
    /// 获取策略名称
    ///
    /// 返回策略的唯一标识名称，用于调试输出和日志记录。
    fn name(&self) -> &str;

    /// 生成房间地形
    ///
    /// 根据房间信息、门锚点、地形配置和随机数生成器，生成完整的房间地形数据。
    ///
    /// # 参数
    ///
    /// * `room` - 目标房间，包含房间类型、边界等信息
    /// * `anchors` - 属于该房间的门锚点列表
    /// * `config` - 地形生成配置（障碍物密度、最小可通行比例等）
    /// * `rng` - 确定性随机数生成器
    ///
    /// # 返回
    ///
    /// 成功时返回生成的 `Terrain`，失败时返回 `PcgError`
    ///
    /// # 不变量
    ///
    /// 实现必须保证：
    /// 1. 所有门口位置标记为 `Doorway`
    /// 2. 门口之间存在可通行路径（连通性）
    /// 3. 可通行面积不低于配置的最小比例
    fn generate(
        &self,
        room: &Room,
        anchors: &[DoorAnchor],
        config: &TerrainConfig,
        rng: &mut StableRng,
    ) -> PcgResult<Terrain>;
}
