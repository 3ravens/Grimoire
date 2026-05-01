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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, Int32Array, Int64Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Connection;
use serde::{Deserialize, Serialize};
use tauri::Manager;

// Default embedding dimensions for nomic-embed-text.
const DIMS: i32 = 768;
const TABLE: &str = "notes";

// Runtime telemetry for batch embedding degradation. This helps the UI surface
// when we had to split batches or fall back to single-item embeds.
static EMBED_BATCH_SPLIT_COUNT: AtomicU64 = AtomicU64::new(0);
static EMBED_BATCH_SINGLE_FALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Return the embedding dimension for a given model name.
#[allow(dead_code)]
pub fn dims_for_model(model: &str) -> i32 {
    if model.contains("mxbai") { 1024 } else { 768 }
}

pub fn reset_embed_batch_telemetry() {
    EMBED_BATCH_SPLIT_COUNT.store(0, Ordering::Relaxed);
    EMBED_BATCH_SINGLE_FALLBACK_COUNT.store(0, Ordering::Relaxed);
}

pub fn snapshot_embed_batch_telemetry() -> (u64, u64) {
    (
        EMBED_BATCH_SPLIT_COUNT.load(Ordering::Relaxed),
        EMBED_BATCH_SINGLE_FALLBACK_COUNT.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// Wraps the LanceDB connection so Tauri can manage it as app state.
/// Connection is Arc-backed and cheap to clone.
pub struct VectorDb(pub Connection);

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

fn note_schema(dims: i32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("note_id", DataType::Int64, false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dims,
            ),
            false,
        ),
    ]))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Open the notes table, creating it with the correct schema if it doesn't exist yet.
/// If the table exists but has the old pre-chunking schema (no chunk_index column),
/// or if its vector dimension doesn't match `dims` (pass 0 to skip the check),
/// it is dropped and recreated automatically.
async fn open_table(conn: &Connection, dims: i32) -> Result<lancedb::Table, String> {
    let effective_dims = if dims > 0 { dims } else { DIMS };
    match conn.open_table(TABLE).execute().await {
        Ok(t) => {
            let schema = t.schema().await.map_err(|e| e.to_string())?;
            // Drop if pre-chunking schema (missing chunk_index).
            if schema.field_with_name("chunk_index").is_err() {
                conn.drop_table(TABLE).await.map_err(|e| e.to_string())?;
                return conn.create_empty_table(TABLE, note_schema(effective_dims))
                    .execute().await.map_err(|e| e.to_string());
            }
            // Drop if vector dimension doesn't match the current model.
            if dims > 0 {
                let actual = schema
                    .field_with_name("vector").ok()
                    .and_then(|f| if let DataType::FixedSizeList(_, n) = f.data_type() { Some(*n) } else { None })
                    .unwrap_or(0);
                if actual != dims {
                    log::info!("[open_table] dimension mismatch (table={actual}, model={dims}) — recreating");
                    conn.drop_table(TABLE).await.map_err(|e| e.to_string())?;
                    return conn.create_empty_table(TABLE, note_schema(dims))
                        .execute().await.map_err(|e| e.to_string());
                }
            }
            Ok(t)
        }
        Err(_) => conn
            .create_empty_table(TABLE, note_schema(effective_dims))
            .execute()
            .await
            .map_err(|e| e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Connect to LanceDB, storing data in the same app-data directory as SQLite.
pub async fn init(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("lancedb");

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir
        .to_str()
        .ok_or("Database path contains non-UTF8 characters")?;

    let conn = lancedb::connect(path)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    // Pre-create the table so the first write doesn't pay the schema-creation cost.
    open_table(&conn, DIMS).await?;

    Ok(conn)
}

/// Evict all currently loaded Ollama models *except* the one we're about to use.
/// On AMD RDNA4 with Vulkan, running two models simultaneously causes GPU crashes.
/// Skipping the target model avoids the cost of unloading and reloading it when
/// it is already resident from the previous call (e.g. during a bulk reindex).
async fn evict_other_models(client: &reqwest::Client, keep_model: &str) {
    #[derive(Deserialize)]
    struct RunningModel { name: String }
    #[derive(Deserialize)]
    struct PsResp { models: Vec<RunningModel> }
    #[derive(Serialize)]
    struct UnloadReq<'a> { model: &'a str, keep_alive: i32, stream: bool }

    let Ok(resp) = client.get("http://localhost:11434/api/ps").send().await else { return };
    let Ok(ps) = resp.json::<PsResp>().await else { return };
    for m in ps.models {
        if m.name == keep_model { continue; }
        let _ = client
            .post("http://localhost:11434/api/generate")
            .json(&UnloadReq { model: &m.name, keep_alive: 0, stream: false })
            .send()
            .await;
    }
}

/// Call Ollama's /api/embeddings endpoint and return the embedding vector.
/// Evicts all running models first to prevent Vulkan GPU context conflicts.
/// Uses keep_alive=0 so the embed model is unloaded immediately after use.
/// Retries once on failure — evicting again before the second attempt clears
/// any GPU state that was corrupted by the first crash.
pub async fn embed(text: &str, model: &str) -> Result<Vec<f32>, String> {
    log::info!("[embed] model={model} text_len={}", text.len());
    #[derive(Serialize)]
    struct Req<'a> {
        model: &'a str,
        prompt: &'a str,
        keep_alive: i32,
    }

    #[derive(Deserialize)]
    struct Resp {
        embedding: Vec<f32>,
    }

    // 120-second timeout — embedding a single chunk should never take this long.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut last_err = String::new();
    for attempt in 1u32..=2 {
        // Evict competing models before every attempt. Skips the embed model itself
        // so it stays resident across consecutive calls (e.g. during bulk reindex).
        evict_other_models(&client, model).await;

        let result: Result<Vec<f32>, String> = async {
            let response = client
                .post("http://localhost:11434/api/embeddings")
                .json(&Req { model, prompt: text, keep_alive: 0 })
                .send()
                .await
                .map_err(|e| format!("Could not reach Ollama (embedding): {e}"))?;

            let text_body = response
                .text()
                .await
                .map_err(|e| format!("Could not read embed response: {e}"))?;

            let resp: Resp = serde_json::from_str(&text_body)
                .map_err(|e| format!("Unexpected embed response: {e}\nBody: {text_body}"))?;

            if resp.embedding.is_empty() {
                return Err("Empty embedding response from Ollama".to_string());
            }

            // Normalize to unit length before storing. Different Ollama versions and
            // task prefixes (search_document:, search_query:) can return vectors with
            // varying norms (observed: 1.0–20+). Normalizing here ensures consistent
            // L2² distances in the range [0, 4] regardless of model or configuration.
            let norm: f32 = resp.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 0.1 {
                return Err(format!("Degenerate embedding vector (norm={norm:.4}) — Ollama inference likely crashed"));
            }
            let normalized: Vec<f32> = resp.embedding.iter().map(|x| x / norm).collect();

            Ok(normalized)
        }.await;

        match result {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = e;
                if attempt < 2 {
                    // Wait for Ollama to finish cleaning up the crashed runner.
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_err)
}

/// Maximum content characters to send to the embedding model for a single input.
/// mxbai-embed-large has a 512-token context window (~350 chars safe budget after
/// accounting for the "search_document: {title}\n" prefix). nomic-embed-text
/// supports 8192 tokens so 1500 chars is well within budget.
pub fn content_chars_for_model(model: &str) -> usize {
    if model.contains("mxbai") { 350 } else { 1500 }
}

/// Batch size for /api/embed calls. 64 texts per request works for all models:
/// truncate=true prevents 400 errors, and content_chars_for_model pre-truncates
/// inputs so even mxbai-embed-large (512-token context) stays within budget.
pub fn batch_size_for_model(_model: &str) -> usize {
    64
}

/// Embed a batch of texts in a single Ollama request using `/api/embed`.
/// 5–10× faster than calling `embed()` per-text: one HTTP round-trip per batch,
/// model stays resident (`keep_alive=300`), no per-call eviction overhead.
/// Returns one vector per input in the same order.
pub async fn embed_batch(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>, String> {
    log::info!("[embed_batch] model={model} chunks={}", texts.len());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    async fn embed_batch_once(
        client: &reqwest::Client,
        texts: &[String],
        model: &str,
    ) -> Result<Vec<Vec<f32>>, String> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            input: &'a [String],
            keep_alive: i32,
            truncate: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            embeddings: Option<Vec<Vec<f32>>>,
            embedding: Option<Vec<f32>>,
            error: Option<String>,
        }

        let response = client
            .post("http://localhost:11434/api/embed")
            .json(&Req { model, input: texts, keep_alive: 300, truncate: true })
            .send()
            .await
            .map_err(|e| format!("Could not reach Ollama (batch embed): {e}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Could not read batch embed response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Batch embed HTTP {}: {body}", status));
        }

        let resp: Resp = serde_json::from_str(&body)
            .map_err(|e| format!("Unexpected batch embed response: {e}\\nBody: {body}"))?;

        if let Some(err) = resp.error {
            return Err(format!("Batch embed error: {err}"));
        }

        if let Some(vs) = resp.embeddings {
            return Ok(vs);
        }

        if let Some(v) = resp.embedding {
            return Ok(vec![v]);
        }

        Err(format!("Batch embed response missing embeddings\\nBody: {body}"))
    }

    if texts.is_empty() {
        return Ok(vec![]);
    }

    let mut out: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
    let mut ranges: Vec<(usize, usize)> = vec![(0, texts.len())];

    while let Some((start, end)) = ranges.pop() {
        let slice = &texts[start..end];
        match embed_batch_once(&client, slice, model).await {
            Ok(embs) if embs.len() == slice.len() => {
                for (i, emb) in embs.into_iter().enumerate() {
                    out[start + i] = Some(emb);
                }
            }
            Ok(embs) if embs.len() == 1 && slice.len() == 1 => {
                out[start] = embs.into_iter().next();
            }
            Ok(embs) => {
                let err = format!(
                    "Batch embed returned {} vectors for {} inputs",
                    embs.len(),
                    slice.len()
                );
                if slice.len() == 1 {
                    log::warn!("[embed_batch] single-item batch mismatch: {err}");
                    EMBED_BATCH_SINGLE_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
                    out[start] = Some(embed(&texts[start], model).await?);
                } else {
                    let mid = start + (slice.len() / 2);
                    log::warn!("[embed_batch] splitting batch ({start}..{end}) after mismatch: {err}");
                    EMBED_BATCH_SPLIT_COUNT.fetch_add(1, Ordering::Relaxed);
                    ranges.push((mid, end));
                    ranges.push((start, mid));
                }
            }
            Err(e) => {
                if slice.len() == 1 {
                    log::warn!("[embed_batch] single-item batch failed, falling back to /api/embeddings: {e}");
                    EMBED_BATCH_SINGLE_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
                    out[start] = Some(embed(&texts[start], model).await?);
                } else {
                    let mid = start + (slice.len() / 2);
                    log::warn!("[embed_batch] splitting batch ({start}..{end}) after failure: {e}");
                    EMBED_BATCH_SPLIT_COUNT.fetch_add(1, Ordering::Relaxed);
                    ranges.push((mid, end));
                    ranges.push((start, mid));
                }
            }
        }
    }

    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for (i, maybe_emb) in out.into_iter().enumerate() {
        embeddings.push(maybe_emb.ok_or_else(|| format!("Missing embedding for batch index {i}"))?);
    }

    // Normalize each vector to unit length — see embed() for rationale.
    let normalized: Vec<Vec<f32>> = embeddings.into_iter().enumerate()
        .map(|(i, emb)| {
            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 0.1 {
                return Err(format!("Degenerate embedding at index {i} (norm={norm:.4})"));
            }
            Ok(emb.into_iter().map(|x| x / norm).collect())
        })
        .collect::<Result<_, _>>()?;
    Ok(normalized)
}

/// Insert or replace all chunks for a note in the vector index.
/// Deletes any existing rows for this note_id first, then inserts one row per chunk.
/// Each chunk is a tuple of (chunk_index, chunk_text, embedding).
pub async fn upsert(
    conn: &Connection,
    note_id: i64,
    title: &str,
    chunks: Vec<(i32, String, Vec<f32>)>,
) -> Result<(), String> {
    // Infer the embedding dimension from the first chunk so the table schema
    // is always consistent with whichever model produced the embeddings.
    let dims = chunks.first().map(|(_, _, e)| e.len() as i32).unwrap_or(DIMS);
    let table = open_table(conn, dims).await?;

    // Remove all existing chunks for this note.
    table
        .delete(&format!("note_id = {note_id}"))
        .await
        .map_err(|e| e.to_string())?;

    if chunks.is_empty() {
        return Ok(());
    }

    let n = chunks.len();

    // Flatten all embedding vectors into one contiguous array for the FixedSizeList column.
    let all_floats: Vec<f32> = chunks
        .iter()
        .flat_map(|(_, _, emb)| emb.iter().copied())
        .collect();

    let vector_col = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dims,
        Arc::new(Float32Array::from(all_floats)) as ArrayRef,
        None,
    )
    .map_err(|e| e.to_string())?;

    let schema = note_schema(dims);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![note_id; n])) as ArrayRef,
            Arc::new(Int32Array::from(
                chunks.iter().map(|(i, _, _)| *i).collect::<Vec<i32>>(),
            )) as ArrayRef,
            Arc::new(StringArray::from(vec![title; n])) as ArrayRef,
            Arc::new(StringArray::from(
                chunks
                    .iter()
                    .map(|(_, text, _)| text.as_str())
                    .collect::<Vec<&str>>(),
            )) as ArrayRef,
            Arc::new(vector_col) as ArrayRef,
        ],
    )
    .map_err(|e| e.to_string())?;

    // RecordBatchIterator wraps an iterator of Result<RecordBatch> and implements
    // RecordBatchReader, which is what LanceDB's add() expects.
    let items: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
    let reader = RecordBatchIterator::new(items, schema);

    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Remove a note from the vector index (called on delete).
pub async fn remove(conn: &Connection, note_id: i64) -> Result<(), String> {
    let table = open_table(conn, 0).await?;
    table
        .delete(&format!("note_id = {note_id}"))
        .await
        .map_err(|e| e.to_string())
}

/// Delete all rows from the vector index.
/// Called when a vault password is set — encrypted notes must not remain searchable.
pub async fn purge_all(conn: &Connection) -> Result<(), String> {
    let table = open_table(conn, 0).await?;
    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(());
    }
    // LanceDB delete with a condition that matches every row.
    table
        .delete("note_id >= 0")
        .await
        .map_err(|e| e.to_string())
}

/// Drop the notes index entirely so it can be rebuilt with a different embedding model.
/// The table will be recreated with the correct schema on the next upsert or search.
pub async fn clear_notes_index(conn: &Connection) -> Result<(), String> {
    match conn.drop_table(TABLE).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("does not exist") {
                Ok(())
            } else {
                Err(msg)
            }
        }
    }
}

/// Drop the Wikipedia index so it can be rebuilt after an embedding model change.
pub async fn clear_wiki_index(conn: &Connection) -> Result<(), String> {
    match conn.drop_table(WIKI_TABLE).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("does not exist") {
                Ok(())
            } else {
                Err(msg)
            }
        }
    }
}

/// Drop the scanned-files index so it can be rebuilt after an embedding model change.
pub async fn clear_scanned_index(conn: &Connection) -> Result<(), String> {
    match conn.drop_table(SCANNED_TABLE).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("not found") || msg.to_lowercase().contains("does not exist") {
                Ok(())
            } else {
                Err(msg)
            }
        }
    }
}

/// A note returned from a semantic search. May include multiple excerpts
/// from different chunks of the same note.
#[derive(Debug, Serialize)]
pub struct NoteMatch {
    pub note_id: i64,
    pub title: String,
    pub excerpts: Vec<String>,
    pub distance: f32,
}

/// A raw search hit including the distance score. Used for debugging threshold calibration.
#[derive(Debug, Serialize)]
pub struct RawMatch {
    pub note_id: i64,
    pub title: String,
    pub excerpt: String,
    pub distance: f32,
}

/// Maximum number of distinct notes to include in search results.
const MAX_SOURCE_NOTES: usize = 5;
/// Relative distance factor used to suppress tangentially-related results.
///
/// After the top-N notes are ranked by lowest chunk distance, any note whose
/// best distance is more than RELATIVE_DISTANCE_FACTOR times the best note's
/// distance is dropped. This is model-agnostic: it adapts to whatever distance
/// scale nomic-embed-text produces rather than relying on a magic absolute number.
///
/// 1.15 means: a note is kept only if its distance is within 15% of the best
/// note's distance. Example — best = 0.50, cutoff = 0.575; a note at 0.57 passes
/// but a note at 0.58 is dropped. Tighter than 1.25 to reduce keyword-overlap
/// noise (e.g. a Transformers note surfacing for a "binary search tree" query).
const RELATIVE_DISTANCE_FACTOR: f32 = 1.15;
/// Absolute ceiling on the best note result's distance.
/// The relative filter alone cannot detect "all results are irrelevant" — it just
/// picks the least-bad notes. This ceiling cuts the entire result set when nothing
/// relevant exists, preventing unrelated notes from polluting context.
/// After L2-normalization: random-noise documents score ~1.4–2.0; genuinely related
/// content scores <0.8. 0.9 is a tighter cutoff that filters keyword-overlap noise.
/// NOTE: existing stored note embeddings must be re-indexed (reindex_all) after
/// the normalization change — stored unnormalized vectors give wrong distances.
const NOTE_MAX_DISTANCE: f32 = 0.9;
/// How many raw chunks to retrieve from LanceDB per search.
/// Must be larger than MAX_SOURCE_NOTES to allow deduplication to work — if a
/// long note contributes many top-ranked chunks they will all count as one note,
/// leaving room for shorter/newer notes to appear in the final result set.
pub const CHUNK_FETCH_LIMIT: usize = 100;
/// Search the vector index for notes semantically similar to the query embedding.
/// Returns up to `limit` individual chunk results ordered by similarity.
/// Chunks whose cosine distance exceeds MAX_DISTANCE are silently dropped, so
/// the returned list may be shorter than `limit` when few notes are relevant.
/// Multiple chunks from the same note may be returned if they are all relevant —
/// the caller is responsible for grouping them by note_id/title.
pub async fn search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<NoteMatch>, String> {
    let table = open_table(conn, query.len() as i32).await?;

    // LanceDB errors when searching an empty table in some versions — short-circuit.
    let count = table
        .count_rows(None)
        .await
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(vec![]);
    }

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream
        .try_collect()
        .await
        .map_err(|e| e.to_string())?;

    // Pass 1: find the best distance per note across all chunks.
    // This is used to rank notes and select the top MAX_SOURCE_NOTES.
    let mut best_dist: std::collections::HashMap<i64, (f32, String)> = std::collections::HashMap::new();

    for batch in &batches {
        let ids = batch
            .column_by_name("note_id")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or("missing note_id column in search results")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column in search results")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column in search results")?;

        for i in 0..batch.num_rows() {
            let note_id = ids.value(i);
            let distance = distances.value(i);
            let entry = best_dist.entry(note_id).or_insert((f32::MAX, titles.value(i).to_string()));
            if distance < entry.0 {
                entry.0 = distance;
            }
        }
    }

    // Pick the top MAX_SOURCE_NOTES notes by best chunk distance, then drop any
    // note that is more than RELATIVE_DISTANCE_FACTOR times the best note's distance.
    // This suppresses tangentially-related results without needing a magic absolute number.
    let mut ranked: Vec<(i64, f32, String)> = best_dist
        .into_iter()
        .map(|(id, (dist, title))| (id, dist, title))
        .collect();
    ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(MAX_SOURCE_NOTES);
    if let Some(best_distance) = ranked.first().map(|(_, d, _)| *d) {
        // Absolute ceiling: if even the best note is too far away, return nothing.
        if best_distance > NOTE_MAX_DISTANCE {
            return Ok(vec![]);
        }
        let cutoff = best_distance * RELATIVE_DISTANCE_FACTOR;
        ranked.retain(|(_, d, _)| *d <= cutoff);
    }
    let top_ids: std::collections::HashSet<i64> = ranked.iter().map(|(id, _, _)| *id).collect();

    // Pass 2: collect all chunks that belong to the top notes, preserving chunk order.
    // We keep a map from note_id → list of (chunk_index, excerpt).
    let mut note_chunks: std::collections::HashMap<i64, Vec<(i32, String)>> =
        std::collections::HashMap::new();

    for batch in &batches {
        let ids = batch
            .column_by_name("note_id")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or("missing note_id column in search results")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column in search results")?;
        let chunk_indices = batch
            .column_by_name("chunk_index")
            .and_then(|c| c.as_any().downcast_ref::<Int32Array>())
            .ok_or("missing chunk_index column in search results")?;

        for i in 0..batch.num_rows() {
            let note_id = ids.value(i);
            if !top_ids.contains(&note_id) {
                continue;
            }
            let raw = contents.value(i);
            let excerpt = if raw.chars().count() > 500 {
                let cutoff = raw.char_indices().nth(500).map(|(b, _)| b).unwrap_or(raw.len());
                format!("{}\u{2026}", &raw[..cutoff])
            } else {
                raw.to_string()
            };
            let chunks = note_chunks.entry(note_id).or_default();
            let ci = chunk_indices.value(i);
            if !chunks.iter().any(|(c, _)| *c == ci) {
                chunks.push((ci, excerpt));
            }
        }
    }

    // Assemble final results in ranked order.
    let results = ranked
        .into_iter()
        .map(|(note_id, dist, title)| {
            let mut chunks = note_chunks.remove(&note_id).unwrap_or_default();
            chunks.sort_by_key(|(ci, _)| *ci);
            NoteMatch {
                note_id,
                title,
                excerpts: chunks.into_iter().map(|(_, e)| e).collect(),
                distance: dist,
            }
        })
        .collect();

    Ok(results)
}

/// Like search() but returns all top-N hits with their raw distance scores,
/// ignoring MAX_DISTANCE. Used by the debug_search command to calibrate the threshold.
pub async fn raw_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<RawMatch>, String> {
    let table = open_table(conn, query.len() as i32).await?;
    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(vec![]);
    }

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for batch in &batches {
        let ids = batch
            .column_by_name("note_id")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or("missing note_id column")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column")?;

        for i in 0..batch.num_rows() {
            let raw = contents.value(i);
            let excerpt = if raw.chars().count() > 200 {
                let cutoff = raw
                    .char_indices()
                    .nth(200)
                    .map(|(b, _)| b)
                    .unwrap_or(raw.len());
                format!("{}\u{2026}", &raw[..cutoff])
            } else {
                raw.to_string()
            };
            results.push(RawMatch {
                note_id: ids.value(i),
                title: titles.value(i).to_string(),
                excerpt,
                distance: distances.value(i),
            });
        }
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Wikipedia vector index
// ---------------------------------------------------------------------------

const WIKI_TABLE: &str = "wikipedia_index";

fn wikipedia_schema(dims: i32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        // Stable identifier: "<bundle_id>/<article_path>"
        Field::new("article_id", DataType::Utf8, false),
        Field::new("bundle_id",  DataType::Utf8, false),
        Field::new("title",      DataType::Utf8, false),
        Field::new("content",    DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dims,
            ),
            false,
        ),
    ]))
}

async fn open_wiki_table(conn: &Connection, dims: i32) -> Result<lancedb::Table, String> {
    let effective_dims = if dims > 0 { dims } else { DIMS };
    match conn.open_table(WIKI_TABLE).execute().await {
        Ok(t) => {
            if dims > 0 {
                let schema = t.schema().await.map_err(|e| e.to_string())?;
                let actual = schema
                    .field_with_name("vector").ok()
                    .and_then(|f| if let DataType::FixedSizeList(_, n) = f.data_type() { Some(*n) } else { None })
                    .unwrap_or(0);
                if actual != dims {
                    log::info!("[open_wiki_table] dimension mismatch (table={actual}, model={dims}) — recreating");
                    conn.drop_table(WIKI_TABLE).await.map_err(|e| e.to_string())?;
                    return conn.create_empty_table(WIKI_TABLE, wikipedia_schema(dims))
                        .execute().await.map_err(|e| e.to_string());
                }
            }
            Ok(t)
        }
        Err(_) => conn
            .create_empty_table(WIKI_TABLE, wikipedia_schema(effective_dims))
            .execute()
            .await
            .map_err(|e| e.to_string()),
    }
}

/// A match returned from a wikipedia semantic search.
#[derive(Debug, Serialize)]
pub struct WikiMatch {
    pub article_id: String,
    pub bundle_id:  String,
    pub title:      String,
    pub excerpts:   Vec<String>,
    pub distance:   f32,
}

/// Insert one article chunk into the wikipedia vector index.
/// Callers are responsible for deduplication (delete by article_id first if re-indexing).
/// Upsert a single article. Kept for fallback/test use; prefer `wikipedia_upsert_batch`.
#[allow(dead_code)]
pub async fn wikipedia_upsert(
    conn: &Connection,
    article_id: &str,
    bundle_id:  &str,
    title:      &str,
    content:    &str,
    embedding:  Vec<f32>,
) -> Result<(), String> {
    let dims = embedding.len() as i32;
    let table = open_wiki_table(conn, dims).await?;

    // Remove any existing entry for this article (handles re-index case).
    let safe_id = article_id.replace('\'', "''");
    table
        .delete(&format!("article_id = '{safe_id}'"))
        .await
        .map_err(|e| e.to_string())?;

    let vector_col = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dims,
        Arc::new(Float32Array::from(embedding)) as ArrayRef,
        None,
    )
    .map_err(|e| e.to_string())?;

    let schema = wikipedia_schema(dims);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![article_id])) as ArrayRef,
            Arc::new(StringArray::from(vec![bundle_id]))  as ArrayRef,
            Arc::new(StringArray::from(vec![title]))      as ArrayRef,
            Arc::new(StringArray::from(vec![content]))    as ArrayRef,
            Arc::new(vector_col) as ArrayRef,
        ],
    )
    .map_err(|e| e.to_string())?;

    let items: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
    let reader = RecordBatchIterator::new(items, schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Upsert a batch of articles in a single delete + insert round-trip.
///
/// articles: Vec of (article_id, bundle_id, title, content, embedding)
///
/// Compared to calling `wikipedia_upsert` per article this reduces LanceDB
/// overhead from O(N) opens/deletes/inserts to O(1). On a fast machine this
/// is the dominant bottleneck once the GPU embedding is batched.
#[allow(dead_code)]
pub async fn wikipedia_upsert_batch(
    conn: &Connection,
    articles: Vec<(String, String, String, String, Vec<f32>)>,
) -> Result<(), String> {
    if articles.is_empty() { return Ok(()); }
    let dims = articles.first().map(|(_, _, _, _, e)| e.len() as i32).unwrap_or(DIMS);
    let table = open_wiki_table(conn, dims).await?;

    // One bulk delete covering every article_id in this batch.
    let ids_quoted: Vec<String> = articles
        .iter()
        .map(|(id, _, _, _, _)| format!("'{}'", id.replace('\'', "''")))
        .collect();
    table
        .delete(&format!("article_id IN ({})", ids_quoted.join(",")))
        .await
        .map_err(|e| e.to_string())?;

    // Build one RecordBatch with a row per article.
    let mut ids      = Vec::with_capacity(articles.len());
    let mut bundle_ids = Vec::with_capacity(articles.len());
    let mut titles   = Vec::with_capacity(articles.len());
    let mut contents = Vec::with_capacity(articles.len());
    let mut flat_vec: Vec<f32> = Vec::with_capacity(articles.len() * dims as usize);
    for (id, bid, title, content, emb) in articles {
        ids.push(id);
        bundle_ids.push(bid);
        titles.push(title);
        contents.push(content);
        flat_vec.extend(emb);
    }
    let vector_col = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dims,
        Arc::new(Float32Array::from(flat_vec)) as ArrayRef,
        None,
    )
    .map_err(|e| e.to_string())?;

    let schema = wikipedia_schema(dims);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(ids))        as ArrayRef,
            Arc::new(StringArray::from(bundle_ids)) as ArrayRef,
            Arc::new(StringArray::from(titles))     as ArrayRef,
            Arc::new(StringArray::from(contents))   as ArrayRef,
            Arc::new(vector_col)                    as ArrayRef,
        ],
    )
    .map_err(|e| e.to_string())?;

    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Insert a batch of articles without any delete step.
///
/// This is intended for forward-only indexing passes where each article_id is
/// known to be new in the current table (for example: fresh re-index after
/// clearing a bundle, or resume from a checkpoint that only advances).
pub async fn wikipedia_append_batch(
    conn: &Connection,
    articles: Vec<(String, String, String, String, Vec<f32>)>,
) -> Result<(), String> {
    if articles.is_empty() { return Ok(()); }
    let dims = articles.first().map(|(_, _, _, _, e)| e.len() as i32).unwrap_or(DIMS);
    let table = open_wiki_table(conn, dims).await?;

    let mut ids = Vec::with_capacity(articles.len());
    let mut bundle_ids = Vec::with_capacity(articles.len());
    let mut titles = Vec::with_capacity(articles.len());
    let mut contents = Vec::with_capacity(articles.len());
    let mut flat_vec: Vec<f32> = Vec::with_capacity(articles.len() * dims as usize);
    for (id, bid, title, content, emb) in articles {
        ids.push(id);
        bundle_ids.push(bid);
        titles.push(title);
        contents.push(content);
        flat_vec.extend(emb);
    }

    let vector_col = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dims,
        Arc::new(Float32Array::from(flat_vec)) as ArrayRef,
        None,
    )
    .map_err(|e| e.to_string())?;

    let schema = wikipedia_schema(dims);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(ids)) as ArrayRef,
            Arc::new(StringArray::from(bundle_ids)) as ArrayRef,
            Arc::new(StringArray::from(titles)) as ArrayRef,
            Arc::new(StringArray::from(contents)) as ArrayRef,
            Arc::new(vector_col) as ArrayRef,
        ],
    )
    .map_err(|e| e.to_string())?;

    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Remove all entries for a given bundle from the wikipedia vector index.
/// Called when the user removes a bundle.
pub async fn wikipedia_remove_bundle(conn: &Connection, bundle_id: &str) -> Result<(), String> {
    let table = open_wiki_table(conn, 0).await?;
    let safe_id = bundle_id.replace('\'', "''");
    table
        .delete(&format!("bundle_id = '{safe_id}'"))
        .await
        .map_err(|e| e.to_string())
}

/// Absolute ceiling on the best Wikipedia result's distance.
/// After L2-normalization: a strongly relevant article scores <0.5;
/// clearly irrelevant articles score >1.4. 1.2 is a safe cutoff.
const WIKI_MAX_DISTANCE: f32 = 1.2;

/// Relative spread factor: drop any result whose distance is more than this
/// multiple of the best result's distance. Mirrors RELATIVE_DISTANCE_FACTOR
/// used for notes. 1.30 is tighter than notes (1.40) because Wikipedia results
/// are all drawn from the same large corpus, so spread within a good result set
/// is naturally tighter; a wider spread here means the tail results are noise.
const WIKI_RELATIVE_DISTANCE_FACTOR: f32 = 1.30;

/// Search the wikipedia vector index.
/// Returns up to `limit` results ordered by similarity.
/// Results are filtered: if the best distance exceeds WIKI_MAX_DISTANCE the
/// function returns an empty list rather than injecting irrelevant articles
/// as context. Within a qualifying result set, results more than
/// WIKI_RELATIVE_DISTANCE_FACTOR times the best distance are also dropped.
pub async fn wikipedia_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<WikiMatch>, String> {
    let table = open_wiki_table(conn, query.len() as i32).await?;

    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(vec![]);
    }

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| e.to_string())?;

    let mut results: Vec<WikiMatch> = Vec::new();
    for batch in &batches {
        let article_ids = batch
            .column_by_name("article_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing article_id column in wikipedia search results")?;
        let bundle_ids = batch
            .column_by_name("bundle_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing bundle_id column in wikipedia search results")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column in wikipedia search results")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column in wikipedia search results")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column in wikipedia search results")?;

        for i in 0..batch.num_rows() {
            let raw = contents.value(i);
            let excerpt = if raw.chars().count() > 500 {
                let cutoff = raw.char_indices().nth(500).map(|(b, _)| b).unwrap_or(raw.len());
                format!("{}\u{2026}", &raw[..cutoff])
            } else {
                raw.to_string()
            };
            results.push(WikiMatch {
                article_id: article_ids.value(i).to_string(),
                bundle_id:  bundle_ids.value(i).to_string(),
                title:      titles.value(i).to_string(),
                excerpts:   vec![excerpt],
                distance:   distances.value(i),
            });
        }
    }

    if results.is_empty() {
        return Ok(results);
    }

    // Drop the whole result set if even the best hit is too far away.
    // Injecting irrelevant Wikipedia articles is worse than injecting nothing.
    let best = results.iter().map(|r| r.distance).fold(f32::MAX, f32::min);
    if best > WIKI_MAX_DISTANCE {
        return Ok(vec![]);
    }

    // Drop tail results that are much worse than the best hit.
    let cutoff = best * WIKI_RELATIVE_DISTANCE_FACTOR;
    results.retain(|r| r.distance <= cutoff);

    Ok(results)
}

/// Like wikipedia_search() but returns raw distance scores without any filtering.
/// Used by the debug panel to calibrate WIKI_MAX_DISTANCE.
pub async fn raw_wikipedia_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<RawMatch>, String> {
    let table = open_wiki_table(conn, query.len() as i32).await?;
    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(vec![]);
    }

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for batch in &batches {
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column")?;

        for i in 0..batch.num_rows() {
            let raw = contents.value(i);
            let excerpt = if raw.chars().count() > 200 {
                let cutoff = raw.char_indices().nth(200).map(|(b, _)| b).unwrap_or(raw.len());
                format!("{}\u{2026}", &raw[..cutoff])
            } else {
                raw.to_string()
            };
            results.push(RawMatch {
                note_id: 0,
                title: titles.value(i).to_string(),
                excerpt,
                distance: distances.value(i),
            });
        }
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Scanned files vector index
// ---------------------------------------------------------------------------

const SCANNED_TABLE: &str = "scanned_files";

fn scanned_schema(dims: i32) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        // Absolute path to the file on disk.
        Field::new("file_path",   DataType::Utf8,  false),
        Field::new("chunk_index", DataType::Int32,  false),
        // Human-readable title (filename without extension).
        Field::new("title",       DataType::Utf8,  false),
        Field::new("content",     DataType::Utf8,  false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dims,
            ),
            false,
        ),
    ]))
}

async fn open_scanned_table(conn: &Connection, dims: i32) -> Result<lancedb::Table, String> {
    let effective_dims = if dims > 0 { dims } else { DIMS };
    match conn.open_table(SCANNED_TABLE).execute().await {
        Ok(t) => {
            if dims > 0 {
                let schema = t.schema().await.map_err(|e| e.to_string())?;
                let actual = schema
                    .field_with_name("vector").ok()
                    .and_then(|f| if let DataType::FixedSizeList(_, n) = f.data_type() { Some(*n) } else { None })
                    .unwrap_or(0);
                if actual != dims {
                    log::info!("[open_scanned_table] dimension mismatch (table={actual}, model={dims}) — recreating");
                    conn.drop_table(SCANNED_TABLE).await.map_err(|e| e.to_string())?;
                    return conn.create_empty_table(SCANNED_TABLE, scanned_schema(dims))
                        .execute().await.map_err(|e| e.to_string());
                }
            }
            Ok(t)
        }
        Err(_) => conn
            .create_empty_table(SCANNED_TABLE, scanned_schema(effective_dims))
            .execute()
            .await
            .map_err(|e| e.to_string()),
    }
}

/// A match returned from a scanned-file semantic search.
#[derive(Debug, Serialize)]
pub struct ScannedFileMatch {
    pub file_path: String,
    pub title:     String,
    pub excerpts:  Vec<String>,
    pub distance:  f32,
}

/// Upsert a batch of chunks for a single scanned file.
/// Deletes all existing chunks for the file first, then inserts the new batch.
/// Each item is (file_path, chunk_index, title, content, embedding).
pub async fn scanned_file_upsert_batch(
    conn: &Connection,
    file_path: &str,
    chunks: Vec<(i32, String, String, Vec<f32>)>, // (chunk_index, title, content, embedding)
) -> Result<(), String> {
    if chunks.is_empty() { return Ok(()); }
    let dims = chunks.first().map(|(_, _, _, e)| e.len() as i32).unwrap_or(DIMS);
    let table = open_scanned_table(conn, dims).await?;

    // Delete all existing chunks for this file.
    let safe_path = file_path.replace('\'', "''");
    table
        .delete(&format!("file_path = '{safe_path}'"))
        .await
        .map_err(|e| e.to_string())?;

    let n = chunks.len();
    let mut chunk_indices = Vec::with_capacity(n);
    let mut titles        = Vec::with_capacity(n);
    let mut contents      = Vec::with_capacity(n);
    let mut flat_vec: Vec<f32> = Vec::with_capacity(n * dims as usize);

    for (ci, title, content, emb) in &chunks {
        chunk_indices.push(*ci);
        titles.push(title.as_str());
        contents.push(content.as_str());
        flat_vec.extend(emb);
    }

    let vector_col = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        dims,
        Arc::new(Float32Array::from(flat_vec)) as ArrayRef,
        None,
    )
    .map_err(|e| e.to_string())?;

    let schema = scanned_schema(dims);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![file_path; n])) as ArrayRef,
            Arc::new(Int32Array::from(chunk_indices))        as ArrayRef,
            Arc::new(StringArray::from(titles))              as ArrayRef,
            Arc::new(StringArray::from(contents))            as ArrayRef,
            Arc::new(vector_col)                             as ArrayRef,
        ],
    )
    .map_err(|e| e.to_string())?;

    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Remove all chunks for a given file path from the scanned-files index.
pub async fn scanned_file_remove(conn: &Connection, file_path: &str) -> Result<(), String> {
    let table = open_scanned_table(conn, 0).await?;
    let safe_path = file_path.replace('\'', "''");
    table
        .delete(&format!("file_path = '{safe_path}'"))
        .await
        .map_err(|e| e.to_string())
}

/// Remove all chunks whose file_path starts with the given path prefix.
/// Used when removing a scanned folder.
pub async fn scanned_file_remove_prefix(conn: &Connection, prefix: &str) -> Result<(), String> {
    let table = open_scanned_table(conn, 0).await?;
    // Safety: escape single quotes in the prefix string.
    let safe_prefix = prefix.replace('\'', "''");
    // Use LIKE with % wildcard. Escape LIKE wildcards in the prefix.
    let like_prefix = safe_prefix.replace('%', "\\%").replace('_', "\\_");
    table
        .delete(&format!("file_path LIKE '{like_prefix}%' ESCAPE '\\'"))
        .await
        .map_err(|e| e.to_string())
}

/// Absolute ceiling on the best scanned-file result's distance.
/// Mirrors NOTE_MAX_DISTANCE — scanned files use the same embedding pipeline.
const SCANNED_MAX_DISTANCE: f32 = 0.9;
/// Relative spread factor. Mirrors RELATIVE_DISTANCE_FACTOR for notes.
const SCANNED_RELATIVE_DISTANCE_FACTOR: f32 = 1.15;
/// Maximum distinct files to return.
const MAX_SCANNED_FILES: usize = 3;

/// Search the scanned-files vector index.
/// Returns up to MAX_SCANNED_FILES results ordered by similarity.
/// Applies the same absolute-ceiling + relative-spread filtering as the notes index.
pub async fn scanned_file_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<ScannedFileMatch>, String> {
    let table = open_scanned_table(conn, query.len() as i32).await?;

    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(vec![]);
    }

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| e.to_string())?;

    // Pass 1: best distance per file_path.
    let mut best_dist: std::collections::HashMap<String, (f32, String)> =
        std::collections::HashMap::new();

    for batch in &batches {
        let paths = batch
            .column_by_name("file_path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing file_path column in scanned search results")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column in scanned search results")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column in scanned search results")?;

        for i in 0..batch.num_rows() {
            let path = paths.value(i).to_string();
            let dist = distances.value(i);
            let entry = best_dist.entry(path)
                .or_insert((f32::MAX, titles.value(i).to_string()));
            if dist < entry.0 {
                entry.0 = dist;
            }
        }
    }

    // Rank and filter.
    let mut ranked: Vec<(String, f32, String)> = best_dist
        .into_iter()
        .map(|(path, (dist, title))| (path, dist, title))
        .collect();
    ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(MAX_SCANNED_FILES);

    if let Some(best) = ranked.first().map(|(_, d, _)| *d) {
        if best > SCANNED_MAX_DISTANCE {
            return Ok(vec![]);
        }
        let cutoff = best * SCANNED_RELATIVE_DISTANCE_FACTOR;
        ranked.retain(|(_, d, _)| *d <= cutoff);
    }

    let top_paths: std::collections::HashSet<String> =
        ranked.iter().map(|(p, _, _)| p.clone()).collect();

    // Pass 2: collect chunks for top files.
    let mut file_chunks: std::collections::HashMap<String, Vec<(i32, String)>> =
        std::collections::HashMap::new();

    for batch in &batches {
        let paths = batch
            .column_by_name("file_path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing file_path column")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column")?;
        let chunk_indices = batch
            .column_by_name("chunk_index")
            .and_then(|c| c.as_any().downcast_ref::<Int32Array>())
            .ok_or("missing chunk_index column")?;

        for i in 0..batch.num_rows() {
            let path = paths.value(i).to_string();
            if !top_paths.contains(&path) { continue; }

            let raw = contents.value(i);
            let excerpt = if raw.chars().count() > 500 {
                let cutoff = raw.char_indices().nth(500).map(|(b, _)| b).unwrap_or(raw.len());
                format!("{}\u{2026}", &raw[..cutoff])
            } else {
                raw.to_string()
            };
            let ci = chunk_indices.value(i);
            let chunks = file_chunks.entry(path).or_default();
            if !chunks.iter().any(|(c, _)| *c == ci) {
                chunks.push((ci, excerpt));
            }
        }
    }

    let results = ranked
        .into_iter()
        .map(|(path, dist, title)| {
            let mut chunks = file_chunks.remove(&path).unwrap_or_default();
            chunks.sort_by_key(|(ci, _)| *ci);
            ScannedFileMatch {
                file_path: path,
                title,
                excerpts: chunks.into_iter().map(|(_, e)| e).collect(),
                distance: dist,
            }
        })
        .collect();

    Ok(results)
}
