//! `page_token` 游标编解码（keyset 语义）。

use base64::engine::general_purpose::{GeneralPurpose, URL_SAFE_NO_PAD};
use base64::Engine;

use super::Error;

/// 游标内部的分隔符（ASCII US，0x1F）。
const SEPARATOR: char = '\u{1f}';

/// base64 URL-safe 无填充编码器。
const BASE64: GeneralPurpose = URL_SAFE_NO_PAD;

/// 编码游标：`base64(sort_order ‖ US ‖ option_id)`。
pub fn encode_cursor(sort_order: i64, option_id: &str) -> String {
    let raw = format!("{sort_order}{SEPARATOR}{option_id}");
    BASE64.encode(raw.as_bytes())
}

/// 解码游标；空串或全空白表示请求第一页。
///
/// 非法 base64、非 UTF-8、缺分隔符或排序键不是整数时 fail-closed。
pub fn decode_cursor(raw: &str) -> Result<Option<(i64, String)>, Error> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let bytes = BASE64.decode(trimmed).map_err(|_| Error::InvalidCursor)?;
    let decoded = String::from_utf8(bytes).map_err(|_| Error::InvalidCursor)?;
    let (order, id) = decoded.split_once(SEPARATOR).ok_or(Error::InvalidCursor)?;
    let sort_order = order.parse::<i64>().map_err(|_| Error::InvalidCursor)?;
    Ok(Some((sort_order, id.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_and_hides_the_key() {
        for (order, id) in [(42, "dept_sales"), (-7, "部门_销售"), (i64::MAX, "a:b c")] {
            let encoded = encode_cursor(order, id);
            assert_eq!(
                decode_cursor(&encoded).unwrap_or_else(|error| panic!("{error}")),
                Some((order, id.to_string()))
            );
            assert!(!encoded.contains(id));
        }
    }

    #[test]
    fn empty_cursor_is_the_first_page() {
        assert_eq!(
            decode_cursor("").unwrap_or_else(|error| panic!("{error}")),
            None
        );
        assert_eq!(
            decode_cursor("  ").unwrap_or_else(|error| panic!("{error}")),
            None
        );
    }

    #[test]
    fn malformed_cursor_fails_closed() {
        assert!(decode_cursor("not-base64!!").is_err());
        assert!(decode_cursor(&BASE64.encode(b"12345")).is_err());
        assert!(decode_cursor(&BASE64.encode("abc\u{1f}x".as_bytes())).is_err());
    }
}
