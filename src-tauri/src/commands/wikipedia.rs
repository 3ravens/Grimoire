
//! Wikipedia local knowledge source.
//!
//! Users can download Kiwix ZIM bundles (wikipedia, nopic flavour only),
//! index them into LanceDB for semantic search, and have Wikipedia articles
//! included as context in the RAG pipeline.
//!
//! Architecture:
//!   - SQLite: bundle metadata + checkpointing + highlights + FTS5 over indexed articles
//!   - LanceDB: one embedding per article (title + intro text, ≤1500 chars)
//!   - Ollama: same embedding model as notes (nomic-embed-text by default)
//!
//! Privacy contract: nothing leaves the machine. The catalogue fetch is the
//! only outbound network call. Downloads go to a user-specified local path.

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tauri::{AppHandle, Emitter, State};
use tauri::Manager;
use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use rayon::prelude::*;
use std::io::Write;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use crate::{AppError, AppResult};
use crate::config::SharedConfig;

// ---------------------------------------------------------------------------
// Wikipedia SQLite FTS5 (lexical fallback, blended with LanceDB in search_wikipedia)
// ---------------------------------------------------------------------------

const WIKI_FTS_RRF_K: f64 = 60.0;

fn wiki_rrf_score(rank: usize) -> f64 {
    1.0 / (WIKI_FTS_RRF_K + rank as f64)
}

/// Best-effort: populate FTS rows for one indexing window (same articles as phase 2).
/// Uses one SQLite transaction per call so inserts stay bounded and predictable.
async fn wiki_fts_insert_articles_batch(
    pool: &SqlitePool,
    bundle_id: &str,
    articles: &[(u32, String, String, String)],
) {
    if articles.is_empty() {
        return;
    }
    let Ok(mut tx) = pool.begin().await else {
        return;
    };
    let mut qb = QueryBuilder::<Sqlite>::new(
        "INSERT INTO wikipedia_articles_fts(article_id, bundle_id, article_path, title, content) ",
    );
    qb.push_values(articles.iter(), |mut b, (_, article_id, title, content)| {
        let article_path = article_id
            .strip_prefix(bundle_id)
            .and_then(|s| s.strip_prefix('/'))
            .unwrap_or(article_id.as_str());
        b.push_bind(article_id)
            .push_bind(bundle_id)
            .push_bind(article_path)
            .push_bind(title)
            .push_bind(content);
    });
    let _ = qb.build().execute(&mut *tx).await;
    let _ = tx.commit().await;
}

async fn wiki_fts_backfill_chunk(
    pool: &SqlitePool,
    bundle_id: &str,
    rows: Vec<(String, String, String)>,
) {
    if rows.is_empty() {
        return;
    }
    let Ok(mut tx) = pool.begin().await else {
        return;
    };
    let mut qb = QueryBuilder::<Sqlite>::new(
        "INSERT INTO wikipedia_articles_fts(article_id, bundle_id, article_path, title, content) ",
    );
    qb.push_values(rows.into_iter(), |mut b, (article_id, title, content)| {
        let article_path = article_id
            .strip_prefix(bundle_id)
            .and_then(|s| s.strip_prefix('/'))
            .unwrap_or(article_id.as_str())
            .to_string();
        b.push_bind(article_id)
            .push_bind(bundle_id)
            .push_bind(article_path)
            .push_bind(title)
            .push_bind(content);
    });
    let _ = qb.build().execute(&mut *tx).await;
    let _ = tx.commit().await;
}

async fn wiki_append_with_salvage(
    conn: &lancedb::Connection,
    rows: Vec<(String, String, String, String, Vec<f32>)>,
    max_retries: i64,
    cancel: &Arc<std::sync::atomic::AtomicBool>,
    lance_retry_extra_out: &mut u32,
) -> Result<(i64, i64), AppError> {
    if rows.is_empty() {
        return Ok((0, 0));
    }
    const MIN_SPLIT_BATCH: usize = 64;
    const LANCE_RETRY_MAX_DELAY_MS: u64 = 800;
    let mut indexed: i64 = 0;
    let mut skipped: i64 = 0;
    let mut queue = vec![rows];
    while let Some(batch) = queue.pop() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AppError::InvalidInput("Indexing cancelled".to_string()));
        }
        let batch_len = batch.len();
        let append_result = crate::retry::with_retries_counting_background(
            max_retries,
            Some((
                cancel,
                AppError::InvalidInput("Indexing cancelled".to_string()),
            )),
            LANCE_RETRY_MAX_DELAY_MS,
            || {
                let b = batch.clone();
                async move {
                    crate::vector::wiki::wikipedia_append_batch(conn, b).await.map_err(|e| {
                        AppError::VectorStore(format!("Failed to append wikipedia window batch: {e}"))
                    })
                }
            },
        )
        .await;
        match append_result {
            Ok(((), attempt)) => {
                *lance_retry_extra_out += attempt.saturating_sub(1);
                indexed += batch_len as i64;
            }
            Err(AppError::InvalidInput(m)) if m == "Indexing cancelled" => {
                return Err(AppError::InvalidInput(m));
            }
            Err(e) => {
                if batch_len >= MIN_SPLIT_BATCH {
                    let mid = batch_len / 2;
                    let left = batch[..mid].to_vec();
                    let right = batch[mid..].to_vec();
                    queue.push(right);
                    queue.push(left);
                } else {
                    log::warn!(
                        "wikipedia_append_batch failed after retries ({} articles): {}",
                        batch_len,
                        e
                    );
                    skipped += batch_len as i64;
                }
            }
        }
    }
    Ok((indexed, skipped))
}

#[derive(Debug, sqlx::FromRow)]
struct WikiFtsHit {
    article_id: String,
    bundle_id: String,
    title: String,
    snippet: String,
}

async fn wiki_fts_search_inner(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
) -> Result<Vec<WikiFtsHit>, sqlx::Error> {
    let fts_query = super::search::build_fts_query(query);
    if fts_query.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, WikiFtsHit>(
        r#"SELECT article_id, bundle_id, title,
                  snippet(wikipedia_articles_fts, 4, '<b>', '</b>', '...', 32) AS snippet
           FROM wikipedia_articles_fts
           WHERE wikipedia_articles_fts MATCH ?
           ORDER BY bm25(wikipedia_articles_fts)
           LIMIT ?"#,
    )
    .bind(&fts_query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
}

/// One-shot backfill: for each fully indexed bundle with no FTS rows, copy title+content
/// from LanceDB into SQLite FTS (no Ollama / re-embed).
pub async fn wikipedia_fts_initial_sync(pool: &SqlitePool, conn: &lancedb::Connection) {
    let bundles: Vec<String> = match sqlx::query_scalar::<_, String>(
        "SELECT id FROM wikipedia_bundles WHERE indexing_state = 'done'",
    )
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            log::warn!("wikipedia_fts_initial_sync: failed to list bundles: {e}");
            return;
        }
    };

    let pool = pool.clone();
    for bid in bundles {
        let fts_n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wikipedia_articles_fts WHERE bundle_id = ?",
        )
        .bind(&bid)
        .fetch_one(&pool)
        .await
        .unwrap_or(-1);
        if fts_n > 0 {
            continue;
        }

        let pool_inner = pool.clone();
        let bid_inner = bid.clone();
        if let Err(e) =
            crate::vector::wiki::for_each_wikipedia_bundle_batch(conn, &bid, |rows| {
                let handle = tokio::runtime::Handle::current();
                let pool_c = pool_inner.clone();
                let b = bid_inner.clone();
                handle.block_on(wiki_fts_backfill_chunk(&pool_c, &b, rows));
            })
            .await
        {
            log::warn!("wikipedia_fts_initial_sync: bundle {bid}: {e}");
        }
    }
}

fn resolve_wiki_perf_log_path(app: &AppHandle) -> Option<PathBuf> {
    let base = app.path().app_data_dir().ok()?;
    let logs = base.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    Some(logs.join("wiki-index-perf.log"))
}

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

    static SKIP_SELECTOR: OnceLock<Selector> = OnceLock::new();
    static BODY_SELECTOR: OnceLock<Selector> = OnceLock::new();

    let document = Html::parse_document(html);

    // Skip script, style, and nav elements — they add noise with no information.
    let skip_sel = SKIP_SELECTOR
        .get_or_init(|| Selector::parse("script, style, nav, .toc, #toc").expect("valid skip selector"));

    // Collect text nodes that are NOT inside skipped elements.
    let body_sel = BODY_SELECTOR
        .get_or_init(|| Selector::parse("body").expect("valid body selector"));
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
fn parse_catalogue(xml: &str) -> AppResult<Vec<CatalogueEntry>> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| AppError::InvalidInput(format!("Failed to parse catalogue XML: {e}")))?;

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
// Network + disk preflight (catalogue fetch / bundle download)
// ---------------------------------------------------------------------------

const KIWIX_HOST_ROOT: &str = "https://library.kiwix.org/";
const KIWIX_CATALOGUE_URL: &str =
    "https://library.kiwix.org/catalog/v2/entries?lang=eng&category=wikipedia&count=500";

/// Result of a lightweight connectivity probe to Kiwix.
#[derive(Debug, Serialize)]
pub struct WikipediaConnectivityResult {
    pub online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Successful download preflight (disk space OK).
#[derive(Debug, Serialize)]
pub struct WikipediaDownloadPreflightOk {
    pub ok: bool,
    pub required_bytes: i64,
    pub available_bytes: u64,
}

fn map_wikipedia_transport_error(e: &reqwest::Error) -> AppError {
    if e.is_timeout() || e.is_connect() {
        AppError::Io(crate::error::WIKIPEDIA_OFFLINE_MSG.to_string())
    } else {
        AppError::Io(format!("Network error: {e}"))
    }
}

/// Probe Kiwix reachability before catalogue fetch or download.
async fn ensure_wikipedia_network_available() -> AppResult<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Io(format!("Failed to build HTTP client: {e}")))?;

    client
        .head(KIWIX_HOST_ROOT)
        .send()
        .await
        .map_err(|e| map_wikipedia_transport_error(&e))?;

    Ok(())
}

/// Upper-bound estimate for the `.zim` file size when the catalogue omits `length`.
fn wikipedia_estimated_zim_bytes(expected_size_bytes: Option<i64>) -> i64 {
    const DEFAULT_UNKNOWN_ZIM: i64 = 5 * 1024 * 1024 * 1024; // 5 GiB conservative
    expected_size_bytes
        .unwrap_or(DEFAULT_UNKNOWN_ZIM)
        .max(16 * 1024 * 1024)
}

/// Extra bytes reserved for LanceDB index + SQLite FTS + write amplification during indexing.
fn wikipedia_download_overhead_bytes(zim_bytes: i64) -> i64 {
    const MIN_OVERHEAD: i64 = 512 * 1024 * 1024; // 512 MiB floor
    let pct = zim_bytes / 4;
    MIN_OVERHEAD.max(pct)
}

fn fmt_bytes_human(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 {
        return format!("{gb:.1} GB");
    }
    let mb = bytes as f64 / 1e6;
    format!("{mb:.0} MB")
}

/// Free space on the volume that contains `dest_dir` (best-effort via mount-point prefix match).
fn disk_available_bytes_for_dir(dest_dir: &Path) -> AppResult<u64> {
    use sysinfo::Disks;

    let dest_str = dest_dir.to_str().ok_or_else(|| {
        AppError::InvalidInput("Destination path must be valid UTF-8 for disk space check".to_string())
    })?;
    let dest_lower = dest_str.to_lowercase();

    let disks = Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for disk in disks.list() {
        let Some(mount_str) = disk.mount_point().to_str() else {
            continue;
        };
        let mount_lower = mount_str.to_lowercase();
        if dest_lower.starts_with(&mount_lower) {
            let len = mount_lower.len();
            let avail = disk.available_space();
            if best.map(|(bl, _)| len > bl).unwrap_or(true) {
                best = Some((len, avail));
            }
        }
    }

    best.map(|(_, b)| b).ok_or_else(|| {
        AppError::Io(
            "Could not determine free disk space for the selected folder. Try an absolute path (e.g. C:\\Users\\…)."
                .to_string(),
        )
    })
}

fn assert_wikipedia_download_disk_space(dest_dir: &Path, expected_size_bytes: Option<i64>) -> AppResult<()> {
    let zim = wikipedia_estimated_zim_bytes(expected_size_bytes);
    let overhead = wikipedia_download_overhead_bytes(zim);
    let required = (zim as i128).saturating_add(overhead as i128);
    let required_u64 = u64::try_from(required).unwrap_or(u64::MAX);

    let available = disk_available_bytes_for_dir(dest_dir)?;
    if available < required_u64 {
        return Err(AppError::InvalidInput(format!(
            "Not enough free disk space (about {} required including index overhead; about {} available).",
            fmt_bytes_human(required_u64),
            fmt_bytes_human(available),
        )));
    }
    Ok(())
}

/// Lightweight probe for UI; does not throw on failure — returns `{ online: false, message }`.
#[tauri::command]
pub async fn check_wikipedia_connectivity() -> AppResult<WikipediaConnectivityResult> {
    match ensure_wikipedia_network_available().await {
        Ok(()) => Ok(WikipediaConnectivityResult {
            online: true,
            message: None,
        }),
        Err(e) => Ok(WikipediaConnectivityResult {
            online: false,
            message: Some(e.to_string()),
        }),
    }
}

/// Hard-block preflight: returns `Ok(details)` only if there is enough free space.
#[tauri::command]
pub async fn check_wikipedia_download_preflight(
    dest_dir: String,
    expected_size_bytes: Option<i64>,
) -> AppResult<WikipediaDownloadPreflightOk> {
    let dir = Path::new(&dest_dir);
    if !dir.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Destination directory does not exist: {dest_dir}"
        )));
    }

    let zim = wikipedia_estimated_zim_bytes(expected_size_bytes);
    let overhead = wikipedia_download_overhead_bytes(zim);
    let required = (zim as i128).saturating_add(overhead as i128);
    let required_i64 = i64::try_from(required).unwrap_or(i64::MAX);
    let required_u64 = u64::try_from(required).unwrap_or(u64::MAX);

    let available = disk_available_bytes_for_dir(dir)?;
    if available < required_u64 {
        return Err(AppError::InvalidInput(format!(
            "Not enough free disk space (about {} required including index overhead; about {} available).",
            fmt_bytes_human(required_u64),
            fmt_bytes_human(available),
        )));
    }

    Ok(WikipediaDownloadPreflightOk {
        ok: true,
        required_bytes: required_i64,
        available_bytes: available,
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Fetch the Kiwix OPDS catalogue and return all nopic wikipedia entries.
/// This is the only command that makes an outbound network request.
#[tauri::command]
pub async fn fetch_wikipedia_catalogue() -> AppResult<Vec<CatalogueEntry>> {
    ensure_wikipedia_network_available().await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Io(format!("Failed to build HTTP client: {e}")))?;

    let body = client
        .get(KIWIX_CATALOGUE_URL)
        .send()
        .await
        .map_err(|e| map_wikipedia_transport_error(&e))?
        .error_for_status()
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                map_wikipedia_transport_error(&e)
            } else {
                AppError::Io(format!("Catalogue request failed: {e}"))
            }
        })?
        .text()
        .await
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                map_wikipedia_transport_error(&e)
            } else {
                AppError::Io(format!("Failed to read catalogue response: {e}"))
            }
        })?;

    parse_catalogue(&body)
}

/// List all locally tracked wikipedia bundles.
#[tauri::command]
pub async fn list_wikipedia_bundles(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<WikiBundle>> {
    sqlx::query_as::<_, WikiBundle>(
        "SELECT id, name, flavour, title, article_count, size_bytes,
                zim_path, installed_at, last_synced, indexing_state
         FROM wikipedia_bundles ORDER BY title",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(Into::into)
}

/// Reset a bundle's indexing_state. Used by the frontend to clear stuck 'indexing'
/// states after an app restart.
#[tauri::command]
pub async fn set_bundle_indexing_state(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    state: String,
) -> AppResult<()> {
    // Only allow safe state values.
    if !matches!(state.as_str(), "none" | "done" | "error") {
        return Err(AppError::InvalidInput(format!("Invalid state: {state}")));
    }
    sqlx::query("UPDATE wikipedia_bundles SET indexing_state = ? WHERE id = ?")
        .bind(&state)
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        ?;
    Ok(())
}

/// Request cancellation for an in-progress bundle indexing run.
///
/// This is idempotent: if no in-flight task is registered, the command still
/// updates the persisted state so the UI can recover from stale "indexing"
/// markers after crashes/restarts.
#[tauri::command]
pub async fn cancel_wikipedia_indexing(
    pool: State<'_, SqlitePool>,
    cancel_map: State<'_, super::CancelMap>,
    bundle_id: String,
) -> AppResult<()> {
    {
        let map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        if let Some(flag) = map.get(&bundle_id) {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    sqlx::query("UPDATE wikipedia_bundles SET indexing_state = 'none' WHERE id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        ?;

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
) -> AppResult<String> {
    use tokio::io::AsyncWriteExt;

    // Validate dest_dir is an existing directory to prevent path traversal.
    let dir = Path::new(&dest_dir);
    if !dir.is_dir() {
        return Err(AppError::InvalidInput(format!("Destination directory does not exist: {dest_dir}")));
    }

    ensure_wikipedia_network_available().await?;
    assert_wikipedia_download_disk_space(dir, expected_size_bytes)?;

    // Derive filename from the URL.
    let filename = download_url
        .split('/')
        .last()
        .filter(|s| s.ends_with(".zim"))
        .ok_or_else(|| AppError::InvalidInput("Download URL does not end with a .zim filename".to_string()))?;

    let zim_path = dir.join(filename);
    let zim_path_str = zim_path
        .to_str()
        .ok_or_else(|| AppError::InvalidInput("Destination path contains non-UTF8 characters".to_string()))?
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|e| AppError::Io(format!("Failed to build HTTP client: {e}")))?;

    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| map_wikipedia_transport_error(&e))?;

    if !resp.status().is_success() {
        return Err(AppError::Io(format!("Download returned HTTP {}", resp.status())));
    }

    let total = resp.content_length().map(|l| l as i64).or(expected_size_bytes);

    let mut file = tokio::fs::File::create(&zim_path)
        .await
        .map_err(|e| AppError::Io(format!("Failed to create file {zim_path_str}: {e}")))?;

    let mut downloaded: i64 = 0;
    let mut last_emit: i64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                map_wikipedia_transport_error(&e)
            } else {
                AppError::Io(format!("Download stream error: {e}"))
            }
        })?;
        file.write_all(&bytes)
            .await
            .map_err(|e| AppError::Io(format!("Failed to write to file: {e}")))?;
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

    file.flush().await.map_err(|e| AppError::Io(format!("Failed to flush file: {e}")))?;

    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

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
    ?;

    Ok(zim_path_str)
}

/// Remove a bundle: delete SQLite rows, remove from LanceDB, optionally delete the .zim file.
#[tauri::command]
pub async fn remove_wikipedia_bundle(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    cancel_map: State<'_, super::CancelMap>,
    bundle_id: String,
    delete_file: bool,
) -> AppResult<()> {
    // Signal cancellation for any in-progress indexing of this bundle.
    {
        let map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        if let Some(flag) = map.get(&bundle_id) {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // Fetch zim_path before deleting the row if we need to delete the file.
    let zim_path: Option<String> = if delete_file {
        sqlx::query_scalar("SELECT zim_path FROM wikipedia_bundles WHERE id = ?")
            .bind(&bundle_id)
            .fetch_optional(pool.inner())
            .await
            ?
            .flatten()
    } else {
        None
    };

    // Mark highlights as orphaned rather than deleting them.
    sqlx::query("UPDATE wikipedia_highlights SET status = 'orphaned' WHERE bundle_id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        ?;

    let _ = sqlx::query("DELETE FROM wikipedia_articles_fts WHERE bundle_id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await;

    // Delete bundle rows (cascade deletes checkpoint).
    sqlx::query("DELETE FROM wikipedia_bundles WHERE id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        ?;

    // Remove from LanceDB.
    crate::vector::wiki::wikipedia_remove_bundle(&vdb.0, &bundle_id).await.map_err(|e| AppError::VectorStore(e))?;

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
/// Skips: redirects, HTML meta-refresh soft redirects, stubs (<200 chars after HTML stripping), disambiguation pages.
#[tauri::command]
pub async fn index_wikipedia_bundle(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    cancel_map: State<'_, super::CancelMap>,
    config: State<'_, SharedConfig>,
    indexing_plan: State<'_, Arc<crate::indexing_profile::IndexingThroughputPlan>>,
    bundle_id: String,
    reset: Option<bool>,
) -> AppResult<()> {
    crate::vector::embedder::reset_embed_batch_telemetry();

    // Look up the bundle.
    let bundle: WikiBundle = sqlx::query_as(
        "SELECT id, name, flavour, title, article_count, size_bytes,
                zim_path, installed_at, last_synced, indexing_state
         FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .ok_or_else(|| AppError::NotFound(format!("Bundle not found: {bundle_id}")))?;

    let zim_path = bundle.zim_path.ok_or_else(|| AppError::NotFound("Bundle has no zim_path".to_string()))?;

    // Mark as indexing.
    sqlx::query("UPDATE wikipedia_bundles SET indexing_state = 'indexing' WHERE id = ?")
        .bind(&bundle_id)
        .execute(pool.inner())
        .await
        ?;

    // Register a cancel flag so remove_bundle can stop this indexing run.
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let mut map = cancel_map.0.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;
        map.insert(bundle_id.clone(), cancel.clone());
    }

    // If resetting, clear checkpoint and LanceDB entries to start fresh.
    if reset.unwrap_or(false) {
        sqlx::query("DELETE FROM wikipedia_index_checkpoint WHERE bundle_id = ?")
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            ?;
        let _ = sqlx::query("DELETE FROM wikipedia_articles_fts WHERE bundle_id = ?")
            .bind(&bundle_id)
            .execute(pool.inner())
            .await;
        crate::vector::wiki::wikipedia_remove_bundle(&vdb.0, &bundle_id).await.map_err(|e| AppError::VectorStore(e))?;
    }

    // Load checkpoint (resume offset + previously indexed count).
    let (start_entry, base_indexed): (u32, i64) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT last_indexed_entry, indexed_count FROM wikipedia_index_checkpoint WHERE bundle_id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .map(|(e, c)| (e as u32, c))
    .unwrap_or((0, 0));

    let pool_clone = pool.inner().clone();
    let vdb_conn  = vdb.0.clone();
    let bundle_id_clone = bundle_id.clone();
    let app_clone = app.clone();
    let embedding_model = config.read().unwrap().embedding_model.clone();
    let perf_logging_enabled = config.read().unwrap().wiki_perf_logging;
    let perf_log_path: Option<PathBuf> = if perf_logging_enabled {
        resolve_wiki_perf_log_path(&app)
    } else {
        None
    };
    let cancel_clone = cancel.clone();

    let max_retries = config.read().unwrap().background_max_retries;
    let permanently_skipped_arc = Arc::new(AtomicI64::new(0));
    let permanently_skipped_inner = permanently_skipped_arc.clone();
    let indexing_plan_spawn = indexing_plan.inner().clone();

    let result: AppResult<()> = tokio::task::spawn_blocking(move || {
        let plan = &*indexing_plan_spawn;
        let rt = tokio::runtime::Handle::current();
        use zim_rs::archive::Archive;
        let archive = Archive::new(&zim_path)
            .map_err(|_| AppError::Io(format!("Failed to open ZIM file: {zim_path}")))?;

        let total_entries = archive.get_all_entrycount();
        let article_count = bundle.article_count
            .map(|c| c as u32)
            .unwrap_or_else(|| archive.get_articlecount());

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
        ?;

        let model = embedding_model;
        let embed_bulk_opts = crate::vector::embedder::EmbedBatchOptions {
            skip_ollama_entry_eviction: true,
            ..Default::default()
        };

        rt.block_on(crate::vector::embedder::evict_ollama_models_except(&model));

        let mut indexed = base_indexed;
        let mut last_checkpoint_idx = start_entry;

        // Process ZIM entries in sliding windows:
        //   Phase 1 — sequential ZIM reads      (libzim is not thread-safe)
        //   Phase 2 — parallel HTML→text parse  (rayon uses all CPU cores)
        //   Phase 3 — batched GPU embedding      (BATCH_SIZE texts per Ollama call)
        //   Phase 4 — bulk LanceDB upsert        (one delete+insert per batch)
        //
        // Scan window, embed batch caps, and Lance chunk sizes are scaled by
        // [`crate::indexing_profile::IndexingThroughputPlan`] from host hardware.
        let base_batch_size: usize = plan.embed_cap_for_model(&model).max(8);
        let content_chars: usize = crate::vector::embedder::content_chars_for_model(&model);
        let scan_window: u32 = plan.wiki_scan_window;
        let embed_ceiling: usize = plan.wiki_dynamic_embed_ceiling;
        let lance_chunk_rows: usize = plan.wiki_lance_initial_chunk_rows;
        // Re-check Ollama for competing models every N completed windows; bulk embed
        // skips per-batch eviction (see EmbedBatchOptions::skip_ollama_entry_eviction).
        const WIKI_INDEX_OLLAMA_EVICT_EVERY_WINDOWS: u64 = 10;

        let mut window_idx: u64 = 0;
        let mut total_read_ms: u128 = 0;
        let mut total_parse_ms: u128 = 0;
        let mut total_embed_ms: u128 = 0;
        let mut total_upsert_ms: u128 = 0;
        let mut total_checkpoint_emit_ms: u128 = 0;

        let mut scan_pos = start_entry;
        while scan_pos < total_entries {
            if cancel_clone.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(AppError::InvalidInput("Indexing cancelled".to_string()));
            }
            if window_idx > 0 && window_idx % WIKI_INDEX_OLLAMA_EVICT_EVERY_WINDOWS == 0 {
                rt.block_on(crate::vector::embedder::evict_ollama_models_except(&model));
            }
            let window_end = (scan_pos + scan_window).min(total_entries);

            // ── Phase 1: sequential ZIM reads ──────────────────────────────────
            let phase_read_t0 = Instant::now();
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
            let read_ms = phase_read_t0.elapsed().as_millis();

            // ── Phase 2: parallel HTML→text parse on all CPU cores ─────────────
            let phase_parse_t0 = Instant::now();
            let articles: Vec<(u32, String, String, String)> = {
                let parse = || {
                    raw.into_par_iter()
                        .filter_map(|(idx, path, title, html_bytes)| {
                    // Skip MediaWiki CSS/template/module pages by path prefix.
                    // In ZIM files these appear as paths starting with "-/"
                    // or containing namespace prefixes like "MediaWiki:", "Module:".
                    // Paths starting with "." alone (not "./") are internal metadata.
                    let path_lower = path.to_lowercase();
                    if (path.starts_with('.') && !path.starts_with("./"))
                        || path.starts_with("-/")
                        || path_lower.contains("mediawiki:")
                        || path_lower.contains("module:")
                        || path_lower.contains("template:")
                        || path_lower.contains("wikipedia:")
                        || path_lower.contains("file:")
                    {
                        return None;
                    }
                    // Skip HTML soft-redirect pages (meta refresh, not caught by
                    // ZIM-level is_redirect). These have no article content.
                    // Check small pages for the refresh pattern to skip them
                    // before the expensive html_to_text call.
                    if html_bytes.len() < 800 {
                        let s = String::from_utf8_lossy(&html_bytes).to_lowercase();
                        if s.contains("http-equiv") && s.contains("refresh") {
                            return None;
                        }
                    }
                    let text = html_to_text(&String::from_utf8_lossy(&html_bytes));
                    let content: String = text.chars().take(content_chars).collect();
                    if content.chars().count() < 200 { return None; }
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
                    Some((idx, format!("{bundle_id_clone}/{path}"), title, content))
                })
                .collect()
                };
                if let Some(parse_pool) = plan.wiki_parse_pool() {
                    parse_pool.install(parse)
                } else {
                    parse()
                }
            };
            let parse_ms = phase_parse_t0.elapsed().as_millis();

            let _ = rt.block_on(wiki_fts_insert_articles_batch(
                &pool_clone,
                &bundle_id_clone,
                &articles,
            ));

            // ── Phase 3 + 4: embed in batches, then bulk-upsert to LanceDB ──────
            let (split_win_t0, single_win_t0) = crate::vector::embedder::snapshot_embed_batch_telemetry();
            let mut embed_ms: u128 = 0;
            let mut upsert_ms: u128 = 0;
            let mut batch_count: u64 = 0;
            let mut window_embed_retries_extra: u32 = 0;
            let mut window_lance_retries_extra: u32 = 0;
            let mut window_upsert_batch: Vec<(String, String, String, String, Vec<f32>)> = Vec::new();
            let mut window_last_checkpoint_idx: u32 = last_checkpoint_idx;
            let mut dynamic_batch_size = base_batch_size;
            let mut batch_cursor = 0usize;
            while batch_cursor < articles.len() {
                let chunk_end = (batch_cursor + dynamic_batch_size).min(articles.len());
                let chunk = &articles[batch_cursor..chunk_end];
                batch_count += 1;
                let doc_texts: Vec<String> =
                    chunk
                        .iter()
                        .map(|(_, _, title, content)| format!("search_document: {title}\n{content}"))
                        .collect();
                let phase_embed_t0 = Instant::now();
                let (splits_before, singles_before) = crate::vector::embedder::snapshot_embed_batch_telemetry();
                const EMBED_RETRY_MAX_DELAY_MS: u64 = 800;
                let embeddings: Vec<Vec<f32>> = match rt.block_on(
                    crate::retry::with_retries_counting_background(
                        max_retries,
                        Some((
                            &cancel_clone,
                            AppError::InvalidInput("Indexing cancelled".to_string()),
                        )),
                        EMBED_RETRY_MAX_DELAY_MS,
                        || async {
                            crate::vector::embedder::embed_batch_with_options(
                                &doc_texts,
                                &model,
                                embed_bulk_opts.clone(),
                            )
                            .await
                            .map_err(AppError::EmbeddingFailed)
                        },
                    ),
                ) {
                    Ok((embs, attempt)) => {
                        window_embed_retries_extra += attempt.saturating_sub(1);
                        embs
                    }
                    Err(AppError::InvalidInput(m)) if m == "Indexing cancelled" => {
                        return Err(AppError::InvalidInput(m));
                    }
                    Err(_) => {
                        let mut out = Vec::with_capacity(chunk.len());
                        for (_, _, title, content) in chunk.iter() {
                            if cancel_clone.load(std::sync::atomic::Ordering::Relaxed) {
                                return Err(AppError::InvalidInput("Indexing cancelled".to_string()));
                            }
                            let doc_text = format!("search_document: {title}\n{content}");
                            match rt.block_on(crate::retry::with_retries_counting_background(
                                max_retries,
                                Some((
                                    &cancel_clone,
                                    AppError::InvalidInput("Indexing cancelled".to_string()),
                                )),
                                EMBED_RETRY_MAX_DELAY_MS,
                                || async {
                                    crate::vector::embedder::embed_with_keep_alive(&doc_text, &model, 300)
                                        .await
                                        .map_err(AppError::EmbeddingFailed)
                                },
                            )) {
                                Ok((v, attempt)) => {
                                    window_embed_retries_extra += attempt.saturating_sub(1);
                                    if v.is_empty() {
                                        permanently_skipped_inner.fetch_add(1, Ordering::Relaxed);
                                        out.push(Vec::new());
                                    } else {
                                        out.push(v);
                                    }
                                }
                                Err(AppError::InvalidInput(m)) if m == "Indexing cancelled" => {
                                    return Err(AppError::InvalidInput(m));
                                }
                                Err(_) => {
                                    permanently_skipped_inner.fetch_add(1, Ordering::Relaxed);
                                    out.push(Vec::new());
                                }
                            }
                        }
                        out
                    }
                };
                embed_ms += phase_embed_t0.elapsed().as_millis();
                let (splits_after, singles_after) = crate::vector::embedder::snapshot_embed_batch_telemetry();
                let split_delta = splits_after.saturating_sub(splits_before);
                let single_delta = singles_after.saturating_sub(singles_before);

                // Collect the valid articles as a single batch for LanceDB.
                let phase_upsert_t0 = Instant::now();
                let upsert_batch: Vec<(String, String, String, String, Vec<f32>)> = chunk
                    .iter()
                    .zip(embeddings)
                    .filter_map(|((arc_idx, article_id, title, content), embedding)| {
                        if embedding.is_empty() { return None; }
                        window_last_checkpoint_idx = *arc_idx;
                        Some((article_id.clone(), bundle_id_clone.clone(), title.clone(), content.clone(), embedding))
                    })
                    .collect();

                window_upsert_batch.extend(upsert_batch);
                upsert_ms += phase_upsert_t0.elapsed().as_millis();
                if split_delta > 0 || single_delta > 0 {
                    dynamic_batch_size = (dynamic_batch_size / 2).max(8);
                } else if dynamic_batch_size < base_batch_size {
                    dynamic_batch_size = (dynamic_batch_size + 8).min(base_batch_size);
                } else {
                    dynamic_batch_size = (dynamic_batch_size + 8).min(embed_ceiling);
                }
                batch_cursor = chunk_end;
            }

            let (split_win_t1, single_win_t1) = crate::vector::embedder::snapshot_embed_batch_telemetry();
            let window_split_delta =
                split_win_t1.saturating_sub(split_win_t0);
            let window_single_fallback_delta =
                single_win_t1.saturating_sub(single_win_t0);

            if !window_upsert_batch.is_empty() {
                let lance_chunks: Vec<Vec<(String, String, String, String, Vec<f32>)>> =
                    if window_upsert_batch.len() <= lance_chunk_rows {
                        vec![window_upsert_batch]
                    } else {
                        window_upsert_batch
                            .chunks(lance_chunk_rows)
                            .map(|c| c.to_vec())
                            .collect()
                    };
                for window_chunk in lance_chunks {
                    let chunk_len = window_chunk.len() as i64;
                    let phase_chunk_t0 = Instant::now();
                    let append_result = rt.block_on(wiki_append_with_salvage(
                        &vdb_conn,
                        window_chunk,
                        max_retries,
                        &cancel_clone,
                        &mut window_lance_retries_extra,
                    ));
                    upsert_ms += phase_chunk_t0.elapsed().as_millis();

                    match append_result {
                        Ok((indexed_ok, skipped)) => {
                            indexed += indexed_ok;
                            if skipped > 0 {
                                permanently_skipped_inner.fetch_add(skipped, Ordering::Relaxed);
                            }
                            last_checkpoint_idx = window_last_checkpoint_idx;
                        }
                        Err(AppError::InvalidInput(m)) if m == "Indexing cancelled" => {
                            return Err(AppError::InvalidInput(m));
                        }
                        Err(e) => {
                            log::warn!("wikipedia_append_with_salvage failed: {}", e);
                            permanently_skipped_inner.fetch_add(chunk_len, Ordering::Relaxed);
                            last_checkpoint_idx = window_last_checkpoint_idx;
                        }
                    }
                }
            }

            // ── Checkpoint + progress at each window boundary ──────────────────
            let phase_checkpoint_t0 = Instant::now();
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
            let checkpoint_ms = phase_checkpoint_t0.elapsed().as_millis();

            let phase_emit_t0 = Instant::now();
            let (batch_splits, single_fallbacks) = crate::vector::embedder::snapshot_embed_batch_telemetry();
            let mut payload = serde_json::json!({
                "bundle_id": bundle_id_clone,
                "indexed": indexed,
                "scanned": window_end,
                "total": total_entries,
                "article_count": article_count,
                "permanently_skipped": permanently_skipped_inner.load(Ordering::Relaxed),
                "batch_splits": batch_splits,
                "single_fallbacks": single_fallbacks,
                "done": false,
                "error": null,
            });
            if perf_logging_enabled {
                payload["timings_ms"] = serde_json::json!({
                    "read": read_ms,
                    "parse": parse_ms,
                    "embed": embed_ms,
                    "upsert": upsert_ms,
                    "checkpoint": checkpoint_ms,
                });
                payload["batch_count"] = serde_json::json!(batch_count);
                payload["last_window_articles"] = serde_json::json!(articles.len());
                payload["last_window_embed_ms"] = serde_json::json!(embed_ms);
                payload["last_window_embed_retries_extra"] = serde_json::json!(window_embed_retries_extra);
                payload["last_window_lance_retries_extra"] = serde_json::json!(window_lance_retries_extra);
                payload["last_window_split_delta"] = serde_json::json!(window_split_delta);
                payload["last_window_single_fallback_delta"] = serde_json::json!(window_single_fallback_delta);
            }
            let _ = app_clone.emit("wikipedia:index-progress", payload);
            let emit_ms = phase_emit_t0.elapsed().as_millis();
            let checkpoint_emit_ms = checkpoint_ms + emit_ms;

            window_idx += 1;
            total_read_ms += read_ms;
            total_parse_ms += parse_ms;
            total_embed_ms += embed_ms;
            total_upsert_ms += upsert_ms;
            total_checkpoint_emit_ms += checkpoint_emit_ms;

            if perf_logging_enabled && window_idx % 10 == 0 {
                let avg_read = total_read_ms as f64 / window_idx as f64;
                let avg_parse = total_parse_ms as f64 / window_idx as f64;
                let avg_embed = total_embed_ms as f64 / window_idx as f64;
                let avg_upsert = total_upsert_ms as f64 / window_idx as f64;
                let avg_checkpoint_emit = total_checkpoint_emit_ms as f64 / window_idx as f64;
                log::info!(
                    "[wiki_index_perf] bundle={} windows={} avg_ms read={:.1} parse={:.1} embed={:.1} upsert={:.1} checkpoint_emit={:.1} \
                     last_win_embed_ms={} last_win_articles={} last_win_batches={} embed_retries_x={} lance_retries_x={} split_d={} single_fb_d={}",
                    bundle_id_clone,
                    window_idx,
                    avg_read,
                    avg_parse,
                    avg_embed,
                    avg_upsert,
                    avg_checkpoint_emit,
                    embed_ms,
                    articles.len(),
                    batch_count,
                    window_embed_retries_extra,
                    window_lance_retries_extra,
                    window_split_delta,
                    window_single_fallback_delta,
                );

                if let Some(path) = perf_log_path.as_ref() {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                    {
                        let _ = writeln!(
                            file,
                            "[wiki_index_perf] bundle={} windows={} avg_ms read={:.1} parse={:.1} embed={:.1} upsert={:.1} checkpoint_emit={:.1} \
                             last_win_embed_ms={} last_win_articles={} last_win_batches={} embed_retries_x={} lance_retries_x={} split_d={} single_fb_d={}",
                            bundle_id_clone,
                            window_idx,
                            avg_read,
                            avg_parse,
                            avg_embed,
                            avg_upsert,
                            avg_checkpoint_emit,
                            embed_ms,
                            articles.len(),
                            batch_count,
                            window_embed_retries_extra,
                            window_lance_retries_extra,
                            window_split_delta,
                            window_single_fallback_delta,
                        );
                    }
                }
            }

            scan_pos = window_end;
        }

        Ok(())
    })
    .await
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?;

    // Clean up the cancel flag regardless of outcome.
    if let Ok(mut map) = cancel_map.0.lock() {
        map.remove(&bundle_id);
    }

    match result {
        Ok(()) => {
            let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            sqlx::query(
                "UPDATE wikipedia_bundles SET indexing_state = 'done', last_synced = ? WHERE id = ?",
            )
            .bind(&now)
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            ?;

            sqlx::query(
                "UPDATE wikipedia_index_checkpoint SET completed_at = ? WHERE bundle_id = ?",
            )
            .bind(&now)
            .bind(&bundle_id)
            .execute(pool.inner())
            .await
            ?;

            let (batch_splits, single_fallbacks) = crate::vector::embedder::snapshot_embed_batch_telemetry();
            let _ = app.emit("wikipedia:index-progress", serde_json::json!({
                "bundle_id": bundle_id,
                "batch_splits": batch_splits,
                "single_fallbacks": single_fallbacks,
                "permanently_skipped": permanently_skipped_arc.load(Ordering::Relaxed),
                "done": true,
                "error": null,
            }));

            Ok(())
        }
        Err(e) => {
            // If cancelled, the bundle has already been removed — don't try to update state.
            let is_cancelled = matches!(&e, AppError::InvalidInput(m) if m == "Indexing cancelled");
            if is_cancelled {
                sqlx::query(
                    "UPDATE wikipedia_bundles SET indexing_state = 'none' WHERE id = ?",
                )
                .bind(&bundle_id)
                .execute(pool.inner())
                .await
                ?;

                let (batch_splits, single_fallbacks) = crate::vector::embedder::snapshot_embed_batch_telemetry();
                let _ = app.emit("wikipedia:index-progress", serde_json::json!({
                    "bundle_id": bundle_id,
                    "batch_splits": batch_splits,
                    "single_fallbacks": single_fallbacks,
                    "permanently_skipped": permanently_skipped_arc.load(Ordering::Relaxed),
                    "done": true,
                    "error": null,
                }));
            } else {
                sqlx::query(
                    "UPDATE wikipedia_bundles SET indexing_state = 'error' WHERE id = ?",
                )
                .bind(&bundle_id)
                .execute(pool.inner())
                .await
                ?;

                let (batch_splits, single_fallbacks) = crate::vector::embedder::snapshot_embed_batch_telemetry();
                let _ = app.emit("wikipedia:index-progress", serde_json::json!({
                    "bundle_id": bundle_id,
                    "batch_splits": batch_splits,
                    "single_fallbacks": single_fallbacks,
                    "permanently_skipped": permanently_skipped_arc.load(Ordering::Relaxed),
                    "done": true,
                    "error": e.to_string(),
                }));
            }

            if is_cancelled {
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

/// Lexical + semantic search over indexed Wikipedia articles, merged via RRF.
/// Called by the RAG pipeline in Chat.svelte when wikipedia is enabled.
#[tauri::command]
pub async fn search_wikipedia(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    query: String,
) -> AppResult<Vec<crate::vector::wiki::WikiMatch>> {
    const CANDIDATE_LIMIT: usize = 40;
    const FINAL_LIMIT: usize = 5;

    // Only search if wikipedia is enabled in settings.
    if !config.read().unwrap().wikipedia_enabled {
        return Ok(vec![]);
    }

    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }

    let fts_fut = wiki_fts_search_inner(pool.inner(), &query, CANDIDATE_LIMIT);
    let sem_fut = async {
        let model = config.read().unwrap().embedding_model.clone();
        match super::rag::embed_query(&query, &model).await {
            Ok(emb) => crate::vector::wiki::wikipedia_search(&vdb.0, emb, CANDIDATE_LIMIT)
                .await
                .unwrap_or_default(),
            Err(_) => vec![],
        }
    };

    let (fts_rows, semantic) = tokio::join!(fts_fut, sem_fut);
    let fts_rows = fts_rows.unwrap_or_default();

    struct Entry {
        article_id: String,
        bundle_id: String,
        title: String,
        score: f64,
        fts: bool,
        sem: bool,
        fts_snippet: Option<String>,
        sem_distance: Option<f32>,
        sem_excerpts: Vec<String>,
    }

    let mut entries: HashMap<String, Entry> = HashMap::new();

    for (rank, r) in fts_rows.iter().enumerate() {
        let e = entries.entry(r.article_id.clone()).or_insert(Entry {
            article_id: r.article_id.clone(),
            bundle_id: r.bundle_id.clone(),
            title: r.title.clone(),
            score: 0.0,
            fts: false,
            sem: false,
            fts_snippet: None,
            sem_distance: None,
            sem_excerpts: vec![],
        });
        e.score += wiki_rrf_score(rank + 1);
        e.fts = true;
        e.fts_snippet = Some(r.snippet.clone());
        e.title = r.title.clone();
    }

    for (rank, m) in semantic.iter().enumerate() {
        let e = entries.entry(m.article_id.clone()).or_insert(Entry {
            article_id: m.article_id.clone(),
            bundle_id: m.bundle_id.clone(),
            title: m.title.clone(),
            score: 0.0,
            fts: false,
            sem: false,
            fts_snippet: None,
            sem_distance: None,
            sem_excerpts: vec![],
        });
        e.score += wiki_rrf_score(rank + 1);
        e.sem = true;
        e.sem_distance = Some(m.distance);
        if e.sem_excerpts.is_empty() {
            e.sem_excerpts = m.excerpts.clone();
        }
        if e.title.is_empty() {
            e.title = m.title.clone();
        }
    }

    let mut scored: Vec<(f64, crate::vector::wiki::WikiMatch)> = entries
        .into_values()
        .map(|e| {
            let score = e.score;
            let mut excerpts = e.sem_excerpts;
            if let Some(s) = e.fts_snippet {
                if !s.is_empty() && !excerpts.iter().any(|x| x == &s) {
                    excerpts.insert(0, s);
                }
            }
            if excerpts.is_empty() {
                excerpts.push(String::new());
            }
            let m = crate::vector::wiki::WikiMatch {
                article_id: e.article_id,
                bundle_id: e.bundle_id,
                title: e.title,
                excerpts,
                distance: e.sem_distance.unwrap_or(0.0),
            };
            (score, m)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(FINAL_LIMIT);
    let out: Vec<crate::vector::wiki::WikiMatch> = scored.into_iter().map(|(_, m)| m).collect();

    let _ = crate::audit::log_event(
        pool.inner(),
        "search_wikipedia",
        Some("wikipedia"),
        None,
        None,
        Some(&query),
    )
    .await;

    Ok(out)
}

/// Read a single article from a ZIM bundle by its entry path.
/// Returns the plain text (HTML stripped) for display in the frontend.
#[tauri::command]
pub async fn read_wikipedia_article(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    article_path: String,
) -> AppResult<serde_json::Value> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .flatten()
    .ok_or_else(|| AppError::NotFound(format!("Bundle not found or has no file: {bundle_id}")))?;

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
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?;

    let result = result.map_err(|e| AppError::NotFound(e))?;
    let title = result.get("title").and_then(|t| t.as_str()).map(|s| s.to_string());
    let _ = crate::audit::log_event(
        pool.inner(), "wikipedia_read", Some("wikipedia"),
        None, title.as_deref(), Some(&article_path),
    ).await;
    Ok(result)
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
) -> AppResult<serde_json::Value> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .flatten()
    .ok_or_else(|| AppError::NotFound(format!("Bundle not found or has no file: {bundle_id}")))?;

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
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?;

    let result = result.map_err(|e| AppError::NotFound(e))?;
    Ok(result)
}
/// Returns an empty string if the image is not found (non-fatal).
#[tauri::command]
pub async fn serve_wikipedia_image(
    pool: State<'_, SqlitePool>,
    bundle_id: String,
    image_path: String,
) -> AppResult<String> {
    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .flatten()
    .ok_or_else(|| AppError::NotFound(format!("Bundle not found: {bundle_id}")))?;

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
        Ok::<_, String>(String::new())
    })
    .await
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?
    .map_err(|e| AppError::Io(e))
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
) -> AppResult<Option<serde_json::Value>> {
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
    ?;

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
        .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?;

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
) -> AppResult<Vec<serde_json::Value>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let zim_path: String = sqlx::query_scalar(
        "SELECT zim_path FROM wikipedia_bundles WHERE id = ?",
    )
    .bind(&bundle_id)
    .fetch_optional(pool.inner())
    .await
    ?
    .flatten()
    .ok_or_else(|| AppError::NotFound(format!("Bundle not found: {bundle_id}")))?;

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
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))?
    .map_err(|e| AppError::Io(e))
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
) -> AppResult<Vec<WikiHighlight>> {
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
    .map_err(Into::into)
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
) -> AppResult<i64> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
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
    ?;

    Ok(id)
}

/// Delete a Wikipedia highlight by ID.
#[tauri::command]
pub async fn delete_wikipedia_highlight(
    pool: State<'_, SqlitePool>,
    id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM wikipedia_highlights WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;
    Ok(())
}

// ---------------------------------------------------------------------------
// PoC command (debug-only)
// ---------------------------------------------------------------------------

/// Open a ZIM file at `zim_path`, iterate its entries, and return a JSON
/// report that lets us evaluate whether the `zim` crate is usable.
/// This command is debug-only — it is not registered in release builds.
#[tauri::command]
pub async fn test_zim_parse(zim_path: String) -> AppResult<serde_json::Value> {
    // ZIM parsing is blocking I/O + CPU; keep it off the async executor.
    let result = tokio::task::spawn_blocking(move || run_zim_poc(&zim_path))
        .await
        .map_err(|e| AppError::Io(format!("Task panicked: {e}")))??;
    Ok(result)
}

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
pub struct WikiEvalCase {
    pub query: String,
    pub expected_article_ids: Vec<String>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
pub struct WikiEvalSummary {
    pub total_cases: usize,
    pub recall_at_k: f64,
    pub mrr_at_k: f64,
    pub hits_at_k: usize,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
pub struct WikiEvalCaseResult {
    pub query: String,
    pub hit: bool,
    pub reciprocal_rank: f64,
    pub matched_article_id: Option<String>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
pub struct WikiIndexBenchmarkResult {
    pub model: String,
    pub total_entries_in_zim: u32,
    pub benchmark_entries: u32,
    pub scanned_entries: u32,
    pub accepted_articles: u32,
    pub embedded_articles: u32,
    pub windows: u32,
    pub total_ms: u128,
    pub read_ms: u128,
    pub parse_ms: u128,
    pub embed_ms: u128,
    pub entries_per_sec: f64,
    pub accepted_per_sec: f64,
    pub embedded_per_sec: f64,
}

/// Benchmark helper for developer mode:
/// evaluates wikipedia retrieval quality on a fixed query set using Recall@K and MRR@K.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn benchmark_wikipedia_quality(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, SharedConfig>,
    cases: Vec<WikiEvalCase>,
    k: Option<usize>,
) -> AppResult<serde_json::Value> {
    let k = k.unwrap_or(5).clamp(1, 20);
    let mut hits = 0usize;
    let mut mrr_sum = 0.0f64;
    let mut per_case: Vec<WikiEvalCaseResult> = Vec::with_capacity(cases.len());

    for case in cases {
        let query = case.query.trim().to_string();
        if query.is_empty() {
            per_case.push(WikiEvalCaseResult {
                query: case.query,
                hit: false,
                reciprocal_rank: 0.0,
                matched_article_id: None,
            });
            continue;
        }
        let expected: std::collections::HashSet<String> = case.expected_article_ids.into_iter().collect();
        if expected.is_empty() {
            per_case.push(WikiEvalCaseResult {
                query: case.query,
                hit: false,
                reciprocal_rank: 0.0,
                matched_article_id: None,
            });
            continue;
        }

        let fts_fut = wiki_fts_search_inner(pool.inner(), &query, 40);
        let sem_fut = async {
            let model = config.read().unwrap().embedding_model.clone();
            match super::rag::embed_query(&query, &model).await {
                Ok(emb) => crate::vector::wiki::wikipedia_search(&vdb.0, emb, 40)
                    .await
                    .unwrap_or_default(),
                Err(_) => vec![],
            }
        };
        let (fts_rows, semantic) = tokio::join!(fts_fut, sem_fut);
        let fts_rows = fts_rows.unwrap_or_default();

        let mut scores: HashMap<String, f64> = HashMap::new();
        for (rank, r) in fts_rows.iter().enumerate() {
            *scores.entry(r.article_id.clone()).or_insert(0.0) += wiki_rrf_score(rank + 1);
        }
        for (rank, r) in semantic.iter().enumerate() {
            *scores.entry(r.article_id.clone()).or_insert(0.0) += wiki_rrf_score(rank + 1);
        }
        let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(k);

        let mut hit = false;
        let mut reciprocal_rank = 0.0;
        let mut matched_article_id: Option<String> = None;
        for (idx, (article_id, _)) in ranked.iter().enumerate() {
            if expected.contains(article_id) {
                hit = true;
                reciprocal_rank = 1.0 / (idx + 1) as f64;
                matched_article_id = Some(article_id.clone());
                break;
            }
        }
        if hit {
            hits += 1;
            mrr_sum += reciprocal_rank;
        }
        per_case.push(WikiEvalCaseResult {
            query: case.query,
            hit,
            reciprocal_rank,
            matched_article_id,
        });
    }

    let total = per_case.len().max(1);
    let summary = WikiEvalSummary {
        total_cases: per_case.len(),
        recall_at_k: hits as f64 / total as f64,
        mrr_at_k: mrr_sum / total as f64,
        hits_at_k: hits,
    };
    Ok(serde_json::json!({
        "summary": summary,
        "cases": per_case,
        "k": k
    }))
}

/// Developer benchmark helper for indexing performance regression checks.
///
/// Runs a bounded wikipedia indexing simulation (read + parse + embed) over a local
/// ZIM file without writing to SQLite/LanceDB. Use this to snapshot a stable
/// baseline and detect performance regressions after feature changes.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn benchmark_wikipedia_indexing(
    config: State<'_, SharedConfig>,
    indexing_plan: State<'_, Arc<crate::indexing_profile::IndexingThroughputPlan>>,
    zim_path: String,
    max_entries: Option<u32>,
) -> AppResult<WikiIndexBenchmarkResult> {
    let embedding_model = config.read().unwrap().embedding_model.clone();
    let scan_budget = max_entries.unwrap_or(20_000).max(1);
    let model_for_task = embedding_model.clone();
    let path_for_task = zim_path.clone();
    let plan_arc = indexing_plan.inner().clone();

    let result = tokio::task::spawn_blocking(move || {
        let plan = &*plan_arc;
        use zim_rs::archive::Archive;

        let rt = tokio::runtime::Handle::current();
        let archive = Archive::new(&path_for_task)
            .map_err(|_| AppError::Io(format!("Failed to open ZIM file: {path_for_task}")))?;

        let total_entries = archive.get_all_entrycount();
        let limit = total_entries.min(scan_budget);
        let batch_size: usize = plan.embed_cap_for_model(&model_for_task).max(8);
        let content_chars: usize = crate::vector::embedder::content_chars_for_model(&model_for_task);
        let scan_window = plan.wiki_scan_window;

        let mut scan_pos: u32 = 0;
        let mut windows: u32 = 0;
        let mut scanned_entries: u32 = 0;
        let mut accepted_articles: u32 = 0;
        let mut embedded_articles: u32 = 0;
        let mut total_read_ms: u128 = 0;
        let mut total_parse_ms: u128 = 0;
        let mut total_embed_ms: u128 = 0;
        let total_t0 = Instant::now();

        let embed_bulk_opts = crate::vector::embedder::EmbedBatchOptions {
            skip_ollama_entry_eviction: true,
            ..Default::default()
        };
        rt.block_on(crate::vector::embedder::evict_ollama_models_except(&model_for_task));

        while scan_pos < limit {
            if windows > 0 && windows % 10 == 0 {
                rt.block_on(crate::vector::embedder::evict_ollama_models_except(&model_for_task));
            }
            let window_end = (scan_pos + scan_window).min(limit);
            let window_len = window_end - scan_pos;
            scanned_entries = scanned_entries.saturating_add(window_len);

            let phase_read_t0 = Instant::now();
            let raw: Vec<(String, String, Vec<u8>)> = (scan_pos..window_end)
                .filter_map(|idx| {
                    let entry = archive.get_entry_bypath_index(idx).ok()?;
                    if entry.is_redirect() {
                        return None;
                    }
                    let item = entry.get_item(false).ok()?;
                    if !item.get_mimetype().unwrap_or_default().starts_with("text/html") {
                        return None;
                    }
                    let html = item.get_data().ok()?.data().to_vec();
                    Some((entry.get_path(), entry.get_title(), html))
                })
                .collect();
            total_read_ms += phase_read_t0.elapsed().as_millis();

            let phase_parse_t0 = Instant::now();
            let articles: Vec<(String, String)> = {
                let parse = || {
                    raw.into_par_iter()
                        .filter_map(|(path, title, html_bytes)| {
                    let path_lower = path.to_lowercase();
                    if (path.starts_with('.') && !path.starts_with("./"))
                        || path.starts_with("-/")
                        || path_lower.contains("mediawiki:")
                        || path_lower.contains("module:")
                        || path_lower.contains("template:")
                        || path_lower.contains("wikipedia:")
                        || path_lower.contains("file:")
                    {
                        return None;
                    }
                    if html_bytes.len() < 800 {
                        let s = String::from_utf8_lossy(&html_bytes).to_lowercase();
                        if s.contains("http-equiv") && s.contains("refresh") {
                            return None;
                        }
                    }
                    let text = html_to_text(&String::from_utf8_lossy(&html_bytes));
                    let content: String = text.chars().take(content_chars).collect();
                    if content.chars().count() < 200 {
                        return None;
                    }
                    let title_lower = title.to_lowercase();
                    if title_lower.ends_with("(disambiguation)")
                        || path_lower.contains("_(disambiguation)")
                    {
                        return None;
                    }
                    Some((title, content))
                })
                .collect()
                };
                if let Some(parse_pool) = plan.wiki_parse_pool() {
                    parse_pool.install(parse)
                } else {
                    parse()
                }
            };
            total_parse_ms += phase_parse_t0.elapsed().as_millis();

            accepted_articles = accepted_articles.saturating_add(articles.len() as u32);

            let phase_embed_t0 = Instant::now();
            for chunk in articles.chunks(batch_size) {
                let doc_texts: Vec<String> = chunk
                    .iter()
                    .map(|(title, content)| format!("search_document: {title}\n{content}"))
                    .collect();
                let n = match rt.block_on(crate::vector::embedder::embed_batch_with_options(
                    &doc_texts,
                    &model_for_task,
                    embed_bulk_opts.clone(),
                )) {
                    Ok(v) => v.into_iter().filter(|e| !e.is_empty()).count(),
                    Err(_) => {
                        let mut ok = 0usize;
                        for t in &doc_texts {
                            if let Ok(v) = rt.block_on(crate::vector::embedder::embed_with_keep_alive(
                                t,
                                &model_for_task,
                                300,
                            )) {
                                if !v.is_empty() {
                                    ok += 1;
                                }
                            }
                        }
                        ok
                    }
                };
                embedded_articles = embedded_articles.saturating_add(n as u32);
            }
            total_embed_ms += phase_embed_t0.elapsed().as_millis();

            windows = windows.saturating_add(1);
            scan_pos = window_end;
        }

        let total_ms = total_t0.elapsed().as_millis();
        let total_secs = (total_ms as f64 / 1000.0).max(0.001);
        Ok::<WikiIndexBenchmarkResult, AppError>(WikiIndexBenchmarkResult {
            model: model_for_task,
            total_entries_in_zim: total_entries,
            benchmark_entries: limit,
            scanned_entries,
            accepted_articles,
            embedded_articles,
            windows,
            total_ms,
            read_ms: total_read_ms,
            parse_ms: total_parse_ms,
            embed_ms: total_embed_ms,
            entries_per_sec: scanned_entries as f64 / total_secs,
            accepted_per_sec: accepted_articles as f64 / total_secs,
            embedded_per_sec: embedded_articles as f64 / total_secs,
        })
    })
    .await
    .map_err(|e| AppError::Io(format!("Task panicked: {e}")))??;

    Ok(result)
}

fn run_zim_poc(zim_path: &str) -> AppResult<serde_json::Value> {
    use zim_rs::archive::Archive;

    let archive = Archive::new(zim_path)
        .map_err(|_| AppError::Io(format!("Failed to open ZIM file: {zim_path}")))?;

    let total_entries = archive.get_all_entrycount();
    let article_count_header = archive.get_articlecount();
    let has_new_ns = archive.has_new_namespace_scheme();

    let range = archive
        .iter_efficient()
        .map_err(|_| AppError::Io("Failed to create efficient iterator".to_string()))?;

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

#[cfg(test)]
mod tests {
    use super::wiki_rrf_score;

    #[test]
    fn wiki_rrf_score_strictly_decreases_with_rank() {
        let a = wiki_rrf_score(1);
        let b = wiki_rrf_score(2);
        assert!(b < a);
    }
}
