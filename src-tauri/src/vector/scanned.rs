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
    ArrayRef, FixedSizeListArray, Float32Array, Int32Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Connection;
use serde::Serialize;

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
    super::open_or_recreate(conn, SCANNED_TABLE, dims, &[], scanned_schema).await
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// A match returned from a scanned-file semantic search.
#[derive(Debug, Serialize)]
pub struct ScannedFileMatch {
    pub file_path: String,
    pub title:     String,
    pub excerpts:  Vec<String>,
    pub distance:  f32,
}

// ---------------------------------------------------------------------------
// Filtering constants
// ---------------------------------------------------------------------------

/// Absolute ceiling on the best scanned-file result's distance.
/// Mirrors NOTE_MAX_DISTANCE — scanned files use the same embedding pipeline.
const SCANNED_MAX_DISTANCE: f32 = 0.9;

/// Relative spread factor. Mirrors RELATIVE_DISTANCE_FACTOR for notes.
const SCANNED_RELATIVE_DISTANCE_FACTOR: f32 = 1.15;

/// Maximum distinct files to return.
const MAX_SCANNED_FILES: usize = 3;

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

/// Upsert a batch of chunks for a single scanned file.
/// Deletes all existing chunks for the file first, then inserts the new batch.
/// Each item is (chunk_index, title, content, embedding).
pub async fn scanned_file_upsert_batch(
    conn: &Connection,
    file_path: &str,
    chunks: Vec<(i32, String, String, Vec<f32>)>,
) -> Result<(), String> {
    if chunks.is_empty() { return Ok(()); }
    let dims = chunks.first().map(|(_, _, _, e)| e.len() as i32).unwrap_or(super::embedder::DIMS);
    let table = open_scanned_table(conn, dims).await?;

    // Delete all existing chunks for this file.
    table
        .delete(&format!("file_path = '{}'", super::escape_sql(file_path)))
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

    let items: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
    let reader = RecordBatchIterator::new(items, schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Remove all chunks for a given file path from the scanned-files index.
pub async fn scanned_file_remove(conn: &Connection, file_path: &str) -> Result<(), String> {
    let table = open_scanned_table(conn, 0).await?;
    table
        .delete(&format!("file_path = '{}'", super::escape_sql(file_path)))
        .await
        .map_err(|e| e.to_string())
}

/// Remove all chunks whose file_path starts with the given path prefix.
/// Used when removing a scanned folder.
pub async fn scanned_file_remove_prefix(conn: &Connection, prefix: &str) -> Result<(), String> {
    let table = open_scanned_table(conn, 0).await?;
    // Escape single quotes, then escape LIKE wildcards in the prefix.
    let safe_prefix = super::escape_sql(prefix);
    let like_prefix = safe_prefix.replace('%', "\\%").replace('_', "\\_");
    table
        .delete(&format!("file_path LIKE '{like_prefix}%' ESCAPE '\\'"))
        .await
        .map_err(|e| e.to_string())
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

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

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

            let excerpt = super::truncate_excerpt(contents.value(i), 500);
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

/// Raw scanned-file search for debug calibration.
/// Returns the top vector hits with no domain filtering, grouped only by row.
pub async fn raw_scanned_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<super::RawMatch>, String> {
    let table = open_scanned_table(conn, query.len() as i32).await?;

    let stream = table
        .vector_search(query)
        .map_err(|e| e.to_string())?
        .limit(limit)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| e.to_string())?;
    let mut out: Vec<super::RawMatch> = Vec::new();

    for batch in &batches {
        let paths = batch
            .column_by_name("file_path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing file_path column in scanned raw search results")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column in scanned raw search results")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column in scanned raw search results")?;
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
            .ok_or("missing _distance column in scanned raw search results")?;

        for i in 0..batch.num_rows() {
            out.push(super::RawMatch {
                // Reuse note_id as an opaque source identifier for debug UIs.
                note_id: 0,
                title: format!("{} ({})", titles.value(i), paths.value(i)),
                excerpt: super::truncate_excerpt(contents.value(i), 220),
                distance: distances.value(i),
            });
        }
    }

    out.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);
    Ok(out)
}
