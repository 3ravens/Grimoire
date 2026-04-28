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

//! Wikipedia local knowledge source.
//!
//! Users can download Kiwix ZIM bundles (wikipedia, nopic flavour only),
//! index them into LanceDB for semantic search, and have Wikipedia articles
//! included as context in the RAG pipeline.
//!
//! Architecture:
//!   - SQLite: bundle metadata + checkpointing + highlights
//!   - LanceDB: one embedding per article (title + intro text, ≤1500 chars)
//!   - Ollama: same embedding model as notes (nomic-embed-text by default)
//!
//! Privacy contract: nothing leaves the machine. The catalogue fetch is the
//! only outbound network call. Downloads go to a user-specified local path.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use futures::StreamExt;
use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Shared structs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WikiBundle {
    pub id:             String,
    pub name:           String,
    pub flavour:        String,
    pub title:          Option<String>,
    pub article_count:  Option<i64>,
    pub size_bytes:     Option<i64>,
    pub zim_path:       Option<String>,
    pub installed_at:   Option<String>,
    pub last_synced:    Option<String>,
    pub indexing_state: String,
}

/// A catalogue entry returned from the Kiwix OPDS catalogue.
#[derive(Debug, Serialize)]
pub struct CatalogueEntry {
    pub id:            String,
    pub name:          String,
    pub title:         String,
    pub flavour:       String,
    pub article_count: Option<i64>,
    pub size_bytes:    Option<i64>,
    pub download_url:  Option<String>,
    pub sha256_url:    Option<String>,
}

/// A persisted user highlight inside a Wikipedia article.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WikiHighlight {
    pub id:               i64,
    pub highlighted_text: String,
    pub context_before:   Option<String>,
    pub context_after:    Option<String>,
    pub status:           String,
}

/// Internal row type for bundle lookup queries.
#[derive(sqlx::FromRow)]
struct BundleRow {
    id:       String,
    name:     String,
    title:    Option<String>,
    zim_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Strip HTML tags from a string, collapsing whitespace. Used to extract plain
/// text from ZIM article blobs before embedding.
fn html_to_text(html: &str) -> String {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    // Skip script, style, and nav elements — they add noise with no information.
    let skip_sel = Selector::parse("script, style, nav, .toc, #toc").unwrap();

    // Collect text nodes that are NOT inside skipped elements.
    let body_sel = Selector::parse("body").unwrap();
    let body = match document.select(&body_sel).next() {
        Some(b) => b,
        None => return String::new(),
    };

    // Traverse the body element tree and skip subtrees matching skip_sel.
    let skip_ids: std::collections::HashSet<_> = body
        .select(&skip_sel)
        .map(|el| el.id())
        .collect();

    let mut out = String::with_capacity(html.len() / 4);
    for node in body.descendants() {
        if let Some(el) = node.value().as_element() {
            // If this element should be skipped, we skip it and its descendants.
            let id = scraper::ElementRef::wrap(node).map(|e| e.id());
            if let Some(id) = id {
                if skip_ids.contains(&id) {
                    continue;
                }
            }
            // Insert newline after block elements so paragraphs stay separated.
            if matches!(
                el.name(),
                "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "dt" | "dd" | "br"
            ) {
                out.push('\n');
            }
        } else if let Some(text) = node.value().as_text() {
            out.push_str(text);
        }
    }

    // Normalise whitespace: collapse runs of whitespace (preserving newlines),
    // then collapse multiple blank lines down to one.
    let mut result = String::with_capacity(out.len());
    for line in out.lines() {
        let trimmed: String = line
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !trimmed.is_empty() {
            result.push_str(&trimmed);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

/// Parse the Kiwix OPDS Atom XML catalogue and return all nopic wikipedia entries.
/// The catalogue endpoint always returns Atom XML regardless of Accept header:
///   https://library.kiwix.org/catalog/v2/entries?lang=eng&category=wikipedia&count=500
fn parse_catalogue(xml: &str) -> Result<Vec<CatalogueEntry>, String> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| format!("Failed to parse catalogue XML: {e}"))?;

    let root = doc.root_element();
    let atom_ns = "http://www.w3.org/2005/Atom";
    let acq_rel = "http://opds-spec.org/acquisition/open-access";

    let mut result = Vec::new();

    for entry in root.children().filter(|n| n.has_tag_name((atom_ns, "entry"))) {
        let child_text = |tag: &str| -> &str {
            entry
                .children()
                .find(|n| n.has_tag_name((atom_ns, tag)))
                .and_then(|n| n.text())
                .unwrap_or("")
        };

        let flavour = child_text("flavour");
        if flavour != "nopic" {
            continue;
        }

        let raw_id = child_text("id");
        // IDs come as "urn:uuid:<uuid>" — strip the prefix.
        let id = raw_id.trim_start_matches("urn:uuid:").to_string();
        if id.is_empty() {
            continue;
        }

        let name  = child_text("name").to_string();
        let title = child_text("title").to_string();

        let article_count = child_text("articleCount")
            .parse::<i64>()
            .ok();

        // Download link: rel="http://opds-spec.org/acquisition/open-access"
        let mut download_url: Option<String> = None;
        let mut size_bytes:   Option<i64>    = None;

        for link in entry.children().filter(|n| n.has_tag_name((atom_ns, "link"))) {
            if link.attribute("rel") == Some(acq_rel) {
                if let Some(href) = link.attribute("href") {
                    // The href points to a .meta4 MetaLink descriptor.
                    // Strip the .meta4 suffix to get the direct .zim URL.
                    let direct = href.trim_end_matches(".meta4").to_string();
                    download_url = Some(direct);
                }
                if let Some(len) = link.attribute("length") {
                    size_bytes = len.parse::<i64>().ok();
                }
                break;
            }
        }

        result.push(CatalogueEntry {
            id,
            name,
            title,
            flavour: flavour.to_string(),
            article_count,
            size_bytes,
            download_url,
            sha256_url: None,
        });
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Fetch the Kiwix OPDS catalogue and return all nopic wikipedia entries.
/// This is the only command that makes an outbound network request.
#[tauri::command]
pub async fn fetch_wikipedia_catalogue() -> Result<Vec<CatalogueEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let url = "https://library.kiwix.org/catalog/v2/entries?lang=eng&category=wikipedia&count=500";
    let body = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch catalogue: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Failed to read catalogue response: {e}"))?;

    parse_catalogue(&body)
}

/// List all locally tracked wikipedia bundles.
#[tauri::command]
pub async fn list_wikipedia_bundles(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<WikiBundle>, String> {
    sqlx::query_as::<_, WikiBundle>(
        "SELECT id, name, flavour, title, article_count, size_bytes,
                zim_path, installed_at, last_synced, indexing_state
         FROM wikipedia_bundles ORDER BY title",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

/// Reset a bundle's indexing_state. Used by the frontend to clear stuck 'indexing'
/// states after an app restart.
#[tauri::command]
pub async fn set_bundle_indexing_state(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    state: String,
) -> Result<(), String> {
    // Only allow safe state values.
    if !matches!(state.as_str(), "none" | "queued" | "done" | "error") {
        return Err(format!("Invalid state: {state}"));
    }
    sqlx::query("UPDATE wikipedia_bundles SET indexing_state = ? WHERE id = ?")
        .bind(&state)
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Download a ZIM bundle from `download_url` to `dest_dir/filename`.
/// Streams the response body to disk. Emits `wikipedia:download-progress` events
/// with `{ bundle_id, downloaded_bytes, total_bytes }` every 512 KB.
///
/// On completion, inserts a row into `wikipedia_bundles` (or updates the existing one).
#[tauri::command]
pub async fn download_wikipedia_bundle(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    bundle_name: String,
    bundle_title: String,
    download_url: String,
    dest_dir: String,
    expected_size_bytes: Option<i64>,
    article_count: Option<i64>,
) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;

    // Validate dest_dir is an existing directory to prevent path traversal.
    let dir = std::path::Path::new(&dest_dir);
    if !dir.is_dir() {
        return Err(format!("Destination directory does not exist: {dest_dir}"));
    }

    // Derive filename from the URL.
    let filename = download_url
        .split('/')
        .last()
        .filter(|s| s.ends_with(".zim"))
        .ok_or("Download URL does not end with a .zim filename")?;

    let zim_path = dir.join(filename);
    let zim_path_str = zim_path
        .to_str()
        .ok_or("Destination path contains non-UTF8 characters")?
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download returned HTTP {}", resp.status()));
    }

    let total = resp.content_length().map(|l| l as i64).or(expected_size_bytes);

    let mut file = tokio::fs::File::create(&zim_path)
        .await
        .map_err(|e| format!("Failed to create file {zim_path_str}: {e}"))?;

    let mut downloaded: i64 = 0;
    let mut last_emit: i64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Download stream error: {e}"))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("Failed to write to file: {e}"))?;
        downloaded += bytes.len() as i64;
        if downloaded - last_emit >= 512 * 1024 {
            last_emit = downloaded;
            let _ = app.emit("wikipedia:download-progress", serde_json::json!({
                "bundle_id": bundle_id,
                "downloaded_bytes": downloaded,
                "total_bytes": total,
            }));
        }
    }

    file.flush().await.map_err(|e| format!("Failed to flush file: {e}"))?;

    let now = chrono_now();

    // Upsert the bundle record in SQLite.
    sqlx::query(
        "INSERT INTO wikipedia_bundles (id, name, flavour, title, article_count, size_bytes, zim_path, installed_at, indexing_state)
         VALUES (?, ?, 'nopic', ?, ?, ?, ?, ?, 'none')
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name, title = excluded.title,
             article_count = excluded.article_count,
             size_bytes = excluded.size_bytes, zim_path = excluded.zim_path,
             installed_at = excluded.installed_at, indexing_state = 'none'",
    )
    .bind(&bundle_id)
    .bind(&bundle_name)
    .bind(&bundle_title)
    .bind(article_count)
    .bind(downloaded)
    .bind(&zim_path_str)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(zim_path_str)
}

/// Remove a bundle: delete SQLite rows, remove from LanceDB, optionally delete the .zim file.
#[tauri::command]
pub async fn remove_wikipedia_bundle(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    bundle_id: String,
    delete_file: bool,
) -> Result<(), String> {
    // Fetch zim_path before deleting the row if we need to delete the file.
    let zim_path: Option<String> = if delete_file {
        sqlx::query_scalar("SELECT zim_path FROM wikipedia_bundles WHERE id = ?")
            .bind(&bundle_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?
            .flatten()
    } else {
        None
    };

    // Mark highlights as orphaned rather than deleting them.
    sqlx::query("UPDATE wikipedia_highlights SET status = 'orphaned' WHERE bundle_id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    // Delete bundle rows (cascade deletes checkpoint).
    sqlx::query("DELETE FROM wikipedia_bundles WHERE id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    // Remove from LanceDB.
    crate::vector::wikipedia_remove_bundle(&vdb.0, &bundle_id).await?;

    // Optionally delete the .zim file from disk.
    if delete_file {
        if let Some(path) = zim_path {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }

    Ok(())
}

/// Index (or re-index) a wikipedia bundle. Runs in a blocking task.
/// Emits `wikipedia:index-progress` events with
/// `{ bundle_id, indexed, total, done, error }` every 100 articles.
///
/// Resumes from the last checkpoint automatically.
/// Skips: redirects, stubs (<500 chars after HTML stripping), disambiguation pages.
#[tauri::command]
pub async fn index_wikipedia_bundle(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    bundle_id: String,
) -> Result<(), String> {
    // Look up the bundle.
    let bundle: WikiBundle = sqlx::query_as(
        "SELECT id, name, flavour, title, article_count, size_bytes,
                zim_path, installed_at, last_synced, indexing_state
         FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Bundle not found: {bundle_id}"))?;

    let zim_path = bundle.zim_path.ok_or("Bundle has no zim_path")?;

    // Mark as indexing.
    sqlx::query("UPDATE wikipedia_bundles SET indexing_state = 'indexing' WHERE id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    // Load checkpoint (resume offset + previously indexed count).
    let (start_entry, base_indexed): (u32, i64) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT last_indexed_entry, indexed_count FROM wikipedia_index_checkpoint WHERE bundle_id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .map(|(e, c)| (e as u32, c))
    .unwrap_or((0, 0));

    let pool_clone = pool.inner().clone();
    let vdb_conn  = vdb.0.clone();
    let bundle_id_clone = bundle_id.clone();
    let app_clone = app.clone();

    let result: Result<(), String> = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();

        use zim_rs::archive::Archive;
        let archive = Archive::new(&zim_path)
            .map_err(|_| format!("Failed to open ZIM file: {zim_path}"))?;

        let total_entries = archive.get_all_entrycount();

        // Upsert checkpoint row.
        rt.block_on(sqlx::query(
            "INSERT INTO wikipedia_index_checkpoint (bundle_id, last_indexed_entry, total_entries)
             VALUES (?, ?, ?)
             ON CONFLICT(bundle_id) DO UPDATE SET total_entries = excluded.total_entries",
        )
        .bind(&bundle_id_clone)
        .bind(start_entry as i64)
        .bind(total_entries as i64)
        .execute(&pool_clone))
        .map_err(|e| e.to_string())?;

        let model = rt.block_on(super::rag::get_embedding_model_pub(&pool_clone));

        let mut indexed = base_indexed;
        let mut last_checkpoint_idx = start_entry;

        // Process ZIM entries in sliding windows:
        //   Phase 1 — sequential ZIM reads      (libzim is not thread-safe)
        //   Phase 2 — parallel HTML→text parse  (rayon uses all CPU cores)
        //   Phase 3 — batched GPU embedding      (BATCH_SIZE texts per Ollama call)
        //   Phase 4 — bulk LanceDB upsert        (one delete+insert per batch)
        //
        // SCAN_WINDOW and BATCH_SIZE are tuned for a high-end machine
        // (16+ GB VRAM, 32 GB RAM, 8+ core CPU). A future adaptive path
        // will scale these down based on hardware detection at startup.
        const BATCH_SIZE: usize  = 64;   // texts per Ollama /api/embed call
        const SCAN_WINDOW: usize = 1024; // ZIM entries read per iteration

        let mut scan_pos = start_entry;
        while scan_pos < total_entries {
            let window_end = (scan_pos + SCAN_WINDOW as u32).min(total_entries);

            // ── Phase 1: sequential ZIM reads ──────────────────────────────────
            let raw: Vec<(u32, String, String, Vec<u8>)> =
                (scan_pos..window_end).filter_map(|idx| {
                    let entry = archive.get_entry_bypath_index(idx).ok()?;
                    if entry.is_redirect() { return None; }
                    let item = entry.get_item(false).ok()?;
                    if !item.get_mimetype().unwrap_or_default().starts_with("text/html") {
                        return None;
                    }
                    let html = item.get_data().ok()?.data().to_vec();
                    Some((idx, entry.get_path(), entry.get_title(), html))
                }).collect();

            // ── Phase 2: parallel HTML→text parse on all CPU cores ─────────────
            let articles: Vec<(u32, String, String, String, String)> = raw
                .into_par_iter()
                .filter_map(|(idx, path, title, html_bytes)| {
                    // Skip MediaWiki CSS/template/module pages by path prefix.
                    // In ZIM files these appear as paths starting with "." or "-/"
                    // or containing namespace prefixes like "MediaWiki:", "Module:".
                    let path_lower = path.to_lowercase();
                    if path.starts_with('.')
                        || path.starts_with("-/")
                        || path_lower.contains("mediawiki:")
                        || path_lower.contains("module:")
                        || path_lower.contains("template:")
                        || path_lower.contains("wikipedia:")
                        || path_lower.contains("file:")
                    {
                        return None;
                    }
                    let text = html_to_text(&String::from_utf8_lossy(&html_bytes));
                    if text.chars().count() < 500 { return None; }
                    // Skip CSS pages: content that is mostly stylesheet definitions.
                    if text.contains(".mw-parser-output") || text.starts_with(".mw-") || text.contains("/* start https://") {
                        return None;
                    }
                    // Skip disambiguation pages. Wikipedia marks them with
                    // "(disambiguation)" in the title / path. Do NOT check body
                    // text for the word "disambiguation" — nearly every real article
                    // contains it in a hatnote ("For other uses, see X (disambiguation).")
                    // and would be incorrectly filtered out.
                    let title_lower = title.to_lowercase();
                    if title_lower.ends_with("(disambiguation)")
                        || path_lower.contains("_(disambiguation)")
                    {
                        return None;
                    }
                    // "may refer to:" is the opening line of disambiguation pages.
                    // Only check the first ~300 bytes to avoid false positives in
                    // article body prose. Walk back from byte 300 to the nearest
                    // char boundary so multi-byte characters (e.g. en-dash) don't panic.
                    let cap = 300.min(text.len());
                    let head_end = (0..=cap).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
                    let text_head = &text[..head_end];
                    if text_head.contains("may refer to:") {
                        return None;
                    }
                    let content: String = text.chars().take(1500).collect();
                    let doc_text = format!("search_document: {title}\n{content}");
                    Some((idx, format!("{bundle_id_clone}/{path}"), title, content, doc_text))
                })
                .collect();

            // ── Phase 3 + 4: embed in batches, then bulk-upsert to LanceDB ──────
            for chunk in articles.chunks(BATCH_SIZE) {
                let doc_texts: Vec<String> =
                    chunk.iter().map(|(_, _, _, _, dt)| dt.clone()).collect();
                let embeddings =
                    match rt.block_on(crate::vector::embed_batch(&doc_texts, &model)) {
                        Ok(e) => e,
                        Err(_) => chunk.iter()
                            .map(|(_, _, _, _, dt)| {
                                rt.block_on(crate::vector::embed(dt, &model)).unwrap_or_default()
                            })
                            .collect(),
                    };

                // Collect the valid articles as a single batch for LanceDB.
                let upsert_batch: Vec<(String, String, String, String, Vec<f32>)> = chunk
                    .iter()
                    .zip(embeddings)
                    .filter_map(|((arc_idx, article_id, title, content, _), embedding)| {
                        if embedding.is_empty() { return None; }
                        last_checkpoint_idx = *arc_idx;
                        Some((article_id.clone(), bundle_id_clone.clone(), title.clone(), content.clone(), embedding))
                    })
                    .collect();

                indexed += upsert_batch.len() as i64;
                let _ = rt.block_on(crate::vector::wikipedia_upsert_batch(&vdb_conn, upsert_batch));
            }

            // ── Checkpoint + progress at each window boundary ──────────────────
            if !articles.is_empty() {
                let _ = rt.block_on(sqlx::query(
                    "UPDATE wikipedia_index_checkpoint
                     SET last_indexed_entry = ?, indexed_count = ? WHERE bundle_id = ?",
                )
                .bind(last_checkpoint_idx as i64 + 1)
                .bind(indexed)
                .bind(&bundle_id_clone)
                .execute(&pool_clone));
            }
            let _ = app_clone.emit("wikipedia:index-progress", serde_json::json!({
                "bundle_id": bundle_id_clone,
                "indexed": indexed,
                "scanned": window_end,
                "total": total_entries,
                "done": false,
                "error": null,
            }));

            scan_pos = window_end;
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Ok(()) => {
            let now = chrono_now();
            sqlx::query(
                "UPDATE wikipedia_bundles SET indexing_state = 'done', last_synced = ? WHERE id = ?",
            )
            .bind(&now)
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

            sqlx::query(
                "UPDATE wikipedia_index_checkpoint SET completed_at = ? WHERE bundle_id = ?",
            )
            .bind(&now)
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

            let _ = app.emit("wikipedia:index-progress", serde_json::json!({
                "bundle_id": bundle_id,
                "done": true,
                "error": null,
            }));

            Ok(())
        }
        Err(e) => {
            sqlx::query(
                "UPDATE wikipedia_bundles SET indexing_state = 'error' WHERE id = ?",
            )
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            .map_err(|err| err.to_string())?;

            let _ = app.emit("wikipedia:index-progress", serde_json::json!({
                "bundle_id": bundle_id,
                "done": true,
                "error": e,
            }));

            Err(e)
        }
    }
}

/// Semantic search over the indexed wikipedia articles.
/// Called by the RAG pipeline in Chat.svelte when wikipedia is enabled.
#[tauri::command]
pub async fn search_wikipedia(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    query: String,
) -> Result<Vec<crate::vector::WikiMatch>, String> {
    // Only search if wikipedia is enabled in settings.
    let enabled: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'wikipedia_enabled' LIMIT 1",
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or_default();

    if enabled != "true" {
        return Ok(vec![]);
    }

    let model = super::rag::get_embedding_model_pub(pool.inner()).await;
    let embedding = super::rag::embed_query(&query, &model).await?;
    crate::vector::wikipedia_search(&vdb.0, embedding, 5).await
}

/// Read a single article from a ZIM bundle by its entry path.
/// Returns the plain text (HTML stripped) for display in the frontend.
#[tauri::command]
pub async fn read_wikipedia_article(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    article_path: String,
) -> Result<serde_json::Value, String> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten()
    .ok_or_else(|| format!("Bundle not found or has no file: {bundle_id}"))?;

    let path_clone = article_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        use zim_rs::archive::Archive;
        let archive = Archive::new(&zim_path)
            .map_err(|_| format!("Failed to open ZIM file: {zim_path}"))?;

        let entry = archive
            .get_entry_bypath_str(&path_clone)
            .map_err(|_| format!("Article not found: {path_clone}"))?;

        let item = entry
            .get_item(true)
            .map_err(|_| "Failed to get article item".to_string())?;

        let blob = item.get_data().map_err(|_| "Failed to read article data".to_string())?;
        let html = String::from_utf8_lossy(blob.data()).to_string();
        let text = html_to_text(&html);

        Ok::<_, String>(serde_json::json!({
            "title": entry.get_title(),
            "path":  entry.get_path(),
            "text":  text,
        }))
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

// ---------------------------------------------------------------------------
// Article HTML reader
// ---------------------------------------------------------------------------

/// Rewrite `href` attributes in an HTML fragment for the Wikipedia reader.
///
/// Rules:
///   - External URLs (`http://`, `https://`, protocol-relative `//`) →
///     `href="#"` + `data-external="true"` + tooltip.
///   - Fragment-only (`#…`) or empty → kept as-is.
///   - All other (internal ZIM) links → `href="#"` +
///     `data-wiki-path="<normalised path>"`.
fn rewrite_hrefs_in_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 1024);
    let mut pos = 0;

    while pos < html.len() {
        match html[pos..].find("href=") {
            None => {
                out.push_str(&html[pos..]);
                break;
            }
            Some(rel) => {
                let href_start = pos + rel;
                // Emit everything before "href="
                out.push_str(&html[pos..href_start]);

                let after_eq = href_start + 5; // skip past "href="
                if after_eq >= html.len() {
                    out.push_str("href=");
                    pos = after_eq;
                    continue;
                }

                let quote = html.as_bytes()[after_eq];
                if quote != b'"' && quote != b'\'' {
                    // Unquoted attribute – copy and move on
                    out.push_str("href=");
                    pos = after_eq;
                    continue;
                }

                let q = quote as char;
                let val_start = after_eq + 1;

                match html[val_start..].find(q) {
                    None => {
                        // Unterminated attribute – copy as-is and bail
                        out.push_str("href=");
                        pos = after_eq;
                    }
                    Some(val_len) => {
                        let val_end = val_start + val_len;
                        let val = &html[val_start..val_end];

                        if val.starts_with("http://")
                            || val.starts_with("https://")
                            || val.starts_with("//")
                        {
                            // External: disable, mark for tooltip
                            out.push_str(
                                "href=\"#\" data-external=\"true\" title=\"External links are disabled\"",
                            );
                        } else if val.is_empty() || val.starts_with('#') {
                            // Fragment or empty – keep as-is
                            out.push_str(&format!("href=\"{val}\""));
                        } else {
                            // Internal wiki link – normalise to plain ZIM path
                            let clean = normalize_wiki_path(val);
                            // HTML-escape the path so it is safe in an attribute value
                            let escaped = clean
                                .replace('&', "&amp;")
                                .replace('"', "&quot;")
                                .replace('<', "&lt;")
                                .replace('>', "&gt;");
                            out.push_str(&format!(
                                "href=\"#\" data-wiki-path=\"{escaped}\""
                            ));
                        }

                        pos = val_end + 1; // skip past closing quote
                    }
                }
            }
        }
    }

    out
}

/// Strip the common ZIM path prefixes introduced by Kiwix to obtain the
/// plain article path (e.g. `./Photosynthesis` → `Photosynthesis`).
/// Preserves any `#fragment` suffix so section links still work.
fn normalize_wiki_path(href: &str) -> String {
    // Split off any fragment first
    let (path, fragment) = if let Some(hash) = href.find('#') {
        (&href[..hash], &href[hash..])
    } else {
        (href, "")
    };

    let clean = path
        .trim_start_matches("./")
        .trim_start_matches("../A/")
        .trim_start_matches("../")
        .trim_start_matches("/wiki/")
        .trim_start_matches("A/");

    if fragment.is_empty() {
        clean.to_string()
    } else {
        format!("{clean}{fragment}")
    }
}

/// Read a Wikipedia article and return sanitised HTML for the reader pane.
///
/// Only the `#mw-content-text` subtree is returned (page chrome stripped).
/// Internal `href` attributes are rewritten for in-app navigation;
/// external `href` values are disabled with a tooltip.
///
/// Returns `{ title, path, html }`.
#[tauri::command]
pub async fn read_wikipedia_article_html(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    article_path: String,
) -> Result<serde_json::Value, String> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten()
    .ok_or_else(|| format!("Bundle not found or has no file: {bundle_id}"))?;

    let path_clone = article_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        use zim_rs::archive::Archive;
        use scraper::{Html, Selector};

        let archive = Archive::new(&zim_path)
            .map_err(|_| format!("Failed to open ZIM file: {zim_path}"))?;

        let entry = archive
            .get_entry_bypath_str(&path_clone)
            .map_err(|_| format!("Article not found: {path_clone}"))?;

        let item = entry
            .get_item(true) // follow redirects
            .map_err(|_| "Failed to get article item".to_string())?;

        let blob = item
            .get_data()
            .map_err(|_| "Failed to read article data".to_string())?;

        let raw_html = String::from_utf8_lossy(blob.data()).to_string();
        let title     = entry.get_title();
        let final_path = item.get_path(); // may differ from entry path if redirect

        // Extract only the article body, discarding the page shell.
        let doc = Html::parse_document(&raw_html);
        let content_sel  = Selector::parse("#mw-content-text").unwrap();
        let body_sel     = Selector::parse("body").unwrap();

        let content_html = if let Some(el) = doc.select(&content_sel).next() {
            el.inner_html()
        } else if let Some(el) = doc.select(&body_sel).next() {
            el.inner_html()
        } else {
            raw_html.clone()
        };

        // Rewrite hrefs for in-app navigation
        let html_out = rewrite_hrefs_in_html(&content_html);

        Ok::<_, String>(serde_json::json!({
            "title": title,
            "path":  final_path,
            "html":  html_out,
        }))
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Serve a Wikipedia article image as a `data:<mime>;base64,…` string.
/// The image bytes are read directly from the ZIM file.
/// Returns an empty string if the image is not found (non-fatal).
#[tauri::command]
pub async fn serve_wikipedia_image(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    image_path: String,
) -> Result<String, String> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten()
    .ok_or_else(|| format!("Bundle not found: {bundle_id}"))?;

    tokio::task::spawn_blocking(move || {
        use zim_rs::archive::Archive;
        use base64::Engine;

        let archive = Archive::new(&zim_path)
            .map_err(|_| format!("Failed to open ZIM: {zim_path}"))?;

        // Try the path as given, then common ZIM prefixes
        let paths_to_try: &[&str] = &[
            &image_path,
        ];

        for path in paths_to_try {
            if let Ok(entry) = archive.get_entry_bypath_str(path) {
                if let Ok(item) = entry.get_item(true) {
                    let mime = item.get_mimetype().unwrap_or_default();
                    if let Ok(blob) = item.get_data() {
                        let b64 = base64::engine::general_purpose::STANDARD
                            .encode(blob.data());
                        return Ok(format!("data:{mime};base64,{b64}"));
                    }
                }
            }
        }

        // Image not found – return empty string so broken images are invisible
        Ok(String::new())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Check whether a Wikipedia article path resolves to any installed bundle.
///
/// Checks the current bundle first (short-circuits on first match), then
/// all other installed bundles in alphabetical order.
///
/// Returns `{ bundle_id, article_path, title, bundle_title }` if found,
/// or `null` if no installed bundle contains the article.
#[tauri::command]
pub async fn resolve_wikipedia_link(
    pool: State<'_, SqlitePool>,
    current_bundle_id: String,
    article_path: String,
) -> Result<Option<serde_json::Value>, String> {
    // Fetch all installed bundles, current bundle first to short-circuit early.
    let bundles: Vec<BundleRow> = sqlx::query_as(
        "SELECT id, name, title, zim_path
         FROM wikipedia_bundles
         WHERE zim_path IS NOT NULL
         ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, COALESCE(title, name)",
    )
    .bind(&current_bundle_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    for bundle in &bundles {
        let zim_path = match &bundle.zim_path {
            Some(p) if !p.is_empty() => p.clone(),
            _ => continue,
        };
        let bundle_id    = bundle.id.clone();
        let bundle_title = bundle.title.clone().unwrap_or_else(|| bundle.name.clone());
        let path         = article_path.clone();

        let found = tokio::task::spawn_blocking(move || {
            use zim_rs::archive::Archive;

            let archive = match Archive::new(&zim_path) {
                Ok(a)  => a,
                Err(_) => return None,
            };

            // Try the path as-is; also try with / without the "A/" namespace prefix
            // to handle both old-namespace and new-namespace ZIM files.
            let candidates: Vec<String> = {
                let mut v = vec![path.clone()];
                if path.starts_with("A/") {
                    v.push(path[2..].to_string()); // strip "A/"
                } else {
                    v.push(format!("A/{path}")); // add "A/"
                }
                v
            };

            for candidate in &candidates {
                if archive.has_entry_bypath(candidate) {
                    let title = archive
                        .get_entry_bypath_str(candidate)
                        .map(|e| e.get_title())
                        .unwrap_or_else(|_| candidate.clone());
                    return Some((bundle_id, candidate.clone(), title, bundle_title));
                }
            }

            None
        })
        .await
        .map_err(|e| e.to_string())?;

        if let Some((bid, apath, title, btitle)) = found {
            return Ok(Some(serde_json::json!({
                "bundle_id":    bid,
                "article_path": apath,
                "title":        title,
                "bundle_title": btitle,
            })));
        }
    }

    Ok(None)
}

/// Title-prefix autocomplete for articles within a single bundle.
/// Uses libzim's built-in suggestion index (title prefix search).
/// Returns up to 10 results matching the query prefix.
#[tauri::command]
pub async fn suggest_wikipedia_articles(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    query: String,
) -> Result<Vec<serde_json::Value>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten()
    .ok_or_else(|| format!("Bundle not found: {bundle_id}"))?;

    tokio::task::spawn_blocking(move || {
        use zim_rs::archive::Archive;
        use zim_rs::suggestion::SuggestionSearcher;

        let archive = Archive::new(&zim_path)
            .map_err(|_| "Failed to open ZIM".to_string())?;

        // ── 1. Title-prefix suggestion search ───────────────────────────────
        let mut suggestions = Vec::new();
        if let Ok(mut searcher) = SuggestionSearcher::new(&archive) {
            if let Ok(search) = searcher.suggest(&query) {
                if let Ok(result_set) = search.get_results(0, 10) {
                    for item_result in result_set {
                        if let Ok(item) = item_result {
                            suggestions.push(serde_json::json!({
                                "title": item.get_title(),
                                "path":  item.get_path(),
                            }));
                        }
                    }
                }
            }
        }

        // ── 2. Title-index binary search fallback ───────────────────────────
        // Used when the ZIM has no Xapian suggestion index (common for nopic ZIMs).
        // We binary-search the alphabetical title list, then scan forward collecting
        // entries whose title starts with the query prefix. This is O(log N + K).
        if suggestions.is_empty() {
            let query_lower = query.to_lowercase();
            let total = archive.get_entrycount();

            if total > 0 {
                // Binary search: find the first title-index position >= query_lower.
                let mut lo = 0u32;
                let mut hi = total;
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let title_lower = archive
                        .get_entry_bytitle_index(mid)
                        .map(|e| e.get_title().to_lowercase())
                        .unwrap_or_default();
                    if title_lower < query_lower {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }

                // Scan forward from lo.  Since titles are alphabetically ordered,
                // once we've seen 300 consecutive non-matching real entries after
                // the last hit we know we've left the prefix range.
                let mut skipped = 0u32;
                for idx in lo..total {
                    if suggestions.len() >= 10 || skipped > 300 {
                        break;
                    }
                    let Ok(entry) = archive.get_entry_bytitle_index(idx) else {
                        skipped += 1;
                        continue;
                    };
                    // Skip redirects and empty-title entries without counting them
                    // toward the skip budget (dense redirect runs would abort too early).
                    if entry.is_redirect() {
                        continue;
                    }
                    let title = entry.get_title();
                    if title.is_empty() {
                        continue;
                    }
                    if title.to_lowercase().starts_with(&query_lower) {
                        suggestions.push(serde_json::json!({
                            "title": title,
                            "path":  entry.get_path(),
                        }));
                        skipped = 0; // reset on each hit
                    } else {
                        skipped += 1;
                    }
                }
            }
        }

        Ok::<_, String>(suggestions)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Article highlights
// ---------------------------------------------------------------------------

/// Load all saved highlights for a specific Wikipedia article.
#[tauri::command]
pub async fn load_wikipedia_highlights(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    article_path: String,
) -> Result<Vec<WikiHighlight>, String> {
    sqlx::query_as::<_, WikiHighlight>(
        "SELECT id, highlighted_text, context_before, context_after, status
         FROM wikipedia_highlights
         WHERE bundle_id = ? AND article_path = ?
         ORDER BY id",
    )
    .bind(&bundle_id)
    .bind(&article_path)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

/// Save a new highlight for a Wikipedia article. Returns the new highlight ID.
#[tauri::command]
pub async fn save_wikipedia_highlight(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    article_path: String,
    highlighted_text: String,
    context_before: String,
    context_after: String,
) -> Result<i64, String> {
    let now = chrono_now();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO wikipedia_highlights
             (bundle_id, article_path, highlighted_text, context_before, context_after, created_at, status)
         VALUES (?, ?, ?, ?, ?, ?, 'active')
         RETURNING id",
    )
    .bind(&bundle_id)
    .bind(&article_path)
    .bind(&highlighted_text)
    .bind(&context_before)
    .bind(&context_after)
    .bind(&now)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

/// Delete a Wikipedia highlight by ID.
#[tauri::command]
pub async fn delete_wikipedia_highlight(
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM wikipedia_highlights WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// PoC command (debug-only)
// ---------------------------------------------------------------------------

/// Open a ZIM file at `zim_path`, iterate its entries, and return a JSON
/// report that lets us evaluate whether the `zim` crate is usable.
/// This command is debug-only — it is not registered in release builds.
#[tauri::command]
pub async fn test_zim_parse(zim_path: String) -> Result<serde_json::Value, String> {
    // ZIM parsing is blocking I/O + CPU; keep it off the async executor.
    tokio::task::spawn_blocking(move || run_zim_poc(&zim_path))
        .await
        .map_err(|e| e.to_string())?
}

fn run_zim_poc(zim_path: &str) -> Result<serde_json::Value, String> {
    use zim_rs::archive::Archive;

    let archive = Archive::new(zim_path)
        .map_err(|_| format!("Failed to open ZIM file: {zim_path}"))?;

    let total_entries = archive.get_all_entrycount();
    let article_count_header = archive.get_articlecount();
    let has_new_ns = archive.has_new_namespace_scheme();

    let range = archive
        .iter_efficient()
        .map_err(|_| "Failed to create efficient iterator".to_string())?;

    let mut article_count = 0usize;
    let mut redirect_count = 0usize;
    let mut samples: Vec<serde_json::Value> = Vec::new();

    for entry_result in range {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.is_redirect() {
            redirect_count += 1;
            continue;
        }

        let item = match entry.get_item(false) {
            Ok(i) => i,
            Err(_) => continue,
        };

        let mime = item.get_mimetype().unwrap_or_default();
        if !mime.starts_with("text/html") {
            continue;
        }

        article_count += 1;

        if samples.len() < 5 {
            let content_preview = match item.get_data() {
                Ok(blob) => String::from_utf8_lossy(blob.data())
                    .chars()
                    .take(600)
                    .collect::<String>(),
                Err(_) => "(blob error)".to_string(),
            };

            samples.push(serde_json::json!({
                "title":           entry.get_title(),
                "path":            entry.get_path(),
                "byte_count":      item.get_size(),
                "content_preview": content_preview,
            }));
        }

        if article_count >= 500 {
            break;
        }
    }

    Ok(serde_json::json!({
        "total_entries":        total_entries,
        "article_count_header": article_count_header,
        "article_count":        article_count,
        "redirect_count":       redirect_count,
        "has_new_namespace":    has_new_ns,
        "samples":              samples,
    }))
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn chrono_now() -> String {
    // Use SystemTime since we can't add the `chrono` crate dependency.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as a basic ISO-8601-like timestamp (UTC, seconds precision).
    let secs_in_day = secs % 86400;
    let days = secs / 86400;
    // Days since Unix epoch to calendar date (Gregorian proleptic calendar).
    let (year, month, day) = days_to_ymd(days);
    let h = secs_in_day / 3600;
    let m = (secs_in_day % 3600) / 60;
    let s = secs_in_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
