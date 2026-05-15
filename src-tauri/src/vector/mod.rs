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

pub mod embedder;
pub mod notes;
pub mod scanned;
pub mod wiki;

use std::sync::Arc;

use arrow_schema::{DataType, Schema};
use lancedb::Connection;
use serde::Serialize;
use tauri::Manager;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// Wraps the LanceDB connection so Tauri can manage it as app state.
/// Connection is Arc-backed and cheap to clone.
pub struct VectorDb(pub Connection);

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// A raw search hit including the distance score. Used for debugging threshold
/// calibration. Shared by both the notes and Wikipedia raw search paths.
/// `note_id` is 0 for Wikipedia results.
#[derive(Debug, Serialize)]
pub struct RawMatch {
    pub note_id: i64,
    pub title: String,
    pub excerpt: String,
    pub distance: f32,
}

// ---------------------------------------------------------------------------
// Shared internal helpers
// (private to this module and its sub-modules by Rust's privacy rules)
// ---------------------------------------------------------------------------

/// Open a LanceDB table by name, creating it with `make_schema` if it doesn't
/// exist yet. Recreates the table if:
///   - any column listed in `required_columns` is absent (schema migration), or
///   - the stored vector dimension doesn't match `dims` (pass 0 to skip that check).
///
/// When `dims` is 0 the table is opened or created with the default 768 dimensions.
async fn open_or_recreate(
    conn: &Connection,
    table_name: &str,
    dims: i32,
    required_columns: &[&str],
    make_schema: impl Fn(i32) -> Arc<Schema>,
) -> Result<lancedb::Table, String> {
    let effective_dims = if dims > 0 { dims } else { embedder::DIMS };
    match conn.open_table(table_name).execute().await {
        Ok(t) => {
            let schema = t.schema().await.map_err(|e| e.to_string())?;

            // Recreate if any required column is missing (handles schema migrations).
            for col in required_columns {
                if schema.field_with_name(col).is_err() {
                    conn.drop_table(table_name).await.map_err(|e| e.to_string())?;
                    return conn
                        .create_empty_table(table_name, make_schema(effective_dims))
                        .execute()
                        .await
                        .map_err(|e| e.to_string());
                }
            }

            // Recreate if the stored vector dimension doesn't match the current model.
            if dims > 0 {
                let actual = schema
                    .field_with_name("vector")
                    .ok()
                    .and_then(|f| {
                        if let DataType::FixedSizeList(_, n) = f.data_type() {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if actual != dims {
                    log::info!(
                        "[open_or_recreate] dimension mismatch in '{table_name}' \
                         (table={actual}, model={dims}) — recreating"
                    );
                    conn.drop_table(table_name).await.map_err(|e| e.to_string())?;
                    return conn
                        .create_empty_table(table_name, make_schema(dims))
                        .execute()
                        .await
                        .map_err(|e| e.to_string());
                }
            }

            Ok(t)
        }
        Err(_) => conn
            .create_empty_table(table_name, make_schema(effective_dims))
            .execute()
            .await
            .map_err(|e| e.to_string()),
    }
}

/// Truncate a string to at most `limit` Unicode characters, appending `…` if truncated.
fn truncate_excerpt(s: &str, limit: usize) -> String {
    if s.chars().count() > limit {
        let cutoff = s
            .char_indices()
            .nth(limit)
            .map(|(b, _)| b)
            .unwrap_or(s.len());
        format!("{}\u{2026}", &s[..cutoff])
    } else {
        s.to_string()
    }
}

/// Escape single quotes in a string for use in a LanceDB filter expression.
fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

/// Connect to LanceDB, storing data in the same app-data directory as SQLite.
/// Pre-creates the notes table so the first write doesn't pay the schema-creation cost.
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

    // Pre-create the notes table so the first write doesn't pay the schema-creation cost.
    notes::open_notes_table(&conn, embedder::DIMS).await?;

    Ok(conn)
}

/// LanceDB in a directory (for `perf-budget` and other debug harnesses).
#[cfg(debug_assertions)]
pub async fn connect_dir(dir: &std::path::Path) -> Result<Connection, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir
        .to_str()
        .ok_or_else(|| "Database path contains non-UTF8 characters".to_string())?;
    let conn = lancedb::connect(path)
        .execute()
        .await
        .map_err(|e| e.to_string())?;
    notes::open_notes_table(&conn, embedder::DIMS).await?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_excerpt_under_limit_unchanged() {
        assert_eq!(truncate_excerpt("hello", 10), "hello");
    }

    #[test]
    fn truncate_excerpt_inserts_ellipsis_when_longer() {
        let s = truncate_excerpt("abcdefghij", 4);
        assert!(s.contains('\u{2026}'));
        assert!(s.len() < "abcdefghij".len());
    }

    #[test]
    fn escape_sql_doubles_single_quotes() {
        assert_eq!(escape_sql("a'b"), "a''b");
    }

    #[tokio::test]
    #[ignore]
    async fn lancedb_connect_temp_dir_smoke() {
        let dir = tempfile::tempdir().unwrap();
        let uri = dir.path().to_str().unwrap();
        lancedb::connect(uri).execute().await.unwrap();
    }
}
