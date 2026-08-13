
//! Generic key/value settings persistence.
//!
//! Values are always stored as text; the frontend is responsible for
//! serialising and deserialising specific types (e.g. "true"/"false" for bools).

use std::fs;

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::app_paths::resolve_app_data_dir;

use crate::app_data_migration::{MIGRATION_SENTINEL_FILE, SETTING_MIGRATION_BANNER_DISMISSED};
use crate::config::SharedConfig;
use crate::error::AppError;
use crate::AppResult;

/// Read a setting value by key. Returns an empty string if the key is absent.
#[tauri::command]
pub async fn get_setting(key: String, db: State<'_, SqlitePool>) -> AppResult<String> {
    let value = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = ?1 LIMIT 1",
    )
    .bind(&key)
    .fetch_optional(db.inner())
    .await
    ?
    .unwrap_or_default();

    Ok(value)
}

/// Write (upsert) a setting value.
#[tauri::command]
pub async fn set_setting(key: String, value: String, db: State<'_, SqlitePool>, config: State<'_, SharedConfig>) -> AppResult<()> {
    log::info!("[set_setting] key={key}");
    sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(&key)
        .bind(&value)
        .execute(db.inner())
        .await
        ?;

    config.write().unwrap().apply_change(&key, &value);

    Ok(())
}

/// Returns a one-time user-facing message after app data was copied from a
/// preview bundle-id folder (`app_data_migrated_from.txt`), unless dismissed.
#[tauri::command]
pub async fn get_app_data_migration_banner(
    app: AppHandle,
    db: State<'_, SqlitePool>,
) -> AppResult<Option<String>> {
    let dismissed: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = ?1 LIMIT 1",
    )
    .bind(SETTING_MIGRATION_BANNER_DISMISSED)
    .fetch_optional(db.inner())
    .await?;

    if dismissed.as_deref() == Some("1") {
        return Ok(None);
    }

    let dir = resolve_app_data_dir(&app).map_err(|e| AppError::Io(e.to_string()))?;
    let path = dir.join(MIGRATION_SENTINEL_FILE);
    if !path.is_file() {
        return Ok(None);
    }

    let body = fs::read_to_string(&path).unwrap_or_default();
    let migrated_from = body
        .lines()
        .find(|l| l.starts_with("migrated_from="))
        .map(|l| l.trim_start_matches("migrated_from=").to_owned())
        .unwrap_or_default();

    let msg = format!(
        "Your notes and app database were copied from a preview install ({migrated_from}). \
         Layout preferences stored by the browser may have reset; your vault files were not moved."
    );

    Ok(Some(msg))
}

#[tauri::command]
pub async fn dismiss_app_data_migration_banner(
    db: State<'_, SqlitePool>,
    config: State<'_, SharedConfig>,
) -> AppResult<()> {
    let key = SETTING_MIGRATION_BANNER_DISMISSED.to_string();
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&key)
    .bind("1")
    .execute(db.inner())
    .await?;

    config.write().unwrap().apply_change(&key, "1");

    Ok(())
}
