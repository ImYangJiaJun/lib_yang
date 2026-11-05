//! 字段类型判定、字段 id → 当前字段名解析、响应单元格按 field_id 重映射。

use std::collections::BTreeMap;

use crate::error::{Error, Result};

use super::dto::FieldItem;

/// 官方《多维表格记录数据结构》里值形态为**单值**的字段类型。
pub fn is_single_value_field_type(field_type: i32) -> bool {
    matches!(field_type, 1 | 2 | 3 | 5 | 13 | 15 | 20 | 1005)
}

/// 可以作为**选项来源列**的字段类型（比单值版多接受多选 4）。
pub fn is_option_source_field_type(field_type: i32) -> bool {
    is_single_value_field_type(field_type) || field_type == 4
}

/// 校验取数列的字段类型是否可用，返回可读的类型名。
pub fn check_coordinate_field(field: &FieldItem) -> Result<String> {
    let label = field
        .ui_type
        .clone()
        .unwrap_or_else(|| format!("type={:?}", field.field_type));
    let Some(field_type) = field.field_type else {
        // 缺元数据时放行：宁可放行，也不要因元数据缺失拒绝一个本来可用的配置。
        return Ok(label);
    };
    if !is_option_source_field_type(field_type) {
        return Err(Error::fatal(format!(
            "字段「{}」的类型是 {}（type={field_type}），不能作为选项来源：\
             取数列必须指向文本、数字、单选、多选、日期、电话、超链接或公式列。\
             人员/附件/关联/群组/复选框/地理位置不是可用的选项值",
            field.field_name, label
        )));
    }
    Ok(label)
}

/// 把 `field_id` 映射成**精确字段名**。
pub fn resolve_field_name(fields: &[FieldItem], field_id: &str) -> Result<String> {
    let matched = fields
        .iter()
        .find(|field| field.field_id == field_id)
        .map(|field| field.field_name.trim().to_string())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            Error::fatal(format!(
                "多维表格里找不到 field_id {field_id} 对应的字段（或该字段名为空）"
            ))
        })?;

    let duplicates = fields
        .iter()
        .filter(|field| field.field_name.trim() == matched)
        .count();
    if duplicates != 1 {
        return Err(Error::fatal(format!(
            "字段名「{matched}」在表内不唯一（{duplicates} 列同名）：\
             接口按名字匹配会取到不确定的列，请改用唯一列名"
        )));
    }
    Ok(matched)
}

/// 把一组 `field_id` 批量映射成**当前**的精确字段名（保序、点名全部缺失）。
pub fn resolve_field_names(
    fields: &[FieldItem],
    field_ids: &[String],
) -> Result<Vec<(String, String)>> {
    let name_of = |field_id: &str| -> Option<String> {
        fields
            .iter()
            .find(|field| field.field_id == field_id)
            .map(|field| field.field_name.trim().to_string())
            .filter(|name| !name.is_empty())
    };

    let missing: Vec<&str> = field_ids
        .iter()
        .map(String::as_str)
        .filter(|field_id| name_of(field_id).is_none())
        .collect();
    if !missing.is_empty() {
        return Err(Error::fatal(format!(
            "多维表格里找不到这些字段（可能已被删除，或该字段名为空）：{}",
            missing.join("、")
        )));
    }

    let resolved: Vec<(String, String)> = field_ids
        .iter()
        .filter_map(|field_id| name_of(field_id).map(|name| (field_id.clone(), name)))
        .collect();

    for (_, name) in &resolved {
        let in_table = fields
            .iter()
            .filter(|field| field.field_name.trim() == name)
            .count();
        if in_table != 1 {
            return Err(Error::fatal(format!(
                "字段名「{name}」在表内不唯一（{in_table} 列同名）：\
                 接口按名字匹配会取到不确定的列，请改用唯一列名"
            )));
        }
    }
    Ok(resolved)
}

/// `automatic_fields: true` 时可能随记录返回的系统字段键（都没有 field_id）。
pub const SYSTEM_FIELD_KEYS: &[&str] = &[
    "created_by",
    "created_time",
    "last_modified_by",
    "last_modified_time",
];

/// 把官方响应的 `fields`（按字段名作键）重映射成按 `field_id` 作键。
///
/// 未知键一律点名报错，不静默保留；唯一例外是 [`SYSTEM_FIELD_KEYS`]。
pub fn rekey_cells_by_field_id(
    fields: &[FieldItem],
    cells: &BTreeMap<String, serde_json::Value>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut out = serde_json::Map::new();
    let mut unknown: Vec<&str> = Vec::new();
    for (key, value) in cells {
        if SYSTEM_FIELD_KEYS.contains(&key.as_str()) {
            continue;
        }
        if fields.iter().any(|field| field.field_id == key.as_str()) {
            out.insert(key.clone(), value.clone());
            continue;
        }
        match fields
            .iter()
            .find(|field| field.field_name.trim() == key.trim())
        {
            Some(field) => {
                out.insert(field.field_id.clone(), value.clone());
            }
            None => unknown.push(key),
        }
    }

    if !unknown.is_empty() {
        return Err(Error::fatal(format!(
            "响应里的这些键既不是字段 id 也不是已知列名：{}（它们多半是未投影的系统字段，或该列已从表里删除）",
            unknown.join("、")
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(id: &str, name: &str, field_type: i32) -> FieldItem {
        FieldItem {
            is_primary: false,
            field_id: id.to_string(),
            field_name: name.to_string(),
            field_type: Some(field_type),
            ui_type: None,
            property: None,
        }
    }

    #[test]
    fn resolves_by_stable_id_and_keeps_order() {
        let fields = vec![field("f1", "旧名", 1), field("f2", "B", 2)];
        let resolved = resolve_field_names(&fields, &["f2".into(), "f1".into()])
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            resolved,
            vec![("f2".into(), "B".into()), ("f1".into(), "旧名".into())]
        );
    }

    #[test]
    fn missing_fields_are_all_named() {
        let fields = vec![field("f1", "A", 1)];
        let error = resolve_field_names(&fields, &["f1".into(), "f2".into()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("f2"));
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let fields = vec![field("f1", "同名", 1), field("f2", "同名", 2)];
        assert!(resolve_field_name(&fields, "f1").is_err());
    }

    #[test]
    fn field_type_predicates_match_the_host_sets() {
        for value in [1, 2, 3, 5, 13, 15, 20, 1005] {
            assert!(is_single_value_field_type(value));
        }
        assert!(is_option_source_field_type(4));
        for value in [7, 11, 17, 18, 21, 22, 23] {
            assert!(!is_option_source_field_type(value));
        }
    }

    #[test]
    fn rekey_maps_names_and_skips_system_fields() {
        let fields = vec![field("f1", "申请人", 11)];
        let mut cells = BTreeMap::new();
        cells.insert("申请人".to_string(), serde_json::json!("ou_1"));
        cells.insert("created_by".to_string(), serde_json::json!({}));
        let out =
            rekey_cells_by_field_id(&fields, &cells).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(out.get("f1"), Some(&serde_json::json!("ou_1")));
        assert!(!out.contains_key("created_by"));
    }

    #[test]
    fn rekey_fails_loudly_for_unknown_keys() {
        let fields = vec![field("f1", "A", 1)];
        let mut cells = BTreeMap::new();
        cells.insert("B".to_string(), serde_json::json!(1));
        let error = rekey_cells_by_field_id(&fields, &cells)
            .unwrap_err()
            .to_string();
        assert!(error.contains("B"));
    }
}
