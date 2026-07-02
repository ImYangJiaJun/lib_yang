// 性能基准测试
// 测量不同规模地图生成的耗时和资源消耗
//
// 运行方式: cargo test -p yang-pcg --test generation_bench -- --ignored --nocapture

use std::time::Instant;

use yang_pcg::config::{GenerationConfig, RangeU16};
use yang_pcg::generator::MapGenerator;
use yang_pcg::model::request::GenerationRequest;

/// 基准测试迭代次数
const ITERATIONS: usize = 10;

/// 单次基准运行结果
struct BenchRun {
    /// 总耗时（毫秒）
    total_ms: f64,
    /// 房间数
    room_count: usize,
    /// 走廊数
    corridor_count: usize,
    /// 分块数
    chunk_count: usize,
    /// 地形数
    terrain_count: usize,
    /// 各阶段耗时（阶段名, 毫秒）
    stage_durations: Vec<(String, u64)>,
}

/// 基准测试汇总结果
struct BenchSummary {
    /// 配置名称
    name: String,
    /// 目标房间数
    target_rooms: u16,
    /// 迭代次数
    iterations: usize,
    /// 各次运行结果
    runs: Vec<BenchRun>,
}

impl BenchSummary {
    /// 计算平均总耗时（毫秒）
    fn avg_total_ms(&self) -> f64 {
        let sum: f64 = self.runs.iter().map(|r| r.total_ms).sum();
        sum / self.runs.len() as f64
    }

    /// 计算最小总耗时（毫秒）
    fn min_total_ms(&self) -> f64 {
        self.runs
            .iter()
            .map(|r| r.total_ms)
            .fold(f64::MAX, f64::min)
    }

    /// 计算最大总耗时（毫秒）
    fn max_total_ms(&self) -> f64 {
        self.runs
            .iter()
            .map(|r| r.total_ms)
            .fold(f64::MIN, f64::max)
    }

    /// 计算平均房间数
    fn avg_room_count(&self) -> f64 {
        let sum: usize = self.runs.iter().map(|r| r.room_count).sum();
        sum as f64 / self.runs.len() as f64
    }

    /// 计算平均走廊数
    fn avg_corridor_count(&self) -> f64 {
        let sum: usize = self.runs.iter().map(|r| r.corridor_count).sum();
        sum as f64 / self.runs.len() as f64
    }

    /// 计算平均分块数
    fn avg_chunk_count(&self) -> f64 {
        let sum: usize = self.runs.iter().map(|r| r.chunk_count).sum();
        sum as f64 / self.runs.len() as f64
    }

    /// 计算平均地形数
    fn avg_terrain_count(&self) -> f64 {
        let sum: usize = self.runs.iter().map(|r| r.terrain_count).sum();
        sum as f64 / self.runs.len() as f64
    }

    /// 计算各阶段平均耗时
    fn avg_stage_durations(&self) -> Vec<(String, f64)> {
        if self.runs.is_empty() {
            return Vec::new();
        }

        // 收集所有阶段名称（以第一次运行为基准）
        let first_run = &self.runs[0];
        let stage_names: Vec<String> = first_run
            .stage_durations
            .iter()
            .map(|(name, _)| name.clone())
            .collect();

        stage_names
            .iter()
            .map(|name| {
                let sum: u64 = self
                    .runs
                    .iter()
                    .flat_map(|r| r.stage_durations.iter())
                    .filter(|(n, _)| n == name)
                    .map(|(_, ms)| *ms)
                    .sum();
                let avg = sum as f64 / self.runs.len() as f64;
                (name.clone(), avg)
            })
            .collect()
    }

    /// 计算峰值分配估算（基于结果中的元素总数）
    fn peak_allocation_estimate(&self) -> usize {
        self.runs
            .iter()
            .map(|r| r.room_count + r.corridor_count + r.chunk_count + r.terrain_count)
            .max()
            .unwrap_or(0)
    }

    /// 打印结构化报告
    fn print_report(&self) {
        println!();
        println!("============================================================");
        println!("  基准: {} (目标房间数: {})", self.name, self.target_rooms);
        println!("============================================================");
        println!("  迭代次数: {}", self.iterations);
        println!("  ---");
        println!("  总耗时:");
        println!("    平均: {:.2} ms", self.avg_total_ms());
        println!("    最小: {:.2} ms", self.min_total_ms());
        println!("    最大: {:.2} ms", self.max_total_ms());
        println!("  ---");
        println!("  产出统计 (平均):");
        println!("    房间数: {:.1}", self.avg_room_count());
        println!("    走廊数: {:.1}", self.avg_corridor_count());
        println!("    分块数: {:.1}", self.avg_chunk_count());
        println!("    地形数: {:.1}", self.avg_terrain_count());
        println!("  ---");
        println!("  阶段耗时 (平均):");
        for (stage, avg_ms) in self.avg_stage_durations() {
            println!("    {}: {:.2} ms", stage, avg_ms);
        }
        println!("  ---");
        println!(
            "  峰值分配估算 (元素总数): {}",
            self.peak_allocation_estimate()
        );
        println!("============================================================");
    }
}

/// 运行单个配置的基准测试
fn run_benchmark(name: &str, config: GenerationConfig, iterations: usize) -> BenchSummary {
    let mut runs = Vec::with_capacity(iterations);

    for i in 0..iterations {
        // 每次迭代使用不同种子，确保覆盖不同随机路径
        let seed = 1000 + i as u64;

        let mut generator = MapGenerator::new();
        generator.set_debug(true);

        let request = GenerationRequest::new(config.clone())
            .with_seed(seed)
            .with_trace_id(format!("bench-{}-iter-{}", name, i));

        let start = Instant::now();
        let result = generator.generate(request).expect("基准测试生成不应失败");
        let total_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 从 DebugBundle 提取阶段耗时
        let stage_durations = result
            .debug
            .as_ref()
            .map(|debug| {
                debug
                    .stage_stats
                    .iter()
                    .map(|stat| (stat.stage_name.clone(), stat.duration_ms))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        runs.push(BenchRun {
            total_ms,
            room_count: result.rooms.len(),
            corridor_count: result.corridors.len(),
            chunk_count: result.chunks.len(),
            terrain_count: result.terrains.len(),
            stage_durations,
        });
    }

    BenchSummary {
        name: name.to_string(),
        target_rooms: config.room_count.max,
        iterations,
        runs,
    }
}

/// 构建 small 配置：10 房间级
fn small_config() -> GenerationConfig {
    let mut c = GenerationConfig::default();
    c.room_count = RangeU16::new(8, 10);
    c.critical_path_length = RangeU16::new(4, 6);
    c.branch_count = RangeU16::new(1, 2);
    c.dead_end_count = RangeU16::new(0, 1);
    c
}

/// 构建 medium 配置：20 房间级
fn medium_config() -> GenerationConfig {
    let mut c = GenerationConfig::default();
    c.room_count = RangeU16::new(15, 20);
    c.critical_path_length = RangeU16::new(7, 12);
    c.branch_count = RangeU16::new(2, 4);
    c.dead_end_count = RangeU16::new(1, 3);
    c
}

/// 构建 large 配置：40 房间级
fn large_config() -> GenerationConfig {
    let mut c = GenerationConfig::default();
    c.room_count = RangeU16::new(30, 40);
    c.critical_path_length = RangeU16::new(12, 20);
    c.branch_count = RangeU16::new(3, 6);
    c.dead_end_count = RangeU16::new(2, 5);
    c
}

/// 性能基准：small 配置（10 房间级）
///
/// 验证需求: 17.3
#[test]
#[ignore]
fn bench_small_10_rooms() {
    let config = small_config();
    let summary = run_benchmark("small", config, ITERATIONS);
    summary.print_report();

    // 基本断言：确保生成成功且结果合理
    assert!(
        summary.avg_total_ms() < 10_000.0,
        "small 配置平均耗时不应超过 10 秒"
    );
    assert!(
        summary.avg_room_count() >= 8.0,
        "small 配置平均房间数应不少于 8"
    );
}

/// 性能基准：medium 配置（20 房间级）
///
/// 验证需求: 17.3
#[test]
#[ignore]
fn bench_medium_20_rooms() {
    let config = medium_config();
    let summary = run_benchmark("medium", config, ITERATIONS);
    summary.print_report();

    // 基本断言：确保生成成功且结果合理
    assert!(
        summary.avg_total_ms() < 30_000.0,
        "medium 配置平均耗时不应超过 30 秒"
    );
    assert!(
        summary.avg_room_count() >= 15.0,
        "medium 配置平均房间数应不少于 15"
    );
}

/// 性能基准：large 配置（40 房间级）
///
/// 验证需求: 17.3
#[test]
#[ignore]
fn bench_large_40_rooms() {
    let config = large_config();
    let summary = run_benchmark("large", config, ITERATIONS);
    summary.print_report();

    // 基本断言：确保生成成功且结果合理
    assert!(
        summary.avg_total_ms() < 60_000.0,
        "large 配置平均耗时不应超过 60 秒"
    );
    assert!(
        summary.avg_room_count() >= 30.0,
        "large 配置平均房间数应不少于 30"
    );
}

/// 综合性能基准：运行所有规模并输出对比报告
///
/// 记录总耗时、阶段耗时、房间数、Chunk 数、峰值分配
///
/// 验证需求: 17.3, 17.4
#[test]
#[ignore]
fn bench_all_sizes_comparison() {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          yang-pcg 地图生成性能基准测试报告              ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!(
        "║  迭代次数: {:2}                                          ║",
        ITERATIONS
    );
    println!("║  模式: OfflineFullFloor + Debug                        ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let configs = vec![
        ("small (10 rooms)", small_config()),
        ("medium (20 rooms)", medium_config()),
        ("large (40 rooms)", large_config()),
    ];

    let mut summaries = Vec::new();

    for (name, config) in configs {
        let summary = run_benchmark(name, config, ITERATIONS);
        summary.print_report();
        summaries.push(summary);
    }

    // 输出对比表格
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│                    对比汇总表                           │");
    println!("├──────────────────┬──────────┬────────┬────────┬────────┤");
    println!("│ 配置             │ 平均耗时 │ 房间数 │ 分块数 │ 峰值   │");
    println!("├──────────────────┼──────────┼────────┼────────┼────────┤");
    for s in &summaries {
        println!(
            "│ {:16} │ {:6.2} ms│ {:6.1} │ {:6.1} │ {:6} │",
            s.name,
            s.avg_total_ms(),
            s.avg_room_count(),
            s.avg_chunk_count(),
            s.peak_allocation_estimate()
        );
    }
    println!("└──────────────────┴──────────┴────────┴────────┴────────┘");

    // 验证性能随规模线性增长（大致）
    if summaries.len() == 3 {
        let small_ms = summaries[0].avg_total_ms();
        let large_ms = summaries[2].avg_total_ms();
        // large 配置是 small 的 4 倍房间数，耗时不应超过 20 倍
        // （考虑到布局碰撞检测等 O(n²) 因素）
        assert!(
            large_ms < small_ms * 20.0,
            "large 配置耗时({:.2}ms)不应超过 small({:.2}ms) 的 20 倍",
            large_ms,
            small_ms
        );
    }
}
