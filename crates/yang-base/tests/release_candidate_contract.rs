//! 发布候选报告与完成度的可执行契约。

use std::fs;
use std::path::PathBuf;

fn workspace_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("无法读取 {}: {error}", path.display()))
}

#[test]
fn release_report_records_every_required_gate() {
    let report = workspace_file("docs/RELEASE_CANDIDATE_REPORT.md");
    for required in [
        "yang-base 0.1.2",
        "yang-db 0.1.4",
        "clean checkout",
        "MSRV 1.80",
        "stable",
        "feature matrix",
        "doc tests",
        "MySQL 8",
        "PostgreSQL 16",
        "Redis 7",
        "dependency audit",
        "cargo package",
        "adversarial",
    ] {
        assert!(report.contains(required), "发布报告缺少 `{required}`");
    }
}

#[test]
fn completeness_plan_has_no_pending_points() {
    let plan = workspace_file("docs/YANG_BASE_DB_COMPLETENESS_PLAN.md");
    assert!(!plan.contains("— `PENDING`"), "完整度计划仍存在 PENDING 点");
}
