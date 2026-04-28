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
//! Supported formats (v1): `.txt`, `.md`
//! Each indexed file is chunked via the same sentence-chunking pipeline as notes and embedded
//! into the `scanned_files` LanceDB table.

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use serde::Serialize;
use pdf_extract;

use crate::vector::{VectorDb, ScannedFileMatch};
use super::rag::{split_sentences, chunk_sentences, get_embedding_model_pub};

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
    match path.extension().and_then(|e| e.to_str()) {
        Some("txt")  => Some("text/plain"),
        Some("md")   => Some("text/markdown"),
        Some("pdf")  => Some("application/pdf"),
        _ => None,
    }
}

/// Extract the text content of a supported file.
/// For PDFs, text is extracted from the PDF structure via pdf-extract.
/// For plain text / Markdown, the file is read as UTF-8.
/// Returns an error if the file cannot be read or, for PDFs, yields no text.
fn read_file_text(path: &std::path::Path) -> Result<String, String> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pdf") => {
            let text = pdf_extract::extract_text(path).map_err(|e| e.to_string())?;
            if text.trim().is_empty() {
                Err("No text could be extracted from this PDF. It may be a scanned image without embedded text.".to_string())
            } else {
                Ok(text)
            }
        }
        _ => std::fs::read_to_string(path).map_err(|e| e.to_string()),
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

/// Index all files for a scanned_paths row. Emits `filescanner:progress` events.
/// Updates SQLite `scanned_files` rows and the LanceDB scanned_files table.
/// On completion, updates `last_scanned_at` and `file_count` in `scanned_paths`.
async fn index_path(
    app: &AppHandle,
    pool: &SqlitePool,
    vdb: &lancedb::Connection,
    path_id: i64,
    path: &str,
    kind: &str,
) -> Result<(), String> {
    let model = get_embedding_model_pub(pool).await;
    let files = collect_files(path, kind);
    let total = files.len();
    let mut scanned = 0usize;

    // Emit started event.
    let _ = app.emit("filescanner:progress", serde_json::json!({
        "path_id": path_id,
        "scanned": 0,
        "total": total,
        "done": false,
        "error": null,
    }));

    for (file_path, mime) in &files {
        scanned += 1;

        // Read file content.
        let content = match read_file_text(std::path::Path::new(file_path)) {
            Ok(c) => c,
            Err(e) => {
                // Skip unreadable files but report via progress.
                let _ = app.emit("filescanner:progress", serde_json::json!({
                    "path_id": path_id,
                    "scanned": scanned,
                    "total": total,
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

        // Chunk content using the same sentence pipeline as notes.
        let sentences = split_sentences(&content);
        let raw_chunks = chunk_sentences(sentences, 1, 0);
        let doc_texts: Vec<String> = raw_chunks
            .iter()
            .enumerate()
            .map(|(_, chunk)| format!("search_document: {title}\n{chunk}"))
            .collect();

        if doc_texts.is_empty() {
            continue;
        }

        // Embed all chunks in one batch call.
        let embeddings = match crate::vector::embed_batch(&doc_texts, &model).await {
            Ok(e) => e,
            Err(_) => {
                // Fall back to one-at-a-time embedding on batch failure.
                let mut embs = Vec::new();
                for text in &doc_texts {
                    let emb = crate::vector::embed(text, &model).await.unwrap_or_default();
                    embs.push(emb);
                }
                embs
            }
        };

        // Build (chunk_index, title, content, embedding) tuples, skipping empty embeddings.
        let chunks: Vec<(i32, String, String, Vec<f32>)> = raw_chunks
            .into_iter()
            .zip(embeddings)
            .enumerate()
            .filter_map(|(ci, (content_chunk, emb))| {
                if emb.is_empty() { return None; }
                Some((ci as i32, title.clone(), content_chunk, emb))
            })
            .collect();

        // Upsert into LanceDB.
        if let Err(e) = crate::vector::scanned_file_upsert_batch(vdb, file_path, chunks).await {
            let _ = app.emit("filescanner:progress", serde_json::json!({
                "path_id": path_id,
                "scanned": scanned,
                "total": total,
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
        .map_err(|e| e.to_string())?;

        // Emit progress every file.
        let _ = app.emit("filescanner:progress", serde_json::json!({
            "path_id": path_id,
            "scanned": scanned,
            "total": total,
            "done": false,
            "error": null,
        }));
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
    .map_err(|e| e.to_string())?;

    // Emit done event.
    let _ = app.emit("filescanner:progress", serde_json::json!({
        "path_id": path_id,
        "scanned": total,
        "total": total,
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
) -> Result<Vec<ScannedPath>, String> {
    sqlx::query_as::<_, ScannedPath>(
        "SELECT id, path, kind, added_at, last_scanned_at, enabled, file_count, error_msg
         FROM scanned_paths ORDER BY added_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
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
    path: String,
    kind: String,
) -> Result<ScannedPath, String> {
    if kind != "file" && kind != "folder" {
        return Err(format!("Invalid kind '{kind}': must be 'file' or 'folder'"));
    }

    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {path}"));
    }
    if kind == "file" && !p.is_file() {
        return Err(format!("Expected a file but got a directory: {path}"));
    }
    if kind == "folder" && !p.is_dir() {
        return Err(format!("Expected a folder but got a file: {path}"));
    }

    // Guard: reject individual files with unsupported extensions.
    if kind == "file" && mime_for_path(p).is_none() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("none");
        return Err(format!(
            "Unsupported file type '.{ext}'. Only .txt, .md, and .pdf files are supported."
        ));
    }

    // Guard: reject paths inside the vault directory.
    let vault_path_raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'vault_path' LIMIT 1",
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten();

    if let Some(vault_raw) = vault_path_raw {
        let vault_p = std::path::Path::new(&vault_raw);
        if p.starts_with(vault_p) {
            return Err(
                "Cannot scan a path inside the vault — vault files are already indexed separately."
                    .to_string(),
            );
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
            format!("This path is already in the file scanner: {path}")
        } else {
            e.to_string()
        }
    })?;

    // Kick off indexing in the background so the command returns immediately.
    let app_clone  = app.clone();
    let pool_clone = pool.inner().clone();
    let vdb_clone  = vdb.0.clone();
    let path_clone = path.clone();
    let kind_clone = kind.clone();
    let pool_err   = pool.inner().clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = index_path(&app_clone, &pool_clone, &vdb_clone, id, &path_clone, &kind_clone).await {
            // Persist the error so the UI can surface it.
            let _ = sqlx::query(
                "UPDATE scanned_paths SET error_msg = ? WHERE id = ?",
            )
            .bind(&e)
            .bind(id)
            .execute(&pool_err)
            .await;

            let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                "path_id": id,
                "scanned": 0,
                "total": 0,
                "done": true,
                "error": e,
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
    .map_err(|e| e.to_string())
}

/// Remove a scanned path and all its indexed data.
#[tauri::command]
pub async fn remove_scanned_path(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    id: i64,
) -> Result<(), String> {
    // Fetch path details before deletion.
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM scanned_paths WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if let Some((path, kind)) = row {
        // Remove all LanceDB vectors for files under this path.
        if kind == "folder" {
            // Normalise to a prefix with a path separator to avoid partial matches.
            let prefix = if path.ends_with(std::path::MAIN_SEPARATOR) {
                path.clone()
            } else {
                format!("{path}{}", std::path::MAIN_SEPARATOR)
            };
            crate::vector::scanned_file_remove_prefix(&vdb.0, &prefix).await?;
        } else {
            crate::vector::scanned_file_remove(&vdb.0, &path).await?;
        }
    }

    // ON DELETE CASCADE removes scanned_files rows automatically.
    sqlx::query("DELETE FROM scanned_paths WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Toggle the enabled flag for a scanned path.
/// Disabled paths are excluded from RAG search but their vectors remain indexed.
#[tauri::command]
pub async fn toggle_scanned_path(
    pool: State<'_, SqlitePool>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE scanned_paths SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Re-index all files for a scanned path (forced, regardless of mtime).
/// Called by the "Rescan" button in the UI.
#[tauri::command]
pub async fn rescan_path(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    id: i64,
) -> Result<(), String> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT path, kind FROM scanned_paths WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let (path, kind) = row.ok_or_else(|| format!("Scanned path {id} not found"))?;

    let _ = crate::audit::log_event(
        pool.inner(), "file_scan", Some("file"),
        Some(id), Some(&path), None,
    ).await;

    let app_clone  = app.clone();
    let pool_clone = pool.inner().clone();
    let vdb_clone  = vdb.0.clone();
    let pool_err   = pool.inner().clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = index_path(&app_clone, &pool_clone, &vdb_clone, id, &path, &kind).await {
            let _ = sqlx::query(
                "UPDATE scanned_paths SET error_msg = ? WHERE id = ?",
            )
            .bind(&e)
            .bind(id)
            .execute(&pool_err)
            .await;

            let _ = app_clone.emit("filescanner:progress", serde_json::json!({
                "path_id": id,
                "scanned": 0,
                "total": 0,
                "done": true,
                "error": e,
            }));
        }
    });

    Ok(())
}

/// Search scanned files for content semantically similar to `query`.
/// Only searches paths where enabled = 1.
/// Returns an empty list if no enabled paths exist or nothing is relevant.
#[tauri::command]
pub async fn search_scanned_files(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, VectorDb>,
    query: String,
) -> Result<Vec<ScannedFileMatch>, String> {
    // Fast path: if no enabled paths exist, skip embedding entirely.
    let enabled_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM scanned_paths WHERE enabled = 1",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if enabled_count == 0 {
        return Ok(vec![]);
    }

    let model = get_embedding_model_pub(pool.inner()).await;
    let embedding = crate::vector::embed(
        &format!("search_query: {query}"),
        &model,
    )
    .await?;

    let all_matches = crate::vector::scanned_file_search(&vdb.0, embedding, 60).await?;

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
    .map_err(|e| e.to_string())?;

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
) -> Result<super::Note, String> {
    // Only supported types are allowed.
    let p = std::path::Path::new(&file_path);
    if mime_for_path(p).is_none() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("none");
        return Err(format!("Unsupported file type '.{ext}'"));
    }

    let content = read_file_text(p).map_err(|e| e.to_string())?;

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
    .map_err(|e| e.to_string())?;

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
