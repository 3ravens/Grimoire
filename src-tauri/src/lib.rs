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

mod access_filter;
pub mod app_data_migration;
mod audit;
mod auth;
mod chunking;
mod commands;
mod config;
mod crypto;
mod db;
pub mod error;
pub mod folder_tree;
mod hardware;
pub mod indexing_profile;
pub mod perf_budget;
mod note_store;
mod retry;
#[cfg(debug_assertions)]
pub mod test_data;
mod vector;
mod wizard_starter_packs;

#[cfg(debug_assertions)]
pub mod search_quality;

pub use access_filter::AccessFilter;
pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use note_store::EncryptedNoteStore;
pub use vector::connect_dir;
pub use commands::wizard::{
    maybe_backfill_wizard_completed,
    wizard_finish_impl,
    wizard_status_impl,
    WizardFinishResult,
    WizardStatus,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::Manager;

/// In-memory store for derived encryption keys.
/// Keys are never persisted to disk — they live only for the duration of the
/// app session. Restarting the app clears all keys, requiring re-unlock.
///
/// `vault_key`    — Some(...) when the vault password has been entered this session.
/// `folder_keys`  — maps folder_id → derived key for each unlocked folder this session.
pub struct KeyStore {
    pub vault_key: Mutex<Option<[u8; 32]>>,
    pub folder_keys: Mutex<HashMap<i64, [u8; 32]>>,
}

/// Shared handle to the session key store (cloneable for background tasks).
pub type SharedKeyStore = Arc<KeyStore>;

/// Empty session keystore for `perf-budget` and other debug harnesses.
#[cfg(debug_assertions)]
pub fn bench_shared_keystore() -> SharedKeyStore {
    Arc::new(KeyStore {
        vault_key: Mutex::new(None),
        folder_keys: Mutex::new(HashMap::new()),
    })
}

/// Public surface for the `perf-budget` binary (`src/bin/perf-budget.rs`).
#[cfg(debug_assertions)]
pub mod perf_budget_bin {
    pub use crate::commands::chat::measure_rag_chat_ttft;
    pub use crate::commands::notes::save_note_with_version_benchmark_path;
    pub use crate::commands::rag::index_note_vectors_for_benchmark;
    pub use crate::config::AppConfig;
    pub use crate::db::open_sqlite_file;
    pub use crate::test_data::{seed_test_vault_inner, SeedTestVaultParams};
    pub use crate::vector::connect_dir;
}

/// Public surface for the `search-quality` binary (`src/bin/search-quality.rs`).
#[cfg(debug_assertions)]
pub mod search_quality_bin {
    pub use crate::commands::rag::{index_note_vectors_for_benchmark, search_notes_semantic};
    pub use crate::commands::search::fts_search_inner;
    pub use crate::config::AppConfig;
    pub use crate::db::open_sqlite_file;
    pub use crate::search_quality::{
        cases_json_embedded, insert_anchor_notes, load_cases_from_str, SearchCase, SearchQualityFile,
        SEARCH_QUALITY_ANCHOR_COUNT, SEMANTIC_TOP3_PASS_MIN,
    };
    pub use crate::test_data::{seed_test_vault_inner, SeedTestVaultParams};
    pub use crate::vector::connect_dir;
    pub use crate::vector::notes::CHUNK_FETCH_LIMIT;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                app_data_migration::migrate_legacy_app_data_if_needed(&app_handle).unwrap_or_else(
                    |e| {
                        panic!(
                            "Failed to migrate app data from a preview install: {e}\n\
Your note vault on disk is unchanged. Preview app data was not deleted — \
see the app documentation for old folder names."
                        );
                    },
                );

                let grimoire_db = app_handle
                    .path()
                    .app_data_dir()
                    .ok()
                    .map(|dir| dir.join("grimoire.db"));

                let pool = db::init_db(&app_handle).await.unwrap_or_else(|e| {
                    let msg = e.to_string();
                    let hint = if msg.contains("has been modified") {
                        let path_ps = grimoire_db
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "%APPDATA%\\com.grimoire.app\\grimoire.db".to_string());
                        let wal_ps = grimoire_db
                            .as_ref()
                            .map(|p| format!("{}-wal", p.display()))
                            .unwrap_or_default();
                        let shm_ps = grimoire_db
                            .as_ref()
                            .map(|p| format!("{}-shm", p.display()))
                            .unwrap_or_default();
                        format!(
                            "\n\nThat means this database was migrated with an older copy of a migration file than \
what is in your source tree now (SQLx stores a checksum per version).\n\
Dev fix: quit Grimoire, delete the SQLite files below, then start again (migrations re-run).\n\
  Main DB: {path_ps}\n\
  WAL:     {wal_ps}\n\
  SHM:     {shm_ps}\n\
PowerShell (copy/paste):\n\
  Remove-Item -LiteralPath \"{path_ps}\" -Force -ErrorAction SilentlyContinue; \
Remove-Item -LiteralPath \"{wal_ps}\" -Force -ErrorAction SilentlyContinue; \
Remove-Item -LiteralPath \"{shm_ps}\" -Force -ErrorAction SilentlyContinue\n\
To keep this database: repair `_sqlx_migrations.checksum` for the listed migration version, or restore the migration \
SQL file to the exact bytes that were applied originally."
                        )
                    } else {
                        String::new()
                    };
                    panic!("failed to initialise database: {msg}{hint}");
                });
                // Audit retention (best-effort; never blocks startup on failure).
                crate::audit::prune_if_configured(&pool).await;
                let pool_for_fts = pool.clone();
                let pool_for_wiki_fts = pool.clone();

                let app_config = config::AppConfig::load(&pool)
                    .await
                    .expect("failed to load app config");
                app_handle.manage(Arc::new(RwLock::new(app_config)) as config::SharedConfig);

                let hw_snapshot = hardware::detect().await;
                let tier = indexing_profile::tier_from_env()
                    .unwrap_or_else(|| indexing_profile::tier_from_hardware(&hw_snapshot));
                let indexing_plan =
                    Arc::new(indexing_profile::plan_for_tier(tier));
                indexing_profile::init_global(Arc::clone(&indexing_plan));
                app_handle.manage(indexing_plan);

                app_handle.manage(pool);

                tauri::async_runtime::spawn(async move {
                    commands::search::fts_initial_sync(&pool_for_fts).await;
                });

                app_handle.manage(SharedKeyStore::new(KeyStore {
                    vault_key: Mutex::new(None),
                    folder_keys: Mutex::new(HashMap::new()),
                }));

                app_handle.manage(commands::FolderUnlockReindexCoordinator::new());

                app_handle.manage(commands::CancelMap::new());
                app_handle.manage(commands::FileScanCancelMap::new());
                app_handle.manage(commands::VaultReindexGate::new());

                // LanceDB open can be slow on large or legacy stores; do not block the window.
                let app_handle_vdb = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    match vector::init(&app_handle_vdb).await {
                        Ok(vdb) => {
                            let vdb_for_wiki_fts = vdb.clone();
                            app_handle_vdb.manage(vector::VectorDb(vdb));
                            commands::wikipedia_fts_initial_sync(
                                &pool_for_wiki_fts,
                                &vdb_for_wiki_fts,
                            )
                            .await;
                        }
                        Err(e) => {
                            log::error!(
                                "vector database init failed (semantic search unavailable until restart): {e}"
                            );
                        }
                    }
                });
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                #[cfg(debug_assertions)]
                {
                    let _ = window.open_devtools();
                }
            }

            Ok(())
        });

    // Register commands — debug-only commands are excluded from release builds.
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::create_note,
        commands::get_note,
        commands::resolve_note_embed_batch,
        commands::list_notes,
        commands::update_note,
        commands::save_note_with_version,
        commands::move_note,
        commands::rename_note,
        commands::delete_note,
        commands::duplicate_note,
        commands::get_note_versions,
        commands::get_note_version_content,
        commands::restore_note_version,
        commands::create_folder,
        commands::list_folders,
        commands::rename_folder,
        commands::delete_folder,
        commands::move_folder,
        commands::chat,
        commands::list_ollama_installed_models,
        commands::ollama_model_installed,
        commands::pull_ollama_model,
        commands::delete_ollama_model,
        commands::suggest_note_improvement,
        commands::suggest_hunk_refinement,
        commands::index_note,
        commands::remove_note_index,
        commands::search_notes,
        commands::reindex_all,
        commands::start_folder_unlock_reindex,
        commands::vault_reindex_status,
        commands::cancel_vault_reindex,
        commands::abandon_vault_reindex,
        commands::clear_notes_index,
        commands::clear_wiki_index,
        commands::clear_scanned_index,
        commands::sync_note_relations,
        commands::get_note_tags,
        commands::get_note_links,
        commands::get_backlinks,
        commands::get_unlinked_mentions,
        commands::convert_mention_to_link,
        commands::list_notes_by_tag,
        commands::list_all_tags,
        commands::get_graph_data,
        commands::fts_search,
        commands::combined_search,
        commands::list_templates,
        commands::create_template,
        commands::update_template,
        commands::delete_template,
        commands::get_property_defs,
        commands::create_property_def,
        commands::update_property_def,
        commands::delete_property_def,
        commands::reorder_property_def,
        commands::get_note_properties,
        commands::set_note_property,
        commands::list_notes_with_properties,
        commands::apply_template_to_note,
        commands::sync_template_to_notes,
        commands::apply_template_to_folder,
        commands::get_activity_heatmap,
        commands::get_notes_for_day,
        commands::get_or_create_daily_note,
        commands::create_daily_note,
        commands::resolve_daily_note_from_query,
        commands::export_notes,
        commands::export_single_note_markdown,
        commands::save_note_html_export,
        commands::log_note_export_pdf_print,
        commands::list_bookmarks,
        commands::add_bookmark,
        commands::remove_bookmark,
        commands::get_hardware_info,
        commands::get_llm_enabled,
        commands::get_running_models,
        commands::get_setting,
        commands::set_setting,
        commands::get_app_data_migration_banner,
        commands::dismiss_app_data_migration_banner,
        commands::wizard_status,
        commands::wizard_finish,
        auth::vault_has_password,
        auth::is_vault_locked,
        auth::unlock_vault,
        auth::lock_vault,
        auth::set_vault_password,
        auth::remove_vault_password,
        auth::set_folder_password,
        auth::remove_folder_password,
        auth::unlock_folder,
        auth::lock_folder,
        commands::test_zim_parse,
        commands::fetch_wikipedia_catalogue,
        commands::check_wikipedia_connectivity,
        commands::check_wikipedia_download_preflight,
        commands::list_wikipedia_bundles,
        commands::set_bundle_indexing_state,
        commands::cancel_wikipedia_indexing,
        commands::download_wikipedia_bundle,
        commands::remove_wikipedia_bundle,
        commands::index_wikipedia_bundle,
        commands::read_wikipedia_article,
        commands::search_wikipedia,
        commands::read_wikipedia_article_html,
        commands::serve_wikipedia_image,
        commands::resolve_wikipedia_link,
        commands::suggest_wikipedia_articles,
        commands::load_wikipedia_highlights,
        commands::save_wikipedia_highlight,
        commands::delete_wikipedia_highlight,
        commands::get_scanned_paths,
        commands::add_scanned_path,
        commands::update_scanned_path_excludes,
        commands::get_scanned_path_stale_summary,
        commands::clear_stale_scanned_files,
        commands::remove_scanned_path,
        commands::toggle_scanned_path,
        commands::rescan_path,
        commands::cancel_scanned_path_index,
        commands::search_scanned_files,
        commands::import_file_as_note,
        commands::get_audit_log,
        commands::get_audit_log_count,
        commands::clear_audit_log,
        commands::export_audit_log,
        commands::preview_audit_retention_prune,
        commands::prune_audit_log,
        commands::open_bug_report,
        commands::open_external_url,
        // ── Debug-only commands (excluded from release builds) ───────────────
        #[cfg(debug_assertions)]
        commands::debug_search,
        #[cfg(debug_assertions)]
        commands::debug_search_wikipedia,
        #[cfg(debug_assertions)]
        commands::debug_search_scanned_files,
        #[cfg(debug_assertions)]
        commands::benchmark_rag_chat_ttft,
        #[cfg(debug_assertions)]
        commands::benchmark_wikipedia_quality,
        #[cfg(debug_assertions)]
        commands::benchmark_wikipedia_indexing,
        #[cfg(debug_assertions)]
        commands::seed_notes,
        #[cfg(debug_assertions)]
        test_data::generate_test_data,
        #[cfg(debug_assertions)]
        test_data::clean_developer_database,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
