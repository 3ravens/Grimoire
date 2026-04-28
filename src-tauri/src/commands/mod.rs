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

use serde::Serialize;
use crate::KeyStore;

pub mod bookmarks;
pub mod calendar;
pub mod chat;
pub mod file_scanner;
pub mod hardware;
pub mod notes;
pub mod rag;
pub mod search;
pub mod settings;
pub mod tags;
pub mod properties;
pub mod templates;
pub mod wikipedia;

// Re-export all public command functions so lib.rs can keep using commands::create_note etc.
pub use bookmarks::*;
pub use calendar::*;
pub use chat::*;
pub use file_scanner::*;
pub use hardware::*;
pub use notes::*;
pub use rag::*;
pub use search::*;
pub use settings::*;
pub use tags::*;
pub use properties::*;
pub use templates::*;
pub use wikipedia::*;

// ---------------------------------------------------------------------------
// Shared structs
// ---------------------------------------------------------------------------

/// Raw note row as stored in SQLite. Used internally only — not sent to the frontend.
/// When encryption is active, `title` and `content` are base64-encoded ciphertext blobs.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct NoteRow {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub folder_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A note as returned to the frontend.
/// `locked` is true when the note's folder is locked and no session key is available.
/// When `locked` is true, `title` and `content` are empty strings.
#[derive(Debug, Serialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub folder_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub locked: bool,
}

/// Raw folder row as stored in SQLite. Used internally only.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct FolderRow {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
    pub locked: i64,
}

/// A folder as returned to the frontend.
/// `locked` is true when the folder has a password AND no session key is held for it.
#[derive(Debug, Serialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
    pub locked: bool,
}

/// A minimal note reference used for tag/link results — just enough to render
/// a clickable pill in the UI without transferring full note content.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedNote {
    pub id: i64,
    pub title: String,
}

/// A node in the knowledge graph (a note with its id and display title).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GraphNode {
    pub id: i64,
    pub title: String,
}

/// A directed edge between two notes in the knowledge graph.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
}

// ---------------------------------------------------------------------------
// Shared encryption helpers (used by notes, rag, and tags modules)
// ---------------------------------------------------------------------------

/// Resolve the active encryption key for a note given its folder_id.
/// Priority: folder key > vault key > None (plaintext).
pub(crate) fn resolve_key(folder_id: Option<i64>, keys: &KeyStore) -> Option<[u8; 32]> {
    if let Some(fid) = folder_id {
        if let Ok(fk) = keys.folder_keys.lock() {
            if let Some(k) = fk.get(&fid) {
                return Some(*k);
            }
        }
    }
    if let Ok(vk) = keys.vault_key.lock() {
        if let Some(k) = *vk {
            return Some(k);
        }
    }
    None
}

/// Returns true if the folder has a password AND no session key is held for it.
pub(crate) fn folder_is_locked(folder_id: i64, folder_locked_col: bool, keys: &KeyStore) -> bool {
    if !folder_locked_col {
        return false;
    }
    keys.folder_keys
        .lock()
        .map(|fk| !fk.contains_key(&folder_id))
        .unwrap_or(true)
}

/// Decrypt a note's title and content using `key`.
/// Falls back to the raw value if decryption fails (handles unencrypted rows).
pub(crate) fn decrypt_note_fields(
    key: &[u8; 32],
    enc_title: String,
    enc_content: String,
) -> (String, String) {
    let title = crate::crypto::decrypt(key, &enc_title)
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
        .unwrap_or(enc_title);
    let content = crate::crypto::decrypt(key, &enc_content)
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
        .unwrap_or(enc_content);
    (title, content)
}

/// Decrypt a folder name using `key`, falling back to raw value.
pub(crate) fn decrypt_folder_name(key: &[u8; 32], enc_name: String) -> String {
    crate::crypto::decrypt(key, &enc_name)
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
        .unwrap_or(enc_name)
}

/// Map a FolderRow into the public Folder struct.
pub(crate) fn map_folder_row(row: FolderRow, keys: &KeyStore) -> Folder {
    let is_locked = folder_is_locked(row.id, row.locked != 0, keys);
    let name = if is_locked {
        "<locked>".to_string()
    } else if let Some(key) = resolve_key(None, keys) {
        decrypt_folder_name(&key, row.name)
    } else {
        row.name
    };

    Folder {
        id: row.id,
        name,
        parent_id: row.parent_id,
        created_at: row.created_at,
        locked: is_locked,
    }
}

/// Map a NoteRow into the public Note struct, decrypting if a key is available.
pub(crate) fn map_note_row(row: NoteRow, folder_locked: bool, keys: &KeyStore) -> Note {
    let is_locked = row.folder_id
        .map(|fid| folder_is_locked(fid, folder_locked, keys))
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

    let vault_locked = keys.vault_key.lock()
        .map(|vk| vk.is_none())
        .unwrap_or(true);
    let _ = vault_locked;

    let (title, content) = if let Some(key) = resolve_key(row.folder_id, keys) {
        decrypt_note_fields(&key, row.title, row.content)
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
