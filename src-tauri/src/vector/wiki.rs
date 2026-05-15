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
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Connection;
use serde::Serialize;

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
    super::open_or_recreate(conn, WIKI_TABLE, dims, &[], wikipedia_schema).await
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// A match returned from a wikipedia semantic search.
#[derive(Debug, Serialize)]
pub struct WikiMatch {
    pub article_id: String,
    pub bundle_id:  String,
    pub title:      String,
    pub excerpts:   Vec<String>,
    pub distance:   f32,
}

// ---------------------------------------------------------------------------
// Filtering constants
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

/// Upsert a batch of articles in a single delete + insert round-trip.
///
/// articles: Vec of (article_id, bundle_id, title, content, embedding)
///
/// Compared to a single-article upsert this reduces LanceDB overhead from
/// O(N) opens/deletes/inserts to O(1).
#[allow(dead_code)]
pub async fn wikipedia_upsert_batch(
    conn: &Connection,
    articles: Vec<(String, String, String, String, Vec<f32>)>,
) -> Result<(), String> {
    if articles.is_empty() { return Ok(()); }
    let dims = articles.first().map(|(_, _, _, _, e)| e.len() as i32).unwrap_or(super::embedder::DIMS);
    let table = open_wiki_table(conn, dims).await?;

    // One bulk delete covering every article_id in this batch.
    let ids_quoted: Vec<String> = articles
        .iter()
        .map(|(id, _, _, _, _)| format!("'{}'", super::escape_sql(id)))
        .collect();
    table
        .delete(&format!("article_id IN ({})", ids_quoted.join(",")))
        .await
        .map_err(|e| e.to_string())?;

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
    let dims = articles.first().map(|(_, _, _, _, e)| e.len() as i32).unwrap_or(super::embedder::DIMS);
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

    let items: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
    let reader = RecordBatchIterator::new(items, schema);
    table.add(reader).execute().await.map_err(|e| e.to_string())
}

/// Remove all entries for a given bundle from the wikipedia vector index.
/// Called when the user removes a bundle.
pub async fn wikipedia_remove_bundle(conn: &Connection, bundle_id: &str) -> Result<(), String> {
    let table = open_wiki_table(conn, 0).await?;
    table
        .delete(&format!("bundle_id = '{}'", super::escape_sql(bundle_id)))
        .await
        .map_err(|e| e.to_string())
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

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

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
            results.push(WikiMatch {
                article_id: article_ids.value(i).to_string(),
                bundle_id:  bundle_ids.value(i).to_string(),
                title:      titles.value(i).to_string(),
                excerpts:   vec![super::truncate_excerpt(contents.value(i), 500)],
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
#[cfg(debug_assertions)]
pub async fn raw_wikipedia_search(
    conn: &Connection,
    query: Vec<f32>,
    limit: usize,
) -> Result<Vec<super::RawMatch>, String> {
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
            results.push(super::RawMatch {
                note_id: 0,
                title: titles.value(i).to_string(),
                excerpt: super::truncate_excerpt(contents.value(i), 200),
                distance: distances.value(i),
            });
        }
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Full table scan (no vector query) — used to backfill SQLite FTS from LanceDB.
// ---------------------------------------------------------------------------

/// Stream all indexed rows for one bundle as `(article_id, title, content)`.
/// Invokes `on_batch` for each Arrow record batch (keeps memory bounded).
pub async fn for_each_wikipedia_bundle_batch<F>(
    conn: &Connection,
    bundle_id: &str,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Vec<(String, String, String)>),
{
    let table = open_wiki_table(conn, 0).await?;
    let filter = format!("bundle_id = '{}'", super::escape_sql(bundle_id));
    let mut stream = table
        .query()
        .only_if(filter)
        .execute()
        .await
        .map_err(|e| e.to_string())?;

    while let Some(batch) = stream.try_next().await.map_err(|e| e.to_string())? {
        let article_ids = batch
            .column_by_name("article_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing article_id column in wikipedia scan")?;
        let titles = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing title column in wikipedia scan")?;
        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or("missing content column in wikipedia scan")?;

        let mut rows = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            rows.push((
                article_ids.value(i).to_string(),
                titles.value(i).to_string(),
                contents.value(i).to_string(),
            ));
        }
        if !rows.is_empty() {
            on_batch(rows);
        }
    }

    Ok(())
}
