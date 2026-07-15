#![cfg(feature = "admin-metadata")]
#![allow(clippy::expect_used)]

use yang_base::admin::{AdminDisplayKind, AdminMetadata, AdminMetadataRegistry, AdminTarget};

#[test]
fn metadata_covers_display_kinds_and_stable_targets_without_dispatch_objects() {
    let entries = vec![
        AdminMetadata::new(
            "users.menu",
            "用户",
            AdminDisplayKind::Menu,
            AdminTarget::action("users", "list").expect("合法 Action 引用"),
        )
        .expect("合法元数据")
        .icon("users")
        .group("system")
        .order(10),
        AdminMetadata::new(
            "users.create",
            "新建用户",
            AdminDisplayKind::Button,
            AdminTarget::api_operation("users.create").expect("合法 operation id"),
        )
        .expect("合法元数据"),
        AdminMetadata::new(
            "users.list",
            "用户列表",
            AdminDisplayKind::List,
            AdminTarget::table("users", "users").expect("合法表引用"),
        )
        .expect("合法元数据"),
        AdminMetadata::new(
            "org.tree",
            "组织树",
            AdminDisplayKind::Tree,
            AdminTarget::table("org", "departments").expect("合法表引用"),
        )
        .expect("合法元数据"),
        AdminMetadata::new(
            "users.form",
            "用户表单",
            AdminDisplayKind::Form,
            AdminTarget::action("users", "save").expect("合法 Action 引用"),
        )
        .expect("合法元数据"),
    ];
    let registry = AdminMetadataRegistry::new(entries).expect("元数据 ID 唯一");
    assert_eq!(registry.entries().len(), 5);
    assert_eq!(registry.get("users.menu").expect("按 ID 查询").order, 10);
}

#[test]
fn metadata_rejects_duplicate_or_adversarial_stable_ids() {
    for payload in ["", " users", "users/list", "users;drop", "用户", "a\0b"] {
        assert!(
            AdminTarget::api_operation(payload).is_err(),
            "非法 ID 被接受: {payload:?}"
        );
    }
    let first = AdminMetadata::new(
        "users.menu",
        "用户",
        AdminDisplayKind::Menu,
        AdminTarget::action("users", "list").expect("合法引用"),
    )
    .expect("合法元数据");
    let duplicate = AdminMetadata::new(
        "users.menu",
        "重复",
        AdminDisplayKind::Button,
        AdminTarget::action("users", "create").expect("合法引用"),
    )
    .expect("合法元数据");
    assert!(AdminMetadataRegistry::new(vec![first, duplicate]).is_err());
    assert!(AdminMetadata::new(
        "blank.label",
        "   ",
        AdminDisplayKind::Form,
        AdminTarget::table("users", "users").expect("合法引用"),
    )
    .is_err());
}
