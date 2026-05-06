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
mod audit;
mod auth;
mod chunking;
mod commands;
mod config;
mod crypto;
mod db;
pub mod error;
mod hardware;
mod note_store;
mod retry;
mod vector;

pub use access_filter::AccessFilter;
pub use error::{AppError, AppResult};
pub use note_store::EncryptedNoteStore;

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
                let pool = db::init_db(&app_handle)
                    .await
                    .expect("failed to initialise database");
                // Audit retention (best-effort; never blocks startup on failure).
                crate::audit::prune_if_configured(&pool).await;
                let pool_for_fts = pool.clone();
                let pool_for_wiki_fts = pool.clone();

                let app_config = config::AppConfig::load(&pool)
                    .await
                    .expect("failed to load app config");
                app_handle.manage(Arc::new(RwLock::new(app_config)) as config::SharedConfig);

                app_handle.manage(pool);

                tauri::async_runtime::spawn(async move {
                    commands::search::fts_initial_sync(&pool_for_fts).await;
                });

                let vdb = vector::init(&app_handle)
                    .await
                    .expect("failed to initialise vector database");
                let vdb_for_wiki_fts = vdb.clone();
                app_handle.manage(vector::VectorDb(vdb));

                tauri::async_runtime::spawn(async move {
                    commands::wikipedia_fts_initial_sync(&pool_for_wiki_fts, &vdb_for_wiki_fts).await;
                });

                app_handle.manage(KeyStore {
                    vault_key: Mutex::new(None),
                    folder_keys: Mutex::new(HashMap::new()),
                });

                app_handle.manage(commands::CancelMap::new());
                app_handle.manage(commands::FileScanCancelMap::new());
            });

            Ok(())
        });

    // Register commands — debug-only commands are excluded from release builds.
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::create_note,
        commands::get_note,
        commands::list_notes,
        commands::update_note,
        commands::move_note,
        commands::rename_note,
        commands::delete_note,
        commands::duplicate_note,
        commands::create_folder,
        commands::list_folders,
        commands::rename_folder,
        commands::delete_folder,
        commands::move_folder,
        commands::chat,
        commands::suggest_note_improvement,
        commands::suggest_hunk_refinement,
        commands::index_note,
        commands::remove_note_index,
        commands::search_notes,
        commands::reindex_all,
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
        // ── Debug-only commands (excluded from release builds) ───────────────
        #[cfg(debug_assertions)]
        commands::debug_search,
        #[cfg(debug_assertions)]
        commands::debug_search_wikipedia,
        #[cfg(debug_assertions)]
        commands::debug_search_scanned_files,
        #[cfg(debug_assertions)]
        commands::benchmark_wikipedia_quality,
        #[cfg(debug_assertions)]
        commands::benchmark_wikipedia_indexing,
        #[cfg(debug_assertions)]
        commands::seed_notes,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
