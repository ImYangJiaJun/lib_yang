// 配置归一化与摘要示例
// 演示如何使用配置管理功能

use yang_pcg::{config::RangeU16, ConfigDigest, GenerationConfig, PcgResult};

fn main() -> PcgResult<()> {
    println!("=== 配置归一化与摘要示例 ===\n");

    // 1. 使用默认配置
    println!("1. 默认配置:");
    let default_config = GenerationConfig::default();
    let _normalized = default_config.normalize()?;
    let digest = ConfigDigest::from_config(&default_config);
    println!(
        "   房间数量: {}-{}",
        default_config.room_count.min, default_config.room_count.max
    );
    println!("   配置摘要: {}", digest);
    println!();

    // 2. 创建自定义配置
    println!("2. 自定义配置:");
    let mut custom_config = GenerationConfig {
        room_count: RangeU16 { min: 15, max: 25 },
        theme_tags: vec!["dungeon".to_string(), "dark".to_string()],
        ..Default::default()
    };
    custom_config.terrain.obstacle_density = 0.3;

    let _custom_normalized = custom_config.normalize()?;
    let custom_digest = ConfigDigest::from_config(&custom_config);
    println!(
        "   房间数量: {}-{}",
        custom_config.room_count.min, custom_config.room_count.max
    );
    println!("   主题标签: {:?}", custom_config.theme_tags);
    println!("   障碍物密度: {}", custom_config.terrain.obstacle_density);
    println!("   配置摘要: {}", custom_digest);
    println!();

    // 3. 配置合并
    println!("3. 配置合并:");
    let base_config = GenerationConfig::default();
    let override_config = GenerationConfig {
        room_count: RangeU16 { min: 20, max: 30 },
        theme_tags: vec!["forest".to_string()],
        ..Default::default()
    };

    let merged_config = base_config.merge(&override_config);
    let merged_digest = ConfigDigest::from_config(&merged_config);
    println!(
        "   合并后房间数量: {}-{}",
        merged_config.room_count.min, merged_config.room_count.max
    );
    println!("   合并后主题: {:?}", merged_config.theme_tags);
    println!("   合并后摘要: {}", merged_digest);
    println!();

    // 4. 配置验证
    println!("4. 配置验证:");
    let invalid_config = GenerationConfig {
        room_count: RangeU16 { min: 30, max: 10 }, // 非法范围
        ..Default::default()
    };

    match invalid_config.normalize() {
        Ok(_) => println!("   配置有效"),
        Err(err) => println!("   配置错误: {}", err),
    }
    println!();

    // 5. 摘要稳定性验证
    println!("5. 摘要稳定性验证:");
    let config1 = GenerationConfig::default();
    let config2 = GenerationConfig::default();
    let digest1 = ConfigDigest::from_config(&config1);
    let digest2 = ConfigDigest::from_config(&config2);
    println!("   相同配置生成相同摘要: {}", digest1 == digest2);
    println!("   摘要1: {}", digest1);
    println!("   摘要2: {}", digest2);
    println!();

    // 6. 摘要唯一性验证
    println!("6. 摘要唯一性验证:");
    let config_a = GenerationConfig::default();
    let config_b = GenerationConfig {
        room_count: RangeU16 { min: 15, max: 25 },
        ..Default::default()
    };
    let digest_a = ConfigDigest::from_config(&config_a);
    let digest_b = ConfigDigest::from_config(&config_b);
    println!("   不同配置生成不同摘要: {}", digest_a != digest_b);
    println!("   摘要A: {}", digest_a);
    println!("   摘要B: {}", digest_b);
    println!();

    // 7. 配置序列化
    println!("7. 配置序列化:");
    let config = GenerationConfig::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    println!(
        "   JSON 配置 (前 200 字符):\n{}",
        &json[..200.min(json.len())]
    );
    println!();

    // 8. 配置反序列化
    println!("8. 配置反序列化:");
    let deserialized: GenerationConfig = serde_json::from_str(&json).unwrap();
    let deserialized_digest = ConfigDigest::from_config(&deserialized);
    println!("   反序列化成功");
    println!("   摘要匹配: {}", digest.matches(&deserialized));
    println!("   反序列化摘要: {}", deserialized_digest);

    Ok(())
}
