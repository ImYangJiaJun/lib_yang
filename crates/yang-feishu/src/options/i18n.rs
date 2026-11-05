//! `@i18n@` 占位符与 `i18nResources` 构造。

use std::collections::BTreeMap;

use super::protocol::{FeishuI18nResource, FeishuOption, FeishuResultBody};

/// 未指定语言时的默认语言环境。
pub const DEFAULT_LOCALE: &str = "zh_cn";

/// `value` 的固定前缀。
pub const I18N_PREFIX: &str = "@i18n@";

/// 由选项 id 生成 `@i18n@` 键。
pub fn i18n_key(option_id: &str) -> String {
    format!("{I18N_PREFIX}{option_id}")
}

/// 把飞书回传的联动值归一成可用的父值。
///
/// 有前缀才剥、没前缀原样用、两端一律 trim。真机回传的是文案，不是 `option_id`。
pub fn normalize_linkage_value(raw: &str) -> &str {
    let trimmed = raw.trim();
    let stripped = trimmed.strip_prefix(I18N_PREFIX).unwrap_or(trimmed);
    stripped.trim()
}

/// 构造响应所需的一行选项数据。
#[derive(Debug, Clone)]
pub struct OptionRow {
    /// 选项唯一标识。
    pub option_id: String,
    /// 默认语言下的文案。
    pub label: String,
    /// 额外语言下的文案。
    pub i18n: BTreeMap<String, String>,
    /// 稳定排序键（供游标编码）。
    pub sort_order: i64,
    /// 是否默认选项。
    pub is_default: bool,
}

/// 组装 `data.result` 的明文内容。
///
/// 语言集合 = 默认语言 ∪ 各行额外语言；默认语言恒存在并标记 `isDefault`；
/// 某语言缺某条文案时回退默认文案。
pub fn build_result_body(
    rows: &[OptionRow],
    default_locale: &str,
    has_more: bool,
    next_page_token: Option<String>,
) -> FeishuResultBody {
    let options = rows
        .iter()
        .map(|row| FeishuOption {
            id: row.option_id.clone(),
            value: i18n_key(&row.option_id),
            is_default: row.is_default.then_some(true),
        })
        .collect();

    let mut locales: Vec<String> = vec![default_locale.to_string()];
    let mut extra: Vec<&String> = rows
        .iter()
        .flat_map(|row| row.i18n.keys())
        .filter(|locale| locale.as_str() != default_locale)
        .collect();
    extra.sort();
    extra.dedup();
    locales.extend(extra.into_iter().cloned());

    let i18n_resources = locales
        .into_iter()
        .map(|locale| {
            let is_default = locale == default_locale;
            let texts = rows
                .iter()
                .map(|row| {
                    let text = if is_default {
                        row.label.clone()
                    } else {
                        row.i18n
                            .get(&locale)
                            .cloned()
                            .unwrap_or_else(|| row.label.clone())
                    };
                    (i18n_key(&row.option_id), text)
                })
                .collect::<BTreeMap<_, _>>();
            FeishuI18nResource {
                locale,
                is_default,
                texts,
            }
        })
        .collect();

    FeishuResultBody {
        options,
        i18n_resources,
        has_more,
        next_page_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, label: &str, extra: &[(&str, &str)]) -> OptionRow {
        OptionRow {
            option_id: id.into(),
            label: label.into(),
            i18n: extra
                .iter()
                .map(|(locale, text)| ((*locale).to_string(), (*text).to_string()))
                .collect(),
            sort_order: 0,
            is_default: false,
        }
    }

    #[test]
    fn default_locale_always_exists_and_keys_align() {
        let rows = vec![row("1", "A", &[("en_us", "A-en")]), row("2", "B", &[])];
        let body = build_result_body(&rows, DEFAULT_LOCALE, false, None);
        assert_eq!(body.i18n_resources.len(), 2);
        assert_eq!(body.i18n_resources[0].locale, "zh_cn");
        assert!(body.i18n_resources[0].is_default);
        for resource in &body.i18n_resources {
            assert_eq!(resource.texts.len(), 2);
            assert!(resource.texts.contains_key("@i18n@1"));
            assert!(resource.texts.contains_key("@i18n@2"));
        }
        assert_eq!(body.i18n_resources[1].texts["@i18n@2"], "B");
    }

    #[test]
    fn empty_rows_still_emit_the_default_locale() {
        let body = build_result_body(&[], DEFAULT_LOCALE, false, None);
        assert_eq!(body.i18n_resources.len(), 1);
        assert!(body.i18n_resources[0].texts.is_empty());
    }

    #[test]
    fn linkage_value_is_normalized_by_prefix_and_trim() {
        assert_eq!(normalize_linkage_value(" @i18n@ 成都 "), "成都");
        assert_eq!(normalize_linkage_value("成都"), "成都");
        assert_eq!(normalize_linkage_value(""), "");
    }
}
