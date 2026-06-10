// UE5 适配层模块
// 负责将生成结果映射为 UE5 PCG 兼容数据

pub mod adapter;
pub mod channels;
pub mod params;
pub mod points;
pub mod streaming;

pub use adapter::{export_named_channels, export_named_channels_json};
pub use channels::{ChannelKind, NamedChannel};
pub use params::map_config_to_graph_params;
pub use points::{PcgPoint, PropertyValue};
