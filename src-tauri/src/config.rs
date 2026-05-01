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

//! Centralised, typed application configuration.
//!
//! `AppConfig` is loaded once at startup from the `settings` table and stored
//! in Tauri managed state as `SharedConfig` (`Arc<RwLock<AppConfig>>`).
//!
//! When `set_setting` writes a key, it calls `AppConfig::apply_change` so the
//! in-memory config stays current — no restart required.
//!
//! Command handlers that need a setting declare `config: State<'_, SharedConfig>`
//! and read typed fields (e.g. `config.read().unwrap().embedding_model.clone()`).
//! Raw `SELECT value FROM settings WHERE key = '...'` queries at call sites are
//! replaced by reads from this struct.

use std::sync::{Arc, RwLock};
use sqlx::SqlitePool;

/// Typed snapshot of the application settings that are read by Rust command
/// handlers.  Values are kept in sync with the `settings` table via
/// `apply_change`.
pub struct AppConfig {
    /// The Ollama model used for embedding notes, files, and Wikipedia articles.
    /// Defaults to `"nomic-embed-text"` when absent from the DB.
    pub embedding_model: String,

    /// Absolute path to the vault directory.  Used by the file scanner to
    /// reject paths that are already managed by the vault.
    /// Defaults to an empty string when absent (scanner guard becomes a no-op).
    pub vault_path: String,

    /// When true the user has opted in to LLM features despite hardware being
    /// below the recommended threshold.
    pub llm_force_enabled: bool,

    /// When true Wikipedia search is active and articles are included in RAG.
    pub wikipedia_enabled: bool,

    /// When true, emit periodic `[wiki_index_perf]` timing logs during
    /// wikipedia indexing runs. Disabled by default to avoid noisy logs.
    pub wiki_perf_logging: bool,
}

/// Convenience alias — the shared, mutable config handle stored in Tauri state.
pub type SharedConfig = Arc<RwLock<AppConfig>>;

impl AppConfig {
    /// Load all tracked settings from the database and return a populated
    /// `AppConfig`.  Missing keys fall back to their documented defaults.
    pub async fn load(db: &SqlitePool) -> Result<Self, sqlx::Error> {
        // Fetch every key we care about in a single query so we only hit the
        // DB once at startup.
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT key, value FROM settings
             WHERE key IN (
               'embedding_model',
               'vault_path',
               'llm_force_enabled',
                             'wikipedia_enabled',
                             'wiki_perf_logging'
             )",
        )
        .fetch_all(db)
        .await?;

        let mut embedding_model = "nomic-embed-text".to_string();
        let mut vault_path = String::new();
        let mut llm_force_enabled = false;
        let mut wikipedia_enabled = false;
        let mut wiki_perf_logging = false;

        for (key, value) in rows {
            match key.as_str() {
                "embedding_model"  => embedding_model = value,
                "vault_path"       => vault_path = value,
                "llm_force_enabled" => llm_force_enabled = value == "true",
                "wikipedia_enabled" => wikipedia_enabled = value == "true",
                "wiki_perf_logging" => wiki_perf_logging = value == "true",
                _ => {}
            }
        }

        Ok(AppConfig {
            embedding_model,
            vault_path,
            llm_force_enabled,
            wikipedia_enabled,
            wiki_perf_logging,
        })
    }

    /// Apply a single key/value change to the in-memory config.
    ///
    /// Called by `set_setting` after the DB write so the config stays in sync
    /// without requiring a restart.  Unknown keys are silently ignored.
    pub fn apply_change(&mut self, key: &str, value: &str) {
        match key {
            "embedding_model"  => self.embedding_model = value.to_string(),
            "vault_path"       => self.vault_path = value.to_string(),
            "llm_force_enabled" => self.llm_force_enabled = value == "true",
            "wikipedia_enabled" => self.wikipedia_enabled = value == "true",
            "wiki_perf_logging" => self.wiki_perf_logging = value == "true",
            _ => {}
        }
    }
}
