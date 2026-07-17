#[test]
fn dsl_rejects_invalid_definitions_at_compile_time() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("trybuild/*.rs");
}
