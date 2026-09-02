//! 发布文档的跨文件一致性契约。

use std::fs;
use std::path::PathBuf;

fn workspace_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("无法读取 {}: {error}", path.display()))
}

/// 从 crate 清单读取 `[package]` 版本号（清单中第一个 `version = "..."` 行），
/// 让文档版本断言始终跟随真实发布版本，杜绝文档版本漂移。
fn crate_version(manifest_relative: &str) -> String {
    let manifest = workspace_file(manifest_relative);
    for line in manifest.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                return value.trim().trim_matches('"').to_string();
            }
        }
    }
    panic!("{manifest_relative} 缺少 package version");
}

fn assert_contains_all(document: &str, name: &str, expected: &[&str]) {
    for needle in expected {
        assert!(
            document.contains(needle),
            "{name} 缺少当前能力契约 `{needle}`"
        );
    }
}

fn assert_contains_none(document: &str, name: &str, forbidden: &[&str]) {
    for needle in forbidden {
        assert!(
            !document.contains(needle),
            "{name} 不应继续发布已删除契约 `{needle}`"
        );
    }
}

#[test]
fn crate_readmes_publish_current_versions_and_features() {
    let base_version = crate_version("crates/yang-base/Cargo.toml");
    let db_version = crate_version("crates/yang-db/Cargo.toml");

    let base = workspace_file("crates/yang-base/README.md");
    assert_contains_all(
        &base,
        "yang-base README",
        &[
            base_version.as_str(),
            "`token`",
            "`http`",
            "`mysql`",
            "`redis`",
            "`validator`",
            "`plugin-schema`",
            "`metrics`",
            "`openapi`",
            "`admin-metadata`",
            "TableDefinition",
            "Record",
            "Api::get",
            ".crud()",
        ],
    );
    assert!(!base.contains("v0.0.1 (当前版本)"));

    let db = workspace_file("crates/yang-db/README.md");
    assert_contains_all(
        &db,
        "yang-db README",
        &[
            db_version.as_str(),
            "MySQL 8",
            "PostgreSQL 16",
            "Redis 7",
            "non-goal",
        ],
    );
}

#[test]
fn api_overviews_cover_current_public_contracts() {
    let base_version = crate_version("crates/yang-base/Cargo.toml");
    let db_version = crate_version("crates/yang-db/Cargo.toml");
    let base_version_tag = format!("版本：{base_version}");
    let db_version_tag = format!("版本：{db_version}");

    let base = workspace_file("docs/yang-base.md");
    assert_contains_all(
        &base,
        "docs/yang-base.md",
        &[
            base_version_tag.as_str(),
            "Table + Field",
            "TableDefinition",
            "Record",
            "Api::{get,post,put,patch,delete}",
            "ModuleRouter::table",
            "ModuleRouter::schema",
            ".crud()",
            "RequestMeta",
            "ApiCatalog",
            "OpenAPI 3.1",
            "admin-metadata",
            "DatabaseInitializer",
            "SchemaValidationReport",
            "sync_app_schema",
            "validate_table_definition",
        ],
    );

    let db = workspace_file("docs/yang-db.md");
    assert_contains_all(
        &db,
        "docs/yang-db.md",
        &[
            db_version_tag.as_str(),
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
    let base_version = crate_version("crates/yang-base/Cargo.toml");
    let base_version_tag = format!("yang-base {base_version}");

    let matrix = workspace_file("docs/BASE_DB_CAPABILITY_MATRIX.md");
    assert_contains_all(
        &matrix,
        "能力矩阵",
        &[
            base_version_tag.as_str(),
            "TableDefinition",
            "Record",
            "Api",
            "br-addon",
            "br-db",
            "non-goal",
            "受控 SQL",
            "真实消费者",
        ],
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
            "schema-first 公共边界",
            "ModuleRouter::table(definition).crud()",
            "TableDefinition",
            "Record",
        ],
    );
}

#[test]
fn versioning_and_current_docs_lock_schema_first_release_boundary() {
    let base_version = crate_version("crates/yang-base/Cargo.toml");
    let derive_version = crate_version("crates/yang-base-derive/Cargo.toml");
    let db_version = crate_version("crates/yang-db/Cargo.toml");
    let base_version_tag = format!("`yang-base` {base_version}");
    let derive_version_tag = format!("`yang-base-derive` {derive_version}");
    let db_version_tag = format!("`yang-db` {db_version}");

    let versioning = workspace_file("docs/VERSIONING.md");
    assert_contains_all(
        &versioning,
        "VERSIONING",
        &[
            base_version_tag.as_str(),
            derive_version_tag.as_str(),
            db_version_tag.as_str(),
            "schema-first",
            "TableDefinition",
            "Record",
            "ApiCatalog",
        ],
    );

    let current_documents = [
        workspace_file("crates/yang-base/README.md"),
        workspace_file("docs/yang-base.md"),
        versioning.clone(),
        workspace_file("docs/BASE_DB_CAPABILITY_MATRIX.md"),
    ]
    .join("\n");

    let documents = [current_documents.clone(), workspace_file("docs/BACKLOG.md")].join("\n");
    assert_contains_none(
        &documents,
        "yang-base 当前文档与 BACKLOG",
        &[
            "TableEntity",
            "DynamicRow",
            "table_typed",
            "TableConfig",
            "FieldConfig",
            "with_table_config",
            "with_schema_table",
            "register_action",
            "register_route",
        ],
    );

    // Global* 单例已删除；BACKLOG 属历史工作日志允许提及，当前文档不得再发布。
    assert_contains_none(
        &current_documents,
        "yang-base 当前文档",
        &["GlobalDatabase", "GlobalRedis", "DatabaseBundle"],
    );
}
