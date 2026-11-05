//! 审批表单值编码：把多维表格单元格组装成 `instance/create` 的 `form` 字符串。
//!
//! 产物是**压缩后的 JSON 数组字符串**（不是对象）。所有定位信息（控件 id / 字段 id）
//! 都保留在错误里，因为失败原因会写回多维表格给业务人员看。

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::form::{FormWidget, WidgetOptions};
use super::widget_rules::{self, is_container};

/// 一个审批控件在库里的映射快照。
#[derive(Debug, Clone)]
pub struct WidgetMap {
    /// 审批控件 id。
    pub widget_id: String,
    /// 审批控件类型。
    pub widget_type: String,
    /// 是否必填（快照）。
    pub required: bool,
    /// 多维表格字段 id（取值来源）。
    pub source_field: String,
    /// 日期区间结束列的字段 id。
    pub interval_end_field: Option<String>,
    /// 值转换器。
    pub converter: Converter,
    /// 选项映射：多维表格文案 → 审批控件 option value。
    pub option_map: BTreeMap<String, String>,
    /// 外部选项来源 key。
    pub option_source_key: Option<String>,
    /// 金额控件币种。
    pub currency: Option<String>,
}

/// 金额控件的币种：审批定义里该控件 `option.currencyRange` 的首值。
pub fn amount_currency(form_snapshot: &str, widget_id: &str) -> Option<String> {
    fn find<'a>(nodes: &'a [FormWidget], id: &str) -> Option<&'a FormWidget> {
        for node in nodes {
            if node.id == id {
                return Some(node);
            }
            if let Some(found) = find(&node.children, id) {
                return Some(found);
            }
        }
        None
    }
    let form: Vec<FormWidget> = serde_json::from_str(form_snapshot).ok()?;
    let node = find(&form, widget_id)?;
    if node.r#type != "amount" {
        return None;
    }
    let WidgetOptions::Config(option) = node.option.as_ref()? else {
        return None;
    };
    let currency = option
        .get("currencyRange")?
        .as_array()?
        .first()?
        .as_str()?
        .trim();
    (!currency.is_empty()).then(|| currency.to_string())
}

/// 值转换器；持久化为 `direct` / `date` / `option` 三个字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Converter {
    /// 文本/数字类直取。
    Direct,
    /// 多维表格毫秒时间戳 → 带偏移量的 RFC3339。
    Date,
    /// 多维表格选项文案 → 审批控件 option value。
    Option,
}

impl Converter {
    /// 从库里的字符串恢复；未知值 fail-closed。
    pub fn parse(value: &str) -> Result<Self, ConvertError> {
        match value {
            "direct" => Ok(Self::Direct),
            "date" => Ok(Self::Date),
            "option" => Ok(Self::Option),
            other => Err(ConvertError::UnknownConverter(other.to_string())),
        }
    }

    /// 持久化字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Date => "date",
            Self::Option => "option",
        }
    }
}

/// 组装失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// 定义快照与映射不一致。
    InvalidDefinition(String),
    /// 库里存了未知的转换器标识。
    UnknownConverter(String),
    /// 必填控件取不到值。
    MissingRequired {
        /// 控件 id。
        widget_id: String,
    },
    /// 值存在但形态不对。
    BadValue {
        /// 控件 id。
        widget_id: String,
        /// 可读原因。
        reason: String,
    },
    /// 控件类型无法通过 API 提单。
    Unsupported {
        /// 控件 id。
        widget_id: String,
        /// 控件类型。
        widget_type: String,
    },
    /// 选项文案查不到映射。
    UnmappedOption {
        /// 控件 id。
        widget_id: String,
        /// 未映射的文案。
        label: String,
    },
    /// 转换器与控件类型不匹配。
    ConverterMismatch {
        /// 控件 id。
        widget_id: String,
        /// 控件类型。
        widget_type: String,
        /// 当前配置的转换器。
        converter: &'static str,
    },
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefinition(reason) => write!(formatter, "审批定义或映射不可用：{reason}"),
            Self::UnknownConverter(value) => write!(formatter, "未知的值转换器「{value}」"),
            Self::MissingRequired { widget_id } => {
                write!(formatter, "缺少必填控件「{widget_id}」的值")
            }
            Self::BadValue { widget_id, reason } => {
                write!(formatter, "控件「{widget_id}」的值不可用：{reason}")
            }
            Self::Unsupported {
                widget_id,
                widget_type,
            } => write!(
                formatter,
                "控件「{widget_id}」（{widget_type}）不支持通过 API 提单，需人工准备值"
            ),
            Self::UnmappedOption { widget_id, label } => write!(
                formatter,
                "控件「{widget_id}」的选项「{label}」没有配置映射"
            ),
            Self::ConverterMismatch {
                widget_id,
                widget_type,
                converter,
            } => write!(
                formatter,
                "控件「{widget_id}」（{widget_type}）与转换器「{converter}」不匹配：请把该映射的转换器改为对应的类型"
            ),
        }
    }
}

impl std::error::Error for ConvertError {}

/// 组装 `form` 所需的全部输入。
#[derive(Debug, Clone, Copy)]
pub struct FormInput<'a> {
    /// 控件映射。
    pub widgets: &'a [WidgetMap],
    /// 审批定义原始表单快照。
    pub form_snapshot: &'a str,
    /// 多维表格单元格值，键是字段 id。
    pub cells: &'a Map<String, Value>,
    /// 审批上传接口返回的 file code（widget_id → codes）。
    pub attachment_codes: &'a BTreeMap<String, Vec<String>>,
    /// 多维表格时区偏移。
    pub timezone_offset: FixedOffset,
    /// 关联审批允许的审批应用 appId；`None` 表示调用方自行把 AppLink 预解析成
    /// 实例 Code（crate 不硬编码产品 appId）。
    pub approval_app_id: Option<&'a str>,
}

/// 聚合表单里的一行。
#[derive(Debug, Clone, Copy)]
pub struct GroupFormRow<'a> {
    /// 记录 id（只用于错误定位）。
    pub record_id: &'a str,
    /// 该记录的单元格。
    pub cells: &'a Map<String, Value>,
    /// 该记录的附件 code。
    pub attachment_codes: &'a BTreeMap<String, Vec<String>>,
}

/// 聚合表单的组装输入。
#[derive(Debug, Clone, Copy)]
pub struct GroupFormInput<'a> {
    /// 控件映射。
    pub widgets: &'a [WidgetMap],
    /// 审批定义原始表单快照。
    pub form_snapshot: &'a str,
    /// 成员行。
    pub rows: &'a [GroupFormRow<'a>],
    /// 多维表格时区偏移。
    pub timezone_offset: FixedOffset,
    /// 关联审批允许的审批应用 appId；语义同 [`FormInput::approval_app_id`]。
    pub approval_app_id: Option<&'a str>,
}

/// 固定的 UTC 偏移（秒）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedOffset {
    seconds: i32,
}

impl FixedOffset {
    /// 由秒数构造，范围 ±18 小时。
    pub fn from_seconds(seconds: i32) -> Result<Self, ConvertError> {
        const LIMIT: i32 = 18 * 3600;
        if seconds.abs() > LIMIT {
            return Err(ConvertError::BadValue {
                widget_id: String::new(),
                reason: format!("时区偏移 {seconds} 秒超出 ±18 小时"),
            });
        }
        Ok(Self { seconds })
    }

    /// 按 IANA 名构造（只认少数常用名）。
    pub fn from_iana(name: &str) -> Result<Self, ConvertError> {
        match name {
            "Asia/Shanghai" | "Asia/Chongqing" | "Asia/Harbin" | "PRC" => {
                Self::from_seconds(8 * 3600)
            }
            "UTC" | "Etc/UTC" => Self::from_seconds(0),
            "Asia/Tokyo" => Self::from_seconds(9 * 3600),
            "Asia/Singapore" | "Asia/Hong_Kong" => Self::from_seconds(8 * 3600),
            other => Err(ConvertError::BadValue {
                widget_id: String::new(),
                reason: format!("不支持的 Base 时区「{other}」"),
            }),
        }
    }

    /// 偏移秒数。
    pub fn seconds(self) -> i32 {
        self.seconds
    }

    fn render(self) -> String {
        let sign = if self.seconds < 0 { '-' } else { '+' };
        let total = self.seconds.unsigned_abs();
        format!("{sign}{:02}:{:02}", total / 3600, (total % 3600) / 60)
    }
}

/// 解析审批定义快照并校验映射表。
fn prepare_definition<'a>(
    form_snapshot: &str,
    widgets: &'a [WidgetMap],
) -> Result<(Vec<FormWidget>, BTreeMap<&'a str, &'a WidgetMap>), ConvertError> {
    let definition: Vec<FormWidget> = serde_json::from_str(form_snapshot)
        .map_err(|_| ConvertError::InvalidDefinition("表单快照不是有效的控件数组".to_string()))?;
    if definition.is_empty() {
        return Err(ConvertError::InvalidDefinition("表单快照为空".to_string()));
    }
    let mut nodes = BTreeMap::new();
    for node in definition.iter().flat_map(FormWidget::walk) {
        if node.id.is_empty()
            || node.r#type.is_empty()
            || (!is_container(&node.r#type) && !node.children.is_empty())
            || nodes.insert(&node.id, node).is_some()
        {
            return Err(ConvertError::InvalidDefinition(format!(
                "控件「{}」结构非法、缺少 id/type 或 id 重复",
                node.id
            )));
        }
    }
    let mut mappings = BTreeMap::new();
    for mapping in widgets {
        let node = nodes.get(&mapping.widget_id).ok_or_else(|| {
            ConvertError::InvalidDefinition(format!(
                "映射控件「{}」已不在定义中",
                mapping.widget_id
            ))
        })?;
        if widget_rules::is_control_group(&node.r#type) {
            continue;
        }
        if node.r#type != mapping.widget_type
            || node.r#type == "fieldList"
            || mappings
                .insert(mapping.widget_id.as_str(), mapping)
                .is_some()
        {
            return Err(ConvertError::InvalidDefinition(format!(
                "控件「{}」类型不匹配或映射重复",
                mapping.widget_id
            )));
        }
    }
    Ok((definition, mappings))
}

/// 把一条多维表格记录组装成 `form` 字符串。
pub fn build_form(input: &FormInput<'_>) -> Result<String, ConvertError> {
    let row = GroupFormRow {
        record_id: "",
        cells: input.cells,
        attachment_codes: input.attachment_codes,
    };
    assemble(
        input.widgets,
        input.form_snapshot,
        std::slice::from_ref(&row),
        input.timezone_offset,
        input.approval_app_id,
        false,
    )
}

/// 把一组同签名的记录组装成多行明细的 `form` 字符串。
pub fn build_group_form(input: &GroupFormInput<'_>) -> Result<String, ConvertError> {
    assemble(
        input.widgets,
        input.form_snapshot,
        input.rows,
        input.timezone_offset,
        input.approval_app_id,
        true,
    )
}

fn assemble(
    widgets: &[WidgetMap],
    form_snapshot: &str,
    rows: &[GroupFormRow<'_>],
    timezone_offset: FixedOffset,
    approval_app_id: Option<&str>,
    enforce_member_rows: bool,
) -> Result<String, ConvertError> {
    let (definition, mappings) = prepare_definition(form_snapshot, widgets)?;
    if rows.is_empty() {
        return Err(ConvertError::BadValue {
            widget_id: String::new(),
            reason: "表单组装至少需要一行记录".to_string(),
        });
    }
    let values = assemble_definition(
        &definition,
        &mappings,
        rows,
        timezone_offset,
        approval_app_id,
        false,
        enforce_member_rows,
    )?;
    serde_json::to_string(&Value::Array(values)).map_err(|error| ConvertError::BadValue {
        widget_id: String::new(),
        reason: format!("form 序列化失败：{error}"),
    })
}

/// 一致性签名：把提交时会产生差异的部分规范化成确定性 JSON 字符串。
pub fn build_signature(input: &FormInput<'_>) -> Result<String, ConvertError> {
    let (definition, mappings) = prepare_definition(input.form_snapshot, input.widgets)?;
    let mut values = Map::new();
    collect_signature(&definition, &mappings, input, &mut values)?;
    serde_json::to_string(&Value::Object(values)).map_err(|error| ConvertError::BadValue {
        widget_id: String::new(),
        reason: format!("签名序列化失败：{error}"),
    })
}

fn collect_signature(
    nodes: &[FormWidget],
    mappings: &BTreeMap<&str, &WidgetMap>,
    input: &FormInput<'_>,
    values: &mut Map<String, Value>,
) -> Result<(), ConvertError> {
    for node in nodes {
        if node.r#type == "fieldList" {
            continue;
        }
        if is_container(&node.r#type) {
            collect_signature(&node.children, mappings, input, values)?;
            continue;
        }
        let Some(mapping) = mappings.get(node.id.as_str()) else {
            continue;
        };
        let mut mapping = (*mapping).clone();
        mapping.required |= node.required;
        let value = match assemble_widget(
            &mapping,
            input.cells,
            input.attachment_codes,
            input.timezone_offset,
            input.approval_app_id,
        ) {
            Ok(Some(value)) => value,
            Ok(None) | Err(ConvertError::MissingRequired { .. }) => Value::Null,
            Err(error) => return Err(error),
        };
        let value = canonical_signature_value(value);
        if values.insert(node.id.clone(), value).is_some() {
            return Err(ConvertError::InvalidDefinition(format!(
                "控件「{}」在签名里重复",
                node.id
            )));
        }
    }
    Ok(())
}

fn canonical_signature_value(value: Value) -> Value {
    match value {
        Value::Number(number) => Value::String(canonical_number(&number)),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(canonical_signature_value).collect())
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, canonical_signature_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn canonical_number(number: &serde_json::Number) -> String {
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    match number.as_f64() {
        Some(value) => value.to_string(),
        None => number.to_string(),
    }
}

fn assemble_definition(
    definition: &[FormWidget],
    mappings: &BTreeMap<&str, &WidgetMap>,
    rows: &[GroupFormRow<'_>],
    timezone_offset: FixedOffset,
    approval_app_id: Option<&str>,
    inside_detail_row: bool,
    enforce_member_rows: bool,
) -> Result<Vec<Value>, ConvertError> {
    let primary = rows.first().ok_or_else(|| ConvertError::BadValue {
        widget_id: String::new(),
        reason: "表单组装至少需要一行记录".to_string(),
    })?;
    let mut values = Vec::new();
    for node in definition {
        if node.r#type == "fieldList" {
            if !node
                .walk()
                .iter()
                .any(|child| mappings.contains_key(child.id.as_str()))
            {
                continue;
            }
            if !inside_detail_row && enforce_member_rows {
                let mut detail_rows = Vec::with_capacity(rows.len());
                for row in rows {
                    let has_value = node.walk().iter().any(|child| {
                        mappings
                            .get(child.id.as_str())
                            .and_then(|mapping| row.cells.get(&mapping.source_field))
                            .is_some_and(|cell| !is_empty(cell))
                    });
                    if !has_value {
                        return Err(ConvertError::BadValue {
                            widget_id: node.id.clone(),
                            reason: format!("记录 {} 的明细为空", row.record_id),
                        });
                    }
                    detail_rows.push(assemble_definition(
                        &node.children,
                        mappings,
                        std::slice::from_ref(row),
                        timezone_offset,
                        approval_app_id,
                        true,
                        false,
                    )?);
                }
                values.push(json!({"id": node.id, "type": "fieldList", "value": detail_rows}));
                continue;
            }
            let has_value = node.walk().iter().any(|child| {
                mappings
                    .get(child.id.as_str())
                    .and_then(|mapping| primary.cells.get(&mapping.source_field))
                    .is_some_and(|cell| !is_empty(cell))
            });
            if !has_value {
                if node.required {
                    return Err(ConvertError::MissingRequired {
                        widget_id: node.id.clone(),
                    });
                }
                continue;
            }
            let row = assemble_definition(
                &node.children,
                mappings,
                std::slice::from_ref(primary),
                timezone_offset,
                approval_app_id,
                true,
                false,
            )?;
            values.push(json!({"id": node.id, "type": "fieldList", "value": [row]}));
        } else if is_container(&node.r#type) {
            if !node
                .walk()
                .iter()
                .any(|child| mappings.contains_key(child.id.as_str()))
            {
                continue;
            }
            let children = assemble_definition(
                &node.children,
                mappings,
                rows,
                timezone_offset,
                approval_app_id,
                inside_detail_row,
                false,
            )?;
            if children.is_empty() {
                if node.required {
                    return Err(ConvertError::MissingRequired {
                        widget_id: node.id.clone(),
                    });
                }
                continue;
            }
            values.push(json!({"id": node.id, "type": node.r#type, "value": children}));
        } else if let Some(mapping) = mappings.get(node.id.as_str()) {
            let mut mapping = (*mapping).clone();
            mapping.required |= node.required;
            if let Some(value) = assemble_widget(
                &mapping,
                primary.cells,
                primary.attachment_codes,
                timezone_offset,
                approval_app_id,
            )? {
                if let Err(reason) = node.connect_definition_codes() {
                    return Err(ConvertError::InvalidDefinition(format!(
                        "关联审批控件「{}」配置无效：{reason}，请刷新配置",
                        node.id,
                    )));
                }
                values.push(value);
            }
        } else if node.required && widget_rules::is_mappable(&node.r#type) {
            return Err(ConvertError::MissingRequired {
                widget_id: node.id.clone(),
            });
        }
    }
    Ok(values)
}

fn assemble_widget(
    widget: &WidgetMap,
    cells: &Map<String, Value>,
    attachment_codes: &BTreeMap<String, Vec<String>>,
    timezone_offset: FixedOffset,
    approval_app_id: Option<&str>,
) -> Result<Option<Value>, ConvertError> {
    let cell = cells.get(&widget.source_field);

    if widget_rules::is_unsupported(&widget.widget_type) {
        return Err(ConvertError::Unsupported {
            widget_id: widget.widget_id.clone(),
            widget_type: widget.widget_type.clone(),
        });
    }

    let Some(cell) = cell.filter(|value| !is_empty(value)) else {
        return if widget.required {
            Err(ConvertError::MissingRequired {
                widget_id: widget.widget_id.clone(),
            })
        } else {
            Ok(None)
        };
    };

    let interval_end_cell = if widget.widget_type == "dateInterval" {
        match widget.interval_end_field.as_deref() {
            None => None,
            Some(field) => match cells.get(field).filter(|value| !is_empty(value)) {
                Some(value) => Some(value),
                None => {
                    return if widget.required {
                        Err(ConvertError::MissingRequired {
                            widget_id: widget.widget_id.clone(),
                        })
                    } else {
                        Ok(None)
                    };
                }
            },
        }
    } else {
        None
    };

    check_converter_matches(widget)?;

    let value = if widget_rules::is_media(&widget.widget_type) {
        let label = if widget_rules::upload_type(&widget.widget_type) == "image" {
            "图片"
        } else {
            "附件"
        };
        let codes = attachment_codes
            .get(&widget.widget_id)
            .filter(|codes| !codes.is_empty() && codes.iter().all(|code| !code.trim().is_empty()))
            .ok_or_else(|| ConvertError::BadValue {
                widget_id: widget.widget_id.clone(),
                reason: format!("{label}尚未搬运到审批系统"),
            })?;
        json!(codes)
    } else {
        match widget.converter {
            Converter::Direct => assemble_direct(widget, cell, approval_app_id)?,
            Converter::Date => assemble_date(widget, cell, interval_end_cell, timezone_offset)?,
            Converter::Option => assemble_option(widget, cell)?,
        }
    };

    let mut object = Map::new();
    object.insert("id".to_string(), Value::String(widget.widget_id.clone()));
    object.insert(
        "type".to_string(),
        Value::String(widget.widget_type.clone()),
    );
    if widget.widget_type == "contact" {
        object.insert("open_ids".to_string(), value);
        object.insert("value".to_string(), json!([]));
    } else {
        object.insert("value".to_string(), value);
    }
    if let Some(currency) = &widget.currency {
        object.insert("currency".to_string(), Value::String(currency.clone()));
    }
    Ok(Some(Value::Object(object)))
}

fn check_converter_matches(widget: &WidgetMap) -> Result<(), ConvertError> {
    if widget.converter == Converter::Direct
        && widget.widget_type == "radioV2"
        && widget_rules::is_raw_option_child(&widget.widget_id)
        && widget.option_source_key.is_none()
        && widget.option_map.is_empty()
    {
        return Ok(());
    }
    let expected = match widget.widget_type.as_str() {
        "date" | "dateInterval" => Converter::Date,
        "radio" | "radioV2" | "checkbox" | "checkboxV2" => Converter::Option,
        _ => Converter::Direct,
    };
    if widget.converter == expected {
        return Ok(());
    }
    Err(ConvertError::ConverterMismatch {
        widget_id: widget.widget_id.clone(),
        widget_type: widget.widget_type.clone(),
        converter: widget.converter.as_str(),
    })
}

fn assemble_direct(
    widget: &WidgetMap,
    cell: &Value,
    approval_app_id: Option<&str>,
) -> Result<Value, ConvertError> {
    match widget.widget_type.as_str() {
        "number" | "amount" | "formula" => number_from_cell(widget, cell),
        "telephone" => assemble_telephone(widget, cell),
        "connect" => connection_codes(cell, approval_app_id)
            .map(|codes| json!(codes))
            .map_err(|reason| ConvertError::BadValue {
                widget_id: widget.widget_id.clone(),
                reason,
            }),
        "contact" => Ok(json!(open_ids_from_cell(widget, cell)?)),
        "department" => Ok(Value::Array(
            open_ids_from_cell(widget, cell)?
                .into_iter()
                .map(|id| json!({ "open_id": id }))
                .collect(),
        )),
        _ => Ok(Value::String(text_from_cell(cell))),
    }
}

/// 关联审批：AppLink / 实例 Code → 实例 Code 数组。
///
/// `approval_app_id` 由调用方传入（设计 §0.3 #9 不把产品 appId 硬编码进 crate）。
/// 传 `None` 时只接受 36 位实例 Code，AppLink 由调用方预解析。
fn connection_codes(cell: &Value, approval_app_id: Option<&str>) -> Result<Vec<String>, String> {
    fn query_pairs(query: &str) -> Result<Vec<(String, String)>, String> {
        fn decode(raw: &str) -> Result<String, String> {
            let mut decoded = Vec::new();
            let mut bytes = raw.bytes();
            while let Some(byte) = bytes.next() {
                decoded.push(match byte {
                    b'%' => {
                        let high = bytes.next().and_then(|b| (b as char).to_digit(16));
                        let low = bytes.next().and_then(|b| (b as char).to_digit(16));
                        match (high, low) {
                            (Some(high), Some(low)) => (high * 16 + low) as u8,
                            _ => return Err("审批链接的百分号编码非法".into()),
                        }
                    }
                    b'+' => b' ',
                    other => other,
                });
            }
            String::from_utf8(decoded).map_err(|_| "审批链接编码非法".into())
        }
        query
            .split('&')
            .map(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                Ok((decode(key)?, decode(value)?))
            })
            .collect()
    }
    fn code(raw: &str, approval_app_id: Option<&str>) -> Result<String, String> {
        let raw = raw.trim();
        if raw.len() == 36 && uuid::Uuid::parse_str(raw).is_ok() {
            return Ok(raw.to_ascii_uppercase());
        }
        let Some(approval_app_id) = approval_app_id else {
            return Err(
                "关联审批的 AppLink 需由调用方提供审批应用 appId 或预解析为实例 Code".to_string(),
            );
        };
        let query = raw
            .strip_prefix("https://applink.feishu.cn/client/mini_program/open?")
            .ok_or_else(|| "请填写飞书审批 AppLink 或实例 Code，不能仅填申请编号".to_string())?;
        let params = query_pairs(query.split('#').next().unwrap_or_default())?;
        let apps: Vec<_> = params.iter().filter(|(key, _)| key == "appId").collect();
        if apps.len() != 1 || apps[0].1 != approval_app_id {
            return Err("链接不是飞书审批应用".to_string());
        }
        let mut codes = Vec::new();
        for (_, path) in params
            .iter()
            .filter(|(key, _)| key == "path" || key == "path_pc")
        {
            let (page, query) = path
                .split_once('?')
                .ok_or_else(|| "审批链接路径缺少实例 Code".to_string())?;
            if !matches!(page, "pages/detail/index" | "pc/pages/in-process/index") {
                return Err("审批链接路径非法".to_string());
            }
            let ids: Vec<_> = query_pairs(query)?
                .into_iter()
                .filter(|(key, _)| key == "instanceId")
                .collect();
            if ids.len() != 1 || ids[0].1.len() != 36 || uuid::Uuid::parse_str(&ids[0].1).is_err() {
                return Err("审批链接缺少有效的实例 Code".to_string());
            }
            codes.push(ids[0].1.to_ascii_uppercase());
        }
        if codes.is_empty() || codes.iter().any(|id| id != &codes[0]) {
            return Err("审批链接的实例 Code 缺失或不一致".to_string());
        }
        Ok(codes.remove(0))
    }
    fn collect(
        cell: &Value,
        codes: &mut Vec<String>,
        approval_app_id: Option<&str>,
    ) -> Result<(), String> {
        match cell {
            Value::Array(items) => {
                for item in items {
                    collect(item, codes, approval_app_id)?;
                }
            }
            Value::Object(map) => {
                let raw = map
                    .get("link")
                    .or_else(|| map.get("text"))
                    .ok_or_else(|| "关联审批单元格需要 link 或 text".to_string())?;
                collect(raw, codes, approval_app_id)?;
            }
            Value::String(text) => {
                // 完整 URL 中的逗号不当分隔符；文本 Code 支持逗号或换行分隔。
                let values: Vec<_> = if text.trim().starts_with("https://") {
                    vec![text.as_str()]
                } else {
                    text.split([',', '，', ';', '；', '\n']).collect()
                };
                for raw in values.into_iter().filter(|raw| !raw.trim().is_empty()) {
                    let id = code(raw, approval_app_id)?;
                    if !codes.contains(&id) {
                        codes.push(id);
                    }
                }
            }
            _ => return Err("关联审批需要超链接或实例 Code 文本".to_string()),
        }
        Ok(())
    }
    let mut codes = Vec::new();
    collect(cell, &mut codes, approval_app_id)?;
    if codes.is_empty() {
        return Err("关联审批没有实例 Code".to_string());
    }
    Ok(codes)
}

fn assemble_date(
    widget: &WidgetMap,
    cell: &Value,
    interval_end_cell: Option<&Value>,
    offset: FixedOffset,
) -> Result<Value, ConvertError> {
    if widget.widget_type == "dateInterval" {
        if let Some(end_cell) = interval_end_cell {
            let start = cell.as_i64();
            let end = end_cell.as_i64();
            let (Some(start), Some(end)) = (start, end) else {
                return Err(ConvertError::BadValue {
                    widget_id: widget.widget_id.clone(),
                    reason: format!(
                        "日期区间的开始/结束列都应是毫秒时间戳，实际是 {cell} / {end_cell}"
                    ),
                });
            };
            return Ok(date_interval_value(start, end, offset));
        }
        let start = cell.get("start").and_then(Value::as_i64);
        let end = cell.get("end").and_then(Value::as_i64);
        let (Some(start), Some(end)) = (start, end) else {
            return Err(ConvertError::BadValue {
                widget_id: widget.widget_id.clone(),
                reason: format!("日期区间字段期望 {{start, end}} 两个毫秒时间戳，实际是 {cell}"),
            });
        };
        return Ok(date_interval_value(start, end, offset));
    }

    let millis = cell.as_i64().ok_or_else(|| ConvertError::BadValue {
        widget_id: widget.widget_id.clone(),
        reason: format!("日期字段期望毫秒时间戳，实际是 {cell}"),
    })?;
    Ok(Value::String(format_rfc3339(millis, offset)))
}

fn date_interval_value(start: i64, end: i64, offset: FixedOffset) -> Value {
    let days = (end - start) as f64 / 86_400_000.0;
    json!({
        "start": format_rfc3339(start, offset),
        "end": format_rfc3339(end, offset),
        "interval": days,
    })
}

fn assemble_option(widget: &WidgetMap, cell: &Value) -> Result<Value, ConvertError> {
    let labels = labels_from_cell(cell);
    let mut values = Vec::with_capacity(labels.len());
    for label in labels {
        let mapped = widget
            .option_map
            .get(&label)
            .ok_or_else(|| ConvertError::UnmappedOption {
                widget_id: widget.widget_id.clone(),
                label: label.clone(),
            })?;
        values.push(Value::String(mapped.clone()));
    }

    if matches!(widget.widget_type.as_str(), "radio" | "radioV2") {
        match values.len() {
            0 => Ok(Value::Null),
            1 => Ok(values.remove(0)),
            _ => Err(ConvertError::BadValue {
                widget_id: widget.widget_id.clone(),
                reason: format!("单选控件只能有一个选项，实际 {} 个", values.len()),
            }),
        }
    } else {
        Ok(Value::Array(values))
    }
}

fn assemble_telephone(widget: &WidgetMap, cell: &Value) -> Result<Value, ConvertError> {
    let raw = text_from_cell(cell);
    let trimmed = raw.trim();
    let (country, national) = if let Some(rest) = trimmed.strip_prefix('+') {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || digits.len() > 3 {
            return Err(ConvertError::BadValue {
                widget_id: widget.widget_id.clone(),
                reason: format!("电话「{trimmed}」的区号无法识别"),
            });
        }
        let national: String = rest[digits.len()..]
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        (format!("+{digits}"), national)
    } else {
        let national: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        ("+86".to_string(), national)
    };
    if national.is_empty() {
        return Err(ConvertError::BadValue {
            widget_id: widget.widget_id.clone(),
            reason: format!("电话「{trimmed}」没有号码部分"),
        });
    }
    Ok(json!({ "countryCode": country, "nationalNumber": national }))
}

fn number_from_cell(widget: &WidgetMap, cell: &Value) -> Result<Value, ConvertError> {
    let value = cell.get("value").unwrap_or(cell);
    let scalar = match value {
        Value::Array(items) if items.len() == 1 => Some(&items[0]),
        Value::Array(_) => None,
        other => Some(other),
    };
    if let Some(scalar) = scalar {
        if scalar.is_number() {
            return Ok(scalar.clone());
        }
        if let Some(text) = scalar.as_str() {
            if let Ok(parsed) = text.trim().parse::<f64>() {
                return Ok(json!(parsed));
            }
        }
    }
    Err(ConvertError::BadValue {
        widget_id: widget.widget_id.clone(),
        reason: format!("数字字段期望数值，实际是 {cell}"),
    })
}

fn open_ids_from_cell(widget: &WidgetMap, cell: &Value) -> Result<Vec<String>, ConvertError> {
    match cell {
        Value::Array(items) => {
            let mut ids = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Object(map) => {
                        if let Some(id) = map.get("id").and_then(Value::as_str) {
                            ids.push(id.to_string());
                        }
                    }
                    Value::String(id) => ids.push(id.clone()),
                    _ => {}
                }
            }
            Ok(ids)
        }
        Value::String(text) => Ok(text
            .split([',', '，'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()),
        _ => Err(ConvertError::BadValue {
            widget_id: widget.widget_id.clone(),
            reason: format!("人员字段期望人员数组，实际是 {cell}"),
        }),
    }
}

/// 剥掉 v3 批量读取对公式/查找字段的包装 `{"type":2,"value":[...]}`。
pub fn unwrap_typed_cell(cell: &Value) -> &Value {
    match cell {
        Value::Object(map) => match (map.get("type"), map.get("value")) {
            (Some(_), Some(inner)) => unwrap_typed_cell(inner),
            _ => cell,
        },
        _ => cell,
    }
}

/// 从单元格里取出选项文案列表（数字也算文案）。
pub fn labels_from_cell(cell: &Value) -> Vec<String> {
    let labels = match unwrap_typed_cell(cell) {
        Value::Array(items) => items.iter().flat_map(labels_from_cell).collect(),
        Value::Object(map) => ["text", "name", "value"]
            .iter()
            .find_map(|key| map.get(*key))
            .map(labels_from_cell)
            .unwrap_or_default(),
        Value::String(text) => vec![text.clone()],
        Value::Number(number) => vec![number.to_string()],
        _ => Vec::new(),
    };
    labels
        .into_iter()
        .map(|label| label.trim().to_string())
        .collect()
}

/// 十进制文案归一：`9.1330` → `9.133`，`1.0000` → `1`。
pub fn normalize_decimal_label(label: &str) -> Option<String> {
    let text = label.trim();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (integer, fraction) = match rest.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (rest, None),
    };
    if integer.chars().any(|c| !c.is_ascii_digit())
        || fraction.is_some_and(|fraction| fraction.chars().any(|c| !c.is_ascii_digit()))
        || (integer.is_empty() && fraction.map_or(true, str::is_empty))
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.unwrap_or_default().trim_end_matches('0');
    let canonical = if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    };
    Some(if negative && canonical != "0" {
        format!("-{canonical}")
    } else {
        canonical
    })
}

/// 两侧文案是否数值等价。
pub fn labels_equivalent(left: &str, right: &str) -> bool {
    left == right
        || normalize_decimal_label(left)
            .zip(normalize_decimal_label(right))
            .is_some_and(|(left, right)| left == right)
}

fn text_from_cell(cell: &Value) -> String {
    match unwrap_typed_cell(cell) {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(items) => items.iter().map(text_from_cell).collect(),
        Value::Object(map) => map
            .get("text")
            .or_else(|| map.get("name"))
            .map(text_from_cell)
            .unwrap_or_default(),
        Value::Null => String::new(),
    }
}

/// 空值判定（null / 空串 / 空数组 / 只有空文本的多态单元格）。
pub fn is_empty(value: &Value) -> bool {
    match unwrap_typed_cell(value) {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.iter().all(is_empty),
        Value::Object(map) => {
            if map.is_empty() {
                return true;
            }
            let content_keys = ["text", "name", "value"];
            content_keys
                .iter()
                .filter_map(|key| map.get(*key))
                .all(is_empty)
                && map
                    .iter()
                    .filter(|(key, _)| !content_keys.contains(&key.as_str()))
                    .all(|(_, nested)| is_empty(nested))
        }
        _ => false,
    }
}

fn format_rfc3339(millis: i64, offset: FixedOffset) -> String {
    let shifted = millis + i64::from(offset.seconds) * 1_000;
    let total_seconds = shifted.div_euclid(1_000);
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{}",
        offset.render()
    )
}

/// Unix 秒 → `YYYY-MM-DD HH:MM:SS`（按给定固定偏移）。
pub fn format_local(seconds: i64, offset: FixedOffset) -> String {
    let shifted = seconds + i64::from(offset.seconds);
    let days = shifted.div_euclid(86_400);
    let seconds_of_day = shifted.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shanghai() -> FixedOffset {
        FixedOffset::from_iana("Asia/Shanghai").unwrap_or_else(|error| panic!("{error}"))
    }

    fn snapshot() -> String {
        serde_json::json!([
            {"id": "w_input", "name": "文本", "type": "input", "required": true},
            {"id": "w_number", "name": "数字", "type": "number", "required": true},
            {"id": "w_date", "name": "日期", "type": "date", "required": true},
            {"id": "w_radio", "name": "单选", "type": "radioV2", "required": true,
             "option": [{"text": "A", "value": "opt-a"}]}
        ])
        .to_string()
    }

    fn widget(id: &str, widget_type: &str, source: &str, converter: Converter) -> WidgetMap {
        WidgetMap {
            widget_id: id.to_string(),
            widget_type: widget_type.to_string(),
            required: true,
            source_field: source.to_string(),
            interval_end_field: None,
            converter,
            option_map: BTreeMap::new(),
            option_source_key: None,
            currency: None,
        }
    }

    fn cells() -> Map<String, Value> {
        let mut cells = Map::new();
        cells.insert("f_input".into(), json!("hello"));
        cells.insert("f_number".into(), json!(42));
        cells.insert("f_date".into(), json!(1_790_750_400_000i64));
        cells.insert("f_radio".into(), json!(["A"]));
        cells
    }

    #[test]
    fn builds_a_compact_json_array_string() {
        let mut radio = widget("w_radio", "radioV2", "f_radio", Converter::Option);
        radio
            .option_map
            .insert("A".to_string(), "opt-a".to_string());
        let widgets = vec![
            widget("w_input", "input", "f_input", Converter::Direct),
            widget("w_number", "number", "f_number", Converter::Direct),
            widget("w_date", "date", "f_date", Converter::Date),
            radio,
        ];
        let cells = cells();
        let input = FormInput {
            widgets: &widgets,
            form_snapshot: &snapshot(),
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: shanghai(),
            approval_app_id: None,
        };
        let form = build_form(&input).unwrap_or_else(|error| panic!("{error}"));
        assert!(form.starts_with('['));
        assert!(!form.contains('\n'));
        let parsed: Value = serde_json::from_str(&form).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed[0]["value"], "hello");
        assert_eq!(parsed[1]["value"], 42);
        assert_eq!(parsed[2]["value"], "2026-09-30T14:40:00+08:00");
        assert_eq!(parsed[3]["value"], "opt-a");
    }

    #[test]
    fn missing_required_names_the_widget() {
        let widgets = vec![widget("w_input", "input", "f_missing", Converter::Direct)];
        let cells = Map::new();
        let input = FormInput {
            widgets: &widgets,
            form_snapshot: &snapshot(),
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: shanghai(),
            approval_app_id: None,
        };
        let error = build_form(&input).unwrap_err().to_string();
        assert!(error.contains("w_input"));
    }

    #[test]
    fn date_interval_supports_two_columns_and_objects() {
        let mut widget = widget("w_interval", "dateInterval", "f_start", Converter::Date);
        widget.interval_end_field = Some("f_end".into());
        let snapshot = serde_json::json!([
            {"id": "w_interval", "name": "区间", "type": "dateInterval", "required": true}
        ])
        .to_string();
        let mut cells = Map::new();
        cells.insert("f_start".into(), json!(0));
        cells.insert("f_end".into(), json!(86_400_000));
        let input = FormInput {
            widgets: &[widget],
            form_snapshot: &snapshot,
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: FixedOffset::from_seconds(0).unwrap(),
            approval_app_id: None,
        };
        let form = build_form(&input).unwrap_or_else(|error| panic!("{error}"));
        let parsed: Value = serde_json::from_str(&form).unwrap();
        assert_eq!(parsed[0]["value"]["interval"], 1.0);
        assert_eq!(parsed[0]["value"]["start"], "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn option_mapping_requires_an_entry() {
        let widgets = vec![{
            let mut widget = widget("w_radio", "radioV2", "f_radio", Converter::Option);
            widget.required = false;
            widget
        }];
        let mut cells = Map::new();
        cells.insert("f_radio".into(), json!(["X"]));
        let snapshot = serde_json::json!([
            {"id": "w_radio", "name": "单选", "type": "radioV2", "required": true,
             "option": [{"text": "A", "value": "opt-a"}]}
        ])
        .to_string();
        let input = FormInput {
            widgets: &widgets,
            form_snapshot: &snapshot,
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: shanghai(),
            approval_app_id: None,
        };
        let error = build_form(&input).unwrap_err();
        assert!(matches!(error, ConvertError::UnmappedOption { .. }));
    }

    #[test]
    fn signature_normalizes_numbers_and_skips_detail_lists() {
        let widgets = vec![widget("w_number", "number", "f_number", Converter::Direct)];
        let snapshot = serde_json::json!([
            {"id": "w_number", "type": "number"},
            {"id": "detail", "type": "fieldList", "children": [
                {"id": "w_detail_number", "type": "number"}
            ]}
        ])
        .to_string();
        let mut cells = Map::new();
        cells.insert("f_number".into(), json!(100.0));
        let input = FormInput {
            widgets: &widgets,
            form_snapshot: &snapshot,
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: shanghai(),
            approval_app_id: None,
        };
        let signature = build_signature(&input).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            signature,
            r#"{"w_number":{"id":"w_number","type":"number","value":"100"}}"#
        );
    }

    #[test]
    fn connect_applink_is_parsed_with_the_caller_supplied_app_id() {
        let instance_id = "4202ad96-9ec1-4f0a-8f7e-123456789abc";
        let mut cells = Map::new();
        cells.insert(
            "f_connect".into(),
            json!([{
                "text": "审批单",
                "link": format!(
                    "https://applink.feishu.cn/client/mini_program/open?appId=cli_app&path=pages/detail/index?instanceId={instance_id}&source=bitable"
                )
            }]),
        );
        let snapshot = serde_json::json!([
            {"id": "w_connect", "name": "关联审批", "type": "connect", "required": true,
             "value": ["cli_other"]}
        ])
        .to_string();
        let widgets = vec![widget(
            "w_connect",
            "connect",
            "f_connect",
            Converter::Direct,
        )];
        let input = FormInput {
            widgets: &widgets,
            form_snapshot: &snapshot,
            cells: &cells,
            attachment_codes: &BTreeMap::new(),
            timezone_offset: shanghai(),
            approval_app_id: Some("cli_app"),
        };
        let form = build_form(&input).unwrap_or_else(|error| panic!("{error}"));
        let parsed: Value = serde_json::from_str(&form).unwrap();
        assert_eq!(parsed[0]["value"][0], instance_id.to_uppercase());
    }

    #[test]
    fn helpers_match_the_host_cell_rules() {
        assert_eq!(
            unwrap_typed_cell(&json!({"type": 2, "value": [1]})),
            &json!([1])
        );
        assert_eq!(labels_from_cell(&json!([{"text": " A "}])), vec!["A"]);
        assert_eq!(normalize_decimal_label("9.1330"), Some("9.133".into()));
        assert_eq!(normalize_decimal_label("1.0000"), Some("1".into()));
        assert_eq!(normalize_decimal_label("1e3"), None);
        assert!(labels_equivalent("9.1330", "9.133"));
        assert!(is_empty(&json!([{"text": ""}])));
        assert!(!is_empty(&json!([{"text": "x"}])));
        assert_eq!(
            format_local(1_790_750_400, FixedOffset::from_seconds(0).unwrap()),
            "2026-09-30 06:40:00"
        );
    }
}
