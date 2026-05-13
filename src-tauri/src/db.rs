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

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tauri::{AppHandle, Manager};

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool, sqlx::Error> {
    // Resolve a path inside the app's data directory, e.g.:
    // C:\Users\<user>\AppData\Roaming\grimoire\grimoire.db
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("could not resolve app data directory");

    std::fs::create_dir_all(&app_dir).expect("could not create app data directory");

    let db_path = app_dir.join("grimoire.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // Enable WAL journal mode. This allows reads and writes to proceed concurrently
    // instead of serialising — eliminates the 5s write-queue delays under load.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
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
        let _ = std::fs::create_dir_all(parent);
    }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    forget_removed_migration_16_if_present(&pool).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

