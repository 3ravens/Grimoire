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

//! Search quality benchmark: hand-authored cases + gold note ids (debug builds).

pub const SEARCH_QUALITY_ANCHOR_COUNT: usize = 20;
/// Minimum semantic passes (gold in top 3): 85% of 20, rounded up.
pub const SEMANTIC_TOP3_PASS_MIN: usize = 17;

use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Deserialize)]
pub struct SearchQualityFile {
    pub version: u32,
    pub cases: Vec<SearchCase>,
}

#[derive(Debug, Deserialize)]
pub struct SearchCase {
    pub id: String,
    pub title: String,
    pub fts_query: String,
    pub semantic_query: String,
    pub body: String,
}

/// Bundled [`search_quality_cases.json`](../../search_quality_cases.json) (crate root).
pub fn cases_json_embedded() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/search_quality_cases.json"
    ))
}

pub fn load_cases_from_str(json: &str) -> Result<SearchQualityFile, serde_json::Error> {
    serde_json::from_str(json)
}

/// Insert each case as an unfiled note and refresh FTS (Rust-managed `notes_fts`).
///
/// Returns `(case_id, note_id)` in the same order as `cases`.
pub async fn insert_anchor_notes(
    pool: &SqlitePool,
    cases: &[SearchCase],
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let mut out = Vec::with_capacity(cases.len());
    for c in cases {
        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, NULL) RETURNING id",
        )
        .bind(&c.title)
        .bind(&c.body)
        .fetch_one(pool)
        .await?;
        crate::commands::search::fts_upsert(pool, note_id, &c.title, &c.body).await;
        out.push((c.id.clone(), note_id));
    }
    Ok(out)
}
