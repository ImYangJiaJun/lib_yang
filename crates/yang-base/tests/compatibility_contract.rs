#[test]
fn breaking_release_versions_are_explicit() {
    assert_eq!(yang_base::VERSION, "0.6.0");
    assert_eq!(yang_db::VERSION, "0.3.1");
}
