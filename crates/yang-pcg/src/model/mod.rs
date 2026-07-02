// 数据模型模块
// 定义核心数据结构

/// 分块标识符
pub type ChunkId = String;

pub mod chunk;
pub mod geometry;
pub mod request;
pub mod result;
pub mod room;
pub mod spawn;
pub mod terrain;

#[cfg(test)]
mod __tests__;
