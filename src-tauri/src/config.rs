
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

    /// Retries for transient embedding / LanceDB failures in background pipelines
    /// (Wikipedia index, file scanner, note re-index). Setting key `background_max_retries`.
    /// Default 2; clamped to 0..=10 when loaded or updated.
    pub background_max_retries: i64,
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
                             'wiki_perf_logging',
                             'background_max_retries'
             )",
        )
        .fetch_all(db)
        .await?;

        let mut embedding_model = "nomic-embed-text".to_string();
        let mut vault_path = String::new();
        let mut llm_force_enabled = false;
        let mut wikipedia_enabled = false;
        let mut wiki_perf_logging = false;
        let mut background_max_retries: Option<i64> = None;

        for (key, value) in rows {
            match key.as_str() {
                "embedding_model"  => embedding_model = value,
                "vault_path"       => vault_path = value,
                "llm_force_enabled" => llm_force_enabled = value == "true",
                "wikipedia_enabled" => wikipedia_enabled = value == "true",
                "wiki_perf_logging" => wiki_perf_logging = value == "true",
                "background_max_retries" => {
                    background_max_retries = value.parse::<i64>().ok();
                }
                _ => {}
            }
        }

        let background_max_retries = background_max_retries.unwrap_or(2).clamp(0, 10);

        Ok(AppConfig {
            embedding_model,
            vault_path,
            llm_force_enabled,
            wikipedia_enabled,
            wiki_perf_logging,
            background_max_retries,
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
            "background_max_retries" => {
                self.background_max_retries = value
                    .parse::<i64>()
                    .unwrap_or(2)
                    .clamp(0, 10);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn load_defaults_embedding_model_when_absent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let cfg = AppConfig::load(&pool).await.unwrap();
        assert_eq!(cfg.embedding_model, "nomic-embed-text");
        assert_eq!(cfg.background_max_retries, 2);
    }

    #[test]
    fn apply_change_clamps_background_max_retries() {
        let mut cfg = AppConfig {
            embedding_model: "m".into(),
            vault_path: String::new(),
            llm_force_enabled: false,
            wikipedia_enabled: false,
            wiki_perf_logging: false,
            background_max_retries: 2,
        };
        cfg.apply_change("background_max_retries", "99");
        assert_eq!(cfg.background_max_retries, 10);
        cfg.apply_change("background_max_retries", "-5");
        assert_eq!(cfg.background_max_retries, 0);
    }
}
