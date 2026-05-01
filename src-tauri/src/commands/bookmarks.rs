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

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;
use crate::AppResult;

// ---------------------------------------------------------------------------
// Bookmarks
// ---------------------------------------------------------------------------

/// A bookmarked note as returned to the frontend.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BookmarkEntry {
    pub note_id: i64,
    pub title: String,
}

/// Return all bookmarks joined with their note titles, ordered by insertion time.
#[tauri::command]
pub async fn list_bookmarks(pool: State<'_, SqlitePool>) -> AppResult<Vec<BookmarkEntry>> {
    sqlx::query_as::<_, BookmarkEntry>(
        "SELECT b.note_id, n.title
         FROM bookmarks b
         JOIN notes n ON n.id = b.note_id
         ORDER BY b.added_at ASC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(Into::into)
}

/// Add a note to bookmarks. Does nothing if it is already bookmarked.
#[tauri::command]
pub async fn add_bookmark(pool: State<'_, SqlitePool>, note_id: i64) -> AppResult<()> {
    sqlx::query("INSERT OR IGNORE INTO bookmarks (note_id) VALUES (?)")
        .bind(note_id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(Into::into)
}

/// Remove a note from bookmarks. Does nothing if it was not bookmarked.
#[tauri::command]
pub async fn remove_bookmark(pool: State<'_, SqlitePool>, note_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM bookmarks WHERE note_id = ?")
        .bind(note_id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(Into::into)
}
