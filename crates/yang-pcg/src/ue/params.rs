// 图参数映射

use std::collections::BTreeMap;

use crate::config::GenerationConfig;

use super::points::PropertyValue;

/// 将生成配置映射为稳定的图参数字典。
pub fn map_config_to_graph_params(config: &GenerationConfig) -> BTreeMap<String, PropertyValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "room_count.min".to_string(),
        PropertyValue::Int(i64::from(config.room_count.min)),
    );
    params.insert(
        "room_count.max".to_string(),
        PropertyValue::Int(i64::from(config.room_count.max)),
    );
    params.insert(
        "critical_path_length.min".to_string(),
        PropertyValue::Int(i64::from(config.critical_path_length.min)),
    );
    params.insert(
        "critical_path_length.max".to_string(),
        PropertyValue::Int(i64::from(config.critical_path_length.max)),
    );
    params.insert(
        "corridor.width".to_string(),
        PropertyValue::Int(i64::from(config.corridor.width)),
    );
    params.insert(
        "generation_mode".to_string(),
        PropertyValue::String(format!("{:?}", config.generation_mode)),
    );
    params.insert(
        "theme.primary".to_string(),
        PropertyValue::String(
            config
                .theme_tags
                .first()
                .cloned()
                .unwrap_or_else(|| "default".to_string()),
        ),
    );
    params
}
