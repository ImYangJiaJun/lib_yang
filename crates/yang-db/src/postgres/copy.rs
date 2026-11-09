//! PostgreSQL COPY 协议封装（`COPY ... FROM STDIN` / `COPY ... TO STDOUT`）。
//!
//! 设计要点：
//! - COPY 在独立连接上执行：入口从连接池借出一条连接，正常结束后归还；
//!   任何协议状态不确定的失败都会 `detach` 该连接（关闭而非放回池），
//!   避免污染池内连接的协议状态。
//! - 导入入口 [`Database::copy_from_csv`] 接受行迭代器，按块发送 CSV 数据；
//!   导出入口 [`Database::copy_to_csv`] 把输出一次性收集为字节。
//! - 本模块不做 CSV 转义：行内容按调用方提供的字节原样发送。
//! - `COPY` 是单条语句：中途失败时服务端会整体丢弃本批数据。

use crate::error::DbError;
use crate::postgres::Database;
use sqlx::postgres::PgConnection;
use std::future::poll_fn;

/// 发送缓冲上限：达到后立即向服务端发送一块 COPY 数据，避免逐行一条协议消息。
const COPY_SEND_BUFFER_BYTES: usize = 64 * 1024;

/// 校验 COPY 语句非空并去掉首尾空白。
///
/// 语句形状（`FROM STDIN` / `TO STDOUT`）由 sqlx 与服务端校验，这里只拦住空语句，
/// 避免把明显的调用方错误变成协议层错误。
fn validate_copy_sql(sql: &str) -> Result<&str, DbError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(DbError::InvalidArgument(
            "PostgreSQL COPY 语句不能为空".to_string(),
        ));
    }
    Ok(trimmed)
}

/// 把一行 CSV 追加到发送缓冲；行内容缺少换行时补一个 `\n`。
///
/// 项内容按字节原样保留（引号字段内的换行不受影响），本函数只负责行尾换行。
fn append_csv_row(buffer: &mut Vec<u8>, row: &[u8]) {
    buffer.extend_from_slice(row);
    if !row.ends_with(b"\n") {
        buffer.push(b'\n');
    }
}

impl Database {
    /// 通过 `COPY ... FROM STDIN` 批量导入 CSV 行。
    ///
    /// `sql` 必须是形如
    /// `COPY <表> [(列...)] FROM STDIN WITH (FORMAT csv, ...)` 的语句，其余
    /// 格式选项由 SQL 文本决定。`rows` 的每一项是一条完整 CSV 记录：末尾缺少
    /// `\n` 时自动补一个；字段内的 CSV 转义（引号、逗号、换行）由调用方负责，
    /// 本方法按字节原样发送，不做转义。
    ///
    /// 返回服务端确认的写入行数。失败时若连接协议状态已无法确认，该连接会被
    /// 关闭而不是放回连接池。
    pub async fn copy_from_csv<I, R>(&self, sql: &str, rows: I) -> Result<u64, DbError>
    where
        I: IntoIterator<Item = R>,
        R: AsRef<[u8]>,
    {
        let sql = validate_copy_sql(sql)?;
        let mut conn = self.acquire().await?;
        match copy_in_rows(&mut conn, sql, rows).await {
            Ok(affected) => Ok(affected),
            Err(error) => {
                // 协议状态未知：关闭连接，不把脏连接放回池。
                drop(conn.detach());
                Err(error)
            }
        }
    }

    /// 通过 `COPY ... TO STDOUT` 导出 CSV 数据。
    ///
    /// `sql` 必须是形如
    /// `COPY (SELECT ...) TO STDOUT WITH (FORMAT csv, ...)` 的语句。返回服务端
    /// 输出的完整原始字节（CSV 文本）。
    ///
    /// 输出会一次性收集到内存；导出超大结果集时请改用
    /// [`stream_query`](Self::stream_query) 分页读取。
    ///
    /// 失败时若连接协议状态已无法确认，该连接会被关闭而不是放回连接池。
    pub async fn copy_to_csv(&self, sql: &str) -> Result<Vec<u8>, DbError> {
        let sql = validate_copy_sql(sql)?;
        let mut conn = self.acquire().await?;
        match copy_out_bytes(&mut conn, sql).await {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                // 未读完的 COPY OUT 会残留服务端待发送数据：关闭连接。
                drop(conn.detach());
                Err(error)
            }
        }
    }
}

/// 在指定连接上执行 `COPY ... FROM STDIN` 并逐块发送行数据。
async fn copy_in_rows<I, R>(conn: &mut PgConnection, sql: &str, rows: I) -> Result<u64, DbError>
where
    I: IntoIterator<Item = R>,
    R: AsRef<[u8]>,
{
    let mut copy = conn.copy_in_raw(sql).await?;
    let mut buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
    for row in rows {
        let row = row.as_ref();
        if buffer.len() + row.len() + 1 > COPY_SEND_BUFFER_BYTES && !buffer.is_empty() {
            copy.send(buffer.as_slice()).await?;
            buffer.clear();
        }
        append_csv_row(&mut buffer, row);
    }
    if !buffer.is_empty() {
        copy.send(buffer.as_slice()).await?;
    }
    Ok(copy.finish().await?)
}

/// 在指定连接上执行 `COPY ... TO STDOUT` 并把全部数据块收集为字节。
async fn copy_out_bytes(conn: &mut PgConnection, sql: &str) -> Result<Vec<u8>, DbError> {
    let mut stream = conn.copy_out_raw(sql).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        bytes.extend_from_slice(&chunk?);
    }
    Ok(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn validate_copy_sql_rejects_blank_statements() {
        for blank in ["", "   ", "\n\t"] {
            assert!(
                matches!(validate_copy_sql(blank), Err(DbError::InvalidArgument(_))),
                "空语句应被拒绝: {blank:?}"
            );
        }
        assert_eq!(
            validate_copy_sql("  COPY t FROM STDIN  ").unwrap(),
            "COPY t FROM STDIN"
        );
    }

    #[test]
    fn append_csv_row_adds_missing_newline_and_preserves_existing() {
        let mut buffer = Vec::new();
        append_csv_row(&mut buffer, b"1,alice");
        assert_eq!(buffer, b"1,alice\n");

        // 已带行尾换行时不重复追加。
        append_csv_row(&mut buffer, b"2,bob\n");
        assert_eq!(buffer, b"1,alice\n2,bob\n");

        // 空记录写入一个空行。
        append_csv_row(&mut buffer, b"");
        assert_eq!(buffer, b"1,alice\n2,bob\n\n");

        // 引号字段内的换行按原样保留。
        let mut quoted = Vec::new();
        append_csv_row(&mut quoted, b"\"a\nb\",c");
        assert_eq!(quoted, b"\"a\nb\",c\n");
    }
}
