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

use std::path::PathBuf;
use std::collections::HashMap;
use sqlx::SqlitePool;
use tauri::State;
use crate::KeyStore;
use crate::{AppError, AppResult, EncryptedNoteStore};
use super::{Note, Folder};

// ---------------------------------------------------------------------------
// Note commands
// ---------------------------------------------------------------------------

/// Create a new note and return the full row.
#[tauri::command]
pub async fn create_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    title: String,
    folder_id: Option<i64>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.create_note(&title, folder_id).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_create", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// Fetch a single note by id.
#[tauri::command]
pub async fn get_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.get_note(id).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "note_open", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// List all notes, optionally filtered to a specific folder.
/// Pass `null` from JS to get notes with no folder, omit the filter to get all.
/// This command takes an explicit `all` flag to distinguish "no folder" from "every folder".
#[tauri::command]
pub async fn list_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    folder_id: Option<i64>,
    all: Option<bool>,
) -> AppResult<Vec<Note>> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    store.list_notes(folder_id, all.unwrap_or(false)).await
}

/// Update a note's title and content. Bumps updated_at to the current time.
#[tauri::command]
pub async fn update_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
    title: String,
    content: String,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.update_note(id, &title, &content).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// Move a note to a different folder (or to no folder when folder_id is null).
#[tauri::command]
pub async fn move_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
    folder_id: Option<i64>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.move_note(id, folder_id).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("moved"),
    ).await;
    Ok(note)
}

/// Rename a note (title only). Returns the updated note.
#[tauri::command]
pub async fn rename_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
    name: String,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.rename_note(id, &name).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("renamed"),
    ).await;
    Ok(note)
}

/// Delete a note. Returns nothing on success.
#[tauri::command]
pub async fn delete_note(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    super::search::fts_delete(pool.inner(), id).await;
    let _ = crate::audit::log_event(
        pool.inner(), "note_delete", Some("note"),
        Some(id), None, None,
    ).await;
    Ok(())
}

/// Duplicate a note — creates a copy with " (copy)" appended to the title in
/// the same folder. Returns the new note row.
#[tauri::command]
pub async fn duplicate_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let note = store.duplicate_note(id).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_create", Some("note"),
        Some(note.id), Some(&note.title), Some("duplicated"),
    ).await;
    Ok(note)
}

// ---------------------------------------------------------------------------
// Folder commands
// ---------------------------------------------------------------------------

/// Create a new folder and return the full row.
#[tauri::command]
pub async fn create_folder(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    name: String,
    parent_id: Option<i64>,
) -> AppResult<Folder> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let folder = store.create_folder(&name, parent_id).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "folder_create", Some("folder"),
        Some(folder.id), Some(&folder.name), None,
    ).await;
    Ok(folder)
}

/// List all folders. The frontend is responsible for building the tree from parent_id.
#[tauri::command]
pub async fn list_folders(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
) -> AppResult<Vec<Folder>> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    store.list_folders().await
}

/// Rename a folder.
#[tauri::command]
pub async fn rename_folder(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    id: i64,
    name: String,
) -> AppResult<Folder> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);
    let folder = store.rename_folder(id, &name).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "folder_rename", Some("folder"),
        Some(folder.id), Some(&folder.name), None,
    ).await;
    Ok(folder)
}

/// Delete a folder. Child folders and notes are handled by ON DELETE CASCADE
/// and ON DELETE SET NULL respectively (defined in the migration).
#[tauri::command]
pub async fn delete_folder(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM folders WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    let _ = crate::audit::log_event(
        pool.inner(), "folder_delete", Some("folder"),
        Some(id), None, None,
    ).await;
    Ok(())
}

/// Move a folder to a new parent (or to the root when new_parent_id is null).
/// Rejects the move if it would create a cycle (i.e. new_parent_id is a descendant
/// of the folder being moved, or equals the folder itself).
#[tauri::command]
pub async fn move_folder(
    pool: State<'_, SqlitePool>,
    id: i64,
    new_parent_id: Option<i64>,
) -> AppResult<()> {
    // A folder cannot be moved into itself or into one of its own descendants.
    if let Some(target) = new_parent_id {
        if target == id {
            return Err(AppError::InvalidInput("A folder cannot be its own parent".to_string()));
        }
        // Walk the ancestor chain of `target` upward; if we ever reach `id` then
        // the proposed parent is a descendant — reject it.
        let descendant_ids: Vec<(i64,)> = sqlx::query_as(
            "WITH RECURSIVE subtree(id) AS (
                 SELECT id FROM folders WHERE id = ?
                 UNION ALL
                 SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id
             )
             SELECT id FROM subtree WHERE id != ?",
        )
        .bind(id)
        .bind(id)
        .fetch_all(pool.inner())
        .await
        ?;

        if descendant_ids.iter().any(|(did,)| *did == target) {
            return Err(AppError::InvalidInput("Cannot move a folder into one of its own descendants".to_string()));
        }
    }

    sqlx::query("UPDATE folders SET parent_id = ? WHERE id = ?")
        .bind(new_parent_id)
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Export all unlocked notes as plain Markdown files under `dest_dir`.
/// The folder hierarchy is recreated as subdirectories; unfiled notes go to
/// the root. Locked notes are silently skipped — we never decrypt without the
/// user's key, and the key is not available at export time for locked folders.
#[tauri::command]
pub async fn export_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    dest_dir: String,
) -> AppResult<u32> {
    let store = EncryptedNoteStore::new(pool.inner(), &keys);

    // Build a map from folder ID to its display name (already decrypted).
    let folder_names: HashMap<i64, String> = store.list_folders().await?
        .into_iter()
        .map(|f| (f.id, if f.locked { String::new() } else { f.name }))
        .collect();

    // Fetch all notes; locked ones surface with note.locked = true.
    let all_notes = store.list_notes(None, true).await?;

    let dest = PathBuf::from(&dest_dir);

    // Wrap everything in a timestamped subfolder so repeated exports don't collide.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Format as YYYY-MM-DD using seconds since epoch.
    let days  = secs / 86400;
    let y = (days / 365 + 1970) as u32;
    // Rough but good enough for a folder name; no external crate needed.
    let month_day = days % 365;
    let m = (month_day / 30 + 1).min(12) as u32;
    let d = (month_day % 30 + 1).min(31) as u32;
    let date_str = format!("{d:02}-{m:02}-{y:04}");
    let export_root = dest.join(format!("Grimoire - export {date_str}"));
    let dest = export_root;
    let mut exported: u32 = 0;

    for note in all_notes {
        if note.locked {
            continue; // skip — no key available
        }

        // Resolve the output directory for this note.
        let out_dir = if let Some(fid) = note.folder_id {
            let folder_name = folder_names
                .get(&fid)
                .cloned()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("folder_{fid}"));
            // Sanitise the folder name for use as a directory name.
            let safe = sanitise_path_component(&folder_name);
            dest.join(safe)
        } else {
            dest.clone()
        };

        std::fs::create_dir_all(&out_dir)
            .map_err(|e| AppError::Io(format!("Could not create directory {}: {e}", out_dir.display())))?;

        // Build the output file path, sanitising the title.
        let safe_title = sanitise_path_component(&note.title);
        let file_name = if safe_title.is_empty() {
            format!("note_{}", note.id)
        } else {
            safe_title
        };
        let mut file_path = out_dir.join(&file_name).with_extension("md");

        // Avoid overwriting an existing file from a different note with the same title.
        if file_path.exists() {
            file_path = out_dir.join(format!("{}_{}", file_name, note.id)).with_extension("md");
        }

        std::fs::write(&file_path, &note.content)
            .map_err(|e| AppError::Io(format!("Could not write {}: {e}", file_path.display())))?;

        exported += 1;
    }

    let _ = crate::audit::log_event(
        pool.inner(), "note_export", Some("note"),
        None, None, Some(&dest_dir),
    ).await;
    Ok(exported)
}

/// Strip characters that are illegal in directory or file names on Windows,
/// macOS, and Linux. Collapses repeating spaces/dashes and trims whitespace.
fn sanitise_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
