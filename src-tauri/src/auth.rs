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

/// Vault and folder password-protection commands.
///
/// Design decisions:
/// - Keys are derived via Argon2id and held in `KeyStore` (app state) for the session only.
/// - The password itself is never stored anywhere — only a salt and an encrypted sentinel.
/// - Vault and folder locks are mutually exclusive: you cannot set a folder password while
///   the vault has a password set (avoids double-encryption and key management complexity).
/// - Encryption of note content and titles is handled in commands.rs; auth.rs only manages
///   key lifecycle (set/verify/clear) and the bulk re-encryption pass.
use sqlx::SqlitePool;
use tauri::State;
use zeroize::Zeroize;

use crate::{
    crypto,
    vector::VectorDb,
    KeyStore,
};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Return the IDs of all folders in the subtree rooted at `folder_id`,
/// including `folder_id` itself.
async fn folder_subtree_ids(pool: &SqlitePool, folder_id: i64) -> Result<Vec<i64>, String> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "WITH RECURSIVE sub(id) AS (
             SELECT id FROM folders WHERE id = ?
             UNION ALL
             SELECT f.id FROM folders f JOIN sub ON f.parent_id = sub.id
         )
         SELECT id FROM sub",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

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
    keys: State<'_, KeyStore>,
) -> Result<bool, String> {
    let has_pw = vault_has_password(pool).await?;
    if !has_pw {
        return Ok(false);
    }
    let key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
    Ok(key_guard.is_none())
}

/// Attempt to unlock the vault with the given password.
/// Returns true on success, false on wrong password.
/// On success the derived key is stored in `KeyStore` for this session.
#[tauri::command]
pub async fn unlock_vault(
    password: String,
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
) -> Result<bool, String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT salt, sentinel FROM vault_lock WHERE id = 1")
            .fetch_optional(pool.inner())
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

    let mut key = crypto::derive_key(&password, &salt);

    if !crypto::verify_sentinel(&key, &sentinel_b64) {
        key.zeroize();
        return Ok(false);
    }

    let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
    *key_guard = Some(key);
    Ok(true)
}

/// Lock the vault: zeroize and drop the in-memory key, and purge the search index.
/// LanceDB contains plaintext note excerpts — they must not remain readable while locked.
#[tauri::command]
pub async fn lock_vault(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
) -> Result<(), String> {
    {
        let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut k) = *key_guard {
            k.zeroize();
        }
        *key_guard = None;
    } // key_guard dropped here before the await
    // Purge LanceDB so note excerpts aren't readable while the vault is locked.
    crate::vector::purge_all(&vdb.0).await?;
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
    keys: State<'_, KeyStore>,
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
    let mut new_key = crypto::derive_key(&password, &salt);
    let sentinel = crypto::make_sentinel(&new_key);

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&salt);

    // Re-encrypt all notes.
    let note_rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, content FROM notes")
            .fetch_all(pool.inner())
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
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }

    // Re-encrypt all folder names.
    let folder_rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM folders WHERE locked = 0")
            .fetch_all(pool.inner())
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
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }

    // Write salt + sentinel (INSERT OR REPLACE handles both set and change).
    sqlx::query(
        "INSERT OR REPLACE INTO vault_lock (id, salt, sentinel) VALUES (1, ?, ?)",
    )
    .bind(&salt_b64)
    .bind(&sentinel)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Store new key in memory — drop the guard before any await point.
    {
        let mut key_guard = keys.vault_key.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut k) = *key_guard {
            k.zeroize();
        }
        *key_guard = Some(new_key);
    } // key_guard dropped here

    // Purge LanceDB — encrypted notes must not remain searchable.
    crate::vector::purge_all(&vdb.0).await?;
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
    keys: State<'_, KeyStore>,
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

    let mut verify_key = crypto::derive_key(&password, &salt);
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

    // Decrypt all notes back to plaintext.
    let note_rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, content FROM notes")
            .fetch_all(pool.inner())
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
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }

    // Decrypt all folder names.
    let folder_rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM folders WHERE locked = 0")
            .fetch_all(pool.inner())
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
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }

    // Delete vault_lock row.
    sqlx::query("DELETE FROM vault_lock WHERE id = 1")
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

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
    _keys: State<'_, KeyStore>,
    vdb: State<'_, VectorDb>,
) -> Result<(), String> {
    // Enforce mutual exclusivity.
    let vault_protected = vault_has_password(pool.clone()).await?;
    if vault_protected {
        return Err("vault_already_protected".to_string());
    }

    let salt = crypto::generate_salt();
    let mut new_key = crypto::derive_key(&password, &salt);
    let sentinel = crypto::make_sentinel(&new_key);

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&salt);

    // Collect all folders in the subtree (root + descendants).
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id).await?;

    // Encrypt notes in every folder in the subtree.
    for &fid in &subtree_ids {
        let note_rows: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, title, content FROM notes WHERE folder_id = ?")
                .bind(fid)
                .fetch_all(pool.inner())
                .await
                .map_err(|e| e.to_string())?;

        for (id, title, content) in &note_rows {
            let enc_title = crypto::encrypt(&new_key, title.as_bytes());
            let enc_content = crypto::encrypt(&new_key, content.as_bytes());
            sqlx::query("UPDATE notes SET title = ?, content = ? WHERE id = ?")
                .bind(&enc_title)
                .bind(&enc_content)
                .bind(id)
                .execute(pool.inner())
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
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

        // Remove notes from LanceDB.
        for (id, _, _) in &note_rows {
            crate::vector::remove(&vdb.0, *id).await?;
        }
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
    keys: State<'_, KeyStore>,
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

    let mut key = crypto::derive_key(&password, &salt);
    if !crypto::verify_sentinel(&key, &sentinel_b64) {
        key.zeroize();
        return Err("wrong_password".to_string());
    }

    // Decrypt notes.
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id).await?;

    for &fid in &subtree_ids {
        let note_rows: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, title, content FROM notes WHERE folder_id = ?")
                .bind(fid)
                .fetch_all(pool.inner())
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
                .execute(pool.inner())
                .await
                .map_err(|e| e.to_string())?;
        }

        // Clear folder lock columns.
        sqlx::query(
            "UPDATE folders SET locked = 0, salt = NULL, sentinel = NULL WHERE id = ?",
        )
        .bind(fid)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    }

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
    keys: State<'_, KeyStore>,
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

    let mut key = crypto::derive_key(&password, &salt);
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

/// Lock a folder for this session: zeroize and remove its key from memory.
/// Also locks all descendant folders that were unlocked with the same key.
#[tauri::command]
pub async fn lock_folder(
    folder_id: i64,
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
) -> Result<(), String> {
    let subtree_ids = folder_subtree_ids(pool.inner(), folder_id).await?;
    let mut folder_keys = keys.folder_keys.lock().map_err(|e| e.to_string())?;
    for fid in subtree_ids {
        if let Some(ref mut k) = folder_keys.get_mut(&fid) {
            k.zeroize();
        }
        folder_keys.remove(&fid);
    }
    Ok(())
}
