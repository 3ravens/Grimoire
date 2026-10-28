//! Per-folder note pins for the note list panel.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::access_filter::AccessFilter;
use crate::{AppError, AppResult, SharedKeyStore};

/// A folder pin as returned to the frontend (ids + order only).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FolderPinEntry {
    pub note_id: i64,
    pub position: i64,
}

/// Return pins for a folder, ordered by position.
///
/// Joins notes so orphan rows (folder_id mismatch) are omitted. Inaccessible
/// folders yield an empty list.
#[tauri::command]
pub async fn list_folder_pins(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    folder_id: i64,
) -> AppResult<Vec<FolderPinEntry>> {
    let filter = AccessFilter::load(pool.inner(), keys.inner().as_ref()).await?;
    if !filter.is_accessible(Some(folder_id)) {
        return Ok(vec![]);
    }

    sqlx::query_as::<_, FolderPinEntry>(
        "SELECT fp.note_id, fp.position
         FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ?
           AND n.folder_id = fp.folder_id
         ORDER BY fp.position ASC",
    )
    .bind(folder_id)
    .fetch_all(pool.inner())
    .await
    .map_err(Into::into)
}

/// Pin a note in the expected folder. `folder_id` must match the note's current folder.
#[tauri::command]
pub async fn pin_folder_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    folder_id: i64,
) -> AppResult<()> {
    let filter = AccessFilter::load(pool.inner(), keys.inner().as_ref()).await?;
    if !filter.is_accessible(Some(folder_id)) {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    let mut tx = pool.begin().await?;

    let note_folder: Option<Option<i64>> =
        sqlx::query_scalar("SELECT folder_id FROM notes WHERE id = ?")
            .bind(note_id)
            .fetch_optional(&mut *tx)
            .await?;

    let Some(note_folder) = note_folder else {
        return Err(AppError::NotFound(format!("Note {note_id} not found")));
    };

    if note_folder != Some(folder_id) {
        return Err(AppError::InvalidInput(
            "Note is not in that folder".to_string(),
        ));
    }

    if !filter.is_accessible(note_folder) {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    let max_pos: Option<i64> =
        sqlx::query_scalar("SELECT MAX(position) FROM folder_pins WHERE folder_id = ?")
            .bind(folder_id)
            .fetch_one(&mut *tx)
            .await?;
    let next = max_pos.unwrap_or(-1) + 1;

    sqlx::query(
        "INSERT OR IGNORE INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, ?)",
    )
    .bind(note_id)
    .bind(folder_id)
    .bind(next)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Remove a pin only when it belongs to the expected folder.
#[tauri::command]
pub async fn unpin_folder_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    folder_id: i64,
) -> AppResult<()> {
    let filter = AccessFilter::load(pool.inner(), keys.inner().as_ref()).await?;
    if !filter.is_accessible(Some(folder_id)) {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    sqlx::query("DELETE FROM folder_pins WHERE note_id = ? AND folder_id = ?")
        .bind(note_id)
        .bind(folder_id)
        .execute(pool.inner())
        .await?;
    Ok(())
}

/// Replace pin order for a folder. `ordered_note_ids` must be exactly the current pin set.
#[tauri::command]
pub async fn reorder_folder_pins(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    folder_id: i64,
    ordered_note_ids: Vec<i64>,
) -> AppResult<()> {
    let filter = AccessFilter::load(pool.inner(), keys.inner().as_ref()).await?;
    if !filter.is_accessible(Some(folder_id)) {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    let mut tx = pool.begin().await?;

    let current: Vec<i64> = sqlx::query_scalar(
        "SELECT fp.note_id
         FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ?
           AND n.folder_id = fp.folder_id
         ORDER BY fp.position ASC",
    )
    .bind(folder_id)
    .fetch_all(&mut *tx)
    .await?;

    if current.len() != ordered_note_ids.len() {
        return Err(AppError::InvalidInput(
            "Pin order must include exactly the current pins".to_string(),
        ));
    }

    let mut sorted_cur = current.clone();
    let mut sorted_req = ordered_note_ids.clone();
    sorted_cur.sort_unstable();
    sorted_req.sort_unstable();
    if sorted_cur != sorted_req {
        return Err(AppError::InvalidInput(
            "Pin order must include exactly the current pins".to_string(),
        ));
    }

    for (i, nid) in ordered_note_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE folder_pins SET position = ? WHERE note_id = ? AND folder_id = ?",
        )
        .bind(i as i64)
        .bind(nid)
        .bind(folder_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
