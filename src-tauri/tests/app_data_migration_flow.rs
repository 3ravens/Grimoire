
//! Integration-style checks for preview → release app data folder migration.

use std::fs;

use app_lib::app_data_migration::{
    run_migration_from_legacy_to_new, MIGRATION_SENTINEL_FILE,
};
use tempfile::tempdir;

#[test]
fn migrate_preview_tree_into_empty_destination() {
    let legacy = tempdir().unwrap();
    let dest = tempdir().unwrap();

    fs::write(legacy.path().join("grimoire.db"), b"fake-sqlite").unwrap();
    fs::create_dir_all(legacy.path().join("lancedb/t")).unwrap();
    fs::write(legacy.path().join("lancedb/t/a"), b"x").unwrap();

    run_migration_from_legacy_to_new(legacy.path(), dest.path()).unwrap();

    assert_eq!(
        fs::read_to_string(dest.path().join("grimoire.db")).unwrap(),
        "fake-sqlite"
    );
    assert_eq!(
        fs::read_to_string(dest.path().join("lancedb/t/a")).unwrap(),
        "x"
    );
    assert!(dest.path().join(MIGRATION_SENTINEL_FILE).is_file());
    assert!(legacy.path().join("grimoire.db").exists());
}

#[test]
fn migrate_skips_when_destination_db_exists() {
    let legacy = tempdir().unwrap();
    let dest = tempdir().unwrap();
    fs::write(legacy.path().join("grimoire.db"), b"a").unwrap();
    fs::write(dest.path().join("grimoire.db"), b"b").unwrap();
    assert!(run_migration_from_legacy_to_new(legacy.path(), dest.path()).is_err());
}
