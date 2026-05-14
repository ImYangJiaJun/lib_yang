//! 基础楼层生成示例
//!
//! 演示最简单的地图生成流程：创建配置 → 构建请求 → 生成 → 访问结果。
//!
//! 运行方式：
//! ```bash
//! cargo run -p yang-pcg --example basic_generation
//! ```

use yang_pcg::{GenerationConfig, GenerationRequest, MapGenerator};

fn main() {
    // 1. 创建默认生成配置
    let config = GenerationConfig::default();
    println!("=== 基础楼层生成示例 ===");
    println!(
        "配置: 房间数 {}-{}, 关键路径长度 {}-{}",
        config.room_count.min,
        config.room_count.max,
        config.critical_path_length.min,
        config.critical_path_length.max
    );

    // 2. 构建生成请求（指定固定种子以保证可复现）
    let request = GenerationRequest {
        seed: Some(42),
        config,
        constraints: vec![],
        runtime_context: None,
        trace_id: None,
    };

    // 3. 创建生成器并执行生成
    let generator = MapGenerator::new();
    let result = generator.generate(request).expect("地图生成失败");

    // 4. 访问生成结果
    println!("\n--- 生成结果 ---");
    println!("种子: {}", result.metadata.seed);
    println!("房间数: {}", result.rooms.len());
    println!("走廊数: {}", result.corridors.len());
    println!("地形数: {}", result.terrains.len());
    println!("交互物点位数: {}", result.item_spawns.len());
    println!("敌人点位数: {}", result.enemy_spawns.len());
    println!("分块数: {}", result.chunks.len());

    // 5. 遍历房间信息
    println!("\n--- 房间列表 ---");
    for room in &result.rooms {
        println!(
            "  [{}] 类型={:?}, 难度={}, 深度={}",
            room.id, room.room_type, room.difficulty, room.depth_from_start
        );
    }

    println!("\n生成完成！");
}
