// 缓存机制模块
// 负责生成结果的缓存和重建

pub mod key;
pub mod store;

pub use key::{CacheKey, CacheScope};
pub use store::ResultCache;
