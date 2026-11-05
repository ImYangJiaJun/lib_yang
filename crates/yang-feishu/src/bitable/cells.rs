//! 单元格取值三态。

/// 单元格取值的三态结果。
///
/// 把「合法空值」与「不支持的列类型」分开是刻意的：人员、群组、地理位置是
/// 不含 `text` 键的对象，若都返回空，取数列会静默产出空标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellValue {
    /// 取到了文案；多选列按元素逐个给出。
    Text(Vec<String>),
    /// 单元格确实为空。
    Empty,
    /// 列类型不支持取值。
    Unsupported,
}

/// 从单元格值里取一组用于展示的标签。
pub fn cell_label(value: &serde_json::Value) -> CellValue {
    match value {
        serde_json::Value::String(text) => classify_text(text),
        serde_json::Value::Number(number) => classify_text(&number.to_string()),
        serde_json::Value::Null => CellValue::Empty,
        serde_json::Value::Array(items) => {
            let mut labels = Vec::with_capacity(items.len());
            for item in items {
                let Some(text) = union_text(item) else {
                    return CellValue::Unsupported;
                };
                if !text.trim().is_empty() {
                    labels.push(text);
                }
            }
            if labels.is_empty() {
                CellValue::Empty
            } else {
                CellValue::Text(labels)
            }
        }
        serde_json::Value::Object(_) => match union_text(value) {
            Some(text) => classify_text(&text),
            None => CellValue::Unsupported,
        },
        serde_json::Value::Bool(_) => CellValue::Unsupported,
    }
}

fn union_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Object(map) => map
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn classify_text(text: &str) -> CellValue {
    if text.trim().is_empty() {
        CellValue::Empty
    } else {
        CellValue::Text(vec![text.to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_and_numeric_labels() {
        assert_eq!(
            cell_label(&serde_json::json!("选项")),
            CellValue::Text(vec!["选项".into()])
        );
        assert_eq!(
            cell_label(&serde_json::json!(42)),
            CellValue::Text(vec!["42".into()])
        );
        assert_eq!(cell_label(&serde_json::json!("  ")), CellValue::Empty);
        assert_eq!(cell_label(&serde_json::Value::Null), CellValue::Empty);
    }

    #[test]
    fn single_select_array_and_multi_select_arrays() {
        assert_eq!(
            cell_label(&serde_json::json!(["A"])),
            CellValue::Text(vec!["A".into()])
        );
        assert_eq!(
            cell_label(&serde_json::json!(["A", "B"])),
            CellValue::Text(vec!["A".into(), "B".into()])
        );
        assert_eq!(cell_label(&serde_json::json!([])), CellValue::Empty);
    }

    #[test]
    fn mixed_or_rich_objects_follow_the_documented_rules() {
        assert_eq!(
            cell_label(&serde_json::json!([{"text": "A", "type": 2}])),
            CellValue::Text(vec!["A".into()])
        );
        assert_eq!(
            cell_label(&serde_json::json!([{"text": "A"}, {"id": "ou_1"}])),
            CellValue::Unsupported
        );
        assert_eq!(
            cell_label(&serde_json::json!({"id": "ou_1"})),
            CellValue::Unsupported
        );
        assert_eq!(cell_label(&serde_json::json!(true)), CellValue::Unsupported);
    }
}
