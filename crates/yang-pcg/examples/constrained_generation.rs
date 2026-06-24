//! 约束输入示例
//!
//! 演示如何使用锚点约束、排除区约束和模板引用约束来控制地图生成。
//!
//! 运行方式：
//! ```bash
//! cargo run -p yang-pcg --example constrained_generation
//! ```

use yang_pcg::model::request::{
    AnchorConstraint, Constraint, ExclusionZoneConstraint, TemplateConstraint,
};
use yang_pcg::model::room::RoomType;
use yang_pcg::{GenerationConfig, GenerationRequest, MapGenerator};

fn main() {
    println!("=== 约束输入示例 ===\n");

    // 1. 创建锚点约束：指定 Boss 房间出现在特定位置
    let anchor = Constraint::Anchor(
        AnchorConstraint::new("boss_anchor")
            .with_room_type(RoomType::Boss)
            .with_target_grid_pos(yang_pcg::model::geometry::GridPoint::new(50, 50)),
    );
    println!("锚点约束: 指定 Boss 房间锚定到 (50, 50)");

    // 2. 创建排除区约束：禁止在指定区域放置房间和点位
    let exclusion = Constraint::ExclusionZone(
        ExclusionZoneConstraint::new(
            "forbidden_zone",
            yang_pcg::model::geometry::GridPoint::new(0, 0),
            yang_pcg::model::geometry::GridPoint::new(10, 10),
        ),
    );
    println!("排除区约束: 禁止在 (0,0)-(10,10) 区域放置房间和点位");

    // 3. 创建模板引用约束：为 Treasure 房间指定模板
    let template = Constraint::Template(
        TemplateConstraint::new("treasure_vault_01")
            .with_room_type(RoomType::Treasure),
    );
    println!("模板约束: 为 Treasure 房间指定模板 'treasure_vault_01'");

    // 4. 组合约束并构建请求
    let constraints = vec![anchor, exclusion, template];
    let request = GenerationRequest::new(GenerationConfig::default())
        .with_seed(12345)
        .with_trace_id("constrained-demo-001".to_string())
        .with_constraints(constraints);

    // 5. 执行生成
    let generator = MapGenerator::new();
    let result = generator.generate(request).expect("约束生成失败");

    // 6. 查看结果
    println!("\n--- 生成结果 ---");
    println!("种子: {}", result.metadata.seed);
    println!("房间数: {}", result.rooms.len());
    println!("走廊数: {}", result.corridors.len());
    println!("交互物点位数: {}", result.item_spawns.len());
    println!("敌人点位数: {}", result.enemy_spawns.len());

    // 7. 检查 Boss 房间位置
    println!("\n--- Boss 房间信息 ---");
    for room in &result.rooms {
        if room.room_type == RoomType::Boss {
            println!("  Boss 房间 ID: {}", room.id);
            if let Some(bounds) = room.bounds {
                println!("  边界: ({},{}) - ({},{})", bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y);
            }
        }
    }

    println!("\n约束生成完成！");
}
