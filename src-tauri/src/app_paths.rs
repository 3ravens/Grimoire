//! Resolve the Grimoire app data directory.
//!
//! Normal installs use Tauri's bundle-id path (`com.grimoire.app`). For local wizard /
//! first-run testing, set `GRIMOIRE_APP_DATA_DIR` to an isolated folder so dev runs
//! never touch the production vault under `%APPDATA%`.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

pub const APP_DATA_DIR_ENV: &str = "GRIMOIRE_APP_DATA_DIR";
pub const LEGACY_MIGRATION_FROM_ENV: &str = "GRIMOIRE_LEGACY_MIGRATION_FROM";

/// App data root: override env when set, otherwise Tauri `app_data_dir()`.
pub fn resolve_app_data_dir(app: &AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(path) = app_data_dir_override() {
        return Ok(path);
    }
    app.path().app_data_dir()
}

/// True when `GRIMOIRE_APP_DATA_DIR` points at a non-empty path.
pub fn app_data_dir_override_active() -> bool {
    app_data_dir_override().is_some()
}

fn app_data_dir_override() -> Option<PathBuf> {
    let raw = std::env::var(APP_DATA_DIR_ENV).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// Legacy migration source for sandbox testing only.
///
/// Both `GRIMOIRE_APP_DATA_DIR` and `GRIMOIRE_LEGACY_MIGRATION_FROM` must be set.
/// This prevents accidental migration from arbitrary paths in a normal install.
pub fn legacy_migration_from_for_sandbox() -> Option<PathBuf> {
    if !app_data_dir_override_active() {
        return None;
    }
    let raw = std::env::var(LEGACY_MIGRATION_FROM_ENV).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

pub fn log_sandbox_banner_if_active() {
    if let Some(path) = app_data_dir_override() {
        log::warn!(
            "GRIMOIRE_APP_DATA_DIR is set — using isolated app data at {} (not your normal install folder)",
            path.display()
        );
        if let Some(legacy) = legacy_migration_from_for_sandbox() {
            log::warn!(
                "GRIMOIRE_LEGACY_MIGRATION_FROM is set — preview migration will copy from {}",
                legacy.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvRestore {
        keys: Vec<String>,
    }

    impl EnvRestore {
        fn clear(keys: &[&str]) -> Self {
            let mut saved_keys = Vec::new();
            for key in keys {
                if std::env::var_os(key).is_some() {
                    saved_keys.push((*key).to_string());
                }
                std::env::remove_var(key);
            }
            Self { keys: saved_keys }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for key in &self.keys {
                // Values are not restored; tests only need isolation.
                let _ = std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn override_active_when_env_set() {
        let _g = env_lock().lock().unwrap();
        let _restore = EnvRestore::clear(&[APP_DATA_DIR_ENV, LEGACY_MIGRATION_FROM_ENV]);
        assert!(!app_data_dir_override_active());
        std::env::set_var(APP_DATA_DIR_ENV, r"C:\temp\grimoire-sandbox");
        assert!(app_data_dir_override_active());
        assert_eq!(
            app_data_dir_override().unwrap(),
            PathBuf::from(r"C:\temp\grimoire-sandbox")
        );
    }

    #[test]
    fn legacy_migration_requires_sandbox() {
        let _g = env_lock().lock().unwrap();
        let _restore = EnvRestore::clear(&[APP_DATA_DIR_ENV, LEGACY_MIGRATION_FROM_ENV]);
        std::env::set_var(LEGACY_MIGRATION_FROM_ENV, r"C:\temp\legacy");
        assert!(legacy_migration_from_for_sandbox().is_none());
        std::env::set_var(APP_DATA_DIR_ENV, r"C:\temp\sandbox");
        assert_eq!(
            legacy_migration_from_for_sandbox().unwrap(),
            PathBuf::from(r"C:\temp\legacy")
        );
    }
}
