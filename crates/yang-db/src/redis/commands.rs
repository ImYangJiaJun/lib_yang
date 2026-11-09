//! 常用命令补遗：SET/ZADD 选项、键管理、Bitmap/HLL/GEO 薄封装。
//!
//! 这些命令都走统一的超时、重连与指标路径；复杂命令仍可通过
//! [`RedisClient::execute`](crate::RedisClient::execute) 逃生口发送。

use crate::redis::client::{collect_string_array, optional_string, parse_with_scores};
use crate::{DbError, RedisClient, Result};

/// SET 选项。
#[derive(Debug, Clone, Copy, Default)]
pub struct SetOptions {
    /// 仅当 key 不存在时设置
    pub nx: bool,
    /// 仅当 key 存在时设置
    pub xx: bool,
    /// 返回旧值（与 NX/XX 组合时语义见 Redis 文档）
    pub get: bool,
    /// 过期时间（秒）
    pub ex_seconds: Option<i64>,
    /// 过期时间（毫秒）
    pub px_millis: Option<i64>,
    /// 保留原 TTL
    pub keepttl: bool,
}

/// ZADD 选项。
#[derive(Debug, Clone, Copy, Default)]
pub struct ZAddOptions {
    /// 仅新增
    pub nx: bool,
    /// 仅更新已有成员
    pub xx: bool,
    /// 仅当新分数更高时更新
    pub gt: bool,
    /// 仅当新分数更低时更新
    pub lt: bool,
    /// 返回被修改的成员数（默认返回新增数）
    pub ch: bool,
}

/// GETEX 过期选项。
#[derive(Debug, Clone, Copy)]
pub enum GetExOption {
    /// 秒级过期
    Ex(i64),
    /// 毫秒级过期
    Px(i64),
    /// 移除过期时间
    Persist,
}

impl RedisClient {
    // ==================== String 补遗 ====================

    /// GETDEL：读取并删除。
    pub async fn getdel(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("GETDEL");
        cmd.arg(key.into());
        optional_string(&self.execute(&cmd).await?)
    }

    /// GETEX：读取并更新过期时间。
    pub async fn getex(
        &self,
        key: impl Into<String>,
        option: Option<GetExOption>,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("GETEX");
        cmd.arg(key.into());
        match option {
            Some(GetExOption::Ex(seconds)) => {
                cmd.arg("EX").arg(seconds);
            }
            Some(GetExOption::Px(millis)) => {
                cmd.arg("PX").arg(millis);
            }
            Some(GetExOption::Persist) => {
                cmd.arg("PERSIST");
            }
            None => {}
        }
        optional_string(&self.execute(&cmd).await?)
    }

    /// SET 带选项；`get=true` 时返回旧值。
    pub async fn set_with_options(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
        options: SetOptions,
    ) -> Result<Option<String>> {
        if options.nx && options.xx {
            return Err(DbError::InvalidArgument(
                "SET 选项 NX 与 XX 不能同时使用".to_string(),
            ));
        }
        if options.ex_seconds.is_some() && options.px_millis.is_some() {
            return Err(DbError::InvalidArgument(
                "SET 选项 EX 与 PX 不能同时使用".to_string(),
            ));
        }
        if options.keepttl && (options.ex_seconds.is_some() || options.px_millis.is_some()) {
            return Err(DbError::InvalidArgument(
                "SET 选项 KEEPTTL 不能与 EX/PX 同时使用".to_string(),
            ));
        }
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.into()).arg(value.into());
        if options.nx {
            cmd.arg("NX");
        } else if options.xx {
            cmd.arg("XX");
        }
        if options.get {
            cmd.arg("GET");
        }
        if let Some(seconds) = options.ex_seconds {
            cmd.arg("EX").arg(seconds);
        } else if let Some(millis) = options.px_millis {
            cmd.arg("PX").arg(millis);
        } else if options.keepttl {
            cmd.arg("KEEPTTL");
        }
        let result = self.execute(&cmd).await?;
        if options.get {
            optional_string(&result)
        } else {
            Ok(None)
        }
    }

    /// MSETNX：仅当所有 key 都不存在时批量设置。
    pub async fn msetnx(&self, pairs: &[(String, String)]) -> Result<bool> {
        if pairs.is_empty() {
            return Ok(true);
        }
        let mut cmd = redis::cmd("MSETNX");
        for (key, value) in pairs {
            cmd.arg(key).arg(value);
        }
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    // ==================== Hash 补遗 ====================

    /// HSETNX：字段不存在时设置。
    pub async fn hsetnx(
        &self,
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("HSETNX");
        cmd.arg(key.into()).arg(field.into()).arg(value.into());
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    /// HSET 多字段版本，返回新增字段数。
    pub async fn hset_multiple(
        &self,
        key: impl Into<String>,
        fields: &[(String, String)],
    ) -> Result<i64> {
        if fields.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("HSET");
        cmd.arg(key.into());
        for (field, value) in fields {
            cmd.arg(field).arg(value);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// HRANDFIELD：随机返回一个字段。
    pub async fn hrandfield(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut cmd = redis::cmd("HRANDFIELD");
        cmd.arg(key.into());
        optional_string(&self.execute(&cmd).await?)
    }

    /// HRANDFIELD COUNT：随机返回多个字段。
    pub async fn hrandfield_count(
        &self,
        key: impl Into<String>,
        count: i64,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("HRANDFIELD");
        cmd.arg(key.into()).arg(count);
        collect_string_array(&self.execute(&cmd).await?)
    }

    /// HRANDFIELD COUNT WITHVALUES：随机返回字段与值。
    pub async fn hrandfield_with_values(
        &self,
        key: impl Into<String>,
        count: i64,
    ) -> Result<Vec<(String, String)>> {
        let mut cmd = redis::cmd("HRANDFIELD");
        cmd.arg(key.into()).arg(count).arg("WITHVALUES");
        let flat = collect_string_array(&self.execute(&cmd).await?)?;
        if flat.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "HRANDFIELD WITHVALUES 返回数量不是偶数".to_string(),
            ));
        }
        Ok(flat
            .chunks_exact(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect())
    }

    // ==================== List 补遗 ====================

    /// LMOVE：从源列表一端弹出并推入目标列表一端。
    pub async fn lmove(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
        from_left: bool,
        to_left: bool,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("LMOVE");
        cmd.arg(source.into())
            .arg(destination.into())
            .arg(if from_left { "LEFT" } else { "RIGHT" })
            .arg(if to_left { "LEFT" } else { "RIGHT" });
        optional_string(&self.execute(&cmd).await?)
    }

    /// BRPOPLPUSH：阻塞版 RPOPLPUSH。
    pub async fn brpoplpush(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
        timeout_seconds: i64,
    ) -> Result<Option<String>> {
        let mut cmd = redis::cmd("BRPOPLPUSH");
        cmd.arg(source.into())
            .arg(destination.into())
            .arg(timeout_seconds);
        optional_string(&self.execute(&cmd).await?)
    }

    /// LPOP key count。
    pub async fn lpop_count(&self, key: impl Into<String>, count: u64) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("LPOP");
        cmd.arg(key.into()).arg(count);
        collect_string_array(&self.execute(&cmd).await?)
    }

    /// RPOP key count。
    pub async fn rpop_count(&self, key: impl Into<String>, count: u64) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("RPOP");
        cmd.arg(key.into()).arg(count);
        collect_string_array(&self.execute(&cmd).await?)
    }

    /// LMPOP：从多个列表之一弹出一批元素。
    pub async fn lmpop(
        &self,
        keys: &[String],
        left: bool,
        count: u64,
    ) -> Result<Option<(String, Vec<String>)>> {
        if keys.is_empty() {
            return Ok(None);
        }
        let mut cmd = redis::cmd("LMPOP");
        cmd.arg(keys.len());
        for key in keys {
            cmd.arg(key);
        }
        cmd.arg(if left { "LEFT" } else { "RIGHT" }).arg("COUNT");
        cmd.arg(count);
        parse_pop_reply(&self.execute(&cmd).await?)
    }

    /// BLMPOP：阻塞版 LMPOP（超时 0 表示无限等待）。
    pub async fn blmpop(
        &self,
        timeout_seconds: f64,
        keys: &[String],
        left: bool,
        count: u64,
    ) -> Result<Option<(String, Vec<String>)>> {
        if keys.is_empty() {
            return Ok(None);
        }
        let mut cmd = redis::cmd("BLMPOP");
        cmd.arg(timeout_seconds).arg(keys.len());
        for key in keys {
            cmd.arg(key);
        }
        cmd.arg(if left { "LEFT" } else { "RIGHT" }).arg("COUNT");
        cmd.arg(count);
        parse_pop_reply(&self.execute(&cmd).await?)
    }

    // ==================== Sorted Set 补遗 ====================

    /// ZADD 带选项（不含 INCR）。
    pub async fn zadd_with_options(
        &self,
        key: impl Into<String>,
        members: &[(f64, String)],
        options: ZAddOptions,
    ) -> Result<i64> {
        if options.nx && options.xx {
            return Err(DbError::InvalidArgument(
                "ZADD 选项 NX 与 XX 不能同时使用".to_string(),
            ));
        }
        if options.gt && options.lt {
            return Err(DbError::InvalidArgument(
                "ZADD 选项 GT 与 LT 不能同时使用".to_string(),
            ));
        }
        if members.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("ZADD");
        cmd.arg(key.into());
        if options.nx {
            cmd.arg("NX");
        }
        if options.xx {
            cmd.arg("XX");
        }
        if options.gt {
            cmd.arg("GT");
        }
        if options.lt {
            cmd.arg("LT");
        }
        if options.ch {
            cmd.arg("CH");
        }
        for (score, member) in members {
            cmd.arg(score).arg(member);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// ZADD INCR：返回增加后的分数。
    pub async fn zadd_incr(
        &self,
        key: impl Into<String>,
        increment: f64,
        member: impl Into<String>,
    ) -> Result<Option<f64>> {
        let mut cmd = redis::cmd("ZADD");
        cmd.arg(key.into())
            .arg("INCR")
            .arg(increment)
            .arg(member.into());
        let result = self.execute(&cmd).await?;
        if result.is_nil() {
            return Ok(None);
        }
        result
            .as_f64()
            .or_else(|| result.as_string().and_then(|value| value.parse().ok()))
            .map(Some)
            .ok_or_else(|| DbError::RedisTypeConversionError("ZADD INCR 未返回分数".to_string()))
    }

    /// ZPOPMIN。
    pub async fn zpopmin(
        &self,
        key: impl Into<String>,
        count: Option<u64>,
    ) -> Result<Vec<(String, f64)>> {
        self.zpop("ZPOPMIN", key.into(), count).await
    }

    /// ZPOPMAX。
    pub async fn zpopmax(
        &self,
        key: impl Into<String>,
        count: Option<u64>,
    ) -> Result<Vec<(String, f64)>> {
        self.zpop("ZPOPMAX", key.into(), count).await
    }

    async fn zpop(
        &self,
        command: &str,
        key: String,
        count: Option<u64>,
    ) -> Result<Vec<(String, f64)>> {
        let mut cmd = redis::cmd(command);
        cmd.arg(key);
        if let Some(count) = count {
            cmd.arg(count);
        }
        parse_with_scores(&self.execute(&cmd).await?)
    }

    /// ZMSCORE：批量获取分数。
    pub async fn zmscore(
        &self,
        key: impl Into<String>,
        members: &[String],
    ) -> Result<Vec<Option<f64>>> {
        if members.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd = redis::cmd("ZMSCORE");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let value = self.execute(&cmd).await?;
        let items = value
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("ZMSCORE 结果不是数组".to_string()))?;
        Ok(items
            .iter()
            .map(|item| {
                if item.is_nil() {
                    None
                } else {
                    item.as_f64()
                        .or_else(|| item.as_string().and_then(|value| value.parse().ok()))
                }
            })
            .collect())
    }

    // ==================== 键管理补遗 ====================

    /// UNLINK：异步删除。
    pub async fn unlink(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("UNLINK");
        for key in keys {
            cmd.arg(key);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// TYPE。
    pub async fn key_type(&self, key: impl Into<String>) -> Result<String> {
        let mut cmd = redis::cmd("TYPE");
        cmd.arg(key.into());
        optional_string(&self.execute(&cmd).await?)
            .map(|value| value.unwrap_or_else(|| "none".to_string()))
    }

    /// RENAME。
    pub async fn rename(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
    ) -> Result<()> {
        let mut cmd = redis::cmd("RENAME");
        cmd.arg(source.into()).arg(destination.into());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// COPY（`replace=true` 覆盖目标）。
    pub async fn copy(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
        replace: bool,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("COPY");
        cmd.arg(source.into()).arg(destination.into());
        if replace {
            cmd.arg("REPLACE");
        }
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    /// TOUCH：更新访问时间。
    pub async fn touch(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("TOUCH");
        for key in keys {
            cmd.arg(key);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// PTTL。
    pub async fn pttl(&self, key: impl Into<String>) -> Result<i64> {
        let mut cmd = redis::cmd("PTTL");
        cmd.arg(key.into());
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(-1))
    }

    /// PEXPIRE。
    pub async fn pexpire(&self, key: impl Into<String>, milliseconds: i64) -> Result<bool> {
        let mut cmd = redis::cmd("PEXPIRE");
        cmd.arg(key.into()).arg(milliseconds);
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    // ==================== Bitmap ====================

    /// SETBIT。
    pub async fn setbit(&self, key: impl Into<String>, offset: u64, value: bool) -> Result<i64> {
        let mut cmd = redis::cmd("SETBIT");
        cmd.arg(key.into())
            .arg(offset)
            .arg(if value { 1 } else { 0 });
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// GETBIT。
    pub async fn getbit(&self, key: impl Into<String>, offset: u64) -> Result<bool> {
        let mut cmd = redis::cmd("GETBIT");
        cmd.arg(key.into()).arg(offset);
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    /// BITCOUNT（可选字节范围）。
    pub async fn bitcount(&self, key: impl Into<String>, range: Option<(i64, i64)>) -> Result<i64> {
        let mut cmd = redis::cmd("BITCOUNT");
        cmd.arg(key.into());
        if let Some((start, end)) = range {
            cmd.arg(start).arg(end);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// BITOP。
    pub async fn bitop(
        &self,
        operation: &str,
        destination: impl Into<String>,
        sources: &[String],
    ) -> Result<i64> {
        if sources.is_empty() {
            return Err(DbError::InvalidArgument(
                "BITOP 至少需要一个源 key".to_string(),
            ));
        }
        let mut cmd = redis::cmd("BITOP");
        cmd.arg(operation).arg(destination.into());
        for source in sources {
            cmd.arg(source);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    // ==================== HyperLogLog ====================

    /// PFADD。
    pub async fn pfadd(&self, key: impl Into<String>, elements: &[String]) -> Result<i64> {
        if elements.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("PFADD");
        cmd.arg(key.into());
        for element in elements {
            cmd.arg(element);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// PFCOUNT。
    pub async fn pfcount(&self, keys: &[String]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("PFCOUNT");
        for key in keys {
            cmd.arg(key);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// PFMERGE。
    pub async fn pfmerge(&self, destination: impl Into<String>, sources: &[String]) -> Result<()> {
        let mut cmd = redis::cmd("PFMERGE");
        cmd.arg(destination.into());
        for source in sources {
            cmd.arg(source);
        }
        self.execute(&cmd).await?;
        Ok(())
    }

    // ==================== GEO ====================

    /// GEOADD（经度、纬度、成员）。
    pub async fn geoadd(
        &self,
        key: impl Into<String>,
        members: &[(f64, f64, String)],
    ) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("GEOADD");
        cmd.arg(key.into());
        for (longitude, latitude, member) in members {
            cmd.arg(longitude).arg(latitude).arg(member);
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// GEOPOS：批量获取坐标。
    pub async fn geopos(
        &self,
        key: impl Into<String>,
        members: &[String],
    ) -> Result<Vec<Option<(f64, f64)>>> {
        if members.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd = redis::cmd("GEOPOS");
        cmd.arg(key.into());
        for member in members {
            cmd.arg(member);
        }
        let value = self.execute(&cmd).await?;
        let items = value
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("GEOPOS 结果不是数组".to_string()))?;
        items
            .iter()
            .map(|item| {
                if item.is_nil() {
                    return Ok(None);
                }
                let pair = item.as_array().ok_or_else(|| {
                    DbError::RedisTypeConversionError("GEOPOS 坐标不是数组".to_string())
                })?;
                if pair.len() != 2 {
                    return Ok(None);
                }
                let longitude = pair[0].as_f64().ok_or_else(|| {
                    DbError::RedisTypeConversionError("GEOPOS 经度不是数字".to_string())
                })?;
                let latitude = pair[1].as_f64().ok_or_else(|| {
                    DbError::RedisTypeConversionError("GEOPOS 纬度不是数字".to_string())
                })?;
                Ok(Some((longitude, latitude)))
            })
            .collect()
    }

    /// GEODIST（单位 m/km/mi/ft）。
    pub async fn geodist(
        &self,
        key: impl Into<String>,
        member1: impl Into<String>,
        member2: impl Into<String>,
        unit: Option<&str>,
    ) -> Result<Option<f64>> {
        let mut cmd = redis::cmd("GEODIST");
        cmd.arg(key.into()).arg(member1.into()).arg(member2.into());
        if let Some(unit) = unit {
            cmd.arg(unit);
        }
        let result = self.execute(&cmd).await?;
        if result.is_nil() {
            return Ok(None);
        }
        result
            .as_f64()
            .or_else(|| result.as_string().and_then(|value| value.parse().ok()))
            .map(Some)
            .ok_or_else(|| DbError::RedisTypeConversionError("GEODIST 未返回数字".to_string()))
    }

    /// GEOSEARCH BYRADIUS，返回命中成员（按距离升序）。
    pub async fn geosearch_radius(
        &self,
        key: impl Into<String>,
        longitude: f64,
        latitude: f64,
        radius: f64,
        unit: &str,
    ) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("GEOSEARCH");
        cmd.arg(key.into())
            .arg("FROMLONLAT")
            .arg(longitude)
            .arg(latitude)
            .arg("BYRADIUS")
            .arg(radius)
            .arg(unit)
            .arg("ASC");
        collect_string_array(&self.execute(&cmd).await?)
    }
}

fn parse_pop_reply(value: &crate::RedisValue) -> Result<Option<(String, Vec<String>)>> {
    let Some(items) = value.as_array() else {
        return Ok(None);
    };
    if items.is_empty() {
        return Ok(None);
    }
    let key = items[0].as_string().ok_or_else(|| {
        DbError::RedisTypeConversionError("LMPOP 返回的 key 不是字符串".to_string())
    })?;
    let values = collect_string_array(&items[1])?;
    Ok(Some((key, values)))
}
