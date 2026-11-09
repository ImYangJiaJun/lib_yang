//! Redis Streams。
//!
//! 复用 `redis` crate 的结构化回复类型（`StreamId`/`StreamReadReply` 等），
//! 在客户端上提供 XADD/XRANGE/XREAD/消费者组/Pending 恢复的一等公民入口。
//! 全部走统一的超时、重连与指标路径。

use crate::{DbError, RedisClient, Result};

/// XREADGROUP 选项。
#[derive(Debug, Clone, Copy)]
pub struct XReadGroupOptions<'a> {
    /// 消费者组名
    pub group: &'a str,
    /// 消费者名
    pub consumer: &'a str,
    /// 每个 stream 最多返回条数
    pub count: Option<u64>,
    /// 阻塞毫秒数
    pub block_ms: Option<u64>,
    /// 不写入 PEL
    pub noack: bool,
}

pub use redis::streams::{
    StreamAddOptions, StreamAutoClaimOptions, StreamAutoClaimReply, StreamClaimOptions,
    StreamClaimReply, StreamId, StreamInfoConsumersReply, StreamInfoGroupsReply,
    StreamInfoStreamReply, StreamKey, StreamPendingCountReply, StreamPendingData, StreamPendingId,
    StreamPendingReply, StreamRangeReply, StreamReadOptions, StreamReadReply,
};

impl RedisClient {
    /// XADD，返回生成的 entry ID。
    pub async fn xadd(
        &self,
        key: impl AsRef<[u8]>,
        id: &str,
        fields: &[(&str, &str)],
    ) -> Result<String> {
        self.xadd_with_options(key, id, fields, &StreamAddOptions::default())
            .await
    }

    /// XADD（带 NOMKSTREAM / MAXLEN 等选项）。
    pub async fn xadd_with_options(
        &self,
        key: impl AsRef<[u8]>,
        id: &str,
        fields: &[(&str, &str)],
        options: &StreamAddOptions,
    ) -> Result<String> {
        let mut cmd = redis::cmd("XADD");
        cmd.arg(key.as_ref()).arg(options).arg(id);
        for (field, value) in fields {
            cmd.arg(*field).arg(*value);
        }
        self.execute(&cmd)
            .await?
            .as_string()
            .ok_or_else(|| DbError::RedisTypeConversionError("XADD 未返回 entry ID".to_string()))
    }

    /// XLEN。
    pub async fn xlen(&self, key: impl AsRef<[u8]>) -> Result<u64> {
        let mut cmd = redis::cmd("XLEN");
        cmd.arg(key.as_ref());
        require_u64(&self.execute(&cmd).await?, "XLEN")
    }

    /// XRANGE。
    pub async fn xrange(
        &self,
        key: impl AsRef<[u8]>,
        start: &str,
        end: &str,
        count: Option<u64>,
    ) -> Result<StreamRangeReply> {
        let mut cmd = redis::cmd("XRANGE");
        cmd.arg(key.as_ref()).arg(start).arg(end);
        if let Some(count) = count {
            cmd.arg("COUNT").arg(count);
        }
        self.execute_typed(&cmd).await
    }

    /// XREVRANGE。
    pub async fn xrevrange(
        &self,
        key: impl AsRef<[u8]>,
        end: &str,
        start: &str,
        count: Option<u64>,
    ) -> Result<StreamRangeReply> {
        let mut cmd = redis::cmd("XREVRANGE");
        cmd.arg(key.as_ref()).arg(end).arg(start);
        if let Some(count) = count {
            cmd.arg("COUNT").arg(count);
        }
        self.execute_typed(&cmd).await
    }

    /// XREAD（非消费者组）。
    pub async fn xread(
        &self,
        keys: &[String],
        ids: &[String],
        count: Option<u64>,
        block_ms: Option<u64>,
    ) -> Result<StreamReadReply> {
        if keys.len() != ids.len() {
            return Err(DbError::InvalidArgument(
                "XREAD keys 与 ids 数量必须一致".to_string(),
            ));
        }
        let mut cmd = redis::cmd("XREAD");
        if let Some(count) = count {
            cmd.arg("COUNT").arg(count);
        }
        if let Some(block_ms) = block_ms {
            cmd.arg("BLOCK").arg(block_ms);
        }
        cmd.arg("STREAMS");
        for key in keys {
            cmd.arg(key);
        }
        for id in ids {
            cmd.arg(id);
        }
        self.execute_typed(&cmd).await
    }

    /// XREADGROUP。
    #[allow(clippy::too_many_arguments)]
    pub async fn xread_group(
        &self,
        keys: &[String],
        ids: &[String],
        group: &str,
        consumer: &str,
        count: Option<u64>,
        block_ms: Option<u64>,
        noack: bool,
    ) -> Result<StreamReadReply> {
        self.xread_group_with_options(
            keys,
            ids,
            XReadGroupOptions {
                group,
                consumer,
                count,
                block_ms,
                noack,
            },
        )
        .await
    }

    /// XREADGROUP（选项结构体版本）。
    pub async fn xread_group_with_options(
        &self,
        keys: &[String],
        ids: &[String],
        options: XReadGroupOptions<'_>,
    ) -> Result<StreamReadReply> {
        if keys.len() != ids.len() {
            return Err(DbError::InvalidArgument(
                "XREADGROUP keys 与 ids 数量必须一致".to_string(),
            ));
        }
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP").arg(options.group).arg(options.consumer);
        if let Some(count) = options.count {
            cmd.arg("COUNT").arg(count);
        }
        if let Some(block_ms) = options.block_ms {
            cmd.arg("BLOCK").arg(block_ms);
        }
        if options.noack {
            cmd.arg("NOACK");
        }
        cmd.arg("STREAMS");
        for key in keys {
            cmd.arg(key);
        }
        for id in ids {
            cmd.arg(id);
        }
        self.execute_typed(&cmd).await
    }

    /// XDEL。
    pub async fn xdel(&self, key: impl AsRef<[u8]>, ids: &[String]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("XDEL");
        cmd.arg(key.as_ref());
        for id in ids {
            cmd.arg(id);
        }
        require_u64(&self.execute(&cmd).await?, "XDEL")
    }

    /// XTRIM MAXLEN。
    pub async fn xtrim_maxlen(
        &self,
        key: impl AsRef<[u8]>,
        maxlen: u64,
        approx: bool,
    ) -> Result<u64> {
        let mut cmd = redis::cmd("XTRIM");
        cmd.arg(key.as_ref()).arg("MAXLEN");
        if approx {
            cmd.arg("~");
        }
        cmd.arg(maxlen);
        require_u64(&self.execute(&cmd).await?, "XTRIM")
    }

    /// XGROUP CREATE。
    pub async fn xgroup_create(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        id: &str,
        mkstream: bool,
    ) -> Result<()> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("CREATE").arg(key.as_ref()).arg(group).arg(id);
        if mkstream {
            cmd.arg("MKSTREAM");
        }
        self.execute_typed::<String>(&cmd).await?;
        Ok(())
    }

    /// XGROUP DESTROY。
    pub async fn xgroup_destroy(&self, key: impl AsRef<[u8]>, group: &str) -> Result<u64> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("DESTROY").arg(key.as_ref()).arg(group);
        require_u64(&self.execute(&cmd).await?, "XGROUP DESTROY")
    }

    /// XGROUP CREATECONSUMER。
    pub async fn xgroup_createconsumer(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
    ) -> Result<u64> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("CREATECONSUMER")
            .arg(key.as_ref())
            .arg(group)
            .arg(consumer);
        require_u64(&self.execute(&cmd).await?, "XGROUP CREATECONSUMER")
    }

    /// XGROUP DELCONSUMER。
    pub async fn xgroup_delconsumer(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
    ) -> Result<u64> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("DELCONSUMER")
            .arg(key.as_ref())
            .arg(group)
            .arg(consumer);
        require_u64(&self.execute(&cmd).await?, "XGROUP DELCONSUMER")
    }

    /// XGROUP SETID。
    pub async fn xgroup_setid(&self, key: impl AsRef<[u8]>, group: &str, id: &str) -> Result<()> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("SETID").arg(key.as_ref()).arg(group).arg(id);
        self.execute_typed::<String>(&cmd).await?;
        Ok(())
    }

    /// XACK。
    pub async fn xack(&self, key: impl AsRef<[u8]>, group: &str, ids: &[String]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("XACK");
        cmd.arg(key.as_ref()).arg(group);
        for id in ids {
            cmd.arg(id);
        }
        require_u64(&self.execute(&cmd).await?, "XACK")
    }

    /// XPENDING 摘要。
    pub async fn xpending(&self, key: impl AsRef<[u8]>, group: &str) -> Result<StreamPendingReply> {
        let mut cmd = redis::cmd("XPENDING");
        cmd.arg(key.as_ref()).arg(group);
        self.execute_typed(&cmd).await
    }

    /// XPENDING 范围查询（可选 consumer 过滤）。
    pub async fn xpending_range(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        start: &str,
        end: &str,
        count: u64,
        consumer: Option<&str>,
    ) -> Result<StreamPendingCountReply> {
        let mut cmd = redis::cmd("XPENDING");
        cmd.arg(key.as_ref())
            .arg(group)
            .arg(start)
            .arg(end)
            .arg(count);
        if let Some(consumer) = consumer {
            cmd.arg(consumer);
        }
        self.execute_typed(&cmd).await
    }

    /// XCLAIM（不含 IDLE/TIME/RETRYCOUNT 等高级选项）。
    pub async fn xclaim(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
        min_idle_ms: u64,
        ids: &[String],
    ) -> Result<StreamClaimReply> {
        self.xclaim_with_options(
            key,
            group,
            consumer,
            min_idle_ms,
            ids,
            &StreamClaimOptions::default(),
        )
        .await
    }

    /// XCLAIM（带选项）。
    pub async fn xclaim_with_options(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
        min_idle_ms: u64,
        ids: &[String],
        options: &StreamClaimOptions,
    ) -> Result<StreamClaimReply> {
        let mut cmd = redis::cmd("XCLAIM");
        cmd.arg(key.as_ref())
            .arg(group)
            .arg(consumer)
            .arg(min_idle_ms);
        for id in ids {
            cmd.arg(id);
        }
        cmd.arg(options);
        self.execute_typed(&cmd).await
    }

    /// XAUTOCLAIM。
    pub async fn xautoclaim(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
        min_idle_ms: u64,
        start: &str,
        count: Option<u64>,
    ) -> Result<StreamAutoClaimReply> {
        let mut options = StreamAutoClaimOptions::default();
        if let Some(count) = count {
            options = options.count(count as usize);
        }
        self.xautoclaim_with_options(key, group, consumer, min_idle_ms, start, &options)
            .await
    }

    /// XAUTOCLAIM（带选项）。
    pub async fn xautoclaim_with_options(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
        consumer: &str,
        min_idle_ms: u64,
        start: &str,
        options: &StreamAutoClaimOptions,
    ) -> Result<StreamAutoClaimReply> {
        let mut cmd = redis::cmd("XAUTOCLAIM");
        cmd.arg(key.as_ref())
            .arg(group)
            .arg(consumer)
            .arg(min_idle_ms)
            .arg(start)
            .arg(options);
        self.execute_typed(&cmd).await
    }

    /// XINFO STREAM。
    pub async fn xinfo_stream(&self, key: impl AsRef<[u8]>) -> Result<StreamInfoStreamReply> {
        let mut cmd = redis::cmd("XINFO");
        cmd.arg("STREAM").arg(key.as_ref());
        self.execute_typed(&cmd).await
    }

    /// XINFO GROUPS。
    pub async fn xinfo_groups(&self, key: impl AsRef<[u8]>) -> Result<StreamInfoGroupsReply> {
        let mut cmd = redis::cmd("XINFO");
        cmd.arg("GROUPS").arg(key.as_ref());
        self.execute_typed(&cmd).await
    }

    /// XINFO CONSUMERS。
    pub async fn xinfo_consumers(
        &self,
        key: impl AsRef<[u8]>,
        group: &str,
    ) -> Result<StreamInfoConsumersReply> {
        let mut cmd = redis::cmd("XINFO");
        cmd.arg("CONSUMERS").arg(key.as_ref()).arg(group);
        self.execute_typed(&cmd).await
    }
}

fn require_u64(value: &crate::RedisValue, operation: &str) -> Result<u64> {
    let int = value
        .as_i64()
        .ok_or_else(|| DbError::RedisTypeConversionError(format!("{operation} 未返回整数")))?;
    u64::try_from(int)
        .map_err(|_| DbError::RedisTypeConversionError(format!("{operation} 返回负数")))
}
