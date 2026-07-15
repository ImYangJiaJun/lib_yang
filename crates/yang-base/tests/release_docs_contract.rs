//! 发布文档的跨文件一致性契约。

use std::fs;
use std::path::PathBuf;

fn workspace_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("无法读取 {}: {error}", path.display()))
}

fn assert_contains_all(document: &str, name: &str, expected: &[&str]) {
    for needle in expected {
        assert!(
            document.contains(needle),
            "{name} 缺少当前能力契约 `{needle}`"
        );
    }
}

#[test]
fn crate_readmes_publish_current_versions_and_features() {
    let base = workspace_file("crates/yang-base/README.md");
    assert_contains_all(
        &base,
        "yang-base README",
        &[
            "0.1.2",
            "`token`",
            "`http`",
            "`mysql`",
            "`redis`",
            "`validator`",
            "`plugin-schema`",
            "`metrics`",
            "`openapi`",
            "`admin-metadata`",
        ],
    );
    assert!(!base.contains("v0.0.1 (当前版本)"));

    let db = workspace_file("crates/yang-db/README.md");
    assert_contains_all(
        &db,
        "yang-db README",
        &["0.1.4", "MySQL 8", "PostgreSQL 16", "Redis 7", "non-goal"],
    );
}

#[test]
fn api_overviews_cover_current_public_contracts() {
    let base = workspace_file("docs/yang-base.md");
    assert_contains_all(
        &base,
        "docs/yang-base.md",
        &[
            "版本：0.1.2",
            "RequestMeta",
            "ApiCatalog",
            "OpenAPI 3.1",
            "admin-metadata",
            "DatabaseInitializer",
            "SchemaValidationReport",
        ],
    );

    let db = workspace_file("docs/yang-db.md");
    assert_contains_all(
        &db,
        "docs/yang-db.md",
        &[
            "版本：0.1.4",
            "BackendCapabilities",
            "Subquery",
            "UNION ALL",
            "RowLock",
            "increment",
            "PostgreSQL 16",
        ],
    );
    assert!(!db.contains("MySqlPool 裸指针操作"));
}

#[test]
fn capability_matrix_and_backlog_reconciliation_are_present() {
    let matrix = workspace_file("docs/BASE_DB_CAPABILITY_MATRIX.md");
    assert_contains_all(
        &matrix,
        "能力矩阵",
        &["br-addon", "br-db", "non-goal", "受控 SQL", "真实消费者"],
    );

    let backlog = workspace_file("docs/BACKLOG.md");
    assert_contains_all(
        &backlog,
        "BACKLOG 对账",
        &[
            "2026-07-15 完成度对账",
            "YANG_BASE_DB_COMPLETENESS_PLAN.md",
            "[已完成]",
            "[已失效]",
            "## 🔴 Critical — 生产风险",
        ],
    );
}
