#![allow(deprecated)]

use yang_base::database::DatabaseInitializer;

async fn legacy_migration_api_still_typechecks(initializer: &DatabaseInitializer) {
    let _ = initializer.record_migration("legacy", "v1").await;
}

#[test]
fn patch_release_versions_and_legacy_surface_are_explicit() {
    let _ = legacy_migration_api_still_typechecks;
    assert_eq!(yang_base::VERSION, "0.1.2");
    assert_eq!(yang_db::VERSION, "0.1.4");
}
