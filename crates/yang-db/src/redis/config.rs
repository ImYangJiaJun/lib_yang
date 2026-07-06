use std::time::Duration;

/// Redis 配置
///
/// 用于配置 Redis 客户端的连接池和超时参数。
///
/// 推荐使用 Builder 模式构建：
///
/// ```rust
/// use yang_db::RedisConfig;
///
/// let config = RedisConfig::default()
///     .with_max_connections(20)
///     .with_connect_timeout(10)
///     .with_wait_timeout(15)
///     .with_enable_logging(true);
/// ```
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 最小空闲连接数（连接池自愈参数）
    ///
    /// 连接池会尽量保持至少此数量的空闲连接。设为 0 表示不强制保持。
    pub min_connections: usize,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 等待连接超时时间（秒）
    pub wait_timeout: u64,
    /// 连接空闲回收超时时间（秒）
    ///
    /// 连接池中空闲超过此时间的连接将被回收。
    /// 此值应远大于 `connect_timeout`，否则连接池形同虚设。
    pub idle_timeout: u64,
    /// 连接最大生存时间（秒）
    ///
    /// 连接从创建到强制回收的最大时长。`None` 表示不限制。
    pub max_lifetime: Option<u64>,
    /// 是否在借出连接前进行健康检查
    ///
    /// 启用后每次获取连接前会执行快速探活，确保连接可用。
    /// 会略微增加每次获取连接的延迟。
    pub test_before_acquire: bool,
    /// 是否启用日志
    pub enable_logging: bool,
}

impl Default for RedisConfig {
    /// 创建默认配置
    ///
    /// # 默认值
    /// - max_connections: 10
    /// - min_connections: 0
    /// - connect_timeout: 5 秒
    /// - wait_timeout: 10 秒
    /// - idle_timeout: 300 秒（5 分钟）
    /// - max_lifetime: None（不限制）
    /// - test_before_acquire: false
    /// - enable_logging: false
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            connect_timeout: 5,
            wait_timeout: 10,
            idle_timeout: 300,
            max_lifetime: None,
            test_before_acquire: false,
            enable_logging: false,
        }
    }
}

impl RedisConfig {
    /// 创建新的配置（已弃用）
    ///
    /// 推荐使用 Builder 模式替代位置参数构造函数：
    ///
    /// ```rust
    /// use yang_db::RedisConfig;
    ///
    /// let config = RedisConfig::default()
    ///     .with_max_connections(20)
    ///     .with_connect_timeout(10)
    ///     .with_wait_timeout(15)
    ///     .with_enable_logging(true);
    /// ```
    ///
    /// `idle_timeout` 默认设为 300 秒（5 分钟）。
    #[deprecated(
        since = "0.1.0",
        note = "使用 RedisConfig::default().with_*() Builder 模式替代位置参数构造函数"
    )]
    pub fn new(
        max_connections: usize,
        connect_timeout: u64,
        wait_timeout: u64,
        enable_logging: bool,
    ) -> Self {
        Self {
            max_connections,
            min_connections: 0,
            connect_timeout,
            wait_timeout,
            idle_timeout: 300,
            max_lifetime: None,
            test_before_acquire: false,
            enable_logging,
        }
    }

    // ==================== Builder 方法 ====================

    /// 设置最大连接数
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// 设置最小空闲连接数（连接池自愈参数）
    pub fn with_min_connections(mut self, min: usize) -> Self {
        self.min_connections = min;
        self
    }

    /// 设置连接超时时间（秒）
    pub fn with_connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout = secs;
        self
    }

    /// 设置等待连接超时时间（秒）
    pub fn with_wait_timeout(mut self, secs: u64) -> Self {
        self.wait_timeout = secs;
        self
    }

    /// 设置连接空闲回收超时时间（秒）
    ///
    /// 此值应远大于 `connect_timeout`，否则连接池形同虚设。
    pub fn with_idle_timeout(mut self, secs: u64) -> Self {
        self.idle_timeout = secs;
        self
    }

    /// 设置连接最大生存时间（秒）
    ///
    /// 传 `None` 表示不限制连接生存时间。
    pub fn with_max_lifetime(mut self, secs: Option<u64>) -> Self {
        self.max_lifetime = secs;
        self
    }

    /// 设置是否在借出连接前进行健康检查
    pub fn with_test_before_acquire(mut self, enable: bool) -> Self {
        self.test_before_acquire = enable;
        self
    }

    /// 设置是否启用日志
    pub fn with_enable_logging(mut self, enable: bool) -> Self {
        self.enable_logging = enable;
        self
    }

    /// 校验 Redis 连接池配置是否适合生产运行。
    ///
    /// Builder 方法保持纯赋值以兼容历史调用；真正建池前由调用方显式校验，避免将明显
    /// 非法的配置下推给连接池实现后才在运行时失败。
    pub fn validate(&self) -> std::result::Result<(), crate::error::DbError> {
        if self.max_connections == 0 {
            return Err(crate::error::DbError::InvalidArgument(
                "Redis max_connections 必须大于 0".to_string(),
            ));
        }
        if self.min_connections > self.max_connections {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "Redis min_connections({}) 不能大于 max_connections({})",
                self.min_connections, self.max_connections
            )));
        }
        if self.connect_timeout == 0 {
            return Err(crate::error::DbError::InvalidArgument(
                "Redis connect_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.wait_timeout == 0 {
            return Err(crate::error::DbError::InvalidArgument(
                "Redis wait_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout == 0 {
            return Err(crate::error::DbError::InvalidArgument(
                "Redis idle_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout <= self.connect_timeout {
            return Err(crate::error::DbError::InvalidArgument(format!(
                "Redis idle_timeout({}) 必须大于 connect_timeout({})",
                self.idle_timeout, self.connect_timeout
            )));
        }
        if matches!(self.max_lifetime, Some(0)) {
            return Err(crate::error::DbError::InvalidArgument(
                "Redis max_lifetime 为 Some 时必须大于 0 秒".to_string(),
            ));
        }
        Ok(())
    }

    // ==================== Duration 辅助方法 ====================

    /// 获取连接超时 Duration
    pub(crate) fn connect_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.connect_timeout)
    }

    /// 获取等待超时 Duration
    pub(crate) fn wait_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.wait_timeout)
    }

    /// 获取空闲回收超时 Duration
    pub(crate) fn idle_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.idle_timeout)
    }

    /// 获取连接最大生存时间 Duration
    #[allow(dead_code)]
    pub(crate) fn max_lifetime_duration(&self) -> Option<Duration> {
        self.max_lifetime.map(Duration::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RedisConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 0);
        assert_eq!(config.connect_timeout, 5);
        assert_eq!(config.wait_timeout, 10);
        assert_eq!(config.idle_timeout, 300);
        assert_eq!(config.max_lifetime, None);
        assert!(!config.test_before_acquire);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_builder_pattern() {
        let config = RedisConfig::default()
            .with_max_connections(20)
            .with_min_connections(4)
            .with_connect_timeout(10)
            .with_wait_timeout(15)
            .with_idle_timeout(600)
            .with_max_lifetime(Some(3600))
            .with_test_before_acquire(true)
            .with_enable_logging(true);

        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 4);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.wait_timeout, 15);
        assert_eq!(config.idle_timeout, 600);
        assert_eq!(config.max_lifetime, Some(3600));
        assert!(config.test_before_acquire);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_validate_accepts_default_config() {
        let config = RedisConfig::default();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_zero_max_connections() {
        let err = RedisConfig::default()
            .with_max_connections(0)
            .validate()
            .expect_err("max_connections 为 0 应被拒绝");

        assert!(matches!(err, crate::DbError::InvalidArgument(_)));
    }

    #[test]
    fn test_validate_rejects_min_connections_above_max() {
        let err = RedisConfig::default()
            .with_max_connections(2)
            .with_min_connections(3)
            .validate()
            .expect_err("min_connections 大于 max_connections 应被拒绝");

        assert!(matches!(err, crate::DbError::InvalidArgument(_)));
    }

    #[test]
    fn test_validate_rejects_zero_timeouts() {
        for config in [
            RedisConfig::default().with_connect_timeout(0),
            RedisConfig::default().with_wait_timeout(0),
            RedisConfig::default().with_idle_timeout(0),
            RedisConfig::default().with_max_lifetime(Some(0)),
        ] {
            let err = config.validate().expect_err("零秒超时配置应被拒绝");

            assert!(matches!(err, crate::DbError::InvalidArgument(_)));
        }
    }

    #[test]
    fn test_validate_rejects_idle_timeout_not_greater_than_connect_timeout() {
        let err = RedisConfig::default()
            .with_connect_timeout(5)
            .with_idle_timeout(5)
            .validate()
            .expect_err("idle_timeout 不大于 connect_timeout 应被拒绝");

        assert!(matches!(err, crate::DbError::InvalidArgument(_)));
    }

    #[test]
    fn test_builder_partial_override() {
        // 只覆盖部分字段，其余保持默认
        let config = RedisConfig::default()
            .with_max_connections(5)
            .with_test_before_acquire(true);

        assert_eq!(config.max_connections, 5);
        assert_eq!(config.min_connections, 0); // 默认
        assert_eq!(config.connect_timeout, 5); // 默认
        assert_eq!(config.wait_timeout, 10); // 默认
        assert_eq!(config.idle_timeout, 300); // 默认
        assert_eq!(config.max_lifetime, None); // 默认
        assert!(config.test_before_acquire);
        assert!(!config.enable_logging); // 默认
    }

    #[test]
    #[allow(deprecated)]
    fn test_new_config() {
        let config = RedisConfig::new(20, 10, 15, true);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 0);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.wait_timeout, 15);
        assert_eq!(config.idle_timeout, 300);
        assert_eq!(config.max_lifetime, None);
        assert!(!config.test_before_acquire);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_connect_timeout_duration() {
        let config = RedisConfig::default().with_connect_timeout(5);
        assert_eq!(config.connect_timeout_duration(), Duration::from_secs(5));
    }

    #[test]
    fn test_wait_timeout_duration() {
        let config = RedisConfig::default().with_wait_timeout(10);
        assert_eq!(config.wait_timeout_duration(), Duration::from_secs(10));
    }

    #[test]
    fn test_idle_timeout_duration() {
        let config = RedisConfig::default();
        assert_eq!(config.idle_timeout_duration(), Duration::from_secs(300));
    }

    #[test]
    fn test_max_lifetime_duration_some() {
        let config = RedisConfig::default().with_max_lifetime(Some(3600));
        assert_eq!(
            config.max_lifetime_duration(),
            Some(Duration::from_secs(3600))
        );
    }

    #[test]
    fn test_max_lifetime_duration_none() {
        let config = RedisConfig::default();
        assert_eq!(config.max_lifetime_duration(), None);
    }

    #[test]
    fn test_clone() {
        let config = RedisConfig::default()
            .with_max_connections(15)
            .with_min_connections(2)
            .with_connect_timeout(8)
            .with_wait_timeout(12)
            .with_idle_timeout(500)
            .with_max_lifetime(Some(7200))
            .with_test_before_acquire(true)
            .with_enable_logging(true);

        let cloned = config.clone();
        assert_eq!(config.max_connections, cloned.max_connections);
        assert_eq!(config.min_connections, cloned.min_connections);
        assert_eq!(config.connect_timeout, cloned.connect_timeout);
        assert_eq!(config.wait_timeout, cloned.wait_timeout);
        assert_eq!(config.idle_timeout, cloned.idle_timeout);
        assert_eq!(config.max_lifetime, cloned.max_lifetime);
        assert_eq!(config.test_before_acquire, cloned.test_before_acquire);
        assert_eq!(config.enable_logging, cloned.enable_logging);
    }

    #[test]
    fn test_debug() {
        let config = RedisConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("RedisConfig"));
        assert!(debug_str.contains("max_connections"));
    }
}
