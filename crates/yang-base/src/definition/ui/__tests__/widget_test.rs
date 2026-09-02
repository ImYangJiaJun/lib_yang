//! WidgetHint 映射与各类可降级枚举的未知值安全 fallback 测试。

use super::super::{
    ActionInteraction, ActionPlacement, AvailabilityState, SortDirection, WidgetHint,
};
use crate::definition::{FieldKind, FieldName, FieldSpec};
use serde_json::json;

#[test]
fn widget_hint_maps_field_semantics_without_changing_storage_kind() {
    let field = |kind| FieldSpec::new(FieldName::new("value").expect("字段名应有效"), kind);

    assert_eq!(field(FieldKind::Key).widget_hint(), WidgetHint::Integer);
    assert_eq!(field(FieldKind::Str).widget_hint(), WidgetHint::Text);
    assert_eq!(field(FieldKind::Text).widget_hint(), WidgetHint::Textarea);
    assert_eq!(field(FieldKind::Int).widget_hint(), WidgetHint::Integer);
    assert_eq!(field(FieldKind::Decimal).widget_hint(), WidgetHint::Decimal);
    assert_eq!(field(FieldKind::Switch).widget_hint(), WidgetHint::Switch);
    assert_eq!(field(FieldKind::Radio).widget_hint(), WidgetHint::Radio);
    assert_eq!(
        field(FieldKind::Table).widget_hint(),
        WidgetHint::RelationSelect
    );
    assert_eq!(field(FieldKind::Tree).widget_hint(), WidgetHint::TreeSelect);
    assert_eq!(
        field(FieldKind::Timestamp).widget_hint(),
        WidgetHint::DateTime
    );
}

#[test]
fn widget_hint_explicit_override_and_unknown_value_have_safe_fallbacks() {
    let mut secret = FieldSpec::new(
        FieldName::new("secret").expect("字段名应有效"),
        FieldKind::Str,
    );
    secret.access.secret = true;
    assert_eq!(secret.widget_hint(), WidgetHint::Password);

    secret.presentation.widget = Some(WidgetHint::Email);
    assert_eq!(secret.widget_hint(), WidgetHint::Email);
    assert_eq!(secret.kind, FieldKind::Str, "控件提示不得改变字段数据种类");

    let unknown: WidgetHint =
        serde_json::from_value(json!("future_spatial_editor")).expect("未知提示应安全解析");
    assert_eq!(unknown, WidgetHint::Json);
    assert_eq!(
        serde_json::to_value(unknown).expect("fallback 应可序列化"),
        json!("json")
    );
}

#[test]
fn action_presentation_unknown_values_have_safe_fallbacks() {
    let placement: ActionPlacement =
        serde_json::from_value(json!("floating_palette")).expect("未知位置应安全解析");
    let interaction: ActionInteraction =
        serde_json::from_value(json!("execute_script")).expect("未知交互应安全解析");
    let availability: AvailabilityState =
        serde_json::from_value(json!("scheduled")).expect("未知可用状态应安全解析");
    let sort: SortDirection =
        serde_json::from_value(json!("randomized")).expect("未知排序方向应安全解析");

    assert_eq!(placement, ActionPlacement::Toolbar);
    assert_eq!(interaction, ActionInteraction::Invoke);
    assert_eq!(availability, AvailabilityState::Disabled);
    assert_eq!(sort, SortDirection::Asc);
}
