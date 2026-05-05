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

//! File Scanner — lets users add files/folders from outside the vault as RAG context sources.
//!
//! Supported formats include plain text, Markdown, PDF, CSV, HTML, DOCX, ODT, EPUB, RTF, and `.log`.
//! CSV uses row-aware blocks; EPUB respects spine chapters then sentence chunking; other prose
//! formats use the same sentence-chunking pipeline as notes and embed into `scanned_files`.

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use crate::{AppError, AppResult};
use crate::vector::{VectorDb, ScannedFileMatch};
use crate::config::SharedConfig;
use crate::chunking::{split_sentences, chunk_sentences};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A scanned path row, returned to the frontend.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScannedPath {
    pub id:              i64,
    pub path:            String,
    pub kind:            String, // "file" | "folder"
    pub added_at:        i64,
    pub last_scanned_at: Option<i64>,
    pub enabled:         bool,
    pub file_count:      i64,
    pub error_msg:       Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the mime type for a supported file extension, or None if unsupported.
fn mime_for_path(path: &std::path::Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())?;
    match ext.as_str() {
        "txt" | "log" => Some("text/plain"),
        "md" => Some("text/markdown"),
        "pdf" => Some("application/pdf"),
        "csv" => Some("text/csv"),
        "html" | "htm" => Some("text/html"),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "odt" => Some("application/vnd.oasis.opendocument.text"),
        "epub" => Some("application/epub+zip"),
        "rtf" => Some("application/rtf"),
        _ => None,
    }
}

/// Collect all indexable files under a path.
/// - For kind="file": returns [(path, mime)] if supported, else empty.
/// - For kind="folder": walks recursively, skips unsupported extensions.
fn collect_files(path: &str, kind: &str) -> Vec<(String, &'static str)> {
    let p = std::path::Path::new(path);
    if kind == "file" {
        return if let Some(mime) = mime_for_path(p) {
            vec![(path.to_string(), mime)]
        } else {
            vec![]
        };
    }

    // Folder: walk recursively.
    let mut files = Vec::new();
    let Ok(entries) = walkdir_collect(p) else { return files };
    for entry in entries {
        if let Some(mime) = mime_for_path(&entry) {
            if let Some(s) = entry.to_str() {
                files.push((s.to_string(), mime));
            }
        }
    }
    files
}

/// Recursively collect all file paths under `root`, following symlinks.
fn walkdir_collect(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut result = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let path = entry.path();
            if ft.is_dir() {
                walk(&path, out)?;
            } else if ft.is_file() || ft.is_symlink() {
                out.push(path);
            }
        }
        Ok(())
    }
    walk(root, &mut result)?;
    Ok(result)
}

/// Short label for UI (filename only).
fn file_display_name(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path)
        .to_string()
}

/// Embed `doc_texts` in API-sized batches and emit `filescanner:progress` so the UI can show
/// chunk-level progress (same idea as Wikipedia index windows).
async fn embed_scanned_chunks_with_progress(
    app: &AppHandle,
    path_id: i64,
    visited: usize,
    scanned: usize,
    skipped: usize,
    total: usize,
    file_path: &str,
    doc_texts: &[String],
    model: &str,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<Vec<Vec<f32>>, String> {
    let chunks_total = doc_texts.len();
    let label = file_display_name(file_path);

    let _ = app.emit("filescanner:progress", serde_json::json!({
        "path_id": path_id,
        "visited": visited,
        "scanned": scanned,
        "skipped": skipped,
        "total": total,
        "phase": "embedding",
        "chunks_embedded": 0,
        "chunks_total": chunks_total,
        "current_file": label,
        "done": false,
        "error": null,
    }));

    let bs = crate::vector::batch_size_for_model(model).max(1);
    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(chunks_total);
    let mut offset = 0usize;

    while offset < chunks_total {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err("Indexing cancelled".into());
        }

        let end = (offset + bs).min(chunks_total);
        let slice = &doc_texts[offset..end];

        match crate::vector::embed_batch(slice, model).await {
            Ok(part) if part.len() == slice.len() => {
                embeddings.extend(part);
                offset = end;
            }
            Ok(_) | Err(_) => {
                for j in offset..end {
                    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                        return Err("Indexing cancelled".into());
                    }
                    let emb =
                        crate::vector::embed_with_keep_alive(&doc_texts[j], model, 300)
                            .await
                            .unwrap_or_default();
                    embeddings.push(emb);
                    let emit_every = if chunks_total > 256 { 48usize } else { 1usize };
                    let at_slice_end = j + 1 == end;
                    if at_slice_end || (j + 1 - offset) % emit_every == 0 {
                        let _ = app.emit("filescanner:progress", serde_json::json!({
                            "path_id": path_id,
                            "visited": visited,
                            "scanned": scanned,
                            "skipped": skipped,
                            "total": total,
                            "phase": "embedding",
                            "chunks_embedded": j + 1,
                            "chunks_total": chunks_total,
                            "current_file": label,
                            "done": false,
                            "error": null,
                        }));
                    }
                }
                offset = end;
            }
        }

        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "visited": visited,
            "scanned": scanned,
            "skipped": skipped,
            "total": total,
            "phase": "embedding",
            "chunks_embedded": offset,
            "chunks_total": chunks_total,
            "current_file": label,
            "done": false,
            "error": null,
        }));
    }

    Ok(embeddings)
}

/// Emits `filescanner:progress` with `visited` (files examined, 1-based), `scanned`
/// (files re-indexed), `skipped` (unchanged), and `total`.
/// Updates SQLite `scanned_files` rows and the LanceDB scanned_files table.
/// On completion, updates `last_scanned_at` and `file_count` in `scanned_paths`.
///
/// When `incremental` is true, files whose mtime hasn't changed since the last
/// index are skipped. Stale entries (in the DB but deleted from disk) are cleaned up.
async fn index_path(
    app: &AppHandle,
    pool: &SqlitePool,
    vdb: &lancedb::Connection,
    path_id: i64,
    path: &str,
    kind: &str,
    model: &str,
    cancel: Option<Arc<AtomicBool>>,
    incremental: bool,
) -> AppResult<()> {
    let files = collect_files(path, kind);
    let total = files.len();
    let mut scanned = 0usize;
    let mut skipped = 0usize;

    let known_mtime: HashMap<String, i64> = if incremental {
        sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT file_path, mtime FROM scanned_files WHERE path_id = ?",
        )
        .bind(path_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(p, m)| m.map(|mt| (p, mt)))
        .collect()
    } else {
        HashMap::new()
    };

    // Emit started event.
    let _ = app.emit("filescanner:progress", serde_json::json!({
        "path_id": path_id,
        "visited": 0,
        "scanned": 0,
        "skipped": 0,
        "total": total,
        "phase": "starting",
        "chunks_embedded": 0,
        "chunks_total": 0,
        "current_file": serde_json::Value::Null,
        "done": false,
        "error": null,
    }));

    for (idx, (file_path, mime)) in files.iter().enumerate() {
        let visited = idx + 1;
        if cancel.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(AppError::InvalidInput("Indexing cancelled".to_string()));
        }

        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "visited": visited,
            "scanned": scanned,
            "skipped": skipped,
            "total": total,
            "phase": "walking",
            "chunks_embedded": 0,
            "chunks_total": 0,
            "current_file": serde_json::Value::Null,
            "done": false,
            "error": null,
        }));

        // ── Incremental skip: compare mtime against the last-indexed value ────
        if incremental {
            let old_mtime = known_mtime.get(file_path).copied();

            let now_mtime = std::fs::metadata(file_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);

            if let (Some(old), Some(new)) = (old_mtime, now_mtime) {
                if old == new {
                    skipped += 1;
                    continue;
                }
            }
        }

        scanned += 1;

        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "visited": visited,
            "scanned": scanned,
            "skipped": skipped,
            "total": total,
            "phase": "reading",
            "chunks_embedded": 0,
            "chunks_total": 0,
            "current_file": file_display_name(file_path),
            "done": false,
            "error": null,
        }));

        // Read file content.
        let extracted = match super::scanner_extract::extract(std::path::Path::new(file_path)) {
            Ok(e) => e,
            Err(e) => {
                let _ = app.emit("filescanner:progress", serde_json::json!({
                    "path_id": path_id,
                    "visited": visited,
                    "scanned": scanned,
                    "skipped": skipped,
                    "total": total,
                    "phase": "idle",
                    "chunks_embedded": 0,
                    "chunks_total": 0,
                    "current_file": serde_json::Value::Null,
                    "done": false,
                    "error": format!("Could not read {file_path}: {e}"),
                }));
                continue;
            }
        };

        // Derive a title from the filename (without extension).
        let title = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_path)
            .to_string();

        let raw_chunks = match extracted {
            super::scanner_extract::ScanExtract::FullText(content) => {
                let sentences = split_sentences(&content);
                chunk_sentences(sentences, 1, 0)
            }
            super::scanner_extract::ScanExtract::Chunks(chunks) => chunks,
        };
        let doc_texts: Vec<String> = raw_chunks
            .iter()
            .enumerate()
            .map(|(_, chunk)| format!("search_document: {title}\n{chunk}"))
            .collect();

        if doc_texts.is_empty() {
            continue;
        }

        let embeddings = match embed_scanned_chunks_with_progress(
            app,
            path_id,
            visited,
            scanned,
            skipped,
            total,
            file_path,
            &doc_texts,
            model,
            cancel.as_ref(),
        )
        .await
        {
            Ok(e) => e,
            Err(e) if e == "Indexing cancelled" => {
                return Err(AppError::InvalidInput("Indexing cancelled".to_string()));
            }
            Err(e) => return Err(AppError::Io(e)),
        };

        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "visited": visited,
            "scanned": scanned,
            "skipped": skipped,
            "total": total,
            "phase": "storing",
            "chunks_embedded": embeddings.len(),
            "chunks_total": embeddings.len(),
            "current_file": file_display_name(file_path),
            "done": false,
            "error": null,
        }));

        // Build (chunk_index, title, content, embedding) tuples, skipping empty
        // embeddings and empty/whitespace content chunks.
        let chunks: Vec<(i32, String, String, Vec<f32>)> = raw_chunks
            .into_iter()
            .zip(embeddings)
            .enumerate()
            .filter_map(|(ci, (content_chunk, emb))| {
                if emb.is_empty() || content_chunk.trim().is_empty() {
                    return None;
                }
                Some((ci as i32, title.clone(), content_chunk, emb))
            })
            .collect();

        // If every chunk was empty (e.g. whitespace-only file), skip this file.
        // Storing an empty-content chunk creates a title-only embedding that matches
        // on file name alone but returns a blank excerpt to the LLM.
        if chunks.is_empty() {
            continue;
        }

        // Upsert into LanceDB.
        if let Err(e) = crate::vector::scanned_file_upsert_batch(vdb, file_path, chunks).await {
            let _ = app.emit("filescanner:progress", serde_json::json!({
                "path_id": path_id,
                "visited": visited,
                "scanned": scanned,
                "skipped": skipped,
                "total": total,
                "phase": "idle",
                "chunks_embedded": 0,
                "chunks_total": 0,
                "current_file": serde_json::Value::Null,
                "done": false,
                "error": format!("Failed to index {file_path}: {e}"),
            }));
            continue;
        }

        // Get file mtime.
        let mtime = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Upsert scanned_files row.
        sqlx::query(
            "INSERT INTO scanned_files (path_id, file_path, mime_type, indexed_at, mtime)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(file_path) DO UPDATE SET
                 indexed_at = excluded.indexed_at,
                 mtime = excluded.mtime",
        )
        .bind(path_id)
        .bind(file_path)
        .bind(*mime)
        .bind(now)
        .bind(mtime)
        .execute(pool)
        .await
        ?;

        // Emit progress every file (clear chunk-level fields for the row summary).
        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "visited": visited,
            "scanned": scanned,
            "skipped": skipped,
            "total": total,
            "phase": "idle",
            "chunks_embedded": 0,
            "chunks_total": 0,
            "current_file": serde_json::Value::Null,
            "done": false,
            "error": null,
        }));
    }

    // ── Stale file cleanup (incremental only) ──────────────────────────────
    // Remove entries for files that were previously indexed but no longer exist on disk.
    if incremental {
        let stale_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT file_path FROM scanned_files WHERE path_id = ?",
        )
        .bind(path_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if !stale_rows.is_empty() {
            let _ = app.emit("filescanner:progress", serde_json::json!({
                "path_id": path_id,
                "visited": total,
                "scanned": scanned,
                "skipped": skipped,
                "total": total,
                "phase": "cleanup",
                "chunks_embedded": 0,
                "chunks_total": 0,
                "current_file": serde_json::Value::Null,
                "done": false,
                "error": null,
            }));
        }

        for (stale_path,) in &stale_rows {
            if !std::path::Path::new(stale_path).exists() {
                let _ = crate::vector::scanned_file_remove(vdb, stale_path).await;
                let _ = sqlx::query("DELETE FROM scanned_files WHERE file_path = ? AND path_id = ?")
                    .bind(stale_path)
                    .bind(path_id)
                    .execute(pool)
                    .await;
            }
        }
    }

    // Update scanned_paths.last_scanned_at and file_count.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    sqlx::query(
        "UPDATE scanned_paths SET last_scanned_at = ?, file_count = ?, error_msg = NULL WHERE id = ?",
    )
    .bind(now)
    .bind(total as i64)
    .bind(path_id)
    .execute(pool)
    .await
    ?;

    // Emit done event.
    let _ = app.emit("filescanner:progress", serde_json::json!({
        "path_id": path_id,
        "visited": total,
        "scanned": total,
        "skipped": skipped,
        "total": total,
        "phase": "done",
        "chunks_embedded": 0,
        "chunks_total": 0,
        "current_file": serde_json::Value::Null,
        "done": true,
        "error": null,
    }));

    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// List all scanned paths.
#[tauri::command]
pub async fn get_scanned_paths(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<ScannedPath>> {
    sqlx::query_as::<_, ScannedPath>(
        "SELECT id, path, kind, added_at, last_scanned_at, enabled, file_count, error_msg
         FROM scanned_paths ORDER BY added_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(Into::into)
}

/// Add a new path (file or folder) to the file scanner and immediately index it.
/// Validates:
///   - path exists on disk
///   - kind matches reality (kind='file' → must be a file, kind='folder' → must be a dir)
///   - path is not inside the vault directory (would cause double-indexing)
#[tauri::command]
pub async fn add_scanned_path(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    cancel_map: State<'_, super::FileScanCancelMap>,
    config: State<'_, SharedConfig>,
    path: String,
    kind: String,
) -> AppResult<ScannedPath> {
    if kind != "file" && kind != "folder" {
        return Err(AppError::InvalidInput(format!("Invalid kind '{kind}': must be 'file' or 'folder'")));
    }

    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(AppError::InvalidInput(format!("Path does not exist: {path}")));
    }
    if kind == "file" && !p.is_file() {
        return Err(AppError::InvalidInput(format!("Expected a file but got a directory: {path}")));
    }
    if kind == "folder" && !p.is_dir() {
        return Err(AppError::InvalidInput(format!("Expected a folder but got a file: {path}")));
    }

    // Guard: reject individual files with unsupported extensions.
    if kind == "file" && mime_for_path(p).is_none() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("none");
        return Err(AppError::InvalidInput(format!(
            "Unsupported file type '.{ext}'. Supported extensions include .txt, .md, .pdf, .csv, .html, .htm, .docx, .odt, .epub, .rtf, and .log."
        )));
    }

    // Guard: reject paths inside the vault directory.
    let vault_path_raw = config.read().unwrap().vault_path.clone();
    if !vault_path_raw.is_empty() {
        let vault_p = std::path::Path::new(&vault_path_raw);
        if p.starts_with(vault_p) {
            return Err(AppError::InvalidInput("Cannot scan a path inside the vault — vault files are already indexed separately.".to_string()));
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO scanned_paths (path, kind, added_at, enabled, file_count)
         VALUES (?, ?, ?, 1, 0)
         RETURNING id",
    )
    .bind(&path)
    .bind(&kind)
    .bind(now)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            AppError::InvalidInput(format!("This path is already in the file scanner: {path}"))
        } else {
            AppError::Database(e.to_string())
        }
    })?;

    // Kick off indexing in the background so the command returns immediately.
    let app_clone  = app.clone();
    let pool_clone = pool.inner().clone();
    let vdb_clone  = vdb.0.clone();
    let path_clone = path.clone();
    let kind_clone = kind.clone();
    let cancel_map_clone = cancel_map.0.clone();
    let model_clone = config.read().unwrap().embedding_model.clone();
    let pool_err   = pool.inner().clone();

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        map.insert(id, cancel.clone());
    }

    tauri::async_runtime::spawn(async move {
        let result = index_path(&app_clone, &pool_clone, &vdb_clone, id, &path_clone, &kind_clone, &model_clone, Some(cancel), false).await;

        if let Ok(mut map) = cancel_map_clone.lock() {
            map.remove(&id);
        }

        if let Err(e) = result {
            let is_cancelled = matches!(&e, AppError::InvalidInput(m) if m == "Indexing cancelled");

            if is_cancelled {
                let _ = sqlx::query(
                    "UPDATE scanned_paths SET error_msg = NULL WHERE id = ?",
                )
                .bind(id)
                .execute(&pool_err)
                .await;

                let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                    "path_id": id,
                    "scanned": 0,
                    "skipped": 0,
                    "total": 0,
                    "done": true,
                    "error": null,
                }));

                return;
            }

            // Persist the error so the UI can surface it.
            let e_str = e.to_string();
            let _ = sqlx::query(
                "UPDATE scanned_paths SET error_msg = ? WHERE id = ?",
            )
            .bind(&e_str)
            .bind(id)
            .execute(&pool_err)
            .await;

            let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                "path_id": id,
                "scanned": 0,
                "skipped": 0,
                "total": 0,
                "done": true,
                "error": e_str,
            }));
        }
    });

    // Return the newly created row immediately (indexing runs async).
    sqlx::query_as::<_, ScannedPath>(
        "SELECT id, path, kind, added_at, last_scanned_at, enabled, file_count, error_msg
         FROM scanned_paths WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool.inner())
    .await
    .map_err(Into::into)
}

/// Remove a scanned path and all its indexed data.
#[tauri::command]
pub async fn remove_scanned_path(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    id: i64,
) -> AppResult<()> {
    // Fetch path details before deletion.
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM scanned_paths WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    ?;

    if let Some((path, kind)) = row {
        // Remove all LanceDB vectors for files under this path.
        if kind == "folder" {
            // Normalise to a prefix with a path separator to avoid partial matches.
            let prefix = if path.ends_with(std::path::MAIN_SEPARATOR) {
                path.clone()
            } else {
                format!("{path}{}", std::path::MAIN_SEPARATOR)
            };
            crate::vector::scanned_file_remove_prefix(&vdb.0, &prefix).await.map_err(|e| AppError::VectorStore(e))?;
        } else {
            crate::vector::scanned_file_remove(&vdb.0, &path).await.map_err(|e| AppError::VectorStore(e))?;
        }
    }

    // ON DELETE CASCADE removes scanned_files rows automatically.
    sqlx::query("DELETE FROM scanned_paths WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    Ok(())
}

/// Toggle the enabled flag for a scanned path.
/// Disabled paths are excluded from RAG search but their vectors remain indexed.
#[tauri::command]
pub async fn toggle_scanned_path(
    pool: State<'_, SqlitePool>,
    id: i64,
    enabled: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE scanned_paths SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool.inner())
        .await
        ?;
    Ok(())
}

/// Re-index all files for a scanned path.
/// When `full_rescan` is false or absent, unchanged files (mtime match) are skipped.
/// Called by the "Rescan" button in the UI.
#[tauri::command]
pub async fn rescan_path(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    cancel_map: State<'_, super::FileScanCancelMap>,
    config: State<'_, SharedConfig>,
    id: i64,
    full_rescan: Option<bool>,
) -> AppResult<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM scanned_paths WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    ?;

    let (path, kind) = row.ok_or_else(|| AppError::NotFound(format!("Scanned path {id} not found")))?;

    let _ = crate::audit::log_event(
        pool.inner(), "file_scan", Some("file"),
        Some(id), Some(&path), None,
    ).await;

    let app_clone  = app.clone();
    let pool_clone = pool.inner().clone();
    let vdb_clone  = vdb.0.clone();
    let cancel_map_clone = cancel_map.0.clone();
    let model_clone = config.read().unwrap().embedding_model.clone();
    let pool_err   = pool.inner().clone();
    let incremental = !full_rescan.unwrap_or(false);

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        map.insert(id, cancel.clone());
    }

    tauri::async_runtime::spawn(async move {
        let result = index_path(&app_clone, &pool_clone, &vdb_clone, id, &path, &kind, &model_clone, Some(cancel), incremental).await;

        if let Ok(mut map) = cancel_map_clone.lock() {
            map.remove(&id);
        }

        if let Err(e) = result {
            let is_cancelled = matches!(&e, AppError::InvalidInput(m) if m == "Indexing cancelled");

            if is_cancelled {
                let _ = sqlx::query(
                    "UPDATE scanned_paths SET error_msg = NULL WHERE id = ?",
                )
                .bind(id)
                .execute(&pool_err)
                .await;

                let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                    "path_id": id,
                    "scanned": 0,
                    "total": 0,
                    "done": true,
                    "error": null,
                }));

                return;
            }

            let e_str = e.to_string();
            let _ = sqlx::query(
                "UPDATE scanned_paths SET error_msg = ? WHERE id = ?",
            )
            .bind(&e_str)
            .bind(id)
            .execute(&pool_err)
            .await;

            let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                "path_id": id,
                "scanned": 0,
                "total": 0,
                "done": true,
                "error": e_str,
            }));
        }
    });

    Ok(())
}

/// Request cancellation for an in-progress file-scanner indexing run.
/// Safe to call even when no scan is currently running for the path.
#[tauri::command]
pub async fn cancel_scanned_path_index(
    pool: State<'_, SqlitePool>,
    cancel_map: State<'_, super::FileScanCancelMap>,
    id: i64,
) -> AppResult<()> {
    {
        let map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        if let Some(flag) = map.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    let row_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM scanned_paths WHERE id = ?")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        ?;

    if row_exists.is_none() {
        return Err(AppError::NotFound(format!("Scanned path {id} not found")));
    }

    sqlx::query("UPDATE scanned_paths SET error_msg = NULL WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    Ok(())
}

/// Search scanned files for content semantically similar to `query`.
/// Only searches paths where enabled = 1.
/// Returns an empty list if no enabled paths exist or nothing is relevant.
#[tauri::command]
pub async fn search_scanned_files(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
) -> AppResult<Vec<ScannedFileMatch>> {
    // Fast path: if no enabled paths exist, skip embedding entirely.
    let enabled_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM scanned_paths WHERE enabled = 1",
    )
    .fetch_one(pool.inner())
    .await
    ?;

    if enabled_count == 0 {
        return Ok(vec![]);
    }

    let model = config.read().unwrap().embedding_model.clone();
    let embedding = crate::vector::embed(
        &format!("search_query: {query}"),
        &model,
    )
    .await
    .map_err(|e| AppError::EmbeddingFailed(e))?;

    let all_matches = crate::vector::scanned_file_search(&vdb.0, embedding, 60).await.map_err(|e| AppError::VectorStore(e))?;

    if all_matches.is_empty() {
        return Ok(vec![]);
    }

    // Filter: only return results whose file_path belongs to an enabled scanned_path.
    // We fetch all enabled paths once and check prefix/equality.
    let enabled_paths: Vec<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM scanned_paths WHERE enabled = 1",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let filtered: Vec<ScannedFileMatch> = all_matches
        .into_iter()
        .filter(|m| {
            enabled_paths.iter().any(|(p, kind)| {
                if kind == "file" {
                    m.file_path == *p
                } else {
                    // Check if the file is under this folder.
                    let prefix_with_sep = if p.ends_with(std::path::MAIN_SEPARATOR) {
                        p.clone()
                    } else {
                        format!("{p}{}", std::path::MAIN_SEPARATOR)
                    };
                    m.file_path.starts_with(&prefix_with_sep)
                }
            })
        })
        .collect();

    let _ = crate::audit::log_event(
        pool.inner(), "search_semantic", Some("file"), None, None, Some(&query),
    ).await;
    Ok(filtered)
}

/// Read a scanned file's content and create a new note from it.
/// `folder_id` — the target folder; `None` places the note in Unfiled.
/// Returns the full Note struct so the caller can navigate to it immediately.
#[tauri::command]
pub async fn import_file_as_note(
    pool: State<'_, SqlitePool>,
    file_path: String,
    folder_id: Option<i64>,
) -> AppResult<super::Note> {
    // Only supported types are allowed.
    let p = std::path::Path::new(&file_path);
    if mime_for_path(p).is_none() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("none");
        return Err(AppError::InvalidInput(format!("Unsupported file type '.{ext}'")));
    }

    let extracted = super::scanner_extract::extract(p)?;
    let content = super::scanner_extract::flatten_for_note(&extracted);

    // Derive a title from the filename stem.
    let title = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported note")
        .to_string();

    let row = sqlx::query_as::<_, super::NoteRow>(
        "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?)
         RETURNING id, title, content, folder_id, created_at, updated_at",
    )
    .bind(&title)
    .bind(&content)
    .bind(folder_id)
    .fetch_one(pool.inner())
    .await
    ?;

    // Update FTS index so the note is immediately searchable.
    super::search::fts_upsert(pool.inner(), row.id, &title, &content).await;

    let _ = crate::audit::log_event(
        pool.inner(), "file_import", Some("file"),
        Some(row.id), Some(&file_path), None,
    ).await;

    // Return the full Note struct (locked = false — Unfiled and user-selected folders
    // are never locked at import time; encrypted folders are intentionally unsupported).
    Ok(super::Note {
        id:         row.id,
        title:      row.title,
        content:    row.content,
        folder_id:  row.folder_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        locked:     false,
    })
}
