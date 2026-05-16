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

//! Storage abstraction for notes and folders.
//!
//! `EncryptedNoteStore` wraps `&SqlitePool` and `&KeyStore` and exposes an
//! interface that speaks notes and folders — callers never touch encryption
//! keys or raw ciphertext.  All key resolution, `crypto::encrypt`, and
//! `crypto::decrypt` calls live exclusively inside this module.

use std::collections::HashMap;
use sqlx::{Sqlite, SqlitePool, Transaction};
use crate::{AccessFilter, KeyStore, AppError, AppResult};
use crate::commands::{NoteRow, FolderRow, Note, Folder};

const NOTE_VERSION_LIMIT: i64 = 20;
const PREVIEW_TITLE_MAX: usize = 60;
const PREVIEW_BODY_MAX: usize = 80;

fn truncate_chars_with_ellipsis(s: &str, max_chars: usize) -> String {
    let t = s.trim();
    let count = t.chars().count();
    if count <= max_chars {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(max_chars).collect::<String>())
    }
}

/// First non-empty line (truncated), or trimmed body excerpt if blank lines only.
fn preview_body_excerpt(content: &str, max_chars: usize) -> String {
    for line in content.lines() {
        let t = line.trim();
        if !t.is_empty() {
            return truncate_chars_with_ellipsis(t, max_chars);
        }
    }
    truncate_chars_with_ellipsis(content.trim(), max_chars)
}

#[derive(Debug, sqlx::FromRow)]
struct NoteVersionRow {
    id: i64,
    title: String,
    content: String,
    is_encrypted: i64,
    created_at: i64,
}

/// A thin storage abstraction over SQLite and the session key store.
///
/// Constructed per command call — a borrow of the two Tauri-managed state
/// objects — so there is no allocation or extra synchronisation cost.
pub struct EncryptedNoteStore<'a> {
    pool: &'a SqlitePool,
    keys: &'a KeyStore,
}

impl<'a> EncryptedNoteStore<'a> {
    pub fn new(pool: &'a SqlitePool, keys: &'a KeyStore) -> Self {
        Self { pool, keys }
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    /// Return the active encryption key for the given folder.
    /// Priority: folder key > vault key > None (plaintext vault).
    fn key_for(&self, folder_id: Option<i64>) -> Option<[u8; 32]> {
        if let Some(fid) = folder_id {
            if let Ok(fk) = self.keys.folder_keys.lock() {
                if let Some(k) = fk.get(&fid) {
                    return Some(*k);
                }
            }
        }
        if let Ok(vk) = self.keys.vault_key.lock() {
            if let Some(k) = *vk {
                return Some(k);
            }
        }
        None
    }

    /// Returns true if the folder has a password but no session key is held for it.
    fn folder_locked(&self, folder_id: i64, locked_col: bool) -> bool {
        if !locked_col {
            return false;
        }
        self.keys.folder_keys
            .lock()
            .map(|fk| !fk.contains_key(&folder_id))
            .unwrap_or(true)
    }

    /// Returns `Err(Auth("folder_locked"))` when the note's folder is locked,
    /// letting write operations bail before touching SQLite.
    fn check_writable(&self, folder_id: Option<i64>, locked_col: bool) -> AppResult<()> {
        if folder_id.map(|fid| self.folder_locked(fid, locked_col)).unwrap_or(false) {
            return Err(AppError::Auth("folder_locked".to_string()));
        }
        Ok(())
    }

    /// Encrypt `plaintext` using the active key for `folder_id`, or return it
    /// unchanged when no encryption key is active.
    fn encrypt_str(&self, folder_id: Option<i64>, plaintext: &str) -> String {
        if let Some(key) = self.key_for(folder_id) {
            crate::crypto::encrypt(&key, plaintext.as_bytes())
        } else {
            plaintext.to_string()
        }
    }

    fn decrypt_version_text(&self, folder_id: Option<i64>, raw: String, is_encrypted: bool) -> AppResult<String> {
        if !is_encrypted {
            return Ok(raw);
        }
        let key = self
            .key_for(folder_id)
            .ok_or_else(|| AppError::Auth("folder_locked".to_string()))?;
        let bytes = crate::crypto::decrypt(&key, &raw)
            .map_err(|_| AppError::Auth("folder_locked".to_string()))?;
        String::from_utf8(bytes).map_err(|_| AppError::Auth("folder_locked".to_string()))
    }

    /// Map a raw `NoteRow` to the public `Note` struct, decrypting fields where
    /// a key is available.  Locked notes are returned as empty stubs.
    fn to_note(&self, row: NoteRow, folder_locked_col: bool) -> Note {
        let is_locked = row.folder_id
            .map(|fid| self.folder_locked(fid, folder_locked_col))
            .unwrap_or(false);

        if is_locked {
            return Note {
                id: row.id,
                title: String::new(),
                content: String::new(),
                folder_id: row.folder_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
                locked: true,
            };
        }

        let (title, content) = if let Some(key) = self.key_for(row.folder_id) {
            let title = crate::crypto::decrypt(&key, &row.title)
                .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                .unwrap_or(row.title);
            let content = crate::crypto::decrypt(&key, &row.content)
                .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                .unwrap_or(row.content);
            (title, content)
        } else {
            (row.title, row.content)
        };

        Note {
            id: row.id,
            title,
            content,
            folder_id: row.folder_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            locked: false,
        }
    }

    /// Map a raw `FolderRow` to the public `Folder` struct, decrypting the name.
    fn to_folder(&self, row: FolderRow) -> Folder {
        let is_locked = self.folder_locked(row.id, row.locked != 0);
        let name = if is_locked {
            "<locked>".to_string()
        } else if let Some(key) = self.key_for(None) {
            crate::crypto::decrypt(&key, &row.name)
                .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                .unwrap_or(row.name)
        } else {
            row.name
        };

        Folder {
            id: row.id,
            name,
            parent_id: row.parent_id,
            created_at: row.created_at,
            locked: is_locked,
            password_protected: row.locked != 0,
        }
    }

    // -------------------------------------------------------------------------
    // Notes
    // -------------------------------------------------------------------------

    /// Fetch a single note by id, decrypting its fields.
    pub async fn get_note(&self, id: i64) -> AppResult<Note> {
        let row = sqlx::query_as::<_, NoteRow>(
            "SELECT id, title, content, folder_id, created_at, updated_at
             FROM notes WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        let folder_locked_col = if let Some(fid) = row.folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };

        Ok(self.to_note(row, folder_locked_col))
    }

    async fn get_note_row_with_lock(&self, id: i64) -> AppResult<(NoteRow, bool)> {
        let row = sqlx::query_as::<_, NoteRow>(
            "SELECT id, title, content, folder_id, created_at, updated_at
             FROM notes WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        let folder_locked_col = if let Some(fid) = row.folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };

        Ok((row, folder_locked_col))
    }

    async fn snapshot_current_note_version(&self, note_id: i64) -> AppResult<()> {
        let (row, folder_locked_col) = self.get_note_row_with_lock(note_id).await?;
        self.check_writable(row.folder_id, folder_locked_col)?;
        let is_encrypted = i64::from(self.key_for(row.folder_id).is_some());

        sqlx::query(
            "INSERT INTO note_versions (note_id, title, content, is_encrypted)
             VALUES (?, ?, ?, ?)",
        )
        .bind(note_id)
        .bind(row.title)
        .bind(row.content)
        .bind(is_encrypted)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    async fn prune_note_versions(&self, note_id: i64) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM note_versions
             WHERE note_id = ?
               AND id NOT IN (
                   SELECT id
                   FROM note_versions
                   WHERE note_id = ?
                   ORDER BY created_at DESC, id DESC
                   LIMIT ?
               )",
        )
        .bind(note_id)
        .bind(note_id)
        .bind(NOTE_VERSION_LIMIT)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// List notes, optionally scoped to a specific folder.
    /// Pass `all = true` to fetch every note regardless of folder.
    pub async fn list_notes(&self, folder_id: Option<i64>, all: bool) -> AppResult<Vec<Note>> {
        let rows = if all {
            sqlx::query_as::<_, NoteRow>(
                "SELECT id, title, content, folder_id, created_at, updated_at
                 FROM notes ORDER BY updated_at DESC",
            )
            .fetch_all(self.pool)
            .await?
        } else {
            sqlx::query_as::<_, NoteRow>(
                "SELECT id, title, content, folder_id, created_at, updated_at
                 FROM notes WHERE folder_id IS ? ORDER BY updated_at DESC",
            )
            .bind(folder_id)
            .fetch_all(self.pool)
            .await?
        };

        let filter = AccessFilter::load(self.pool, self.keys).await?;

        Ok(rows.into_iter().map(|row| {
            let fl = !filter.is_accessible(row.folder_id);
            self.to_note(row, fl)
        }).collect())
    }

    /// List notes whose `created_at` or `updated_at` falls on the given UTC date
    /// (`YYYY-MM-DD`).  Used by the calendar heatmap detail view.
    pub async fn list_notes_for_day(&self, date_str: &str) -> AppResult<Vec<Note>> {
        let rows: Vec<NoteRow> = sqlx::query_as(
            "SELECT id, title, content, folder_id, created_at, updated_at
             FROM notes
             WHERE date(created_at, 'unixepoch') = ?1
                OR date(updated_at,  'unixepoch') = ?1",
        )
        .bind(date_str)
        .fetch_all(self.pool)
        .await?;

        let filter = AccessFilter::load(self.pool, self.keys).await?;

        Ok(rows.into_iter().map(|row| {
            let fl = !filter.is_accessible(row.folder_id);
            self.to_note(row, fl)
        }).collect())
    }

    /// Return all notes tagged with the given tag name (case-insensitive), decrypting
    /// each note's fields.  Locked notes are included as empty stubs so the caller
    /// receives the full count — consistent with how `list_notes` behaves.
    pub async fn notes_for_tag(&self, tag: &str) -> AppResult<Vec<Note>> {
        let rows: Vec<NoteRow> = sqlx::query_as(
            "SELECT n.id, n.title, n.content, n.folder_id, n.created_at, n.updated_at
             FROM notes n
             JOIN note_tags nt ON nt.note_id = n.id
             JOIN tags t ON t.id = nt.tag_id
             WHERE t.name = ?
             ORDER BY n.updated_at DESC",
        )
        .bind(tag)
        .fetch_all(self.pool)
        .await?;

        let filter = AccessFilter::load(self.pool, self.keys).await?;

        Ok(rows.into_iter().map(|row| {
            let fl = !filter.is_accessible(row.folder_id);
            self.to_note(row, fl)
        }).collect())
    }


    pub async fn create_note(&self, title: &str, folder_id: Option<i64>) -> AppResult<Note> {
        let folder_locked_col = if let Some(fid) = folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };
        self.check_writable(folder_id, folder_locked_col)?;

        let stored_title = self.encrypt_str(folder_id, title);

        let row = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, folder_id) VALUES (?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(folder_id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, false))
    }

    /// Create a note with title and body in one round-trip (atomic insert).
    pub async fn create_note_with_content(
        &self,
        title: &str,
        content: &str,
        folder_id: Option<i64>,
    ) -> AppResult<Note> {
        let folder_locked_col = if let Some(fid) = folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };
        self.check_writable(folder_id, folder_locked_col)?;

        let stored_title = self.encrypt_str(folder_id, title);
        let stored_content = self.encrypt_str(folder_id, content);

        let row = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(&stored_content)
        .bind(folder_id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, folder_locked_col))
    }

    /// Update a note's title and content, encrypting both.  Rejects locked folders.
    pub async fn update_note(&self, id: i64, title: &str, content: &str) -> AppResult<Note> {
        let current: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT folder_id FROM notes WHERE id = ?")
                .bind(id)
                .fetch_optional(self.pool)
                .await?;

        let folder_id = current.and_then(|(fid,)| fid);

        let folder_locked_col = if let Some(fid) = folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };

        self.check_writable(folder_id, folder_locked_col)?;

        let stored_title   = self.encrypt_str(folder_id, title);
        let stored_content = self.encrypt_str(folder_id, content);

        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes
             SET title = ?, content = ?, updated_at = unixepoch()
             WHERE id = ?
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(&stored_content)
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, folder_locked_col))
    }

    /// Explicit save path: snapshot current persisted note, then apply update.
    pub async fn save_note_with_version(&self, id: i64, title: &str, content: &str) -> AppResult<Note> {
        self.snapshot_current_note_version(id).await?;
        let updated = self.update_note(id, title, content).await?;
        self.prune_note_versions(id).await?;
        Ok(updated)
    }

    pub async fn get_note_versions(
        &self,
        note_id: i64,
    ) -> AppResult<Vec<(i64, i64, bool, String, String)>> {
        let (current_row, _) = self.get_note_row_with_lock(note_id).await?;
        let folder_id = current_row.folder_id;

        let versions: Vec<NoteVersionRow> = sqlx::query_as(
            "SELECT id, title, content, is_encrypted, created_at
             FROM note_versions
             WHERE note_id = ?
             ORDER BY created_at DESC, id DESC",
        )
        .bind(note_id)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(versions.len());
        for row in versions {
            let enc = row.is_encrypted != 0;
            let title_plain = self
                .decrypt_version_text(folder_id, row.title, enc)
                .unwrap_or_default();
            let content_plain = self
                .decrypt_version_text(folder_id, row.content, enc)
                .unwrap_or_default();
            let preview_title = truncate_chars_with_ellipsis(&title_plain, PREVIEW_TITLE_MAX);
            let preview_body = preview_body_excerpt(&content_plain, PREVIEW_BODY_MAX);
            out.push((
                row.id,
                row.created_at,
                enc,
                preview_title,
                preview_body,
            ));
        }
        Ok(out)
    }

    pub async fn get_note_version_content(&self, note_id: i64, version_id: i64) -> AppResult<(String, String, i64)> {
        let (current_row, folder_locked_col) = self.get_note_row_with_lock(note_id).await?;
        self.check_writable(current_row.folder_id, folder_locked_col)?;

        let version: NoteVersionRow = sqlx::query_as(
            "SELECT id, title, content, is_encrypted, created_at
             FROM note_versions
             WHERE id = ? AND note_id = ?",
        )
        .bind(version_id)
        .bind(note_id)
        .fetch_one(self.pool)
        .await?;

        let title = self.decrypt_version_text(
            current_row.folder_id,
            version.title,
            version.is_encrypted != 0,
        )?;
        let content = self.decrypt_version_text(
            current_row.folder_id,
            version.content,
            version.is_encrypted != 0,
        )?;

        Ok((title, content, version.created_at))
    }

    pub async fn restore_note_version(&self, note_id: i64, version_id: i64) -> AppResult<Note> {
        self.snapshot_current_note_version(note_id).await?;
        let (title, content, _) = self.get_note_version_content(note_id, version_id).await?;
        let restored = self.update_note(note_id, &title, &content).await?;
        self.prune_note_versions(note_id).await?;
        Ok(restored)
    }

    /// Rename a note (title only).
    pub async fn rename_note(&self, id: i64, name: &str) -> AppResult<Note> {
        let (row, folder_locked_col) = self.get_note_row_with_lock(id).await?;
        self.check_writable(row.folder_id, folder_locked_col)?;

        let stored_title = self.encrypt_str(row.folder_id, name);

        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes SET title = ?, updated_at = unixepoch() WHERE id = ?
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, folder_locked_col))
    }

    /// Duplicate a note into the same folder, appending " (copy)" to the title.
    /// Rejects locked folders.
    pub async fn duplicate_note(&self, id: i64) -> AppResult<Note> {
        let src_row = sqlx::query_as::<_, NoteRow>(
            "SELECT id, title, content, folder_id, created_at, updated_at FROM notes WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        let folder_id = src_row.folder_id;

        let folder_locked_col = if let Some(fid) = folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };

        self.check_writable(folder_id, folder_locked_col)?;

        let source = self.to_note(src_row, folder_locked_col);
        let new_title = format!("{} (copy)", source.title);

        let stored_title   = self.encrypt_str(folder_id, &new_title);
        let stored_content = self.encrypt_str(folder_id, &source.content);

        let row = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, content, folder_id)
             VALUES (?, ?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(&stored_content)
        .bind(folder_id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, false))
    }

    /// Move a note to a different folder (or to no folder when `folder_id` is None).
    pub async fn move_note(&self, id: i64, folder_id: Option<i64>) -> AppResult<Note> {
        let (row, src_locked_col) = self.get_note_row_with_lock(id).await?;
        self.check_writable(row.folder_id, src_locked_col)?;

        let dest_locked_col = if let Some(fid) = folder_id {
            sqlx::query_scalar::<_, i64>("SELECT locked FROM folders WHERE id = ?")
                .bind(fid)
                .fetch_optional(self.pool)
                .await?
                .unwrap_or(0) != 0
        } else {
            false
        };
        self.check_writable(folder_id, dest_locked_col)?;

        let plain = self.to_note(row, src_locked_col);
        if plain.locked {
            return Err(AppError::Auth("folder_locked".to_string()));
        }

        let stored_title = self.encrypt_str(folder_id, &plain.title);
        let stored_content = self.encrypt_str(folder_id, &plain.content);

        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes
             SET title = ?, content = ?, folder_id = ?, updated_at = unixepoch()
             WHERE id = ?
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(&stored_title)
        .bind(&stored_content)
        .bind(folder_id)
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_note(row, dest_locked_col))
    }

    /// Delete a note by id.
    pub async fn delete_note(&self, id: i64) -> AppResult<()> {
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Folders
    // -------------------------------------------------------------------------

    /// List all folders, decrypting names.
    pub async fn list_folders(&self) -> AppResult<Vec<Folder>> {
        let rows = sqlx::query_as::<_, FolderRow>(
            "SELECT id, name, parent_id, created_at, locked FROM folders ORDER BY name ASC",
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| self.to_folder(r)).collect())
    }

    /// Create a new folder, encrypting its name.
    pub async fn create_folder(&self, name: &str, parent_id: Option<i64>) -> AppResult<Folder> {
        let stored_name = self.encrypt_str(None, name);

        let row = sqlx::query_as::<_, FolderRow>(
            "INSERT INTO folders (name, parent_id) VALUES (?, ?)
             RETURNING id, name, parent_id, created_at, locked",
        )
        .bind(&stored_name)
        .bind(parent_id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_folder(row))
    }

    /// Same as [`Self::create_folder`] but participates in an open transaction.
    pub async fn create_folder_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        name: &str,
        parent_id: Option<i64>,
    ) -> AppResult<i64> {
        let stored_name = self.encrypt_str(None, name);
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO folders (name, parent_id) VALUES (?, ?) RETURNING id",
        )
        .bind(&stored_name)
        .bind(parent_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Insert a note with title and body inside a transaction (wizard starter packs).
    pub async fn insert_note_full_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        title: &str,
        content: &str,
        folder_id: Option<i64>,
    ) -> AppResult<i64> {
        let stored_title = self.encrypt_str(folder_id, title);
        let stored_content = self.encrypt_str(folder_id, content);
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(&stored_title)
        .bind(&stored_content)
        .bind(folder_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Rename a folder, encrypting the new name.
    pub async fn rename_folder(&self, id: i64, name: &str) -> AppResult<Folder> {
        let stored_name = self.encrypt_str(None, name);

        let row = sqlx::query_as::<_, FolderRow>(
            "UPDATE folders SET name = ? WHERE id = ?
             RETURNING id, name, parent_id, created_at, locked",
        )
        .bind(&stored_name)
        .bind(id)
        .fetch_one(self.pool)
        .await?;

        Ok(self.to_folder(row))
    }

    /// Search root folders (parent_id IS NULL) by their plaintext name.
    /// All root folder names are fetched and decrypted in-process; acceptable
    /// since root folder count is small (typically < 50).
    pub async fn find_root_folder_by_name(&self, name: &str) -> AppResult<Option<i64>> {
        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, name FROM folders WHERE parent_id IS NULL")
                .fetch_all(self.pool)
                .await?;

        let found = rows.iter().find(|(_, enc_name)| {
            let decrypted = if let Some(key) = self.key_for(None) {
                crate::crypto::decrypt(&key, enc_name)
                    .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                    .unwrap_or_else(|_| enc_name.clone())
            } else {
                enc_name.clone()
            };
            decrypted == name
        });

        Ok(found.map(|(id, _)| *id))
    }

    // -------------------------------------------------------------------------
    // Indexing support
    // -------------------------------------------------------------------------

    /// Batch-fetch current titles for the given note IDs, skipping locked notes.
    /// Returns a map from `note_id` → decrypted title for all accessible notes.
    ///
    /// Used by `search_notes` to cross-reference LanceDB results with SQLite —
    /// filters locked folders and ensures titles reflect the current ciphertext.
    pub async fn accessible_note_titles(&self, ids: &[i64]) -> AppResult<HashMap<i64, String>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT n.id, n.title, n.folder_id
             FROM notes n
             WHERE n.id IN ({placeholders})"
        );

        let mut q = sqlx::query_as::<_, (i64, String, Option<i64>)>(&sql);
        for id in ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(self.pool).await?;

        let filter = AccessFilter::load(self.pool, self.keys).await?;
        let mut map = HashMap::new();
        for (id, raw_title, folder_id) in rows {
            if !filter.is_accessible(folder_id) {
                continue;
            }

            let title = if let Some(key) = self.key_for(folder_id) {
                crate::crypto::decrypt(&key, &raw_title)
                    .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                    .unwrap_or(raw_title)
            } else {
                raw_title
            };
            map.insert(id, title);
        }

        Ok(map)
    }
}
