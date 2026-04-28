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
use crate::KeyStore;

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
pub(crate) async fn fts_delete(pool: &SqlitePool, id: i64) {
    let _ = sqlx::query("DELETE FROM notes_fts WHERE rowid = ?")
        .bind(id)
        .execute(pool)
        .await;
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
fn build_fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter_map(|tok| {
            let clean = tok.replace('"', "");
            if clean.is_empty() { None } else { Some(format!("\"{clean}\"*")) }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Locked-folder exclusion helper
// ---------------------------------------------------------------------------

/// Returns the set of folder IDs that are password-protected AND have no
/// session key currently held (i.e. they appear locked to this session).
async fn locked_folder_ids(pool: &SqlitePool, keys: &KeyStore) -> Vec<i64> {
    let unlocked_ids: Vec<i64> = keys
        .folder_keys
        .lock()
        .map(|fk| fk.keys().copied().collect())
        .unwrap_or_default();

    let all_locked: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM folders WHERE locked = 1")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    all_locked
        .into_iter()
        .filter(|id| !unlocked_ids.contains(id))
        .collect()
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
async fn fts_search_inner(
    pool: &SqlitePool,
    keys: &KeyStore,
    query: &str,
    limit: usize,
) -> Result<Vec<FtsResult>, String> {
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
    .map_err(|e| e.to_string())?;

    // Secondary filter: exclude notes in folders that are currently locked.
    // FTS should not contain these (fts_upsert skips locked notes), but we
    // guard here in case of a race or stale entry.
    let locked = locked_folder_ids(pool, keys).await;

    let results = raw
        .into_iter()
        .filter(|r| {
            r.folder_id
                .map(|fid| !locked.contains(&fid))
                .unwrap_or(true)
        })
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
    keys: State<'_, KeyStore>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<FtsResult>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }
    let results = fts_search_inner(pool.inner(), &keys, &query, limit.unwrap_or(12)).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "search_fts", None, None, None, Some(&query),
    ).await;
    Ok(results)
}

// ---------------------------------------------------------------------------
// Combined search (FTS + semantic via RRF)
// ---------------------------------------------------------------------------

/// A merged search result from both FTS and semantic search.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub note_id: i64,
    pub title: String,
    pub folder_id: Option<i64>,
    pub snippet: Option<String>,
    pub excerpt: Option<String>,
    pub matched_by: String,
    pub score: f64,
}

const RRF_K: f64 = 60.0;

fn rrf_score(rank: usize) -> f64 {
    1.0 / (RRF_K + rank as f64)
}

/// Search notes using both full-text (FTS5) and semantic (LanceDB) search,
/// merged via Reciprocal Rank Fusion.
///
/// Both searches run concurrently. If Ollama is unavailable the semantic half
/// silently returns no results and FTS results are returned alone.
///
/// For a faster UI experience, prefer calling `fts_search` for instant results
/// and `search_notes` for deferred semantic results, then merging on the
/// frontend with the same RRF logic.
#[tauri::command]
pub async fn combined_search(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let limit = limit.unwrap_or(10);
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }

    let fts_fut = fts_search_inner(pool.inner(), &keys, &query, limit * 2);

    let semantic_fut = async {
        let model = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'embedding_model' LIMIT 1")
            .fetch_optional(pool.inner())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "nomic-embed-text".to_string());
        match crate::commands::rag::embed_query(&query, &model).await {
            Ok(vec) => crate::vector::search(&vdb.0, vec, limit * 2).await.ok(),
            Err(_) => None,
        }
    };

    let (fts_rows, semantic_matches) = tokio::join!(fts_fut, semantic_fut);
    let fts_rows = fts_rows.unwrap_or_default();
    let semantic_matches = semantic_matches.unwrap_or_default();

    use std::collections::HashMap;

    struct Entry {
        title: String,
        folder_id: Option<i64>,
        snippet: Option<String>,
        excerpt: Option<String>,
        fts: bool,
        semantic: bool,
        score: f64,
    }

    let mut entries: HashMap<i64, Entry> = HashMap::new();

    for (rank, row) in fts_rows.iter().enumerate() {
        let e = entries.entry(row.note_id).or_insert(Entry {
            title: row.title.clone(),
            folder_id: row.folder_id,
            snippet: None,
            excerpt: None,
            fts: false,
            semantic: false,
            score: 0.0,
        });
        e.score += rrf_score(rank + 1);
        e.fts = true;
        e.snippet = Some(row.snippet.clone());
    }

    for (rank, m) in semantic_matches.iter().enumerate() {
        let e = entries.entry(m.note_id).or_insert(Entry {
            title: m.title.clone(),
            folder_id: None,
            snippet: None,
            excerpt: None,
            fts: false,
            semantic: false,
            score: 0.0,
        });
        e.score += rrf_score(rank + 1);
        e.semantic = true;
        if e.excerpt.is_none() {
            e.excerpt = m.excerpts.first().cloned();
        }
    }

    // Back-fill folder_id for semantic-only hits.
    let semantic_only_ids: Vec<i64> = entries
        .iter()
        .filter(|(_, e)| e.semantic && e.folder_id.is_none() && !e.fts)
        .map(|(id, _)| *id)
        .collect();

    if !semantic_only_ids.is_empty() {
        let placeholders = semantic_only_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT id, folder_id FROM notes WHERE id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (i64, Option<i64>)>(&sql);
        for id in &semantic_only_ids {
            q = q.bind(id);
        }
        if let Ok(pairs) = q.fetch_all(pool.inner()).await {
            for (id, fid) in pairs {
                if let Some(e) = entries.get_mut(&id) {
                    e.folder_id = fid;
                }
            }
        }
    }

    let mut results: Vec<SearchResult> = entries
        .into_iter()
        .map(|(note_id, e)| SearchResult {
            note_id,
            title: e.title,
            folder_id: e.folder_id,
            snippet: e.snippet,
            excerpt: e.excerpt,
            matched_by: match (e.fts, e.semantic) {
                (true, true) => "both",
                (true, false) => "fts",
                _ => "semantic",
            }
            .to_string(),
            score: e.score,
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    let _ = crate::audit::log_event(
        pool.inner(), "search_combined", None, None, None, Some(&query),
    ).await;
    Ok(results)
}

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
