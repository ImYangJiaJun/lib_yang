//! UE5 适配通道导出示例
//!
//! 演示如何使用 `export_named_channels` 导出 UE5 兼容的具名通道数据，
//! 以及如何使用 `map_config_to_graph_params` 将配置映射为图参数。
//!
//! 运行方式：
//! ```bash
//! cargo run -p yang-pcg --example ue5_export
//! ```

use yang_pcg::ue::adapter::export_named_channels;
use yang_pcg::ue::params::map_config_to_graph_params;
use yang_pcg::{GenerationConfig, GenerationRequest, MapGenerator};

fn main() {
    println!("=== UE5 适配通道导出示例 ===\n");

    // 1. 生成地图
    let config = GenerationConfig::default();
    let request = GenerationRequest {
        seed: Some(7777),
        config: config.clone(),
        constraints: vec![],
        runtime_context: None,
        trace_id: Some("ue5-export-demo".to_string()),
    };

    let generator = MapGenerator::new();
    let result = generator.generate(request).expect("地图生成失败");

    // 2. 导出具名通道
    println!("--- 具名通道导出 ---");
    let channels = export_named_channels(&result).expect("通道导出失败");

    for channel in &channels {
        println!(
            "  通道: {:16} | 类型: {:?} | 点数: {:4} | 折线数: {}",
            channel.name,
            channel.kind,
            channel.points.len(),
            channel.polylines.len()
        );
    }

    // 3. 统计总点数
    let total_points: usize = channels.iter().map(|ch| ch.points.len()).sum();
    let total_polylines: usize = channels.iter().map(|ch| ch.polylines.len()).sum();
    println!("\n  总点数: {}", total_points);
    println!("  总折线数: {}", total_polylines);

    // 4. 映射配置为图参数
    println!("\n--- 图参数映射 ---");
    let params = map_config_to_graph_params(&config);
    for (key, value) in &params {
        println!("  {}: {:?}", key, value);
    }

    println!("\nUE5 导出完成！");
}
