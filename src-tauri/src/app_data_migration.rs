//! One-time copy migration from preview bundle-id folders into the current
//! `app_data_dir()` before SQLite opens. Never deletes the legacy directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

/// Written after a successful migration (support + UI banner).
pub const MIGRATION_SENTINEL_FILE: &str = "app_data_migrated_from.txt";

/// Settings key: user dismissed the post-migration notice.
pub const SETTING_MIGRATION_BANNER_DISMISSED: &str = "app_data_migration_banner_dismissed";

/// Preview / template bundle identifiers that may contain `grimoire.db`.
static LEGACY_BUNDLE_IDS: &[&str] = &[
    "com.tauri.dev",
    "dev.grimoireapp.grimoire",
    "app.grimoire.grimoire",
];

/// If the new app data dir has no `grimoire.db`, copy from the first matching legacy
/// tree (database + `lancedb/` + optional `logs/`). Legacy folders are left in place.
pub fn migrate_legacy_app_data_if_needed(app: &AppHandle) -> Result<(), String> {
    let new_root = app.path().app_data_dir().map_err(|e| e.to_string())?;

    if new_root.join("grimoire.db").is_file() {
        return Ok(());
    }

    let Some(legacy_root) = find_first_legacy_data_root(&new_root) else {
        return Ok(());
    };

    run_migration_from_legacy_to_new(&legacy_root, &new_root)
}

/// Copy app data from `legacy_root` into `new_root`. Used by integration tests and
/// [`migrate_legacy_app_data_if_needed`]. Fails if `new_root/grimoire.db` already exists.
pub fn run_migration_from_legacy_to_new(legacy_root: &Path, new_root: &Path) -> Result<(), String> {
    if new_root.join("grimoire.db").exists() {
        return Err("destination already contains grimoire.db".into());
    }

    let db_src = legacy_root.join("grimoire.db");
    if !db_src.is_file() {
        return Err("legacy grimoire.db is not a file".into());
    }

    fs::create_dir_all(new_root).map_err(|e| e.to_string())?;

    let db_dst_tmp = new_root.join("grimoire.db.partial");
    let db_dst_final = new_root.join("grimoire.db");

    let existed_lancedb = new_root.join("lancedb").exists();
    let existed_logs = new_root.join("logs").exists();
    let existed_db_dst_tmp = db_dst_tmp.exists();
    let existed_db_dst_final = db_dst_final.exists();
    let existed_sentinel = new_root.join(MIGRATION_SENTINEL_FILE).exists();

    let cleanup_new_side = || {
        if !existed_db_dst_tmp {
            let _ = fs::remove_file(&db_dst_tmp);
        }
        if !existed_db_dst_final {
            let _ = fs::remove_file(&db_dst_final);
        }
        if !existed_lancedb {
            let _ = fs::remove_dir_all(new_root.join("lancedb"));
        }
        if !existed_logs {
            let _ = fs::remove_dir_all(new_root.join("logs"));
        }
        if !existed_sentinel {
            let _ = fs::remove_file(new_root.join(MIGRATION_SENTINEL_FILE));
        }
    };

    fs::copy(&db_src, &db_dst_tmp).map_err(|e| {
        cleanup_new_side();
        format!("copy grimoire.db: {e}")
    })?;

    let len = fs::metadata(&db_dst_tmp).map_err(|e| {
        cleanup_new_side();
        e.to_string()
    })?;
    if len.len() == 0 {
        cleanup_new_side();
        return Err("legacy grimoire.db is empty".into());
    }

    fs::rename(&db_dst_tmp, &db_dst_final).map_err(|e| {
        cleanup_new_side();
        e.to_string()
    })?;

    let lance_src = legacy_root.join("lancedb");
    if lance_src.is_dir() {
        if let Err(e) = copy_dir_recursive(&lance_src, &new_root.join("lancedb")) {
            cleanup_new_side();
            return Err(format!("copy lancedb: {e}"));
        }
    }

    let logs_src = legacy_root.join("logs");
    if logs_src.is_dir() {
        let _ = copy_dir_recursive(&logs_src, &new_root.join("logs"));
    }

    write_sentinel(new_root, legacy_root).map_err(|e| {
        cleanup_new_side();
        e
    })?;

    log::info!(
        "App data migrated from {} to {}",
        legacy_root.display(),
        new_root.display()
    );

    Ok(())
}

fn write_sentinel(new_root: &Path, legacy_root: &Path) -> Result<(), String> {
    let ts = chrono::Local::now().to_rfc3339();
    let body = format!(
        "migrated_from={}\nmigrated_at={}\n",
        legacy_root.display(),
        ts
    );
    fs::write(new_root.join(MIGRATION_SENTINEL_FILE), body.as_bytes()).map_err(|e| e.to_string())
}

fn find_first_legacy_data_root(new_root: &Path) -> Option<PathBuf> {
    for legacy_id in LEGACY_BUNDLE_IDS {
        for root in candidate_roots_for_legacy_id(legacy_id) {
            if paths_refer_to_same_dir(&root, new_root) {
                continue;
            }
            if root.join("grimoire.db").is_file() {
                return Some(root);
            }
        }
    }
    None
}

fn paths_refer_to_same_dir(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ac), Ok(bc)) => ac == bc,
        _ => false,
    }
}

fn candidate_roots_for_legacy_id(legacy_id: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = std::env::var_os("APPDATA") {
            v.push(PathBuf::from(p).join(legacy_id));
        }
        if let Some(p) = std::env::var_os("LOCALAPPDATA") {
            v.push(PathBuf::from(p).join(legacy_id));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(h) = std::env::var("HOME") {
            v.push(
                PathBuf::from(h)
                    .join("Library/Application Support")
                    .join(legacy_id),
            );
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(x) = std::env::var("XDG_DATA_HOME") {
            if !x.is_empty() {
                v.push(PathBuf::from(x).join(legacy_id));
            }
        }
        if let Ok(h) = std::env::var("HOME") {
            v.push(PathBuf::from(h).join(".local/share").join(legacy_id));
        }
    }
    v
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migration_copies_db_lancedb_and_sentinel() {
        let legacy = tempdir().unwrap();
        let new_root = tempdir().unwrap();

        fs::create_dir_all(legacy.path().join("lancedb/nested")).unwrap();
        fs::write(legacy.path().join("grimoire.db"), b"sqlite").unwrap();
        fs::write(legacy.path().join("lancedb/nested/x.dat"), b"hi").unwrap();

        run_migration_from_legacy_to_new(legacy.path(), new_root.path()).unwrap();

        assert!(new_root.path().join("grimoire.db").is_file());
        assert_eq!(
            fs::read_to_string(new_root.path().join("grimoire.db")).unwrap(),
            "sqlite"
        );
        assert_eq!(
            fs::read_to_string(new_root.path().join("lancedb/nested/x.dat")).unwrap(),
            "hi"
        );
        let sent = fs::read_to_string(new_root.path().join(MIGRATION_SENTINEL_FILE)).unwrap();
        assert!(sent.contains("migrated_from="));
        assert!(sent.contains("migrated_at="));
    }

    #[test]
    fn migration_refuses_nonempty_destination() {
        let legacy = tempdir().unwrap();
        let new_root = tempdir().unwrap();
        fs::write(legacy.path().join("grimoire.db"), b"x").unwrap();
        fs::write(new_root.path().join("grimoire.db"), b"y").unwrap();
        assert!(run_migration_from_legacy_to_new(legacy.path(), new_root.path()).is_err());
    }
}
