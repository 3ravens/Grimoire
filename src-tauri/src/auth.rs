/// Vault and folder password-protection commands.
///
/// Design decisions:
/// - Keys are derived via Argon2id and held in `KeyStore` (app state) for the session only.
/// - The password itself is never stored anywhere — only a salt and an encrypted sentinel.
/// - Vault and folder locks are mutually exclusive: you cannot set a folder password while
///   the vault has a password set (avoids double-encryption and key management complexity).
/// - Encryption of note content and titles is handled in commands.rs; auth.rs only manages
///   key lifecycle (set/verify/clear) and the bulk re-encryption pass.
use sqlx::{QueryBuilder, SqlitePool};
use tauri::State;
use zeroize::Zeroize;

use crate::{
    commands::search,
    crypto,
    folder_tree::folder_subtree_ids,
    vector::VectorDb,
    SharedKeyStore,
};

// ---------------------------------------------------------------------------
// Vault commands
// ---------------------------------------------------------------------------

/// Returns true if a vault password has been set (i.e. vault_lock table has a row).
#[tauri::command]
pub async fn vault_has_password(pool: State<'_, SqlitePool>) -> Result<bool, String> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vault_lock")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

/// Returns true if the vault is currently locked (has a password but no key in memory).
#[tauri::command]
pub async fn is_vault_locked(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> Result<bool, String> {
    let has_pw = vault_has_password(pool).await?;
    if !has_pw {
        return Ok(false);
    }
    let key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
    Ok(key_guard.is_none())
}

/// Core vault unlock logic (shared by the Tauri command and unit tests).
///
/// Returns `Ok(true)` when no vault password is configured, on successful unlock,
/// or `Ok(false)` when the password does not match the sentinel.
pub(crate) async fn try_unlock_vault(
    pool: &SqlitePool,
    keys: &SharedKeyStore,
    password: &str,
) -> Result<bool, String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT salt, sentinel FROM vault_lock WHERE id = 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let (salt_b64, sentinel_b64) = match row {
        Some(r) => r,
        // No vault password set — nothing to unlock.
        None => return Ok(true),
    };

    use base64::Engine;
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| format!("corrupt salt: {e}"))?;

    let mut key = crypto::derive_key(password, &salt).map_err(|e| e.to_string())?;

    if !crypto::verify_sentinel(&key, &sentinel_b64) {
        key.zeroize();
        return Ok(false);
    }

    let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
    *key_guard = Some(key);
    Ok(true)
}

/// Attempt to unlock the vault with the given password.
/// Returns true on success, false on wrong password.
/// On success the derived key is stored in `KeyStore` for this session.
#[tauri::command]
pub async fn unlock_vault(
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> Result<bool, String> {
    try_unlock_vault(pool.inner(), keys.inner(), &password).await
}

/// Lock the vault: zeroize and drop the in-memory key, and purge the search index.
/// LanceDB contains plaintext note excerpts — they must not remain readable while locked.
#[tauri::command]
pub async fn lock_vault(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
) -> Result<(), String> {
    {
        let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut k) = *key_guard {
            k.zeroize();
        }
        *key_guard = None;
    } // key_guard dropped here before the await
    // FTS must not retain plaintext titles/snippets while the vault is locked.
    search::fts_purge_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    // Purge LanceDB so note excerpts aren't readable while the vault is locked.
    crate::vector::notes::purge_all(&vdb.0).await?;
    crate::commands::rag::clear_vault_reindex_checkpoint(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Set (or change) the vault password.
///
/// - If no password is currently set: encrypts all existing note titles/content and
///   folder names with the new key, then writes salt+sentinel to vault_lock.
/// - If a password is already set (vault must be unlocked): re-encrypts everything
///   with the new key, then updates vault_lock.
/// - Clears the LanceDB index so encrypted notes aren't searchable while locked.
///
/// The user is shown a warning in the UI that there is no recovery option before
/// this command is called.
#[tauri::command]
pub async fn set_vault_password(
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
) -> Result<(), String> {
    // If there's an existing password, the vault must be unlocked.
    let has_pw = vault_has_password(pool.clone()).await?;
    let old_key: Option<[u8; 32]> = if has_pw {
        let key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        match *key_guard {
            Some(k) => Some(k),
            None => return Err("vault_locked".to_string()),
        }
    } else {
        None
    };

    // Derive new key.
    let salt = crypto::generate_salt();
    let mut new_key = crypto::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    let sentinel = crypto::make_sentinel(&new_key);

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&salt);

    // Re-encrypt all notes, folders, and vault_lock in one transaction.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let note_rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, content FROM notes")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    for (id, enc_title, enc_content) in note_rows {
        // If there was an old key, decrypt first; otherwise treat as plaintext.
        let plain_title = if let Some(ref old) = old_key {
            match crypto::decrypt(old, &enc_title) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
                // If decrypt fails, treat as unencrypted plaintext (migration from unprotected state).
                Err(_) => enc_title.clone(),
            }
        } else {
            enc_title
        };

        let plain_content = if let Some(ref old) = old_key {
            match crypto::decrypt(old, &enc_content) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
                Err(_) => enc_content.clone(),
            }
        } else {
            enc_content
        };

        let new_enc_title = crypto::encrypt(&new_key, plain_title.as_bytes());
        let new_enc_content = crypto::encrypt(&new_key, plain_content.as_bytes());

        sqlx::query("UPDATE notes SET title = ?, content = ? WHERE id = ?")
            .bind(&new_enc_title)
            .bind(&new_enc_content)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Re-encrypt all folder names.
    let folder_rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM folders WHERE locked = 0")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    for (id, enc_name) in folder_rows {
        let plain_name = if let Some(ref old) = old_key {
            match crypto::decrypt(old, &enc_name) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
                Err(_) => enc_name.clone(),
            }
        } else {
            enc_name
        };

        let new_enc_name = crypto::encrypt(&new_key, plain_name.as_bytes());
        sqlx::query("UPDATE folders SET name = ? WHERE id = ?")
            .bind(&new_enc_name)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Notes are ciphertext now — drop FTS rows so plaintext never lingers in SQLite FTS.
    search::fts_purge_tx(&mut tx).await.map_err(|e| e.to_string())?;

    // Write salt + sentinel (INSERT OR REPLACE handles both set and change).
    sqlx::query(
        "INSERT OR REPLACE INTO vault_lock (id, salt, sentinel) VALUES (1, ?, ?)",
    )
    .bind(&salt_b64)
    .bind(&sentinel)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    // Store new key in memory — drop the guard before any await point.
    {
        let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut k) = *key_guard {
            k.zeroize();
        }
        *key_guard = Some(new_key);
    } // key_guard dropped here

    // Purge LanceDB — encrypted notes must not remain searchable.
    crate::vector::notes::purge_all(&vdb.0).await?;
    crate::commands::rag::clear_vault_reindex_checkpoint(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    new_key.zeroize();
    Ok(())
}

/// Remove the vault password.
/// Decrypts all note content and folder names back to plaintext,
/// then deletes the vault_lock row.
/// The vault must be unlocked (key in memory) to call this.
#[tauri::command]
pub async fn remove_vault_password(
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> Result<(), String> {
    // Verify the supplied password before doing anything.
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT salt, sentinel FROM vault_lock WHERE id = 1")
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let (salt_b64, sentinel_b64) = row.ok_or("no_password_set")?;

    use base64::Engine;
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| format!("corrupt salt: {e}"))?;

    let mut verify_key = crypto::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    if !crypto::verify_sentinel(&verify_key, &sentinel_b64) {
        verify_key.zeroize();
        return Err("wrong_password".to_string());
    }
    verify_key.zeroize();

    // Get the in-memory key for decryption.
    let key = {
        let key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        match *key_guard {
            Some(k) => k,
            None => return Err("vault_locked".to_string()),
        }
    };

    // Decrypt all notes and folders, then remove vault_lock, in one transaction.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let note_rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, content FROM notes")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    for (id, enc_title, enc_content) in note_rows {
        let plain_title = match crypto::decrypt(&key, &enc_title) {
            Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
            Err(_) => enc_title,
        };
        let plain_content = match crypto::decrypt(&key, &enc_content) {
            Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
            Err(_) => enc_content,
        };
        sqlx::query("UPDATE notes SET title = ?, content = ? WHERE id = ?")
            .bind(&plain_title)
            .bind(&plain_content)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Decrypt all folder names.
    let folder_rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM folders WHERE locked = 0")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    for (id, enc_name) in folder_rows {
        let plain_name = match crypto::decrypt(&key, &enc_name) {
            Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
            Err(_) => enc_name,
        };
        sqlx::query("UPDATE folders SET name = ? WHERE id = ?")
            .bind(&plain_name)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Delete vault_lock row.
    sqlx::query("DELETE FROM vault_lock WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    // Rebuild FTS for plaintext notes now that the vault password is removed.
    search::fts_purge_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    search::fts_initial_sync(pool.inner()).await;

    // Clear the in-memory key.
    let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut k) = *key_guard {
        k.zeroize();
    }
    *key_guard = None;

    Ok(())
}

// ---------------------------------------------------------------------------
// Folder-level lock commands
// ---------------------------------------------------------------------------

/// Set a password on a specific folder.
///
/// Requires: vault must NOT have a password (vault and folder locks are mutually exclusive).
/// Encrypts all note titles and content in the folder, then stores salt+sentinel on the folder row.
#[tauri::command]
pub async fn set_folder_password(
    folder_id: i64,
    password: String,
    pool: State<'_, SqlitePool>,
    _keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
) -> Result<(), String> {
    // Enforce mutual exclusivity.
    let vault_protected = vault_has_password(pool.clone()).await?;
    if vault_protected {
        return Err("vault_already_protected".to_string());
    }

    let salt = crypto::generate_salt();
    let mut new_key = crypto::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    let sentinel = crypto::make_sentinel(&new_key);

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&salt);

    // Collect all folders in the subtree (root + descendants).
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id)
        .await
        .map_err(|e| e.to_string())?;

    let root_locked: i64 =
        sqlx::query_scalar("SELECT COALESCE(locked, 0) FROM folders WHERE id = ?")
            .bind(folder_id)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    if root_locked != 0 {
        return Err("folder_already_locked".to_string());
    }

    let child_ids: Vec<i64> = subtree_ids
        .iter()
        .copied()
        .filter(|&id| id != folder_id)
        .collect();
    if !child_ids.is_empty() {
        let mut qb = QueryBuilder::new(
            "SELECT COUNT(*) FROM folders WHERE COALESCE(locked, 0) != 0 AND id IN (",
        );
        {
            let mut sep = qb.separated(", ");
            for id in &child_ids {
                sep.push_bind(id);
            }
        }
        qb.push(")");
        let locked_children: i64 = qb
            .build_query_scalar()
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
        if locked_children > 0 {
            return Err("folder_subtree_has_locked_children".to_string());
        }
    }

    let mut all_note_ids: Vec<i64> = Vec::new();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // Encrypt notes in every folder in the subtree.
    for &fid in &subtree_ids {
        let note_rows: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, title, content FROM notes WHERE folder_id = ?")
                .bind(fid)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

        for (id, title, content) in &note_rows {
            all_note_ids.push(*id);
            let enc_title = crypto::encrypt(&new_key, title.as_bytes());
            let enc_content = crypto::encrypt(&new_key, content.as_bytes());
            sqlx::query("UPDATE notes SET title = ?, content = ? WHERE id = ?")
                .bind(&enc_title)
                .bind(&enc_content)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            search::fts_delete_tx(&mut tx, *id)
                .await
                .map_err(|e| e.to_string())?;
        }

        // Mark folder as locked with the same salt + sentinel.
        sqlx::query(
            "UPDATE folders SET locked = 1, salt = ?, sentinel = ? WHERE id = ?",
        )
        .bind(&salt_b64)
        .bind(&sentinel)
        .bind(fid)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    // Remove notes from LanceDB after SQLite commit succeeds.
    for id in all_note_ids {
        crate::vector::notes::remove(&vdb.0, id).await?;
    }

    new_key.zeroize();
    Ok(())
}

/// Remove the password from a folder.
/// Decrypts all note titles/content back to plaintext.
#[tauri::command]
pub async fn remove_folder_password(
    folder_id: i64,
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> Result<(), String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT salt, sentinel FROM folders WHERE id = ? AND locked = 1")
            .bind(folder_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let (salt_b64, sentinel_b64) = row.ok_or("folder_not_locked")?;

    use base64::Engine;
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| format!("corrupt salt: {e}"))?;

    let mut key = crypto::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    if !crypto::verify_sentinel(&key, &sentinel_b64) {
        key.zeroize();
        return Err("wrong_password".to_string());
    }

    // Decrypt notes.
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for &fid in &subtree_ids {
        let note_rows: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, title, content FROM notes WHERE folder_id = ?")
                .bind(fid)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

        for (id, enc_title, enc_content) in note_rows {
            let plain_title = match crypto::decrypt(&key, &enc_title) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
                Err(_) => enc_title,
            };
            let plain_content = match crypto::decrypt(&key, &enc_content) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string())?,
                Err(_) => enc_content,
            };
            sqlx::query("UPDATE notes SET title = ?, content = ? WHERE id = ?")
                .bind(&plain_title)
                .bind(&plain_content)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }

        // Clear folder lock columns.
        sqlx::query(
            "UPDATE folders SET locked = 0, salt = NULL, sentinel = NULL WHERE id = ?",
        )
        .bind(fid)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    // FTS + Lance for this subtree: the UI calls `start_folder_unlock_reindex`
    // after a successful remove so plaintext is not searchable until re-index completes.

    // All async work is done — now acquire the mutex to update session keys.
    {
        let mut folder_keys = keys.folder_keys.lock().map_err(|e| e.to_string())?;
        for &fid in &subtree_ids {
            if let Some(ref mut k) = folder_keys.get_mut(&fid) {
                k.zeroize();
            }
            folder_keys.remove(&fid);
        }
    }

    key.zeroize();
    Ok(())
}

/// Unlock a folder for this session.
/// Returns true on success, false on wrong password.
#[tauri::command]
pub async fn unlock_folder(
    folder_id: i64,
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> Result<bool, String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT salt, sentinel FROM folders WHERE id = ? AND locked = 1")
            .bind(folder_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let (salt_b64, sentinel_b64) = match row {
        Some(r) => r,
        None => return Ok(true), // Not locked.
    };

    use base64::Engine;
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&salt_b64)
        .map_err(|e| format!("corrupt salt: {e}"))?;

    let mut key = crypto::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    if !crypto::verify_sentinel(&key, &sentinel_b64) {
        key.zeroize();
        return Ok(false);
    }

    // Get the full subtree first (async), then acquire the mutex (sync-only section).
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id)
        .await
        .map_err(|e| e.to_string())?;

    {
        let mut folder_keys = keys.folder_keys.lock().map_err(|e| e.to_string())?;
        for fid in subtree_ids {
            folder_keys.insert(fid, key);
        }
    }
    key.zeroize();
    Ok(true)
}

/// Remove FTS rows and LanceDB vectors for every note in `folder_ids`.
/// Called when a folder is session-locked so plaintext search indexes are not left on disk.
pub(crate) async fn purge_folder_notes_from_search(
    pool: &SqlitePool,
    vdb: &lancedb::Connection,
    folder_ids: &[i64],
) -> Result<(), String> {
    let mut note_ids: Vec<i64> = Vec::new();
    for &fid in folder_ids {
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM notes WHERE folder_id = ?")
            .bind(fid)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        note_ids.extend(ids);
    }
    for id in &note_ids {
        search::fts_delete(pool, *id)
            .await
            .map_err(|e| e.to_string())?;
    }
    for id in note_ids {
        if let Err(e) = crate::vector::notes::remove(vdb, id).await {
            log::warn!("session lock: failed to remove vectors for note {id}: {e}");
        }
    }
    Ok(())
}

/// Lock a folder for this session: zeroize and remove its key from memory.
/// Also locks all descendant folders that were unlocked with the same key.
#[tauri::command]
pub async fn lock_folder(
    folder_id: i64,
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
    coord: State<'_, crate::commands::FolderUnlockReindexCoordinator>,
) -> Result<(), String> {
    coord.inner().cancel_current_job();
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id)
        .await
        .map_err(|e| e.to_string())?;
    purge_folder_notes_from_search(pool.inner(), &vdb.0, &subtree_ids).await?;
    let mut folder_keys = keys.folder_keys.lock().map_err(|e| e.to_string())?;
    for fid in subtree_ids {
        if let Some(ref mut k) = folder_keys.get_mut(&fid) {
            k.zeroize();
        }
        folder_keys.remove(&fid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use base64::Engine;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::KeyStore;

    async fn memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn empty_keys() -> SharedKeyStore {
        std::sync::Arc::new(KeyStore {
            vault_key: Mutex::new(None),
            folder_keys: Mutex::new(HashMap::new()),
        })
    }

    #[tokio::test]
    async fn try_unlock_no_vault_row_returns_true() {
        let pool = memory_pool().await;
        let keys = empty_keys();
        assert!(try_unlock_vault(&pool, &keys, "any")
            .await
            .expect("no error"));
    }

    #[tokio::test]
    async fn try_unlock_wrong_password_leaves_vault_key_empty() {
        let pool = memory_pool().await;
        let keys = empty_keys();

        let salt = crypto::generate_salt();
        let key = crypto::derive_key("correct-password", &salt).unwrap();
        let sentinel = crypto::make_sentinel(&key);
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);

        sqlx::query("INSERT INTO vault_lock (id, salt, sentinel) VALUES (1, ?, ?)")
            .bind(&salt_b64)
            .bind(&sentinel)
            .execute(&pool)
            .await
            .unwrap();

        assert!(!try_unlock_vault(&pool, &keys, "wrong-password")
            .await
            .unwrap());
        assert!(keys.vault_key.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn try_unlock_correct_password_stores_key() {
        let pool = memory_pool().await;
        let keys = empty_keys();

        let salt = crypto::generate_salt();
        let key = crypto::derive_key("my-vault-secret", &salt).unwrap();
        let sentinel = crypto::make_sentinel(&key);
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);

        sqlx::query("INSERT INTO vault_lock (id, salt, sentinel) VALUES (1, ?, ?)")
            .bind(&salt_b64)
            .bind(&sentinel)
            .execute(&pool)
            .await
            .unwrap();

        assert!(try_unlock_vault(&pool, &keys, "my-vault-secret")
            .await
            .unwrap());
        assert!(keys.vault_key.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn session_lock_purges_fts_for_subtree_notes() {
        use crate::commands::search;
        use crate::vector;

        let pool = memory_pool().await;
        let folder_id: i64 =
            sqlx::query_scalar("INSERT INTO folders (name) VALUES ('locked') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();
        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES ('Secret', 'body', ?) RETURNING id",
        )
        .bind(folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        search::fts_upsert(&pool, note_id, "Secret", "body").await;

        let dir = tempfile::tempdir().unwrap();
        let conn = vector::connect_dir(dir.path()).await.unwrap();

        purge_folder_notes_from_search(&pool, &conn, &[folder_id])
            .await
            .unwrap();

        let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes_fts WHERE rowid = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(cnt, 0);
    }
}
