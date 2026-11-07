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

/// 写方向单元格编码失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CellEncodeError {
    /// 该列类型不支持回填。
    #[error("列类型 {0} 不支持回填")]
    UnsupportedType(i32),
    /// 值的形态与列类型不匹配。
    #[error("{reason}")]
    BadValue {
        /// 可读原因（不含字段 id，由调用方补充定位）。
        reason: String,
    },
}

/// 把业务值编码成多维表格写接口接受的单元格值。
///
/// 覆盖真机探针确认的写方向事实：
/// - `3` 单选：**裸字符串**（数组形态被 `1254062 SingleSelectFieldConvFail` 拒）；
/// - `5` 日期：毫秒时间戳（字符串数字先转换）；
/// - `11` 人员：`[{"id":"ou_.."}]` 数组（空数组表示清空；元素缺 `id` 即 `Err`）；
/// - `15` 链接：只写对象；非对象（例如错误文案）返回 `None`，表示跳过不写；
/// - `1` / `None` 文本：`{"text":..}` 取 text，否则原样写。
///
/// `None` 表示「该值不该写」，与 `Err`（类型/形态错误）刻意区分。
pub fn encode_cell_value(
    field_type: Option<i32>,
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>, CellEncodeError> {
    use serde_json::Value;
    let cell = match field_type {
        Some(15) if value.is_object() => value.clone(),
        Some(15) => return Ok(None),
        Some(3) => {
            let text = value.get("text").unwrap_or(value);
            match text.as_str() {
                Some(text) => Value::String(text.to_string()),
                None => {
                    return Err(CellEncodeError::BadValue {
                        reason: "单选字段的值必须是字符串".to_string(),
                    })
                }
            }
        }
        Some(5) => match value {
            Value::Number(_) => value.clone(),
            Value::String(text) => match text.trim().parse::<i64>() {
                Ok(millis) => Value::from(millis),
                Err(_) => {
                    return Err(CellEncodeError::BadValue {
                        reason: format!("日期字段的值不是毫秒时间戳：{text}"),
                    })
                }
            },
            _ => {
                return Err(CellEncodeError::BadValue {
                    reason: "日期字段的值必须是毫秒时间戳".to_string(),
                })
            }
        },
        Some(11) => {
            let users = value.as_array().ok_or_else(|| CellEncodeError::BadValue {
                reason: "人员字段的值必须是 [{\"id\":\"ou_..\"}] 数组".to_string(),
            })?;
            // 空数组合法（用于清空人员列）；非空时每个元素都得带 id，否则会直达
            // batch_update 报平台错误。
            if users.iter().any(|user| {
                !user
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| !id.trim().is_empty())
            }) {
                return Err(CellEncodeError::BadValue {
                    reason: "人员字段的每个元素都必须是 {\"id\":\"ou_..\"} 对象".to_string(),
                });
            }
            value.clone()
        }
        Some(1) | None => value.get("text").unwrap_or(value).clone(),
        Some(other) => return Err(CellEncodeError::UnsupportedType(other)),
    };
    Ok(Some(cell))
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

    #[test]
    fn encode_text_prefers_text_key() {
        assert_eq!(
            encode_cell_value(Some(1), &serde_json::json!({"text": "hello", "type": 2})),
            Ok(Some(serde_json::json!("hello")))
        );
        assert_eq!(
            encode_cell_value(Some(1), &serde_json::json!("plain")),
            Ok(Some(serde_json::json!("plain")))
        );
        assert_eq!(
            encode_cell_value(None, &serde_json::json!("plain")),
            Ok(Some(serde_json::json!("plain")))
        );
    }

    #[test]
    fn encode_single_select_is_a_bare_string() {
        assert_eq!(
            encode_cell_value(Some(3), &serde_json::json!("A")),
            Ok(Some(serde_json::json!("A")))
        );
        assert_eq!(
            encode_cell_value(Some(3), &serde_json::json!({"text": "A"})),
            Ok(Some(serde_json::json!("A")))
        );
        assert!(matches!(
            encode_cell_value(Some(3), &serde_json::json!(["A"])),
            Err(CellEncodeError::BadValue { .. })
        ));
    }

    #[test]
    fn encode_date_accepts_numbers_and_numeric_strings() {
        assert_eq!(
            encode_cell_value(Some(5), &serde_json::json!(1_790_750_400_000i64)),
            Ok(Some(serde_json::json!(1_790_750_400_000i64)))
        );
        assert_eq!(
            encode_cell_value(Some(5), &serde_json::json!("1790750400000")),
            Ok(Some(serde_json::json!(1_790_750_400_000i64)))
        );
        assert!(matches!(
            encode_cell_value(Some(5), &serde_json::json!("not-a-number")),
            Err(CellEncodeError::BadValue { .. })
        ));
        assert!(matches!(
            encode_cell_value(Some(5), &serde_json::json!(true)),
            Err(CellEncodeError::BadValue { .. })
        ));
    }

    #[test]
    fn encode_user_requires_an_array() {
        let people = serde_json::json!([{"id": "ou_1"}]);
        assert_eq!(
            encode_cell_value(Some(11), &people),
            Ok(Some(people.clone()))
        );
        assert_eq!(
            encode_cell_value(Some(11), &serde_json::json!([])),
            Ok(Some(serde_json::json!([])))
        );
        assert!(matches!(
            encode_cell_value(Some(11), &serde_json::json!("ou_1")),
            Err(CellEncodeError::BadValue { .. })
        ));
    }

    #[test]
    fn encode_user_rejects_elements_without_a_non_empty_id() {
        assert!(matches!(
            encode_cell_value(Some(11), &serde_json::json!(["ou_1"])),
            Err(CellEncodeError::BadValue { .. })
        ));
        assert!(matches!(
            encode_cell_value(Some(11), &serde_json::json!([{"text": "张三"}])),
            Err(CellEncodeError::BadValue { .. })
        ));
        assert!(matches!(
            encode_cell_value(Some(11), &serde_json::json!([{"id": " "}])),
            Err(CellEncodeError::BadValue { .. })
        ));
    }

    #[test]
    fn encode_url_skips_non_objects_and_rejects_unknown_types() {
        assert_eq!(
            encode_cell_value(
                Some(15),
                &serde_json::json!({"text": "x", "link": "https://e"})
            ),
            Ok(Some(serde_json::json!({"text": "x", "link": "https://e"})))
        );
        assert_eq!(
            encode_cell_value(Some(15), &serde_json::json!("错误信息")),
            Ok(None)
        );
        assert_eq!(
            encode_cell_value(Some(2), &serde_json::json!(1)),
            Err(CellEncodeError::UnsupportedType(2))
        );
    }
}
