use std::time::Duration;

/// Redis 配置
///
/// 用于配置 Redis 客户端的连接池和超时参数
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 等待连接超时时间（秒）
    pub wait_timeout: u64,
    /// 是否启用日志
    pub enable_logging: bool,
}

impl Default for RedisConfig {
    /// 创建默认配置
    ///
    /// # 默认值
    /// - max_connections: 10
    /// - connect_timeout: 5 秒
    /// - wait_timeout: 10 秒
    /// - enable_logging: false
    fn default() -> Self {
        Self {
            max_connections: 10,
            connect_timeout: 5,
            wait_timeout: 10,
            enable_logging: false,
        }
    }
}

impl RedisConfig {
    /// 创建新的配置
    ///
    /// # 参数
    /// - `max_connections`: 最大连接数
    /// - `connect_timeout`: 连接超时时间（秒）
    /// - `wait_timeout`: 等待连接超时时间（秒）
    /// - `enable_logging`: 是否启用日志
    ///
    /// # 示例
    /// ```
    /// use yang_db::RedisConfig;
    ///
    /// let config = RedisConfig::new(20, 10, 15, true);
    /// assert_eq!(config.max_connections, 20);
    /// assert_eq!(config.connect_timeout, 10);
    /// ```
    pub fn new(
        max_connections: usize,
        connect_timeout: u64,
        wait_timeout: u64,
        enable_logging: bool,
    ) -> Self {
        Self {
            max_connections,
            connect_timeout,
            wait_timeout,
            enable_logging,
        }
    }

    /// 获取连接超时 Duration
    #[allow(dead_code)]
    pub(crate) fn connect_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.connect_timeout)
    }

    /// 获取等待超时 Duration
    #[allow(dead_code)]
    pub(crate) fn wait_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.wait_timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RedisConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connect_timeout, 5);
        assert_eq!(config.wait_timeout, 10);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_new_config() {
        let config = RedisConfig::new(20, 10, 15, true);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.wait_timeout, 15);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_connect_timeout_duration() {
        let config = RedisConfig::new(10, 5, 10, false);
        assert_eq!(config.connect_timeout_duration(), Duration::from_secs(5));
    }

    #[test]
    fn test_wait_timeout_duration() {
        let config = RedisConfig::new(10, 5, 10, false);
        assert_eq!(config.wait_timeout_duration(), Duration::from_secs(10));
    }

    #[test]
    fn test_clone() {
        let config = RedisConfig::new(15, 8, 12, true);
        let cloned = config.clone();
        assert_eq!(config.max_connections, cloned.max_connections);
        assert_eq!(config.connect_timeout, cloned.connect_timeout);
        assert_eq!(config.wait_timeout, cloned.wait_timeout);
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
