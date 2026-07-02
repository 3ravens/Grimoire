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

//! First-run installation wizard — status, backfill for existing vaults, and finish.

use lancedb::Connection;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::config::SharedConfig;
use crate::note_store::EncryptedNoteStore;
use crate::wizard_starter_packs::{apply_starter_pack_tx, parse_pack_id};
use crate::{AppResult, SharedKeyStore};
use crate::commands::rag::index_note_vectors_inner;

pub(crate) const KEY_WIZARD_DONE: &str = "wizard_v1_completed";
pub(crate) const KEY_STARTER_PACK: &str = "wizard_starter_pack_id";

async fn get_setting_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ? LIMIT 1")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .map(|v| v.unwrap_or_default())
}

async fn upsert_setting_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Run legacy-vault detection: existing notes/folders imply the wizard already de-facto ran.
pub async fn maybe_backfill_wizard_completed(pool: &SqlitePool) -> AppResult<()> {
    let done: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = ? LIMIT 1",
    )
    .bind(KEY_WIZARD_DONE)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    if done == "true" {
        return Ok(());
    }

    let note_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let folder_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    if note_count > 0 || folder_count > 0 {
        let mut tx = pool.begin().await?;
        upsert_setting_tx(&mut tx, KEY_WIZARD_DONE, "true").await?;
        let existing_pack = get_setting_tx(&mut tx, KEY_STARTER_PACK).await?;
        if existing_pack.is_empty() {
            upsert_setting_tx(&mut tx, KEY_STARTER_PACK, "legacy").await?;
        }
        tx.commit().await?;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardStatus {
    pub show_wizard: bool,
    pub note_count: i64,
    pub folder_count: i64,
}

/// Single cheap IPC: counts + whether the first-run wizard should appear.
pub async fn wizard_status_impl(pool: &SqlitePool) -> AppResult<WizardStatus> {
    maybe_backfill_wizard_completed(pool).await?;

    let done: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = ? LIMIT 1",
    )
    .bind(KEY_WIZARD_DONE)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    let note_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let folder_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let show = done != "true";

    Ok(WizardStatus {
        show_wizard: show,
        note_count,
        folder_count,
    })
}

#[tauri::command]
pub async fn wizard_status(pool: State<'_, SqlitePool>) -> AppResult<WizardStatus> {
    wizard_status_impl(pool.inner()).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardFinishResult {
    pub ok: bool,
    pub already_completed: bool,
    pub open_wikipedia_settings: bool,
}

/// Atomically apply starter pack (transactional), mark wizard complete, optional Wikipedia hint.
pub async fn wizard_finish_impl(
    pool: &SqlitePool,
    keys: &SharedKeyStore,
    config: &SharedConfig,
    vdb: &Connection,
    starter_pack_id: String,
    wikipedia_enabled: bool,
    open_wikipedia_settings_after: bool,
    chat_model: Option<String>,
    embedding_model: Option<String>,
) -> AppResult<WizardFinishResult> {
    maybe_backfill_wizard_completed(pool).await?;

    let mut tx = pool.begin().await?;

    let done = get_setting_tx(&mut tx, KEY_WIZARD_DONE).await?;
    if done == "true" {
        tx.commit().await?;
        return Ok(WizardFinishResult {
            ok: true,
            already_completed: true,
            open_wikipedia_settings: false,
        });
    }

    let pack = parse_pack_id(&starter_pack_id)?;
    let pack_key = pack.as_str();

    let store = EncryptedNoteStore::new(pool, keys);

    let indexed = apply_starter_pack_tx(&store, &mut tx, pack).await?;

    upsert_setting_tx(&mut tx, KEY_STARTER_PACK, pack_key).await?;
    upsert_setting_tx(&mut tx, KEY_WIZARD_DONE, "true").await?;

    if wikipedia_enabled {
        upsert_setting_tx(&mut tx, "wikipedia_enabled", "true").await?;
    }

    if let Some(ref m) = chat_model {
        let t = m.trim();
        if !t.is_empty() {
            upsert_setting_tx(&mut tx, "chat_model", t).await?;
        }
    }
    if let Some(ref m) = embedding_model {
        let t = m.trim();
        if !t.is_empty() {
            upsert_setting_tx(&mut tx, "embedding_model", t).await?;
        }
    }

    tx.commit().await?;

    if wikipedia_enabled {
        config
            .write()
            .unwrap()
            .apply_change("wikipedia_enabled", "true");
    }
    if let Some(ref m) = embedding_model {
        let t = m.trim();
        if !t.is_empty() {
            config
                .write()
                .unwrap()
                .apply_change("embedding_model", t);
        }
    }

    let pool_cl = pool.clone();
    let vdb_arc = vdb.clone();
    let should_embed = embedding_model
        .as_ref()
        .is_some_and(|m| !m.trim().is_empty());
    let embed_model_name = config.read().unwrap().embedding_model.clone();
    let max_retries = config.read().unwrap().background_max_retries;

    for (note_id, title, content) in indexed {
        crate::commands::search::fts_upsert(&pool_cl, note_id, &title, &content).await;
        if should_embed {
            let pool_i = pool_cl.clone();
            let conn = vdb_arc.clone();
            let em = embed_model_name.clone();
            let t = title.clone();
            let c = content.clone();
            tauri::async_runtime::spawn(async move {
                let _ = index_note_vectors_inner(
                    &pool_i,
                    &conn,
                    &em,
                    max_retries,
                    note_id,
                    &t,
                    &c,
                    crate::vector::embedder::EmbedBatchOptions::default(),
                )
                .await;
            });
        }
    }

    Ok(WizardFinishResult {
        ok: true,
        already_completed: false,
        open_wikipedia_settings: open_wikipedia_settings_after && wikipedia_enabled,
    })
}

#[tauri::command]
pub async fn wizard_finish(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    config: State<'_, SharedConfig>,
    vdb: State<'_, crate::vector::VectorDb>,
    starter_pack_id: String,
    wikipedia_enabled: bool,
    open_wikipedia_settings_after: bool,
    chat_model: Option<String>,
    embedding_model: Option<String>,
) -> AppResult<WizardFinishResult> {
    wizard_finish_impl(
        pool.inner(),
        keys.inner(),
        config.inner(),
        &vdb.0,
        starter_pack_id,
        wikipedia_enabled,
        open_wikipedia_settings_after,
        chat_model,
        embedding_model,
    )
    .await
}

#[cfg(test)]
mod ipc_shape_tests {
    use super::WizardStatus;

    /// `invoke("wizard_status")` JSON must use camelCase so the Svelte client matches Tauri IPC.
    #[test]
    fn wizard_status_json_keys_are_camel_case() {
        let s = WizardStatus {
            show_wizard: true,
            note_count: 7,
            folder_count: 3,
        };
        let v = serde_json::to_value(&s).expect("serialize WizardStatus");
        let m = v.as_object().expect("object");
        assert_eq!(m.get("showWizard"), Some(&serde_json::json!(true)));
        assert_eq!(m.get("noteCount"), Some(&serde_json::json!(7)));
        assert_eq!(m.get("folderCount"), Some(&serde_json::json!(3)));
        assert!(
            !m.contains_key("show_wizard"),
            "IPC must not emit snake_case show_wizard (frontend reads showWizard)"
        );
    }
}
