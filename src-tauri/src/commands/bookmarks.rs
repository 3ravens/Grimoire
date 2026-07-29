
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
