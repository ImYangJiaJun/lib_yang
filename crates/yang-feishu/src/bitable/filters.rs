//! 多维表格筛选 DSL 组装。

use serde_json::json;

/// 「回填列为空（且批量范围复选框已勾选）」的筛选体。
///
/// `isEmpty` 的 `value` 必须是空数组（传 null 或不传会吃 1254018）；复选框的
/// `value` 是字符串布尔、operator 仅支持 `is`。
pub fn pending_backfill_filter(
    backfill_field_name: &str,
    batch_scope_field_name: Option<&str>,
) -> serde_json::Value {
    let mut conditions = vec![json!({
        "field_name": backfill_field_name,
        "operator": "isEmpty",
        "value": []
    })];
    if let Some(name) = batch_scope_field_name {
        conditions.push(json!({
            "field_name": name,
            "operator": "is",
            "value": ["true"]
        }));
    }
    json!({
        "filter": {
            "conjunction": "and",
            "conditions": conditions
        },
        "automatic_fields": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_uses_empty_array_for_is_empty() {
        let filter = pending_backfill_filter("回填列", None);
        assert_eq!(filter["filter"]["conditions"][0]["operator"], "isEmpty");
        assert_eq!(filter["filter"]["conditions"][0]["value"], json!([]));
        assert_eq!(filter["automatic_fields"], false);
    }

    #[test]
    fn batch_scope_adds_a_string_boolean_condition() {
        let filter = pending_backfill_filter("回填列", Some("批量范围"));
        let conditions = &filter["filter"]["conditions"];
        assert_eq!(conditions.as_array().map(Vec::len), Some(2));
        assert_eq!(conditions[1]["value"], json!(["true"]));
        assert_eq!(conditions[1]["operator"], "is");
    }
}
