//! 多维表格链接解析与坐标类型。

use crate::error::{Error, Result};
use crate::path::validate_path_segment;

/// 一个数据源在多维表格里的坐标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitableCoordinates {
    /// Base token。
    pub app_token: String,
    /// 表 id。
    pub table_id: String,
    /// 视图 id（可选；决定拉取哪些行，不决定能勾哪些字段）。
    pub view_id: Option<String>,
}

/// 从多维表格链接提取的坐标；`table_id` / `view_id` 可缺。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBitableLink {
    /// Base token。
    pub app_token: String,
    /// 表 id。
    pub table_id: Option<String>,
    /// 视图 id。
    pub view_id: Option<String>,
}

/// 解析完整链接或兼容旧的纯 Base Token。
pub fn parse_bitable_link(raw: &str) -> Result<ParsedBitableLink> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(Error::fatal("多维表格链接或 Base Token 不能为空"));
    }

    let Some((scheme, remainder)) = raw.split_once("://") else {
        validate_path_segment("bitable_base_token", raw)
            .map_err(|error| Error::fatal(error.to_string()))?;
        return Ok(ParsedBitableLink {
            app_token: raw.to_string(),
            table_id: None,
            view_id: None,
        });
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return Err(Error::fatal("多维表格链接协议必须是 http 或 https"));
    }
    let slash = remainder
        .find('/')
        .ok_or_else(|| Error::fatal("多维表格链接缺少 /base/ 路径"))?;
    let path_and_query = &remainder[slash..];
    let (path, query_and_fragment) = path_and_query
        .split_once('?')
        .map_or((path_and_query, ""), |(path, query)| (path, query));
    let path = path.split('#').next().unwrap_or_default();
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() != 2 || !segments[0].eq_ignore_ascii_case("base") {
        return Err(Error::fatal("多维表格链接路径必须是 /base/<token>"));
    }
    let app_token = segments[1].to_string();
    validate_path_segment("bitable_base_token", &app_token)
        .map_err(|error| Error::fatal(error.to_string()))?;

    let query = query_and_fragment.split('#').next().unwrap_or_default();
    let mut table_id = None;
    let mut view_id = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = value.trim();
        let target = match key {
            "table" => &mut table_id,
            "view" => &mut view_id,
            _ => continue,
        };
        if target.is_some() {
            return Err(Error::fatal(format!("多维表格链接中的 {key} 参数重复")));
        }
        if value.is_empty() {
            return Err(Error::fatal(format!("多维表格链接中的 {key} 参数不能为空")));
        }
        validate_path_segment(
            if key == "table" {
                "bitable_table_id"
            } else {
                "bitable_view_id"
            },
            value,
        )
        .map_err(|error| Error::fatal(error.to_string()))?;
        *target = Some(value.to_string());
    }

    Ok(ParsedBitableLink {
        app_token,
        table_id,
        view_id,
    })
}

/// 合并完整链接与旧的分字段输入；两边都给了但值不同必须报错。
pub fn resolve_bitable_link(
    base_token: &str,
    table_id: Option<&str>,
    view_id: Option<&str>,
) -> Result<ParsedBitableLink> {
    let mut parsed = parse_bitable_link(base_token)?;
    let merge =
        |name: &str, explicit: Option<&str>, embedded: Option<String>| -> Result<Option<String>> {
            let explicit = explicit.map(str::trim).filter(|value| !value.is_empty());
            if let (Some(explicit), Some(embedded)) = (explicit, embedded.as_deref()) {
                if explicit != embedded {
                    return Err(Error::fatal(format!(
                        "完整链接中的 {name} 与单独字段不一致"
                    )));
                }
            }
            Ok(explicit.map(str::to_string).or(embedded))
        };
    parsed.table_id = merge("bitable_table_id", table_id, parsed.table_id.take())?;
    parsed.view_id = merge("bitable_view_id", view_id, parsed.view_id.take())?;
    if let Some(value) = parsed.table_id.as_deref() {
        validate_path_segment("bitable_table_id", value)
            .map_err(|error| Error::fatal(error.to_string()))?;
    }
    if let Some(value) = parsed.view_id.as_deref() {
        validate_path_segment("bitable_view_id", value)
            .map_err(|error| Error::fatal(error.to_string()))?;
    }
    Ok(parsed)
}

/// 展示链接只接受飞书/Lark HTTPS 地址。
pub fn is_document_link(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let Some((host, _)) = rest.split_once('/') else {
        return false;
    };
    value.len() <= 4096
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        && (host.ends_with(".feishu.cn") || host.ends_with(".larksuite.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_link_without_relying_on_host() {
        let parsed = parse_bitable_link(
            "https://any-tenant.example/base/ZoCWb82JQaCCiAspCqbcUvlsnwg?table=tblfFjlSuIQeMCJF&view=vewAEKSbvO",
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.app_token, "ZoCWb82JQaCCiAspCqbcUvlsnwg");
        assert_eq!(parsed.table_id.as_deref(), Some("tblfFjlSuIQeMCJF"));
        assert_eq!(parsed.view_id.as_deref(), Some("vewAEKSbvO"));
    }

    #[test]
    fn keeps_missing_coordinates_as_none() {
        let parsed = parse_bitable_link("https://tenant/base/appToken")
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.table_id, None);
        assert_eq!(parsed.view_id, None);

        let parsed = parse_bitable_link("https://tenant/base/appToken?table=tblA")
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.table_id.as_deref(), Some("tblA"));
        assert_eq!(parsed.view_id, None);
    }

    #[test]
    fn rejects_non_base_paths_and_unsafe_coordinates() {
        assert!(parse_bitable_link("https://tenant/wiki/appToken?table=tblA").is_err());
        assert!(parse_bitable_link("https://tenant/base/appToken?table=../evil").is_err());
    }

    #[test]
    fn resolves_and_merges_explicit_fields() {
        let parsed = resolve_bitable_link(
            "https://tenant/base/appToken?table=tblA",
            Some("tblA"),
            None,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.table_id.as_deref(), Some("tblA"));
        assert!(resolve_bitable_link(
            "https://tenant/base/appToken?table=tblA",
            Some("tblB"),
            None
        )
        .is_err());
    }

    #[test]
    fn document_links_are_restricted_to_feishu_hosts() {
        assert!(is_document_link("https://x.feishu.cn/base/app"));
        assert!(is_document_link("https://x.larksuite.com/base/app"));
        assert!(!is_document_link("http://x.feishu.cn/base/app"));
        assert!(!is_document_link("https://feishu.cn@evil/base/app"));
        assert!(!is_document_link("https://evil.com/base/app"));
        assert!(!is_document_link("https://x.feishu.cn/base app"));
    }
}
