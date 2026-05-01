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

//! Generic key/value settings persistence.
//!
//! Values are always stored as text; the frontend is responsible for
//! serialising and deserialising specific types (e.g. "true"/"false" for bools).

use sqlx::SqlitePool;
use tauri::State;
use crate::config::SharedConfig;
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
