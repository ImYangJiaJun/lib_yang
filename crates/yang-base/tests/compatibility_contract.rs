#[test]
fn breaking_release_versions_are_explicit() {
    assert_eq!(yang_base::VERSION, "0.2.1");
    assert_eq!(yang_db::VERSION, "0.1.5");
}
