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

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

fn app_sqlite_options(db_path: &std::path::Path) -> Result<SqliteConnectOptions, sqlx::Error> {
    let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    Ok(
        SqliteConnectOptions::from_str(&url)
            .map_err(|e| sqlx::Error::Configuration(e.into()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_millis(10_000)),
    )
}

/// Errors resolving paths, creating directories, or opening/migrating SQLite.
#[derive(Debug)]
pub enum DbInitError {
    AppDataDir(tauri::Error),
    Io(std::io::Error),
    Sqlx(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
}

impl std::fmt::Display for DbInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbInitError::AppDataDir(e) => {
                write!(f, "could not resolve app data directory: {e}")
            }
            DbInitError::Io(e) => write!(f, "could not create app data directory: {e}"),
            DbInitError::Sqlx(e) => write!(f, "database: {e}"),
            DbInitError::Migration(e) => write!(f, "database migration: {e}"),
        }
    }
}

impl std::error::Error for DbInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DbInitError::AppDataDir(e) => Some(e),
            DbInitError::Io(e) => Some(e),
            DbInitError::Sqlx(e) => Some(e),
            DbInitError::Migration(e) => Some(e),
        }
    }
}

impl From<tauri::Error> for DbInitError {
    fn from(value: tauri::Error) -> Self {
        DbInitError::AppDataDir(value)
    }
}

impl From<std::io::Error> for DbInitError {
    fn from(value: std::io::Error) -> Self {
        DbInitError::Io(value)
    }
}

impl From<sqlx::Error> for DbInitError {
    fn from(value: sqlx::Error) -> Self {
        DbInitError::Sqlx(value)
    }
}

impl From<sqlx::migrate::MigrateError> for DbInitError {
    fn from(value: sqlx::migrate::MigrateError) -> Self {
        DbInitError::Migration(value)
    }
}

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool, DbInitError> {
    // Resolve a path inside the app's data directory, e.g.:
    // Windows: C:\Users\<user>\AppData\Roaming\com.grimoire.app\grimoire.db
    let app_dir = app.path().app_data_dir()?;

    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("grimoire.db");
    let options = app_sqlite_options(&db_path)?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Compatibility guard after removing migration v16 from the codebase.
    // Some local databases still have v16 recorded in _sqlx_migrations; with the
    // file gone, sqlx returns VersionMissing(16). Dropping that history row makes
    // migration metadata consistent again while leaving actual user data intact.
    forget_removed_migration_16_if_present(&pool).await?;

    // Run any pending migrations from the migrations/ folder.
    // sqlx tracks which ones have already been applied, so this is safe to call every startup.
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

async fn forget_removed_migration_16_if_present(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // _sqlx_migrations may not exist on a brand-new database yet.
    let has_migrations_table: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;

    if has_migrations_table.is_none() {
        return Ok(());
    }

    // Only remove the orphaned *queue* migration row. Do not delete version 16 when it
    // legitimately refers to `0016_wikipedia_fts` (or any other current v16 migration).
    sqlx::query(
        "DELETE FROM _sqlx_migrations WHERE version = 16 AND description LIKE '%wikipedia_queue%'",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Open a file-backed SQLite pool with the same pragmas and migrations as the
/// desktop app. Intended for debug `perf-budget` runs (not used in production).
#[cfg(debug_assertions)]
pub async fn open_sqlite_file(db_path: &std::path::Path) -> Result<SqlitePool, sqlx::Error> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            sqlx::Error::Configuration(format!("create_dir_all({}): {e}", parent.display()).into())
        })?;
    }
    let options = app_sqlite_options(db_path)?;
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    forget_removed_migration_16_if_present(&pool).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

