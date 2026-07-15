/// 跨后端统一的连接池状态快照。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PoolStatus {
    /// 连接池最大连接数。
    pub max_size: usize,
    /// 当前连接总数。
    pub size: usize,
    /// 当前可用（空闲）连接数。
    pub available: usize,
    /// 正在等待获取连接的请求数；驱动不提供时为 0。
    pub waiting: usize,
}
