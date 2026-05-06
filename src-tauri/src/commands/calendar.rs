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

use std::collections::HashMap;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;
use crate::SharedKeyStore;
use crate::{AppResult, EncryptedNoteStore};
use super::Note;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Re-format an ISO date string (`YYYY-MM-DD`) into the user's preferred display
/// format. Returns the input unchanged for `YYYY-MM-DD` or any unrecognised format.
fn format_display_date(iso: &str, fmt: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() != 3 {
        return iso.to_string();
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    match fmt {
        "DD-MM-YYYY" => format!("{}-{}-{}", d, m, y),
        "MM-DD-YYYY" => format!("{}-{}-{}", m, d, y),
        _ => iso.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Per-day activity counts returned to the frontend for the heatmap.
///
/// `created`  = number of notes whose `created_at` falls on this day.
/// `modified` = number of notes whose `updated_at` falls on this day AND on a
///              different calendar day than their `created_at` (so a note is not
///              double-counted if it was created and saved on the same day).
#[derive(Debug, Serialize)]
pub struct ActivityDay {
    pub date: String, // YYYY-MM-DD (UTC)
    pub created: i64,
    pub modified: i64,
}

/// Internal SQL aggregate row — not exposed to the frontend.
#[derive(sqlx::FromRow)]
struct DateCount {
    date: String,
    cnt: i64,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Return per-day activity counts for the past 365 days.
///
/// Two queries are run: one counting created notes per day, one counting notes
/// that were modified on a different day than they were created. The results
/// are merged in Rust and returned sorted oldest-first.
///
/// Timestamps are stored as UTC Unix epoch integers, so all date grouping uses
/// SQLite's `date(ts, 'unixepoch')` which also returns UTC dates.
#[tauri::command]
pub async fn get_activity_heatmap(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<ActivityDay>> {
    let created: Vec<DateCount> = sqlx::query_as(
        "SELECT date(created_at, 'unixepoch') AS date, COUNT(*) AS cnt
         FROM notes
         WHERE created_at >= unixepoch('now', '-365 days')
         GROUP BY date
         ORDER BY date",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let modified: Vec<DateCount> = sqlx::query_as(
        "SELECT date(updated_at, 'unixepoch') AS date, COUNT(*) AS cnt
         FROM notes
         WHERE updated_at >= unixepoch('now', '-365 days')
           AND date(updated_at, 'unixepoch') != date(created_at, 'unixepoch')
         GROUP BY date
         ORDER BY date",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let mut map: HashMap<String, ActivityDay> = HashMap::new();

    for row in created {
        let entry = map.entry(row.date.clone()).or_insert(ActivityDay {
            date: row.date,
            created: 0,
            modified: 0,
        });
        entry.created += row.cnt;
    }

    for row in modified {
        let entry = map.entry(row.date.clone()).or_insert(ActivityDay {
            date: row.date,
            created: 0,
            modified: 0,
        });
        entry.modified += row.cnt;
    }

    let mut days: Vec<ActivityDay> = map.into_values().collect();
    days.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(days)
}

/// Return all notes that were created or last modified on the given calendar day.
///
/// `date_str` must be a UTC date in `YYYY-MM-DD` format.
/// Locked-folder notes are returned as locked stubs (no title/content), matching
/// the behaviour of `list_notes`.
#[tauri::command]
pub async fn get_notes_for_day(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_str: String,
) -> AppResult<Vec<Note>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    store.list_notes_for_day(&date_str).await
}

/// Find the daily note for `date_str` inside the "Daily Notes" folder, creating
/// both the folder and the note if they do not yet exist.
///
/// The stored note title is the date formatted according to `date_format` (e.g.
/// `DD-MM-YYYY` → `"06-04-2026"`), encrypted with the vault key when one is active.
/// Legacy notes stored as raw ISO (`YYYY-MM-DD`) are matched as a fallback.
///
/// Because AES-GCM is non-deterministic we cannot query by encrypted title directly.
/// Instead, all notes in the folder are fetched and decrypted in Rust to find the
/// match — at most ~365 notes for a year of daily use, so this is acceptable.
#[tauri::command]
pub async fn get_or_create_daily_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_str: String,
    date_format: Option<String>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let fmt = date_format.as_deref().unwrap_or("DD-MM-YYYY");
    let display_title = format_display_date(&date_str, fmt);


    let folder_id: i64 = if let Some(id) = store.find_root_folder_by_name("Daily Notes").await? {
        id
    } else {
        store.create_folder("Daily Notes", None).await?.id
    };

    let notes = store.list_notes(Some(folder_id), false).await?;
    if let Some(note) = notes.into_iter().find(|n| n.title == display_title || n.title == date_str) {
        return Ok(note);
    }

    let note = store.create_note(&display_title, Some(folder_id)).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    Ok(note)
}

/// Always create a new daily note for today. If a note with today's title already
/// exists, append ` (2)`, ` (3)`, etc. until a free title is found.
///
/// Unlike `get_or_create_daily_note` (which opens the existing note), this command
/// is bound to the activity bar button and never re-opens an existing note.
#[tauri::command]
pub async fn create_daily_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_format: Option<String>,
) -> AppResult<Note> {
    // Get today's date in ISO format from SQLite (UTC).
    let (iso_date,): (String,) = sqlx::query_as("SELECT date('now')")
        .fetch_one(pool.inner())
        .await
        ?;

    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let fmt = date_format.as_deref().unwrap_or("DD-MM-YYYY");
    let base_title = format_display_date(&iso_date, fmt);

    let folder_id: i64 = if let Some(id) = store.find_root_folder_by_name("Daily Notes").await? {
        id
    } else {
        store.create_folder("Daily Notes", None).await?.id
    };

    let existing_titles: Vec<String> = store
        .list_notes(Some(folder_id), false).await?
        .into_iter()
        .map(|n| n.title)
        .collect();

    let final_title = if !existing_titles.contains(&base_title) {
        base_title
    } else {
        let mut n = 2u32;
        loop {
            let candidate = format!("{} ({})", base_title, n);
            if !existing_titles.contains(&candidate) {
                break candidate;
            }
            n += 1;
        }
    };

    let note = store.create_note(&final_title, Some(folder_id)).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    Ok(note)
}
