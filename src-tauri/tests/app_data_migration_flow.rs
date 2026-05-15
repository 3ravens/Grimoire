// Copyright (C) 2026 Wim Palland
//
// This file is part of Grimoire.
//
// Grimoire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Grimoire is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Grimoire. If not, see <https://www.gnu.org/licenses/>.

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
