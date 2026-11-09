//! SCAN/HSCAN 的便捷遍历入口。
//!
//! 底层 `scan`/`hscan` 仍然暴露手动游标 API；本模块提供带最大页数保护的
//! 「一次取完」版本，避免误把 `unwrap_or(0)` 式游标解析错误当成遍历结束。

use crate::{DbError, RedisClient, Result};

/// 单次完整遍历的最大页数（防御性上限，正常数据远不会触发）。
const MAX_SCAN_PAGES: usize = 1_000_000;

impl RedisClient {
    /// 遍历匹配 `pattern` 的所有 key。
    ///
    /// Redis Cluster 下 SCAN 游标是节点本地的，redis-rs 会把单条 SCAN 路由到
    /// 任意节点，跨页会重复/漏 key；因此本方法在 Cluster 拓扑下显式拒绝。
    /// 需要全量遍历时请按 master 节点分别建立客户端执行。
    pub async fn scan_all(&self, pattern: Option<&str>, count: Option<i64>) -> Result<Vec<String>> {
        if self.is_cluster() {
            return Err(DbError::UnsupportedOperator(
                "Redis Cluster 不支持 scan_all：需按 master 节点分别遍历".to_string(),
            ));
        }
        let mut cursor = 0u64;
        let mut keys = Vec::new();
        for _ in 0..MAX_SCAN_PAGES {
            let (next, batch) = self.scan(cursor, pattern, count).await?;
            keys.extend(batch);
            cursor = next;
            if cursor == 0 {
                return Ok(keys);
            }
        }
        Err(DbError::RedisCommandError(format!(
            "SCAN 超过最大页数 {MAX_SCAN_PAGES}，疑似游标异常"
        )))
    }

    /// 遍历哈希表的所有字段与值。
    ///
    /// 与 [`scan_all`](Self::scan_all) 相同，Cluster 拓扑下显式拒绝。
    pub async fn hscan_all(
        &self,
        key: impl Into<String>,
        pattern: Option<&str>,
        count: Option<i64>,
    ) -> Result<Vec<(String, String)>> {
        if self.is_cluster() {
            return Err(DbError::UnsupportedOperator(
                "Redis Cluster 不支持 hscan_all：需按 master 节点分别遍历".to_string(),
            ));
        }
        let key = key.into();
        let mut cursor = 0u64;
        let mut pairs = Vec::new();
        for _ in 0..MAX_SCAN_PAGES {
            let (next, batch) = self.hscan(key.clone(), cursor, pattern, count).await?;
            pairs.extend(batch);
            cursor = next;
            if cursor == 0 {
                return Ok(pairs);
            }
        }
        Err(DbError::RedisCommandError(format!(
            "HSCAN 超过最大页数 {MAX_SCAN_PAGES}，疑似游标异常"
        )))
    }
}
