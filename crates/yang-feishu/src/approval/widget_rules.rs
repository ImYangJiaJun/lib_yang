//! 审批控件类型的统一规则表。
//!
//! 来源：飞书《审批实例表单控件参数》。配置期（按名匹配）与派发期（值组装、
//! 附件上传）共用同一份判据，避免各自维护清单后静默漂移。

/// 不能通过实例 API 提单的控件类型。
pub const UNSUPPORTED_TYPES: &[&str] = &[
    "text",
    "mutableGroup",
    "account",
    "serialNumber",
    "tripGroup",
    "apaascorehrOnboardingGroup",
    "apaascorehrRegularateGroup",
    "remedyGroupV2",
    "apaascorehrJobAdjustGroup",
    "apaascorehrOffboardingGroup",
    "shiftGroup",
    "shiftGroupV2",
    "leaveGroup",
    "remedyGroup",
];

/// 配置期必须显式拦下「必填」的控件类型。
pub const CONFIG_REJECT_TYPES: &[&str] = &[
    "account",
    "remedyGroup",
    "remedyGroupV2",
    "leaveGroup",
    "shiftGroup",
    "shiftGroupV2",
];

/// 该类型是否必须在配置期拦下必填项。
pub fn must_reject_at_config(widget_type: &str) -> bool {
    CONFIG_REJECT_TYPES.contains(&widget_type)
}

/// 需要人工准备值的控件：旧 v1 附件、文档、地址。
pub const MANUAL_VALUE_TYPES: &[&str] = &["address", "attachment", "document"];

/// 容器类控件：自身不进映射表，只用于分组展示或携带子控件。
pub const CONTAINER_TYPES: &[&str] = &["fieldList", "leaveGroupV2", "workGroup", "outGroup"];

/// 该类型是否不能通过实例 API 提单。
pub fn is_unsupported(widget_type: &str) -> bool {
    UNSUPPORTED_TYPES.contains(&widget_type)
}

/// 该类型是否需要人工准备值。
pub fn needs_manual_value(widget_type: &str) -> bool {
    MANUAL_VALUE_TYPES.contains(&widget_type)
}

/// 该类型是否是容器。
pub fn is_container(widget_type: &str) -> bool {
    CONTAINER_TYPES.contains(&widget_type)
}

/// 该类型是否是「控件组」（不含明细）。
pub fn is_control_group(widget_type: &str) -> bool {
    matches!(widget_type, "leaveGroupV2" | "workGroup" | "outGroup")
}

/// 该类型是否出现在配置向导里。
pub fn is_configurable(widget_type: &str) -> bool {
    !is_unsupported(widget_type)
}

/// 该类型是否会进映射表。
pub fn is_mappable(widget_type: &str) -> bool {
    !is_container(widget_type) && is_configurable(widget_type)
}

/// 该类型是否是媒体控件（走附件上传链路）。
pub fn is_media(widget_type: &str) -> bool {
    matches!(widget_type, "attachmentV2" | "image" | "imageV2")
}

/// 媒体控件上传到飞书时使用的文件类型。
pub fn upload_type(widget_type: &str) -> &'static str {
    if matches!(widget_type, "image" | "imageV2") {
        "image"
    } else {
        "attachment"
    }
}

/// 单个文件的大小上限：图片 10 MiB，其它附件 50 MiB。
pub fn max_upload_bytes(widget_type: &str) -> usize {
    if matches!(widget_type, "image" | "imageV2") {
        10 * 1024 * 1024
    } else {
        50 * 1024 * 1024
    }
}

/// 控件组的子控件白名单，按官方表单顺序排列。
pub fn group_child_ids(widget_type: &str) -> Option<&'static [&'static str]> {
    match widget_type {
        "leaveGroupV2" => Some(&[
            "widgetLeaveGroupType",
            "widgetLeaveGroupStartTime",
            "widgetLeaveGroupEndTime",
            "widgetLeaveGroupReason",
            "widgetLeaveCertification",
            "widgetLeaveGroupFeedingArrivingLate",
            "widgetLeaveGroupFeedingOffLeaveEarly",
        ]),
        "workGroup" => Some(&[
            "widgetWorkGroupOvertimeWorkers",
            "widgetWorkGroupType",
            "widgetWorkGroupTimeRangeFieldList",
            "widgetWorkGroupReason",
        ]),
        "outGroup" => Some(&[
            "widgetOutGroupType",
            "widgetOutGroupStartTime",
            "widgetOutGroupEndTime",
            "widgetOutGroupReason",
            "widgetOutGroupImage",
        ]),
        _ => None,
    }
}

/// 这两个子控件的值可能是飞书假勤/加班类型 ID（不是文案），允许直取。
pub fn is_raw_option_child(widget_id: &str) -> bool {
    matches!(widget_id, "widgetLeaveGroupType" | "widgetWorkGroupType")
}

/// 不支持的控件类型对应的可读原因。
pub fn unsupported_reason(widget_type: &str) -> &'static str {
    match widget_type {
        "shiftGroup" | "shiftGroupV2" => "本系统暂未支持该控件组的子控件映射（官方实例 API 支持）",
        "leaveGroup" | "remedyGroup" => "该控件组版本不在官方实例 API 支持范围内",
        _ => "官方实例 API 不支持该控件",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_and_manual_sets_are_distinct() {
        assert!(is_unsupported("account"));
        assert!(!is_unsupported("input"));
        assert!(needs_manual_value("attachment"));
        assert!(!needs_manual_value("attachmentV2"));
    }

    #[test]
    fn containers_and_control_groups_are_not_mappable() {
        for value in ["fieldList", "leaveGroupV2", "workGroup", "outGroup"] {
            assert!(is_container(value));
            assert!(!is_mappable(value));
        }
        assert!(is_control_group("leaveGroupV2"));
        assert!(!is_control_group("fieldList"));
        assert!(is_mappable("input"));
    }

    #[test]
    fn media_upload_type_and_limits() {
        for value in ["attachmentV2", "image", "imageV2"] {
            assert!(is_media(value));
        }
        assert_eq!(upload_type("image"), "image");
        assert_eq!(upload_type("imageV2"), "image");
        assert_eq!(upload_type("attachmentV2"), "attachment");
        assert_eq!(max_upload_bytes("image"), 10 * 1024 * 1024);
        assert_eq!(max_upload_bytes("attachmentV2"), 50 * 1024 * 1024);
    }

    #[test]
    fn group_whitelists_keep_official_order() {
        assert_eq!(
            group_child_ids("workGroup").and_then(|ids| ids.first()),
            Some(&"widgetWorkGroupOvertimeWorkers")
        );
        assert_eq!(group_child_ids("input"), None);
        assert!(is_raw_option_child("widgetWorkGroupType"));
        assert!(!is_raw_option_child("widgetWorkGroupReason"));
    }
}
