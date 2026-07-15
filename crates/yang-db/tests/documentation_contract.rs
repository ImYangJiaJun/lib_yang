#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

fn read_crate_readme() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md");
    fs::read_to_string(path).expect("读取 yang-db README")
}

#[test]
fn support_matrix_and_non_goals_are_explicit() {
    let readme = read_crate_readme();
    for required in [
        "## 支持矩阵与 non-goal",
        "MySQL",
        "PostgreSQL",
        "Redis",
        "SQLite",
        "MSSQL",
        "独立 RFC",
        "驱动、类型映射、DDL、事务和 CI",
        "backup",
        "database-create",
        "不进入 `QueryBuilder`",
        "数据库原生工具",
    ] {
        assert!(readme.contains(required), "README 缺少支持边界: {required}");
    }
}

#[test]
fn readme_does_not_claim_implemented_core_features_are_pending() {
    let readme = read_crate_readme();
    assert!(
        !readme.contains("待实现功能："),
        "README 仍包含过期待实现清单"
    );
}
