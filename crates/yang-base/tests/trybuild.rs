//! trybuild 编译失败用例入口（H-1 验收）
//!
//! 验证类型化 Action 系统在编译期拒绝非法用法：非法字段名、where 条件类型
//! 不匹配、数值字段误用 Like、派生缺主键。每个 `compile_fail/*.rs` 都应**编译失败**，
//! 且错误输出匹配对应 `.stderr` 快照。
//!
//! # 维护提示
//!
//! `.stderr` 快照对 rustc 版本敏感。若升级工具链后本测试因错误文案变化而失败，
//! 用 `TRYBUILD=overwrite cargo test --test trybuild` 重新生成快照并人工复核。
//!
//! 仅在 mysql feature 下运行（TableEntity 依赖该 feature）。
#![cfg(feature = "mysql")]

#[test]
fn compile_fail_cases() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
