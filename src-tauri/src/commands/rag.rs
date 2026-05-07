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

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use lancedb::Connection;
use serde::Serialize;
use sqlx::{QueryBuilder, SqlitePool};
use tauri::{Emitter, State};
use crate::folder_tree::folder_subtree_ids;
use crate::{AppError, AppResult, EncryptedNoteStore, SharedKeyStore};
use super::NoteRow;
use crate::chunking::{split_sentences, chunk_sentences};
use crate::config::SharedConfig;

/// CancelMap key for in-flight `reindex_all` (see `cancel_vault_reindex`).
pub const VAULT_REINDEX_CANCEL_KEY: &str = "__vault_notes_reindex__";

fn truncate_reindex_title(title: &str) -> String {
    const MAX: usize = 80;
    let t = title.trim();
    let mut it = t.chars();
    let head: String = it.by_ref().take(MAX).collect();
    if it.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

// ---------------------------------------------------------------------------
// RAG commands (vector index + semantic search)
// ---------------------------------------------------------------------------

// The embed model is configurable via the 'embedding_model' setting.
// nomic-embed-text requires asymmetric prefixes: documents are prefixed with
// "search_document: " and queries with "search_query: " for accurate retrieval.
// Other models may require different prefixes or none at all.

pub(crate) async fn embed_document(text: &str, model: &str) -> AppResult<Vec<f32>> {
    crate::vector::embed(&format!("search_document: {text}"), model).await.map_err(|e| AppError::EmbeddingFailed(e))
}

pub(crate) async fn embed_query(text: &str, model: &str) -> AppResult<Vec<f32>> {
    crate::vector::embed(&format!("search_query: {text}"), model).await.map_err(|e| AppError::EmbeddingFailed(e))
}

/// Build a "Properties: key=value, …" suffix for a note, to be appended to
/// the note content before embedding. Returns an empty string if the note has
/// no properties or no folder.
pub(crate) async fn build_properties_suffix(pool: &SqlitePool, note_id: i64) -> String {
    #[derive(sqlx::FromRow)]
    struct PV {
        name: String,
        value: String,
    }

    let pairs: Vec<PV> = sqlx::query_as(
        "SELECT pd.name, COALESCE(np.value, '') AS value
         FROM property_defs pd
         LEFT JOIN note_properties np ON np.def_id = pd.id AND np.note_id = ?
         WHERE pd.folder_id = (SELECT folder_id FROM notes WHERE id = ?)
         ORDER BY pd.position ASC",
    )
    .bind(note_id)
    .bind(note_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let filled: Vec<String> = pairs
        .into_iter()
        .filter(|p| !p.value.is_empty())
        .map(|p| format!("{}={}", p.name, p.value))
        .collect();

    if filled.is_empty() {
        String::new()
    } else {
        format!("\nProperties: {}", filled.join(", "))
    }
}

/// Chunk, embed, and upsert one note into LanceDB (shared by IPC `index_note`,
/// vault re-index, and folder-unlock background indexing).
pub(crate) async fn index_note_vectors_inner(
    pool: &SqlitePool,
    vdb: &Connection,
    model: &str,
    max_retries: i64,
    note_id: i64,
    title: &str,
    content: &str,
    embed_opts: crate::vector::EmbedBatchOptions,
) -> AppResult<()> {
    let props_suffix = build_properties_suffix(pool, note_id).await;
    let full_content = if props_suffix.is_empty() {
        content.to_string()
    } else {
        format!("{content}{props_suffix}")
    };

    let sentences = split_sentences(&full_content);
    let raw_chunks = chunk_sentences(sentences, 1, 0);

    if raw_chunks.iter().all(|c| c.trim().is_empty()) {
        return crate::vector::remove(vdb, note_id)
            .await
            .map_err(AppError::VectorStore);
    }

    let doc_texts: Vec<String> = raw_chunks
        .iter()
        .map(|chunk| format!("search_document: {title}\n{chunk}"))
        .collect();

    let use_simple_batch = embed_opts.on_slice_progress.is_none()
        && !embed_opts.skip_ollama_entry_eviction;

    let embeddings_result = if use_simple_batch {
        crate::retry::with_retries(max_retries, None, || async {
            crate::vector::embed_batch(&doc_texts, model)
                .await
                .map_err(AppError::EmbeddingFailed)
        })
        .await
    } else {
        crate::retry::with_retries(max_retries, None, || async {
            crate::vector::embed_batch_with_options(&doc_texts, model, embed_opts.clone())
                .await
                .map_err(AppError::EmbeddingFailed)
        })
        .await
    };

    let embeddings = match embeddings_result {
        Ok(e) => e,
        Err(_) => {
            let mut fallback = Vec::with_capacity(raw_chunks.len());
            for chunk in &raw_chunks {
                let emb = crate::retry::with_retries(max_retries, None, || async {
                    embed_document(chunk, model).await
                })
                .await?;
                fallback.push(emb);
            }
            fallback
        }
    };

    let chunks: Vec<(i32, String, Vec<f32>)> = raw_chunks
        .into_iter()
        .enumerate()
        .zip(embeddings)
        .map(|((i, chunk_text), embedding)| (i as i32, chunk_text, embedding))
        .collect();

    crate::retry::with_retries(max_retries, None, || {
        let ch = chunks.clone();
        async {
            crate::vector::upsert(vdb, note_id, title, ch)
                .await
                .map_err(AppError::VectorStore)
        }
    })
    .await?;
    Ok(())
}

/// Embed a note and store it in the vector index.
#[tauri::command]
pub async fn index_note(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    note_id: i64,
    title: String,
    content: String,
) -> AppResult<()> {
    let model = config.read().unwrap().embedding_model.clone();
    let max_retries = config.read().unwrap().background_max_retries;
    index_note_vectors_inner(
        pool.inner(),
        &vdb.0,
        &model,
        max_retries,
        note_id,
        &title,
        &content,
        crate::vector::EmbedBatchOptions::default(),
    )
    .await
}

/// Remove a note from the vector index. Called when a note is deleted.
#[tauri::command]
pub async fn remove_note_index(
    vdb: State<'_, crate::vector::VectorDb>,
    note_id: i64,
) -> AppResult<()> {
    crate::vector::remove(&vdb.0, note_id).await.map_err(|e| AppError::VectorStore(e))
}

/// Embed the query text and return the most semantically similar notes.
///
/// After the LanceDB search, results are cross-referenced with SQLite to:
/// 1. Filter out notes in folders that are currently locked (no session key).
/// 2. Replace the title stored in LanceDB with the current, decrypted title
///    from SQLite — this fixes stale ciphertext titles left over from notes
///    that were indexed before encryption was applied (or before migration 0009).
#[tauri::command]
pub async fn search_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<crate::vector::NoteMatch>> {
    let model = config.read().unwrap().embedding_model.clone();
    let embedding = embed_query(&query, &model).await?;
    let mut matches = crate::vector::search(
        &vdb.0,
        embedding,
        limit.unwrap_or(crate::vector::CHUNK_FETCH_LIMIT),
    )
    .await
    .map_err(|e| AppError::VectorStore(e))?;

    if matches.is_empty() {
        return Ok(matches);
    }

    // Cross-reference LanceDB hits with SQLite: filter locked folders, refresh titles.
    let ids: Vec<i64> = matches.iter().map(|m| m.note_id).collect();
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let accessible = store.accessible_note_titles(&ids).await?;

    // Drop locked/missing results; update titles with current decrypted values.
    matches.retain(|m| accessible.contains_key(&m.note_id));
    for m in &mut matches {
        if let Some(title) = accessible.get(&m.note_id) {
            m.title = title.clone();
        }
    }

    let _ = crate::audit::log_event(
        pool.inner(), "search_semantic", None, None, None, Some(&query),
    ).await;
    Ok(matches)
}

// ---------------------------------------------------------------------------
// Folder unlock — background incremental LanceDB indexing for subtree
// ---------------------------------------------------------------------------

/// Kick off embedding for every note under `root_folder_id` (recursive). Returns
/// immediately; progress is emitted on `folder_unlock_index:*` events.
#[tauri::command]
pub async fn start_folder_unlock_reindex(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    gate: State<'_, super::VaultReindexGate>,
    coord: State<'_, super::FolderUnlockReindexCoordinator>,
    root_folder_id: i64,
) -> AppResult<()> {
    let pool = pool.inner().clone();
    let keys = Arc::clone(keys.inner());
    let vdb = crate::vector::VectorDb(vdb.inner().0.clone());
    let config = Arc::clone(config.inner());
    let gate_mu = gate.inner().0.clone();
    let coord = coord.inner().clone();
    let cancel = coord.begin_job();

    tauri::async_runtime::spawn(async move {
        // Let the WebView process the unlock response and paint before we compete for the CPU.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        let _gate_hold = gate_mu.lock().await;
        run_folder_unlock_reindex_task(app, pool, keys, vdb, config, root_folder_id, cancel, coord)
            .await;
    });

    Ok(())
}

async fn run_folder_unlock_reindex_task(
    app: tauri::AppHandle,
    pool: SqlitePool,
    keys: SharedKeyStore,
    vdb: crate::vector::VectorDb,
    config: SharedConfig,
    root_folder_id: i64,
    cancel: Arc<AtomicBool>,
    coord: super::FolderUnlockReindexCoordinator,
) {
    let outcome = folder_unlock_reindex_task_inner(
        &app,
        &pool,
        &keys,
        &vdb.0,
        &config,
        root_folder_id,
        &cancel,
    )
    .await;

    coord.finish_job(&cancel);

    if let Err(e) = outcome {
        let _ = app.emit(
            "folder_unlock_index:error",
            serde_json::json!({
                "root_folder_id": root_folder_id,
                "message": e.to_string(),
            }),
        );
    }
}

async fn folder_unlock_reindex_task_inner(
    app: &tauri::AppHandle,
    pool: &SqlitePool,
    keys: &SharedKeyStore,
    vdb: &Connection,
    config: &SharedConfig,
    root_folder_id: i64,
    cancel: &AtomicBool,
) -> AppResult<()> {
    let vault_key_absent = keys
        .vault_key
        .lock()
        .map(|vk| vk.is_none())
        .unwrap_or(true);
    if vault_key_absent {
        let has_pw: bool = sqlx::query_scalar("SELECT COUNT(*) FROM vault_lock WHERE id = 1")
            .fetch_one(pool)
            .await
            .map(|n: i64| n > 0)
            .unwrap_or(false);
        if has_pw {
            let _ = app.emit(
                "folder_unlock_index:done",
                serde_json::json!({
                    "root_folder_id": root_folder_id,
                    "affected_folder_ids": Vec::<i64>::new(),
                    "total": 0usize,
                    "processed": 0usize,
                    "indexed_ok": 0usize,
                    "skipped_locked": 0usize,
                    "failed_notes": 0usize,
                    "cancelled": false,
                    "vault_blocked": true,
                }),
            );
            return Ok(());
        }
    }

    let subtree_ids = folder_subtree_ids(pool, root_folder_id).await?;
    if subtree_ids.is_empty() {
        let _ = app.emit(
            "folder_unlock_index:done",
            serde_json::json!({
                "root_folder_id": root_folder_id,
                "affected_folder_ids": Vec::<i64>::new(),
                "total": 0usize,
                "processed": 0usize,
                "indexed_ok": 0usize,
                "skipped_locked": 0usize,
                "failed_notes": 0usize,
                "cancelled": false,
            }),
        );
        return Ok(());
    }

    let affected_folder_ids: Vec<i64> = subtree_ids.clone();

    let mut qb = QueryBuilder::new("SELECT id FROM notes WHERE folder_id IN (");
    {
        let mut sep = qb.separated(", ");
        for fid in &subtree_ids {
            sep.push_bind(fid);
        }
    }
    qb.push(") ORDER BY id ASC");
    let note_rows: Vec<(i64,)> = qb.build_query_as().fetch_all(pool).await?;

    let total_usize = note_rows.len();
    let model = config.read().unwrap().embedding_model.clone();
    let max_retries = config.read().unwrap().background_max_retries;

    let mut indexed_ok = 0usize;
    let mut skipped_locked = 0usize;
    let mut failed_notes = 0usize;
    let mut cancelled = false;
    let mut completed = 0usize;

    let _ = app.emit(
        "folder_unlock_index:progress",
        serde_json::json!({
            "root_folder_id": root_folder_id,
            "affected_folder_ids": &affected_folder_ids,
            "processed": 0usize,
            "total": total_usize,
            "indexed_ok": 0usize,
            "skipped_locked": 0usize,
            "failed_notes": 0usize,
        }),
    );

    for (idx, (note_id,)) in note_rows.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        tokio::task::yield_now().await;

        let note_ord = idx + 1;
        let store = EncryptedNoteStore::new(pool, keys.as_ref());

        let note = match store.get_note(note_id).await {
            Ok(n) => n,
            Err(e) => {
                failed_notes += 1;
                completed += 1;
                log::warn!("folder unlock reindex: get_note {note_id}: {e}");
                let _ = app.emit(
                    "folder_unlock_index:progress",
                    serde_json::json!({
                        "root_folder_id": root_folder_id,
                        "affected_folder_ids": &affected_folder_ids,
                        "processed": completed,
                        "total": total_usize,
                        "indexed_ok": indexed_ok,
                        "skipped_locked": skipped_locked,
                        "failed_notes": failed_notes,
                    }),
                );
                continue;
            }
        };

        if note.locked {
            skipped_locked += 1;
            completed += 1;
            let _ = app.emit(
                "folder_unlock_index:progress",
                serde_json::json!({
                    "root_folder_id": root_folder_id,
                    "affected_folder_ids": &affected_folder_ids,
                    "processed": completed,
                    "total": total_usize,
                    "indexed_ok": indexed_ok,
                    "skipped_locked": skipped_locked,
                    "failed_notes": failed_notes,
                }),
            );
            continue;
        }

        super::search::fts_upsert(pool, note.id, &note.title, &note.content).await;

        let title_short = truncate_reindex_title(&note.title);
        let last_emit_chunks = Arc::new(AtomicUsize::new(0));
        let processed_ui = note_ord;
        let indexed_ui = indexed_ok;
        let skipped_snap = skipped_locked;
        let failed_snap = failed_notes;
        let embed_opts = crate::vector::EmbedBatchOptions {
            on_slice_progress: Some(Arc::new({
                let app = app.clone();
                let last_emit = last_emit_chunks.clone();
                let affected = affected_folder_ids.clone();
                move |ready, tot| {
                    let prev = last_emit.load(Ordering::Relaxed);
                    let min_step = (tot / 40).max(64).min(tot.max(1));
                    if ready > 0 && ready < tot && ready.saturating_sub(prev) < min_step {
                        return;
                    }
                    last_emit.store(ready, Ordering::Relaxed);
                    let _ = app.emit(
                        "folder_unlock_index:progress",
                        serde_json::json!({
                            "root_folder_id": root_folder_id,
                            "affected_folder_ids": affected,
                            "processed": processed_ui,
                            "total": total_usize,
                            "indexed_ok": indexed_ui,
                            "skipped_locked": skipped_snap,
                            "failed_notes": failed_snap,
                            "phase": "embedding",
                            "embedding_chunks": {
                                "done": ready,
                                "total": tot,
                                "note_title": title_short,
                            },
                        }),
                    );
                }
            })),
            ..Default::default()
        };

        match index_note_vectors_inner(
            pool,
            vdb,
            &model,
            max_retries,
            note_id,
            &note.title,
            &note.content,
            embed_opts,
        )
        .await
        {
            Ok(()) => indexed_ok += 1,
            Err(e) => {
                failed_notes += 1;
                log::warn!("folder unlock reindex note {note_id}: {e}");
            }
        }

        completed += 1;
        let _ = app.emit(
            "folder_unlock_index:progress",
            serde_json::json!({
                "root_folder_id": root_folder_id,
                "affected_folder_ids": &affected_folder_ids,
                "processed": completed,
                "total": total_usize,
                "indexed_ok": indexed_ok,
                "skipped_locked": skipped_locked,
                "failed_notes": failed_notes,
            }),
        );
    }

    if failed_notes > 0 {
        let _ = app.emit(
            "folder_unlock_index:error",
            serde_json::json!({
                "root_folder_id": root_folder_id,
                "message": format!(
                    "{failed_notes} note(s) could not be indexed for semantic search after retries."
                ),
                "failed_notes": failed_notes,
            }),
        );
    }

    let _ = app.emit(
        "folder_unlock_index:done",
        serde_json::json!({
            "root_folder_id": root_folder_id,
            "affected_folder_ids": &affected_folder_ids,
            "total": total_usize,
            "processed": completed,
            "indexed_ok": indexed_ok,
            "skipped_locked": skipped_locked,
            "failed_notes": failed_notes,
            "cancelled": cancelled,
        }),
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Vault re-index checkpointing (SQLite queue + cursor; resume after crash)
// ---------------------------------------------------------------------------

/// Invalidate persisted vault re-index progress. Call whenever the notes Lance
/// table is dropped (`clear_notes_index`, vault lock purge) so we never resume
/// against an empty or wrong-shaped index.
pub async fn clear_vault_reindex_checkpoint(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM vault_reindex_queue")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM vault_reindex_state WHERE id = 1")
        .execute(pool)
        .await?;
    Ok(())
}

async fn expected_unlocked_reindex_note_ids(store: &EncryptedNoteStore<'_>) -> AppResult<Vec<i64>> {
    let mut notes = store.list_notes(None, true).await?;
    notes.sort_by_key(|n| n.id);
    Ok(notes
        .into_iter()
        .filter(|n| !n.locked)
        .map(|n| n.id)
        .collect())
}

async fn rebuild_vault_reindex_checkpoint(
    pool: &SqlitePool,
    store: &EncryptedNoteStore<'_>,
    embedding_model: &str,
) -> AppResult<i64> {
    let ids = expected_unlocked_reindex_note_ids(store).await?;
    let total = ids.len() as i64;

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM vault_reindex_queue")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM vault_reindex_state WHERE id = 1")
        .execute(&mut *tx)
        .await?;

    let started = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO vault_reindex_state (id, embedding_model, started_at, next_pos, total, indexed_ok)
         VALUES (1, ?, ?, 0, ?, 0)",
    )
    .bind(embedding_model)
    .bind(&started)
    .bind(total)
    .execute(&mut *tx)
    .await?;

    const BATCH: usize = 500;
    let mut offset = 0usize;
    while offset < ids.len() {
        let end = (offset + BATCH).min(ids.len());
        let mut qb: QueryBuilder<'_, sqlx::Sqlite> =
            QueryBuilder::new("INSERT INTO vault_reindex_queue (pos, note_id) ");
        qb.push_values(ids[offset..end].iter().enumerate(), |mut b, (i, note_id)| {
            let pos = (offset + i) as i64;
            b.push_bind(pos).push_bind(*note_id);
        });
        qb.build().execute(&mut *tx).await?;
        offset = end;
    }
    tx.commit().await?;
    Ok(total)
}

/// Returns `(next_pos, total, indexed_ok)` when the checkpoint matches `model`,
/// the queue row count matches `total`, and the queued note ids match the current
/// set of unlockable notes (otherwise the checkpoint is stale and cleared).
async fn vault_reindex_resume_snapshot(
    pool: &SqlitePool,
    store: &EncryptedNoteStore<'_>,
    model: &str,
) -> AppResult<Option<(i64, i64, i64)>> {
    let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT embedding_model, next_pos, total, indexed_ok FROM vault_reindex_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some((emb, next_pos, total, indexed_ok)) = row else {
        return Ok(None);
    };
    if emb != model || total <= 0 {
        clear_vault_reindex_checkpoint(pool).await?;
        return Ok(None);
    }
    if next_pos >= total {
        clear_vault_reindex_checkpoint(pool).await?;
        return Ok(None);
    }
    let queue_n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vault_reindex_queue")
        .fetch_one(pool)
        .await?;
    if queue_n != total {
        clear_vault_reindex_checkpoint(pool).await?;
        return Ok(None);
    }

    let expected = expected_unlocked_reindex_note_ids(store).await?;
    if expected.len() as i64 != total {
        clear_vault_reindex_checkpoint(pool).await?;
        return Ok(None);
    }
    let queued: Vec<i64> = sqlx::query_scalar("SELECT note_id FROM vault_reindex_queue ORDER BY pos ASC")
        .fetch_all(pool)
        .await?;
    if queued != expected {
        clear_vault_reindex_checkpoint(pool).await?;
        return Ok(None);
    }

    Ok(Some((next_pos, total, indexed_ok)))
}

/// True when there is no checkpoint row, or the queue matches the current set of
/// unlockable note ids (same length and same ordered ids).
async fn vault_reindex_checkpoint_queue_consistent(
    pool: &SqlitePool,
    store: &EncryptedNoteStore<'_>,
) -> AppResult<bool> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT next_pos, total FROM vault_reindex_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some((_next_pos, total)) = row else {
        return Ok(true);
    };
    if total <= 0 {
        return Ok(true);
    }
    let queue_n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vault_reindex_queue")
        .fetch_one(pool)
        .await?;
    if queue_n != total {
        return Ok(false);
    }
    let expected = expected_unlocked_reindex_note_ids(store).await?;
    if expected.len() as i64 != total {
        return Ok(false);
    }
    let queued: Vec<i64> = sqlx::query_scalar("SELECT note_id FROM vault_reindex_queue ORDER BY pos ASC")
        .fetch_all(pool)
        .await?;
    Ok(queued == expected)
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultReindexStatus {
    pub incomplete: bool,
    pub next_pos: i64,
    pub total: i64,
    pub indexed_ok: i64,
    pub embedding_model: String,
    pub started_at: String,
}

#[tauri::command]
pub async fn vault_reindex_status(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> AppResult<VaultReindexStatus> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    if !vault_reindex_checkpoint_queue_consistent(pool.inner(), &store).await? {
        clear_vault_reindex_checkpoint(pool.inner()).await?;
        return Ok(VaultReindexStatus {
            incomplete: false,
            next_pos: 0,
            total: 0,
            indexed_ok: 0,
            embedding_model: String::new(),
            started_at: String::new(),
        });
    }

    let row: Option<(String, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT embedding_model, next_pos, total, indexed_ok, started_at
         FROM vault_reindex_state WHERE id = 1",
    )
    .fetch_optional(pool.inner())
    .await?;
    match row {
        None => Ok(VaultReindexStatus {
            incomplete: false,
            next_pos: 0,
            total: 0,
            indexed_ok: 0,
            embedding_model: String::new(),
            started_at: String::new(),
        }),
        Some((embedding_model, next_pos, total, indexed_ok, started_at)) => {
            let incomplete = next_pos < total && total > 0;
            Ok(VaultReindexStatus {
                incomplete,
                next_pos,
                total,
                indexed_ok,
                embedding_model,
                started_at,
            })
        }
    }
}

/// Request cooperative cancellation of an in-flight `reindex_all`.
#[tauri::command]
pub async fn cancel_vault_reindex(cancel_map: State<'_, super::CancelMap>) -> AppResult<()> {
    let map = cancel_map
        .0
        .lock()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;
    if let Some(flag) = map.get(VAULT_REINDEX_CANCEL_KEY) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Discard checkpoint/queue so the next `reindex_all` starts fresh. Fails if a
/// re-index is currently holding the gate.
#[tauri::command]
pub async fn abandon_vault_reindex(
    pool: State<'_, SqlitePool>,
    gate: State<'_, super::VaultReindexGate>,
) -> AppResult<()> {
    if gate.0.try_lock().is_err() {
        return Err(AppError::InvalidInput(
            "Cannot abandon while a re-index is running".to_string(),
        ));
    }
    clear_vault_reindex_checkpoint(pool.inner()).await
}

/// Re-index every note into LanceDB. Resumes from SQLite checkpoint when possible
/// unless `force_restart` is true.
#[tauri::command]
pub async fn reindex_all(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    cancel_map: State<'_, super::CancelMap>,
    gate: State<'_, super::VaultReindexGate>,
    force_restart: Option<bool>,
) -> AppResult<String> {
    let _hold = gate.0.lock().await;

    let vault_key_absent = keys
        .vault_key
        .lock()
        .map(|vk| vk.is_none())
        .unwrap_or(true);
    if vault_key_absent {
        let has_pw: bool = sqlx::query_scalar("SELECT COUNT(*) FROM vault_lock WHERE id = 1")
            .fetch_one(pool.inner())
            .await
            .map(|n: i64| n > 0)
            .unwrap_or(false);
        if has_pw {
            return Ok("0 notes indexed (vault is locked)".to_string());
        }
    }

    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let model = config.read().unwrap().embedding_model.clone();
    let max_retries = config.read().unwrap().background_max_retries;

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut map = cancel_map
            .0
            .lock()
            .map_err(|e| AppError::InvalidInput(e.to_string()))?;
        map.insert(VAULT_REINDEX_CANCEL_KEY.to_string(), cancel.clone());
    }

    let result = reindex_all_inner(
        &app,
        pool.inner(),
        &store,
        &vdb.0,
        &model,
        max_retries,
        &cancel,
        force_restart.unwrap_or(false),
    )
    .await;

    if let Ok(mut map) = cancel_map.0.lock() {
        map.remove(VAULT_REINDEX_CANCEL_KEY);
    }

    result
}

async fn reindex_all_inner(
    app: &tauri::AppHandle,
    pool: &SqlitePool,
    store: &EncryptedNoteStore<'_>,
    vdb: &Connection,
    model: &str,
    max_retries: i64,
    cancel: &AtomicBool,
    force_restart: bool,
) -> AppResult<String> {
    let (resume_from_checkpoint, mut next_pos, total, mut indexed_ok) = if force_restart {
        let t = rebuild_vault_reindex_checkpoint(pool, store, model).await?;
        (false, 0i64, t, 0i64)
    } else if let Some((np, tot, idx)) = vault_reindex_resume_snapshot(pool, store, model).await? {
        (true, np, tot, idx)
    } else {
        let t = rebuild_vault_reindex_checkpoint(pool, store, model).await?;
        (false, 0i64, t, 0i64)
    };

    let total_usize = total as usize;
    let mut failed: Vec<String> = Vec::new();
    let mut permanently_skipped: usize = 0;
    let mut cancelled = false;

    while next_pos < total {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        let note_id: i64 = sqlx::query_scalar("SELECT note_id FROM vault_reindex_queue WHERE pos = ?")
            .bind(next_pos)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("vault re-index queue row missing".to_string()))?;

        let note = match store.get_note(note_id).await {
            Ok(n) => n,
            Err(e) => {
                permanently_skipped += 1;
                failed.push(format!("note id {note_id} — {e}"));
                next_pos += 1;
                sqlx::query(
                    "UPDATE vault_reindex_state SET next_pos = ?, indexed_ok = ? WHERE id = 1",
                )
                .bind(next_pos)
                .bind(indexed_ok)
                .execute(pool)
                .await?;
                let _ = app.emit(
                    "reindex:progress",
                    serde_json::json!({
                        "indexed": indexed_ok,
                        "processed": next_pos as usize,
                        "total": total_usize,
                        "permanently_skipped": permanently_skipped,
                        "resuming": resume_from_checkpoint,
                    }),
                );
                continue;
            }
        };

        if note.locked {
            next_pos += 1;
            sqlx::query(
                "UPDATE vault_reindex_state SET next_pos = ?, indexed_ok = ? WHERE id = 1",
            )
            .bind(next_pos)
            .bind(indexed_ok)
            .execute(pool)
            .await?;
            let _ = app.emit(
                "reindex:progress",
                serde_json::json!({
                    "indexed": indexed_ok,
                    "processed": next_pos as usize,
                    "total": total_usize,
                    "permanently_skipped": permanently_skipped,
                    "resuming": resume_from_checkpoint,
                }),
            );
            continue;
        }

        super::search::fts_upsert(pool, note.id, &note.title, &note.content).await;
        let props_suffix = build_properties_suffix(pool, note.id).await;
        let full_content = if props_suffix.is_empty() {
            note.content.clone()
        } else {
            format!("{}{props_suffix}", note.content)
        };
        let sentences = split_sentences(&full_content);
        let raw_chunks = chunk_sentences(sentences, 1, 0);
        if raw_chunks.iter().all(|c| c.trim().is_empty()) {
            next_pos += 1;
            sqlx::query(
                "UPDATE vault_reindex_state SET next_pos = ?, indexed_ok = ? WHERE id = 1",
            )
            .bind(next_pos)
            .bind(indexed_ok)
            .execute(pool)
            .await?;
            let _ = app.emit(
                "reindex:progress",
                serde_json::json!({
                    "indexed": indexed_ok,
                    "processed": next_pos as usize,
                    "total": total_usize,
                    "permanently_skipped": permanently_skipped,
                    "resuming": resume_from_checkpoint,
                }),
            );
            continue;
        }

        let doc_texts: Vec<String> = raw_chunks
            .iter()
            .map(|chunk| format!("search_document: {}\n{}", note.title, chunk))
            .collect();

        let processed_ui = next_pos as usize;
        let indexed_ui = indexed_ok;
        let title_short = truncate_reindex_title(&note.title);
        let last_emit_chunks = Arc::new(AtomicUsize::new(0));
        let embed_opts = crate::vector::EmbedBatchOptions {
            on_slice_progress: Some(Arc::new({
                let app = app.clone();
                let last_emit = last_emit_chunks.clone();
                move |ready, tot| {
                    let prev = last_emit.load(Ordering::Relaxed);
                    let min_step = (tot / 40).max(64).min(tot.max(1));
                    if ready > 0 && ready < tot && ready.saturating_sub(prev) < min_step {
                        return;
                    }
                    last_emit.store(ready, Ordering::Relaxed);
                    let _ = app.emit(
                        "reindex:progress",
                        serde_json::json!({
                            "indexed": indexed_ui,
                            "processed": processed_ui,
                            "total": total_usize,
                            "permanently_skipped": permanently_skipped,
                            "resuming": resume_from_checkpoint,
                            "phase": "embedding",
                            "embedding_chunks": {
                                "done": ready,
                                "total": tot,
                                "note_title": title_short,
                            },
                        }),
                    );
                }
            })),
            ..Default::default()
        };

        let embeddings = match crate::retry::with_retries(max_retries, None, || async {
            crate::vector::embed_batch_with_options(&doc_texts, model, embed_opts.clone())
                .await
                .map_err(AppError::EmbeddingFailed)
        })
        .await
        {
            Ok(e) => e,
            Err(_) => {
                let mut fallback = Vec::with_capacity(raw_chunks.len());
                for chunk in &raw_chunks {
                    let emb = crate::retry::with_retries(max_retries, None, || async {
                        embed_document(chunk, model).await
                    })
                    .await?;
                    fallback.push(emb);
                }
                fallback
            }
        };

        let mut chunks: Vec<(i32, String, Vec<f32>)> = Vec::new();
        let mut note_ok = true;
        for ((i, chunk_text), embedding) in raw_chunks.into_iter().enumerate().zip(embeddings) {
            if embedding.is_empty() {
                permanently_skipped += 1;
                failed.push(format!("\"{}\" — empty embedding", note.title));
                note_ok = false;
                break;
            }
            chunks.push((i as i32, chunk_text, embedding));
        }

        if note_ok {
            match crate::retry::with_retries(max_retries, None, || {
                let ch = chunks.clone();
                async {
                    crate::vector::upsert(vdb, note.id, &note.title, ch)
                        .await
                        .map_err(AppError::VectorStore)
                }
            })
            .await
            {
                Ok(()) => indexed_ok += 1,
                Err(e) => {
                    permanently_skipped += 1;
                    failed.push(format!("\"{}\" (upsert) — {}", note.title, e));
                }
            }
        }

        next_pos += 1;
        sqlx::query(
            "UPDATE vault_reindex_state SET next_pos = ?, indexed_ok = ? WHERE id = 1",
        )
        .bind(next_pos)
        .bind(indexed_ok)
        .execute(pool)
        .await?;

        let _ = app.emit(
            "reindex:progress",
            serde_json::json!({
                "indexed": indexed_ok,
                "processed": next_pos as usize,
                "total": total_usize,
                "permanently_skipped": permanently_skipped,
                "resuming": resume_from_checkpoint,
            }),
        );
    }

    if cancelled {
        let _ = app.emit(
            "reindex:progress",
            serde_json::json!({
                "indexed": indexed_ok,
                "processed": next_pos as usize,
                "total": total_usize,
                "permanently_skipped": permanently_skipped,
                "resuming": resume_from_checkpoint,
                "cancelled": true,
            }),
        );
        return Ok(format!(
            "Re-index cancelled after {} of {} unlockable notes (semantic search updated for {} of them so far). Resume from the banner or Settings.",
            next_pos,
            total,
            indexed_ok
        ));
    }

    clear_vault_reindex_checkpoint(pool).await?;

    let embedded = indexed_ok as usize;
    let total_u = total as usize;
    let summary = if failed.is_empty() {
        if embedded == total_u {
            format!("Re-index complete: all {total} unlockable notes have vectors in semantic search.")
        } else if permanently_skipped == 0 {
            let no_vectors = total_u.saturating_sub(embedded);
            format!(
                "Re-index complete: processed all {total} unlockable notes. Semantic search has vectors for {embedded} notes; {no_vectors} had no embeddable text after chunking (often blank or whitespace-only)."
            )
        } else {
            format!(
                "Re-index complete: processed all {total} unlockable notes. Semantic search has vectors for {embedded} notes; {permanently_skipped} skipped after retries."
            )
        }
    } else {
        let skip_part = if permanently_skipped > 0 {
            format!("; {permanently_skipped} skipped after retries")
        } else {
            String::new()
        };
        format!(
            "Re-index finished with issues: processed {total} unlockable notes, semantic search updated for {embedded}{skip_part}. {} failed:\n{}",
            failed.len(),
            failed.join("\n")
        )
    };
    Ok(summary)
}

/// Drop the notes vector index so it can be rebuilt with a new embedding model.
/// The index will be recreated with the correct schema on the next reindex_all.
/// Clears any vault re-index checkpoint so a stale resume cannot run after the drop.
#[tauri::command]
pub async fn clear_notes_index(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
) -> AppResult<()> {
    clear_vault_reindex_checkpoint(pool.inner()).await?;
    crate::vector::clear_notes_index(&vdb.0)
        .await
        .map_err(|e| AppError::VectorStore(e))
}

/// Drop the Wikipedia vector index so it can be rebuilt with a new embedding model.
#[tauri::command]
pub async fn clear_wiki_index(
    vdb: State<'_, crate::vector::VectorDb>,
) -> AppResult<()> {
    crate::vector::clear_wiki_index(&vdb.0).await.map_err(|e| AppError::VectorStore(e))
}

/// Drop the scanned-files vector index so it can be rebuilt with a new embedding model.
#[tauri::command]
pub async fn clear_scanned_index(
    vdb: State<'_, crate::vector::VectorDb>,
) -> AppResult<()> {
    crate::vector::clear_scanned_index(&vdb.0).await.map_err(|e| AppError::VectorStore(e))
}

/// Debug command: returns the top 10 vector search hits with raw distance scores.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_search(
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
) -> AppResult<Vec<crate::vector::RawMatch>> {
    let model = config.read().unwrap().embedding_model.clone();
    let embedding = embed_query(&query, &model).await?;
    crate::vector::raw_search(&vdb.0, embedding, 10).await.map_err(|e| AppError::VectorStore(e))
}

/// Debug command: returns top Wikipedia search hits with raw distance scores, no filtering.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_search_wikipedia(
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
) -> AppResult<Vec<crate::vector::RawMatch>> {
    let model = config.read().unwrap().embedding_model.clone();
    let embedding = embed_query(&query, &model).await?;
    crate::vector::raw_wikipedia_search(&vdb.0, embedding, 10).await.map_err(|e| AppError::VectorStore(e))
}

/// Debug command: returns top scanned-file search hits with raw distance scores, no filtering.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_search_scanned_files(
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
) -> AppResult<Vec<crate::vector::RawMatch>> {
    let model = config.read().unwrap().embedding_model.clone();
    let embedding = embed_query(&query, &model).await?;
    crate::vector::raw_scanned_search(&vdb.0, embedding, 10).await.map_err(|e| AppError::VectorStore(e))
}

/// Insert a set of varied seed notes and index them all.
/// Intended for development/testing only.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn seed_notes(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
) -> AppResult<String> {
    let seeds: &[(&str, &str)] = &[
        (
            "Rust ownership and borrowing",
            "Rust enforces memory safety through a system of ownership with rules that the \
            compiler checks at compile time. Every value in Rust has a single owner. When the \
            owner goes out of scope, the value is dropped. References allow you to refer to a \
            value without taking ownership. The borrow checker ensures references are always \
            valid. Mutable references (&mut T) are exclusive — only one can exist at a time. \
            This prevents data races at compile time, without needing a garbage collector.",
        ),
        (
            "Sleep and cognitive performance",
            "Sleep plays a critical role in memory consolidation. During slow-wave sleep, the \
            hippocampus replays experiences to the neocortex for long-term storage. REM sleep \
            is associated with procedural memory and emotional regulation. Chronic sleep \
            deprivation impairs prefrontal cortex function, reducing decision-making ability, \
            working memory, and attention span. Adults generally need 7–9 hours. Even one night \
            of under-sleeping measurably reduces cognitive performance the following day.",
        ),
        (
            "How Transformer models work",
            "Transformers use self-attention to weigh the relevance of each token in a sequence \
            relative to every other token. Unlike RNNs, they process all tokens in parallel, \
            making them highly amenable to GPU acceleration. The attention mechanism computes \
            query, key, and value matrices and produces weighted sums of values. Positional \
            encodings are added to embeddings to preserve sequence order. Models like GPT are \
            decoder-only (autoregressive); BERT is encoder-only (masked language modeling). \
            LLMs are transformer-based models trained on large corpora to predict the next token.",
        ),
        (
            "Fermentation basics",
            "Fermentation is a metabolic process in which microorganisms like bacteria, yeast, \
            or fungi convert sugars into acids, gases, or alcohol. Lactic acid fermentation \
            (used in yoghurt, kimchi, sauerkraut) produces lactic acid. Alcoholic fermentation \
            (used in beer, wine, bread) produces ethanol and CO2. Temperature, salt concentration, \
            and pH all affect which microorganisms thrive. Fermented foods are rich in probiotics \
            and have been linked to improved gut microbiome diversity. Starter cultures can be \
            used to ensure consistent results.",
        ),
        (
            "The Stoic practice of negative visualisation",
            "Negative visualisation (premeditatio malorum) is a Stoic technique where you \
            deliberately imagine losing things you value — health, relationships, property. The \
            goal is not to induce anxiety but to cultivate gratitude and reduce attachment. Seneca \
            wrote that we should rehearse poverty, illness, and death periodically so that fortune \
            cannot catch us off guard. The practice counteracts hedonic adaptation, the tendency \
            to take good things for granted over time. It pairs well with the dichotomy of \
            control: focusing only on what is within your power.",
        ),
        (
            "How HTTPS and TLS work",
            "HTTPS is HTTP over TLS (Transport Layer Security). A TLS handshake establishes a \
            secure channel before any HTTP data is sent. The client sends a ClientHello with \
            supported cipher suites. The server responds with its certificate (signed by a CA) \
            and the chosen cipher. Key exchange uses asymmetric cryptography (e.g. ECDH) to \
            derive a shared secret without transmitting it. All subsequent traffic is encrypted \
            with symmetric keys derived from that secret. TLS 1.3 removed weak cipher suites and \
            reduced handshake round-trips from two to one.",
        ),
        (
            "Compound interest and long-term investing",
            "Compound interest is interest calculated on both the initial principal and the \
            accumulated interest. Over long periods, the effect is exponential. A 7% annual \
            return doubles money roughly every 10 years (rule of 72). Starting early matters \
            more than the amount invested: £5,000 invested at 25 grows more than £10,000 \
            invested at 35 at the same return rate. Index funds provide broad market exposure \
            with low fees, historically outperforming most actively managed funds over 20+ year \
            windows. Fee drag compounds just as returns do — a 1% annual fee has a significant \
            long-term cost.",
        ),
        (
            "Zettelkasten note-taking method",
            "# Zettelkasten\n\n\
            Zettelkasten is a note-taking method developed by sociologist **Niklas Luhmann**, who \
            used it to write over 70 books.\n\n\
            ## Core idea\n\n\
            Each note (*zettel*) contains a **single atomic idea** and is linked to related notes \
            by reference. Notes are not organised into folders or topics — meaning emerges from \
            the link structure.\n\n\
            ## Note types\n\n\
            1. **Fleeting notes** — quick captures, disposable\n\
            2. **Literature notes** — summaries from sources, in your own words\n\
            3. **Permanent notes** — processed ideas, written as if to a reader\n\n\
            ## Why it works\n\n\
            > The method is designed to build a personal knowledge graph over time, with the \
            network of links surfacing non-obvious connections between ideas.\n\n\
            Links between notes create a `web of knowledge` rather than a hierarchy. \
            Over years, the network becomes a reliable thinking partner.\n\n\
            ---\n\n\
            **See also:** [[How Transformer models work]], [[Sleep and cognitive performance]]",
        ),
    ];

    let model = config.read().unwrap().embedding_model.clone();
    let mut count = 0usize;
    let mut indexed = 0usize;
    for (title, content) in seeds {
        let row = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, content) VALUES (?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(title)
        .bind(content)
        .fetch_one(pool.inner())
        .await
        ?;
        count += 1;

        // Embedding is best-effort: seeding should work even without Ollama
        let sentences = split_sentences(&row.content);
        let raw_chunks = chunk_sentences(sentences, 1, 0);
        let mut chunks: Vec<(i32, String, Vec<f32>)> = Vec::new();
        let mut embed_ok = true;
        for (i, chunk_text) in raw_chunks.into_iter().enumerate() {
            match embed_document(&chunk_text, &model).await {
                Ok(embedding) => chunks.push((i as i32, chunk_text, embedding)),
                Err(_) => { embed_ok = false; break; }
            }
        }
        if embed_ok {
            if let Ok(()) = crate::vector::upsert(&vdb.0, row.id, &row.title, chunks).await {
                indexed += 1;
            }
        }
    }

    // Seed the kanban demo folder (best-effort — don't fail seeding if this errors).
    let _ = seed_kanban_folder(pool.inner()).await;

    if indexed == count {
        Ok(format!("{count}"))
    } else {
        Ok(format!("{count}:{indexed}"))
    }
}

/// Seed a "Kanban Demo" folder with a Status select property, a Priority select
/// property, and a set of notes spread across the Status columns.
/// Called at the end of seed_notes; extracted for readability.
#[cfg(debug_assertions)]
async fn seed_kanban_folder(pool: &SqlitePool) -> AppResult<()> {
    // Create the folder.
    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name) VALUES ('Kanban Demo') RETURNING id",
    )
    .fetch_one(pool)
    .await
    ?;

    // Create "Status" select property (position 0).
    let status_options = r#"["Todo","In Progress","Review","Done"]"#;
    let status_def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Status', 'select', ?, 0) RETURNING id",
    )
    .bind(folder_id)
    .bind(status_options)
    .fetch_one(pool)
    .await
    ?;

    // Create "Priority" select property (position 1).
    let priority_options = r#"["Low","Medium","High"]"#;
    let priority_def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Priority', 'select', ?, 1) RETURNING id",
    )
    .bind(folder_id)
    .bind(priority_options)
    .fetch_one(pool)
    .await
    ?;

    // Notes: (title, content, status, priority)
    let notes: &[(&str, &str, &str, &str)] = &[
        (
            "Design landing page",
            "Sketch wireframes and decide on the hero section layout. Review with the team before moving to high-fidelity.",
            "Todo", "High",
        ),
        (
            "Write API documentation",
            "Document all public endpoints with request/response examples. Include authentication flow and error codes.",
            "Todo", "Medium",
        ),
        (
            "Set up CI pipeline",
            "Configure GitHub Actions to run tests and linting on every pull request. Add a deploy step for staging.",
            "Todo", "Low",
        ),
        (
            "Implement search indexing",
            "Integrate the FTS5 virtual table with the Rust backend. Ensure encrypted notes are indexed with decrypted content.",
            "In Progress", "High",
        ),
        (
            "Refactor authentication module",
            "Replace the hand-rolled session handling with a proper state machine. Add rate limiting to the unlock endpoint.",
            "In Progress", "Medium",
        ),
        (
            "Add dark mode support",
            "Audit all CSS variables and ensure they respond correctly to the prefers-color-scheme media query.",
            "In Progress", "Low",
        ),
        (
            "Write unit tests for crypto module",
            "Cover all encryption and decryption paths. Include edge cases: wrong password, corrupted sentinel, empty content.",
            "Review", "High",
        ),
        (
            "Optimise vector embeddings",
            "Profile the embedding pipeline and reduce latency. Consider batching requests to the Ollama API.",
            "Review", "Medium",
        ),
        (
            "Fix calendar date offset bug",
            "Daily notes created near midnight were being assigned the wrong date in some time zones. Patched and tested.",
            "Done", "High",
        ),
        (
            "Update dependencies",
            "Bumped sqlx to 0.8, tauri to 2.x, and svelte to 5. Resolved all breaking changes and deprecation warnings.",
            "Done", "Low",
        ),
        (
            "Add tag autocomplete",
            "Implemented #tag suggestions in the editor using a floating dropdown. Filters on the current word after #.",
            "Done", "Medium",
        ),
    ];

    for (title, content, status, priority) in notes {
        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(content)
        .bind(folder_id)
        .fetch_one(pool)
        .await
        ?;

        // Set Status property.
        sqlx::query(
            "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
             ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
        )
        .bind(note_id)
        .bind(status_def_id)
        .bind(status)
        .execute(pool)
        .await
        ?;

        // Set Priority property.
        sqlx::query(
            "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
             ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
        )
        .bind(note_id)
        .bind(priority_def_id)
        .bind(priority)
        .execute(pool)
        .await
        ?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::truncate_reindex_title;

    #[test]
    fn truncate_reindex_title_under_limit_unchanged() {
        assert_eq!(truncate_reindex_title("hello"), "hello");
    }

    #[test]
    fn truncate_reindex_title_inserts_ellipsis_when_long() {
        let s = "a".repeat(100);
        let out = truncate_reindex_title(&s);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 81);
    }
}
