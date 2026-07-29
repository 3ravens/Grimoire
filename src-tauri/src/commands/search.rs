use serde::Serialize;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tauri::State;
use crate::AppResult;
use crate::{KeyStore, SharedKeyStore};
use crate::AccessFilter;

// ---------------------------------------------------------------------------
// FTS index helpers â€” called by notes.rs and rag.rs
// ---------------------------------------------------------------------------

/// Insert or replace a note in the FTS index with plaintext title and content.
///
/// Always called with DECRYPTED text. Errors are silently swallowed because
/// FTS is a secondary index â€” a failure here must never break a note save.
/// The worst case is a stale FTS entry until the next Re-index all.
pub(crate) async fn fts_upsert(pool: &SqlitePool, id: i64, title: &str, content: &str) {
    // DELETE + INSERT is the correct upsert pattern for FTS5 self-contained tables.
    let _ = sqlx::query("DELETE FROM notes_fts WHERE rowid = ?")
        .bind(id)
        .execute(pool)
        .await;
    let _ = sqlx::query("INSERT INTO notes_fts(rowid, title, content) VALUES (?, ?, ?)")
        .bind(id)
        .bind(title)
        .bind(content)
        .execute(pool)
        .await;
}

/// Remove a note from the FTS index. Called on note deletion.
pub(crate) async fn fts_delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes_fts WHERE rowid = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remove every row from the note FTS index (vault lock / re-encrypt paths).
pub(crate) async fn fts_purge_all(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes_fts").execute(pool).await?;
    Ok(())
}

pub(crate) async fn fts_purge_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes_fts").execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn fts_delete_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes_fts WHERE rowid = ?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// FTS5 query building
// ---------------------------------------------------------------------------

/// Convert a raw user query string into a safe FTS5 MATCH expression.
///
/// Each whitespace-separated token becomes a quoted prefix term in the FTS5 query.
///
/// Quoting prevents injection of FTS5 operators (OR, AND, NOT). The `*` suffix
/// enables prefix matching so partial words find inflected forms — e.g. "gas"
/// matches "gases" and "gaseous", "run" matches "running" and "runner".
///
/// Example: `rust error` → `"rust"* "error"*`
pub fn build_fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter_map(|tok| {
            let clean = tok.replace('"', "");
            if clean.is_empty() { None } else { Some(format!("\"{clean}\"*")) }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// FTS search â€” result type and command
// ---------------------------------------------------------------------------

/// A single FTS result returned to the frontend.
///
/// `snippet` contains the matching fragment from the note content with matched
/// terms wrapped in `<b>â€¦</b>` tags. It comes from FTS5's `snippet()` function
/// which escapes all note content before adding the highlight tags, so it is
/// safe to render with `{@html}` in Svelte.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FtsResult {
    pub note_id: i64,
    pub title: String,
    pub folder_id: Option<i64>,
    pub snippet: String,
}

/// Run a full-text search and return up to `limit` results, ordered by BM25
/// relevance (best match first).
///
/// Since migration 0009, the FTS index is managed by Rust and always contains
/// plaintext (decrypted) content. Notes in locked folders are never inserted,
/// so the only filter needed here is a secondary safety check on folder lock state.
pub async fn fts_search_inner(
    pool: &SqlitePool,
    keys: &KeyStore,
    query: &str,
    limit: usize,
) -> AppResult<Vec<FtsResult>> {
    let fts_query = build_fts_query(query);
    if fts_query.is_empty() {
        return Ok(vec![]);
    }

    // Fetch more than limit so we have room to post-filter locked folders.
    let raw: Vec<FtsResult> = sqlx::query_as(
        r#"
        SELECT
            n.id            AS note_id,
            notes_fts.title AS title,
            n.folder_id,
            snippet(notes_fts, 1, '<b>', '</b>', '...', 32) AS snippet
        FROM notes_fts
        JOIN notes n ON n.id = notes_fts.rowid
        WHERE notes_fts MATCH ?
        ORDER BY bm25(notes_fts)
        LIMIT ?
        "#,
    )
    .bind(&fts_query)
    .bind((limit * 3) as i64)
    .fetch_all(pool)
    .await
    ?;

    // Secondary filter: exclude notes in folders that are currently locked.
    // FTS should not contain these (fts_upsert skips locked notes), but we
    // guard here in case of a race or stale entry.
    let filter = AccessFilter::load(pool, keys).await?;

    let results = raw
        .into_iter()
        .filter(|r| filter.is_accessible(r.folder_id))
        .take(limit)
        .collect();

    Ok(results)
}

/// Full-text search over note titles and content.
///
/// This is the fast path â€” it runs a pure SQLite query with no Ollama
/// involvement and returns in under a millisecond. Use this for immediate
/// results as the user types; fire `search_notes` separately for semantic
/// results and merge them on the frontend when they arrive.
#[tauri::command]
pub async fn fts_search(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<FtsResult>> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }
    let results = fts_search_inner(pool.inner(), keys.inner().as_ref(), &query, limit.unwrap_or(12)).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "search_fts", None, None, None, Some(&query),
    ).await;
    Ok(results)
}

// ---------------------------------------------------------------------------
// Combined search (FTS + semantic via RRF)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Startup FTS population
// ---------------------------------------------------------------------------

/// Called once at app startup.  Populates `notes_fts` for all notes that are
/// stored as plaintext — i.e. notes in folders with no password (`locked = 0`)
/// and notes with no folder.
///
/// Skips the sync entirely if:
/// - `notes_fts` already has rows (already up to date from a previous run), OR
/// - The vault has a password (meaning all notes may be vault-encrypted at the
///   time of this call; they cannot be decrypted without the vault key, which
///   is not available until the user unlocks. Those notes will be indexed the
///   next time the user edits them or runs Re-index All after unlock).
///
/// This function is idempotent and runs quickly — it is a handful of SQLite
/// queries, not an Ollama embed pass.
pub(crate) async fn fts_initial_sync(pool: &SqlitePool) {
    // If FTS already has content, nothing to do.
    let fts_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes_fts")
        .fetch_one(pool)
        .await
        .unwrap_or(1); // default to 1 to be safe — don't clobber an existing index
    if fts_count > 0 {
        return;
    }

    // If a vault password is set, notes are encrypted and cannot be indexed
    // without the vault key (which is only available after the user unlocks).
    let vault_has_password: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vault_lock")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if vault_has_password > 0 {
        return;
    }

    // Fetch all notes in non-locked folders (or no folder).
    // Since there is no vault password, these notes are stored as plaintext.
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT n.id, n.title, COALESCE(n.content, '')
         FROM notes n
         LEFT JOIN folders f ON f.id = n.folder_id
         WHERE f.locked IS NULL OR f.locked = 0",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (id, title, content) in rows {
        fts_upsert(pool, id, &title, &content).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use sqlx::sqlite::SqlitePoolOptions;

    use crate::KeyStore;

    #[test]
    fn build_fts_query_tokens_quoted_with_prefix() {
        assert_eq!(build_fts_query("rust error"), "\"rust\"* \"error\"*");
    }

    #[test]
    fn build_fts_query_strips_embedded_quotes_in_token() {
        assert_eq!(build_fts_query(r#"foo"bar"#), "\"foobar\"*");
    }

    #[test]
    fn build_fts_query_whitespace_only_empty() {
        assert_eq!(build_fts_query("   \t"), "");
    }

    #[test]
    fn rrf_score_matches_reciprocal_rank_fusion() {
        const RRF_K: f64 = 60.0;
        fn rrf_score(rank: usize) -> f64 {
            1.0 / (RRF_K + rank as f64)
        }
        let s1 = rrf_score(1);
        let s2 = rrf_score(2);
        assert!((s1 - 1.0 / (RRF_K + 1.0)).abs() < 1e-9);
        assert!(s2 < s1);
    }

    #[tokio::test]
    async fn fts_upsert_inserts_matching_row() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let folder_id: i64 =
            sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('Title', 'alpha beta', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        fts_upsert(&pool, note_id, "Title", "alpha beta").await;

        let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes_fts WHERE rowid = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(cnt, 1);
    }

    #[tokio::test]
    async fn fts_delete_removes_row() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let folder_id: i64 =
            sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('T', 'body', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        fts_upsert(&pool, note_id, "T", "body").await;
        fts_delete(&pool, note_id).await.unwrap();

        let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes_fts WHERE rowid = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(cnt, 0);
    }

    #[tokio::test]
    async fn fts_search_inner_filters_stale_fts_in_locked_folder_without_key() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        // locked=1 requires non-null salt/sentinel (folders_lock_chk_* in migration 0020).
        let folder_id: i64 = sqlx::query_scalar(
            "INSERT INTO folders (name, locked, salt, sentinel) VALUES ('secret', 1, 'testsalt', 'testsentinel') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('x', 'staleuniquephrase', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        // Simulate stale FTS row (e.g. race) — real saves skip fts_upsert for locked notes.
        fts_upsert(&pool, note_id, "x", "staleuniquephrase").await;

        let ks = KeyStore {
            vault_key: Mutex::new(None),
            folder_keys: Mutex::new(HashMap::new()),
        };

        let hits = fts_search_inner(&pool, &ks, "staleuniquephrase", 5)
            .await
            .unwrap();
        assert!(hits.is_empty(), "locked folder without session key must not surface hits");
    }

    #[tokio::test]
    async fn fts_search_inner_returns_stale_hit_when_folder_key_present() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let folder_id: i64 = sqlx::query_scalar(
            "INSERT INTO folders (name, locked, salt, sentinel) VALUES ('secret', 1, 'testsalt', 'testsentinel') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('x', 'visiblestaletoken', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        fts_upsert(&pool, note_id, "x", "visiblestaletoken").await;

        let mut folder_keys = HashMap::new();
        folder_keys.insert(folder_id, [9u8; 32]);
        let ks = KeyStore {
            vault_key: Mutex::new(None),
            folder_keys: Mutex::new(folder_keys),
        };

        let hits = fts_search_inner(&pool, &ks, "visiblestaletoken", 5)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].note_id, note_id);
    }

    #[tokio::test]
    async fn simulating_delete_note_removes_fts_row() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let folder_id: i64 =
            sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('T', 'gone', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        fts_upsert(&pool, note_id, "T", "gone").await;

        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(note_id)
            .execute(&pool)
            .await
            .unwrap();
        fts_delete(&pool, note_id).await.unwrap();

        let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes_fts WHERE rowid = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(cnt, 0);
    }
}
