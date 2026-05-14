// 调试输出模块
// 负责生成调试信息和性能分析数据

pub mod profiling;
pub mod report;

pub use profiling::{elapsed_ms, stage_stat, stage_stat_timed, StageStat};
pub use report::{DebugBundle, DebugChannels, RejectionReason, SpawnDebugInfo};
