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

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, Int32Array, Int64Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Connection;
use serde::Serialize;

const TABLE: &str = "notes";

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

/// Open the notes table, creating it with the correct schema if it doesn't exist yet.
/// Recreates automatically if the pre-chunking schema is detected (missing chunk_index)
/// or if the stored vector dimension doesn't match `dims` (pass 0 to skip that check).
pub(super) async fn open_notes_table(conn: &Connection, dims: i32) -> Result<lancedb::Table, String> {
    super::open_or_recreate(conn, TABLE, dims, &["chunk_index"], note_schema).await
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// A note returned from a semantic search. May include multiple excerpts
/// from different chunks of the same note.
#[derive(Debug, Serialize)]
pub struct NoteMatch {
    pub note_id: i64,
    pub title: String,
    pub excerpts: Vec<String>,
    pub distance: f32,
}

// ---------------------------------------------------------------------------
// Filtering constants
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

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
    let dims = chunks.first().map(|(_, _, e)| e.len() as i32).unwrap_or(super::embedder::DIMS);
    if !chunks.is_empty() {
        for (_, _, emb) in &chunks {
            if emb.len() as i32 != dims {
                return Err(format!(
                    "Embedding length mismatch for note chunk: expected {dims}, got {}",
                    emb.len()
                ));
            }
        }
    }
    let table = open_notes_table(conn, dims).await?;

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
    let table = open_notes_table(conn, 0).await?;
    table
        .delete(&format!("note_id = {note_id}"))
        .await
        .map_err(|e| e.to_string())
}

/// Delete all rows from the vector index.
/// Called when a vault password is set — encrypted notes must not remain searchable.
pub async fn purge_all(conn: &Connection) -> Result<(), String> {
    let table = open_notes_table(conn, 0).await?;
    let count = table.count_rows(None).await.map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(());
    }
    // LanceDB: use an always-true predicate so every row is removed regardless of column values.
    table
        .delete("1 = 1")
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

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Search the vector index for notes semantically similar to the query embedding.
/// Returns up to `limit` individual chunk results ordered by similarity.
/// Chunks whose cosine distance exceeds NOTE_MAX_DISTANCE are silently dropped, so
/// the returned list may be shorter than `limit` when few notes are relevant.
/// Multiple chunks from the same note may be returned if they are all relevant —
/// the caller is responsible for grouping them by note_id/title.
pub async fn search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<NoteMatch>, String> {
    let table = open_notes_table(conn, query.len() as i32).await?;

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
            let excerpt = super::truncate_excerpt(contents.value(i), 500);
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
/// ignoring NOTE_MAX_DISTANCE. Used by the debug_search command to calibrate the threshold.
#[cfg(debug_assertions)]
pub async fn raw_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<super::RawMatch>, String> {
    let table = open_notes_table(conn, query.len() as i32).await?;
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
            results.push(super::RawMatch {
                note_id: ids.value(i),
                title: titles.value(i).to_string(),
                excerpt: super::truncate_excerpt(contents.value(i), 200),
                distance: distances.value(i),
            });
        }
    }
    Ok(results)
}
