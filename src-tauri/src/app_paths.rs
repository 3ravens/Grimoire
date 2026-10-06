//! Resolve the Grimoire app data directory.
//!
//! - **Release / website install:** Tauri bundle id `com.grimoire.app`
//!   (`%APPDATA%\com.grimoire.app` on Windows).
//! - **`tauri:dev` (with `tauri.dev.conf.json`):** bundle id `com.grimoire.app.dev`
//!   — a separate vault from the installed app.
//! - **Wizard sandbox:** set `GRIMOIRE_APP_DATA_DIR` to an isolated folder under
//!   `scripts/.local-sandboxes/` so first-run testing never touches either vault.
//!
//! Debug builds also rewrite a bare `com.grimoire.app` path to
//! `com.grimoire.app.dev` so `cargo run` without the overlay cannot mutate the
//! production database.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub const APP_DATA_DIR_ENV: &str = "GRIMOIRE_APP_DATA_DIR";
pub const LEGACY_MIGRATION_FROM_ENV: &str = "GRIMOIRE_LEGACY_MIGRATION_FROM";

/// Production bundle id folder name (must never be written by debug builds).
pub const PRODUCTION_APP_DATA_DIR_NAME: &str = "com.grimoire.app";

/// Isolated local-dev vault folder name (matches `tauri.dev.conf.json` identifier).
pub const DEV_APP_DATA_DIR_NAME: &str = "com.grimoire.app.dev";

/// App data root: override env when set, otherwise Tauri `app_data_dir()`
/// (with a debug-only safety rewrite away from the production folder).
pub fn resolve_app_data_dir(app: &AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(path) = app_data_dir_override() {
        return Ok(path);
    }
    let path = app.path().app_data_dir()?;
    Ok(apply_debug_production_vault_guard(path))
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

/// In debug builds, never use the production vault folder even if the binary
/// was launched without `tauri.dev.conf.json`.
fn apply_debug_production_vault_guard(path: PathBuf) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        if path
            .file_name()
            .and_then(|s| s.to_str())
            == Some(PRODUCTION_APP_DATA_DIR_NAME)
        {
            return path.with_file_name(DEV_APP_DATA_DIR_NAME);
        }
    }
    path
}

/// Pure helper for unit tests (mirrors debug guard logic).
#[cfg(test)]
fn debug_guard_rewrite_for_test(path: PathBuf) -> PathBuf {
    if path
        .file_name()
        .and_then(|s| s.to_str())
        == Some(PRODUCTION_APP_DATA_DIR_NAME)
    {
        return path.with_file_name(DEV_APP_DATA_DIR_NAME);
    }
    path
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

/// Log which vault isolation mode is active (env override and/or debug guard).
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
        return;
    }

    #[cfg(debug_assertions)]
    {
        log::warn!(
            "Debug build — app data uses `{DEV_APP_DATA_DIR_NAME}` (isolated from production `{PRODUCTION_APP_DATA_DIR_NAME}`). \
Website / taskbar install is not touched."
        );
    }
}

/// True when `path`'s final component is the production vault directory name.
pub fn is_production_app_data_dir_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        == Some(PRODUCTION_APP_DATA_DIR_NAME)
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

    #[test]
    fn debug_guard_rewrites_production_folder_name() {
        let prod = PathBuf::from(r"C:\Users\me\AppData\Roaming\com.grimoire.app");
        let rewritten = debug_guard_rewrite_for_test(prod);
        assert_eq!(
            rewritten,
            PathBuf::from(r"C:\Users\me\AppData\Roaming\com.grimoire.app.dev")
        );
        assert!(!is_production_app_data_dir_name(&rewritten));
    }

    #[test]
    fn debug_guard_leaves_dev_and_other_folders_alone() {
        let already_dev = PathBuf::from(r"C:\Users\me\AppData\Roaming\com.grimoire.app.dev");
        assert_eq!(
            debug_guard_rewrite_for_test(already_dev.clone()),
            already_dev
        );
        let other = PathBuf::from(r"C:\temp\sandbox");
        assert_eq!(debug_guard_rewrite_for_test(other.clone()), other);
    }

    #[test]
    fn env_override_takes_precedence_over_path_shape() {
        let _g = env_lock().lock().unwrap();
        let _restore = EnvRestore::clear(&[APP_DATA_DIR_ENV, LEGACY_MIGRATION_FROM_ENV]);
        std::env::set_var(APP_DATA_DIR_ENV, r"D:\sandboxes\wizard");
        // Override is returned as-is; resolve_app_data_dir would not apply the
        // debug guard when override is active (tested via override() here).
        assert_eq!(
            app_data_dir_override().unwrap(),
            PathBuf::from(r"D:\sandboxes\wizard")
        );
    }
}
