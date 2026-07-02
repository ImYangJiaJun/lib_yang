// 确定性随机数生成模块
// 提供稳定的、可派生子流的随机接口

use rand::RngExt;
use rand::TryRng;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64;
use std::convert::Infallible;

// ============================================================================
// RNG 派生标签契约（确定性种子复现的基础）
// ============================================================================
//
// 以下列出 PCG 管线中所有 `StableRng::derive(label)` 调用使用的标签字符串。
// 标签通过 FNV-1a 哈希生成子种子，因此标签的**任何改动**（重命名、增删字符、
// 调整大小写）都会改变派生出的随机流，破坏相同种子下的地图复现性。
//
// 变更纪律：
//   - 绝对不要重命名已有标签。如需新标签，追加新的。
//   - 绝对不要改变标签的派生层级（从哪个父 RNG 派生）。因为父 RNG 的种子不同，
//     同一标签字符串也会产生不同的子种子。
//   - 绝对不要在已有标签之前插入新的 derive() 调用——PCG64 是顺序生成器，
//     额外的 derive 调用会消费父 RNG 的随机字节，使后续 derive 的种子漂移。
//     StableRng::derive() 仅基于当前 seed 做哈希，不消费 inner RNG，
//     所以同层级新增 derive 不会影响已有的派生标签；但在同一父 RNG 上增删不同
//     标签的 derive 调用顺序不影响结果——derive 是纯函数，只依赖 self.seed 和
//     label 字符串。
//
// ============================
// 模式一：OfflineFullFloor（generator.rs: generate）
// ============================
//
// 第一阶段标签（直接派生自根 RNG）：
//   "topology"          generator.rs:66    topology::generate_topology
//   "layout"            generator.rs:73    backend.solve_layout
//   "terrain"           generator.rs:79    backend.generate_terrains
//   "spawn"             generator.rs:90    generate_spawn_full_floor
//
// 第二阶段标签（派生自 "terrain" RNG）：
//   "fallback:{id}"     terrain/mod.rs:60  DefaultCarveStrategy 地形回退
//     ^-- {id} = room.id，如 "fallback:Room_0"
//
// 第二阶段标签（派生自 "spawn" RNG）：
//   "items:{id}"        spawn/mod.rs:81    generate_item_spawns_for_room
//   "enemies:{id}"      spawn/mod.rs:82    generate_enemy_spawns_for_room_excluding
//     ^-- {id} = room.id，如 "items:Room_0"、"enemies:Room_0"
//   （带调试跟踪版本同路径：spawn/mod.rs:132-133）
//
// ============================
// 模式二：RuntimeChunked（chunked.rs: generate_chunk）
// ============================
//
// 第一阶段标签（直接派生自根 RNG）：
//   "topology"          chunked.rs:312     topology::generate_topology
//   "layout"            chunked.rs:317     backend.solve_layout
//
// 第二阶段标签（直接派生自根 RNG）：
//   "terrain:{id}"      chunked.rs:391     per-room terrain generation
//     ^-- {id} = room.id，如 "terrain:Room_0"
//   "items:{id}"        chunked.rs:404     generate_item_spawns_for_room
//   "enemies:{id}"      chunked.rs:405     generate_enemy_spawns_for_room_excluding
//     ^-- {id} = room.id，如 "items:Room_0"、"enemies:Room_0"
//
// 回退标签（地形策略失败时，直接派生自根 RNG）：
//   "terrain:fallback:{id}"  chunked.rs:437  DefaultCarveStrategy fallback
//     ^-- {id} = room.id，如 "terrain:fallback:Room_0"
//
// ============================
// 模式三：HybridPrecompute（chunked.rs: generate_topology_only + fill_chunk_details）
// ============================
//
// 第一阶段（generate_topology_only）：
//   "topology"          chunked.rs:93      topology::generate_topology
//   "layout"            chunked.rs:98      backend.solve_layout
//
// 第二阶段 — 按分块填充（fill_chunk_details），直接派生自根 RNG：
//   "terrain:chunk:{c}:{r}"    chunked.rs:191  per-chunk per-room terrain
//   "items:chunk:{c}:{r}"      chunked.rs:210  per-chunk per-room items
//   "enemies:chunk:{c}:{r}"    chunked.rs:212  per-chunk per-room enemies
//     ^-- {c} = chunk_id, {r} = room.id
//     如 "terrain:chunk:Chunk_0:Room_0"
//
// 回退标签（地形策略失败时，直接派生自根 RNG）：
//   "terrain:fallback:{c}:{r}"  chunked.rs:246  DefaultCarveStrategy fallback
//     ^-- {c} = chunk_id, {r} = room.id
//     如 "terrain:fallback:Chunk_0:Room_0"
//
// ============================
// 跨模式兼容性说明
// ============================
//
// 同一 seed 在不同模式下会产出不同地图，这是**有意设计**：
// 三种模式的 RNG 派生路径不同（标签字符串、派生层级、派生顺序均不同），
// 因此相同的根种子会产生不同的子 RNG 序列。这不是 bug——分块/混合路径需要
// 按 chunk 维度派生标签以避免不同 chunk 间的随机数碰撞，而整层路径按阶段
// 派生更高效。
//
// 如果想跨模式获得可比较的结果，必须在各模式的请求中显式指定相同的 seed，
// 并接受不同模式的自然差异。

/// FNV-1a 64-bit hash — deterministic, stable across all Rust versions.
/// 用于替代 std DefaultHasher（SipHash 无跨版本稳定契约）。
pub(crate) fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 稳定的随机数生成器接口
///
/// 提供确定性的随机数生成能力，支持从根种子派生子随机流。
/// 相同的种子和派生路径保证生成相同的随机序列。
///
/// # 设计原则
///
/// 1. **确定性**：相同输入产生相同输出
/// 2. **可派生**：支持按阶段、房间、走廊等维度派生子流
/// 3. **隔离性**：调试流不影响玩法流
/// 4. **稳定性**：底层 PRNG 算法固定，不随 Rust 版本变化
///
/// # 示例
///
/// ```
/// use yang_pcg::rng::StableRng;
///
/// // 从根种子创建
/// let mut rng = StableRng::from_seed(12345);
///
/// // 生成范围内的随机数 [0, 100)
/// let value = rng.random_range(0, 100);
/// assert!((0..100).contains(&value));
///
/// // 派生子流
/// let mut topology_rng = rng.derive("topology");
/// let mut room_rng = rng.derive("room:0");
///
/// // 相同种子和派生路径产生相同结果
/// let mut rng1 = StableRng::from_seed(12345);
/// let mut rng2 = StableRng::from_seed(12345);
/// assert_eq!(rng1.random_range(0, 100), rng2.random_range(0, 100));
/// ```
#[non_exhaustive]
pub struct StableRng {
    /// 底层 PCG 随机数生成器
    inner: Pcg64,
    /// 当前种子值（用于派生和调试）
    seed: u64,
}

impl StableRng {
    /// 从 u64 种子创建新的随机数生成器
    ///
    /// # 参数
    ///
    /// * `seed` - 随机种子，相同种子产生相同随机序列
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// ```
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: Pcg64::seed_from_u64(seed),
            seed,
        }
    }

    /// 从字节数组种子创建随机数生成器
    ///
    /// 提供更灵活的种子输入方式，适合从外部系统导入种子。
    ///
    /// # 参数
    ///
    /// * `seed` - 32 字节的种子数组
    pub fn from_seed_bytes(seed: [u8; 32]) -> Self {
        let inner = Pcg64::from_seed(seed);
        // 从字节数组计算一个 u64 种子用于记录（FNV-1a，跨版本稳定）
        let seed_u64 = fnv1a_64(&seed);

        Self {
            inner,
            seed: seed_u64,
        }
    }

    /// 派生子随机流
    ///
    /// 基于当前种子和派生标签生成新的独立随机流。
    /// 相同的种子和标签总是产生相同的子流。
    ///
    /// # 参数
    ///
    /// * `label` - 派生标签，用于区分不同用途的随机流
    ///
    /// # 派生标签约定
    ///
    /// 建议使用以下命名约定：
    /// - `"topology"` - 拓扑生成阶段
    /// - `"layout"` - 空间布局阶段
    /// - `"terrain"` - 地形生成阶段
    /// - `"spawn:items"` - 交互物点位生成
    /// - `"spawn:enemies"` - 敌人点位生成
    /// - `"room:{id}"` - 特定房间的随机流
    /// - `"corridor:{id}"` - 特定走廊的随机流
    /// - `"debug:{name}"` - 调试专用随机流（不影响玩法）
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut root_rng = StableRng::from_seed(12345);
    ///
    /// // 派生不同阶段的随机流
    /// let mut topology_rng = root_rng.derive("topology");
    /// let mut layout_rng = root_rng.derive("layout");
    ///
    /// // 派生房间级随机流
    /// let mut room_0_rng = topology_rng.derive("room:0");
    /// let mut room_1_rng = topology_rng.derive("room:1");
    /// ```
    pub fn derive(&self, label: &str) -> Self {
        // 使用当前种子和标签计算新种子（FNV-1a，跨版本稳定）
        let mut bytes = self.seed.to_le_bytes().to_vec();
        bytes.extend_from_slice(label.as_bytes());
        let derived = fnv1a_64(&bytes);

        Self::from_seed(derived)
    }

    /// 获取当前种子值
    ///
    /// 用于调试、日志记录和结果元数据。
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// 生成指定范围内的随机整数 [min, max)
    ///
    /// # 参数
    ///
    /// * `min` - 最小值（包含）
    /// * `max` - 最大值（不包含）
    ///
    /// # Panics
    ///
    /// 当 `min >= max` 时会 panic
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let value = rng.random_range(1, 10); // 生成 [1, 10) 范围内的整数
    /// assert!(value >= 1 && value < 10);
    /// ```
    pub fn random_range<T>(&mut self, min: T, max: T) -> T
    where
        T: rand::distr::uniform::SampleUniform + PartialOrd,
    {
        self.inner.random_range(min..max)
    }

    /// 生成随机布尔值
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let value = rng.gen_bool();
    /// ```
    pub fn gen_bool(&mut self) -> bool {
        self.inner.random()
    }

    /// 以指定概率生成布尔值
    ///
    /// # 参数
    ///
    /// * `probability` - 返回 true 的概率，范围 [0.0, 1.0]
    ///
    /// # Panics
    ///
    /// 当 `probability` 不在 [0.0, 1.0] 范围内时会 panic
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let value = rng.gen_bool_with_probability(0.7); // 70% 概率返回 true
    /// ```
    pub fn gen_bool_with_probability(&mut self, probability: f64) -> bool {
        if !probability.is_finite() {
            return false;
        }
        if probability <= 0.0 {
            return false;
        }
        if probability >= 1.0 {
            return true;
        }
        self.inner.random_bool(probability)
    }

    /// 生成随机浮点数 [0.0, 1.0)
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let value = rng.gen_f64();
    /// assert!(value >= 0.0 && value < 1.0);
    /// ```
    pub fn gen_f64(&mut self) -> f64 {
        self.inner.random()
    }

    /// 生成随机浮点数 [0.0, 1.0)
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let value = rng.gen_f32();
    /// assert!(value >= 0.0 && value < 1.0);
    /// ```
    pub fn gen_f32(&mut self) -> f32 {
        self.inner.random()
    }

    /// 从切片中随机选择一个元素
    ///
    /// # 参数
    ///
    /// * `slice` - 要选择的切片
    ///
    /// # 返回
    ///
    /// 返回随机选中的元素引用，如果切片为空则返回 None
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let items = vec![1, 2, 3, 4, 5];
    /// if let Some(&item) = rng.choose(&items) {
    ///     assert!(items.contains(&item));
    /// }
    /// ```
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            return None;
        }
        let index = self.random_range(0, slice.len());
        Some(&slice[index])
    }

    /// 从切片中随机选择一个元素的可变引用
    ///
    /// # 参数
    ///
    /// * `slice` - 要选择的可变切片
    ///
    /// # 返回
    ///
    /// 返回随机选中的元素可变引用，如果切片为空则返回 None
    pub fn choose_mut<'a, T>(&mut self, slice: &'a mut [T]) -> Option<&'a mut T> {
        if slice.is_empty() {
            return None;
        }
        let index = self.random_range(0, slice.len());
        Some(&mut slice[index])
    }

    /// 打乱切片中的元素顺序
    ///
    /// 使用 Fisher-Yates 洗牌算法，保证每种排列出现的概率相同。
    ///
    /// # 参数
    ///
    /// * `slice` - 要打乱的可变切片
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let mut items = vec![1, 2, 3, 4, 5];
    /// rng.shuffle(&mut items);
    /// // items 现在是随机顺序
    /// ```
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let len = slice.len();
        for i in (1..len).rev() {
            let j = self.random_range(0, i + 1);
            slice.swap(i, j);
        }
    }

    /// 从切片中随机采样 n 个不重复的元素
    ///
    /// # 参数
    ///
    /// * `slice` - 要采样的切片
    /// * `n` - 采样数量
    ///
    /// # 返回
    ///
    /// 返回采样结果的向量，如果 n 大于切片长度，则返回所有元素的随机排列
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let items = vec![1, 2, 3, 4, 5];
    /// let sampled = rng.sample(&items, 3);
    /// assert_eq!(sampled.len(), 3);
    /// ```
    pub fn sample<T: Clone>(&mut self, slice: &[T], n: usize) -> Vec<T> {
        if n >= slice.len() {
            let mut result = slice.to_vec();
            self.shuffle(&mut result);
            return result;
        }

        let mut indices: Vec<usize> = (0..slice.len()).collect();
        self.shuffle(&mut indices);

        indices
            .into_iter()
            .take(n)
            .map(|i| slice[i].clone())
            .collect()
    }

    /// 根据权重从切片中随机选择一个元素
    ///
    /// # 参数
    ///
    /// * `slice` - 要选择的切片
    /// * `weights` - 每个元素的权重，必须与 slice 长度相同
    ///
    /// # 返回
    ///
    /// 返回随机选中的元素引用，如果切片为空或权重总和为 0 则返回 None
    ///
    /// 当 slice 和 weights 长度不匹配时返回 `None`（不会 panic）。
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_pcg::rng::StableRng;
    ///
    /// let mut rng = StableRng::from_seed(42);
    /// let items = vec!["common", "rare", "epic"];
    /// let weights = vec![70.0, 25.0, 5.0]; // 70%, 25%, 5% 的概率
    /// if let Some(&item) = rng.choose_weighted(&items, &weights) {
    ///     assert!(items.contains(&item));
    /// }
    /// ```
    pub fn choose_weighted<'a, T>(&mut self, slice: &'a [T], weights: &[f64]) -> Option<&'a T> {
        if slice.len() != weights.len() {
            return None;
        }

        if slice.is_empty() {
            return None;
        }

        // OPT-L-02: 拒绝负权重
        if weights.iter().any(|&w| w < 0.0) {
            return None;
        }

        let total_weight: f64 = weights.iter().sum();
        if !total_weight.is_finite() || total_weight <= 0.0 {
            return None;
        }

        let mut random_value = self.gen_f64() * total_weight;
        for (i, &weight) in weights.iter().enumerate() {
            random_value -= weight;
            if random_value <= 0.0 {
                return Some(&slice[i]);
            }
        }

        // 由于浮点精度问题，可能会到达这里，返回最后一个元素
        Some(&slice[slice.len() - 1])
    }
}

impl TryRng for StableRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        Ok(self.inner.next_u32())
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        Ok(self.inner.next_u64())
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Infallible> {
        self.inner.fill_bytes(dest);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_generation() {
        // 验证需求 2.2：相同种子产生相同结果
        let mut rng1 = StableRng::from_seed(12345);
        let mut rng2 = StableRng::from_seed(12345);

        for _ in 0..100 {
            assert_eq!(rng1.random::<u32>(), rng2.random::<u32>());
        }
    }

    #[test]
    fn test_different_seeds_produce_different_results() {
        // 验证不同种子产生不同结果
        let mut rng1 = StableRng::from_seed(12345);
        let mut rng2 = StableRng::from_seed(54321);

        let values1: Vec<u32> = (0..10).map(|_| rng1.random()).collect();
        let values2: Vec<u32> = (0..10).map(|_| rng2.random()).collect();

        assert_ne!(values1, values2);
    }

    #[test]
    fn test_derive_stream() {
        // 验证需求 2.3：支持派生子随机流
        let root_rng = StableRng::from_seed(12345);

        let mut topology_rng1 = root_rng.derive("topology");
        let mut topology_rng2 = root_rng.derive("topology");

        // 相同派生路径产生相同结果
        for _ in 0..100 {
            assert_eq!(topology_rng1.random::<u32>(), topology_rng2.random::<u32>());
        }
    }

    #[test]
    fn test_different_derive_labels() {
        // 验证不同派生标签产生不同随机流
        let root_rng = StableRng::from_seed(12345);

        let mut topology_rng = root_rng.derive("topology");
        let mut layout_rng = root_rng.derive("layout");

        let topology_values: Vec<u32> = (0..10).map(|_| topology_rng.random()).collect();
        let layout_values: Vec<u32> = (0..10).map(|_| layout_rng.random()).collect();

        assert_ne!(topology_values, layout_values);
    }

    #[test]
    fn test_nested_derive() {
        // 验证多层派生
        let root_rng = StableRng::from_seed(12345);
        let topology_rng = root_rng.derive("topology");

        let mut room_0_rng1 = topology_rng.derive("room:0");
        let mut room_0_rng2 = topology_rng.derive("room:0");

        // 相同派生路径产生相同结果
        for _ in 0..100 {
            assert_eq!(room_0_rng1.random::<u32>(), room_0_rng2.random::<u32>());
        }
    }

    #[test]
    fn test_debug_stream_isolation() {
        // 验证需求 2.6：调试流不影响玩法流
        let root_rng = StableRng::from_seed(12345);

        // 创建玩法流
        let mut gameplay_rng1 = root_rng.derive("topology");
        let gameplay_values1: Vec<u32> = (0..10).map(|_| gameplay_rng1.random()).collect();

        // 创建玩法流和调试流
        let mut gameplay_rng2 = root_rng.derive("topology");
        let mut _debug_rng = root_rng.derive("debug:visualization");
        // 使用调试流
        for _ in 0..100 {
            _debug_rng.random::<u32>();
        }
        let gameplay_values2: Vec<u32> = (0..10).map(|_| gameplay_rng2.random()).collect();

        // 玩法流结果应该相同
        assert_eq!(gameplay_values1, gameplay_values2);
    }

    #[test]
    fn test_random_range() {
        let mut rng = StableRng::from_seed(42);

        for _ in 0..100 {
            let value = rng.random_range(1, 10);
            assert!((1..10).contains(&value));
        }
    }

    #[test]
    fn test_gen_bool_with_probability() {
        let mut rng = StableRng::from_seed(42);

        // 测试极端概率
        assert!(!rng.gen_bool_with_probability(0.0));
        assert!(rng.gen_bool_with_probability(1.0));

        // 测试中等概率（统计验证）
        let mut true_count = 0;
        let iterations = 10000;
        for _ in 0..iterations {
            if rng.gen_bool_with_probability(0.7) {
                true_count += 1;
            }
        }

        let ratio = true_count as f64 / iterations as f64;
        // 允许 5% 的误差
        assert!((ratio - 0.7).abs() < 0.05);
    }

    #[test]
    fn test_choose() {
        let mut rng = StableRng::from_seed(42);
        let items = vec![1, 2, 3, 4, 5];

        for _ in 0..100 {
            if let Some(&item) = rng.choose(&items) {
                assert!(items.contains(&item));
            }
        }

        // 测试空切片
        let empty: Vec<i32> = vec![];
        assert!(rng.choose(&empty).is_none());
    }

    #[test]
    fn test_shuffle() {
        let mut rng = StableRng::from_seed(42);
        let mut items = vec![1, 2, 3, 4, 5];
        let original = items.clone();

        rng.shuffle(&mut items);

        // 验证元素相同但顺序可能不同
        let mut sorted_items = items.clone();
        sorted_items.sort();
        assert_eq!(sorted_items, original);
    }

    #[test]
    fn test_shuffle_deterministic() {
        // 验证相同种子的洗牌结果相同
        let mut rng1 = StableRng::from_seed(42);
        let mut items1 = vec![1, 2, 3, 4, 5];
        rng1.shuffle(&mut items1);

        let mut rng2 = StableRng::from_seed(42);
        let mut items2 = vec![1, 2, 3, 4, 5];
        rng2.shuffle(&mut items2);

        assert_eq!(items1, items2);
    }

    #[test]
    fn test_sample() {
        let mut rng = StableRng::from_seed(42);
        let items = vec![1, 2, 3, 4, 5];

        let sampled = rng.sample(&items, 3);
        assert_eq!(sampled.len(), 3);

        // 验证采样结果都在原集合中
        for item in &sampled {
            assert!(items.contains(item));
        }

        // 验证采样结果不重复
        let mut sorted_sampled = sampled.clone();
        sorted_sampled.sort();
        sorted_sampled.dedup();
        assert_eq!(sorted_sampled.len(), sampled.len());
    }

    #[test]
    fn test_sample_oversized() {
        let mut rng = StableRng::from_seed(42);
        let items = vec![1, 2, 3];

        let sampled = rng.sample(&items, 10);
        assert_eq!(sampled.len(), 3);

        // 验证包含所有元素
        let mut sorted_sampled = sampled.clone();
        sorted_sampled.sort();
        assert_eq!(sorted_sampled, items);
    }

    #[test]
    fn test_choose_weighted() {
        let mut rng = StableRng::from_seed(42);
        let items = vec!["common", "rare", "epic"];
        let weights = vec![70.0, 25.0, 5.0];

        // 统计验证权重分布
        let mut counts = [0, 0, 0];
        let iterations = 10000;

        for _ in 0..iterations {
            if let Some(&item) = rng.choose_weighted(&items, &weights) {
                let index = items.iter().position(|&x| x == item).unwrap();
                counts[index] += 1;
            }
        }

        // 验证分布接近预期（允许 10% 误差）
        let ratios: Vec<f64> = counts
            .iter()
            .map(|&c| c as f64 / iterations as f64)
            .collect();
        assert!((ratios[0] - 0.70).abs() < 0.10);
        assert!((ratios[1] - 0.25).abs() < 0.10);
        assert!((ratios[2] - 0.05).abs() < 0.10);
    }

    #[test]
    fn test_choose_weighted_empty() {
        let mut rng = StableRng::from_seed(42);
        let items: Vec<i32> = vec![];
        let weights: Vec<f64> = vec![];

        assert!(rng.choose_weighted(&items, &weights).is_none());
    }

    #[test]
    fn test_choose_weighted_zero_weights() {
        let mut rng = StableRng::from_seed(42);
        let items = vec![1, 2, 3];
        let weights = vec![0.0, 0.0, 0.0];

        assert!(rng.choose_weighted(&items, &weights).is_none());
    }

    #[test]
    fn test_seed_retrieval() {
        let rng = StableRng::from_seed(12345);
        assert_eq!(rng.seed(), 12345);

        let derived = rng.derive("test");
        // 派生的种子应该不同
        assert_ne!(derived.seed(), 12345);
    }

    #[test]
    fn test_from_seed_bytes() {
        let seed_bytes = [42u8; 32];
        let rng = StableRng::from_seed_bytes(seed_bytes);

        // 验证可以正常生成随机数
        let mut rng_mut = rng;
        let _value: u32 = rng_mut.random();
    }
}
