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
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;

/// Cancels the previous in-flight folder-unlock reindex when a new one starts.
#[derive(Clone)]
pub struct FolderUnlockReindexCoordinator(pub Arc<Mutex<Option<Arc<AtomicBool>>>>);

impl FolderUnlockReindexCoordinator {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Request cancellation of any prior job; returns this job's cancel flag.
    pub fn begin_job(&self) -> Arc<AtomicBool> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(prev) = g.take() {
            prev.store(true, Ordering::SeqCst);
        }
        let c = Arc::new(AtomicBool::new(false));
        *g = Some(c.clone());
        c
    }

    pub fn finish_job(&self, cancel: &Arc<AtomicBool>) {
        if let Ok(mut g) = self.0.lock() {
            if g.as_ref().is_some_and(|cur| Arc::ptr_eq(cur, cancel)) {
                *g = None;
            }
        }
    }

    /// Signal cancellation for any in-flight folder-unlock reindex (e.g. user locked the folder).
    pub fn cancel_current_job(&self) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(prev) = g.take() {
            prev.store(true, Ordering::SeqCst);
        }
    }
}

/// Shared map of cancellation flags for in-progress indexing operations,
/// keyed by bundle_id. Set to true to request cancellation.
pub struct CancelMap(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

impl CancelMap {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

/// Shared map of cancellation flags for in-progress file scanner indexing,
/// keyed by scanned_paths.id. Set to true to request cancellation.
pub struct FileScanCancelMap(pub Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>);

impl FileScanCancelMap {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

/// Ensures only one vault note re-index (`reindex_all`) runs at a time.
pub struct VaultReindexGate(pub Arc<tokio::sync::Mutex<()>>);

impl VaultReindexGate {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(())))
    }
}

pub mod audit;
pub mod bookmarks;
pub mod calendar;
pub mod chat;
pub mod file_scanner;
pub mod hardware;
pub mod notes;
pub mod rag;
mod scanner_extract;
pub mod search;
pub mod settings;
pub mod tags;
pub mod properties;
pub mod templates;
pub mod wikipedia;

// Re-export all public command functions so lib.rs can keep using commands::create_note etc.
pub use audit::*;
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
/// `password_protected` is true when a folder password is set in the database (even if
/// the user has unlocked the folder for this session).
#[derive(Debug, Serialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
    pub locked: bool,
    pub password_protected: bool,
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
    pub folder_id: Option<i64>,
}

/// A directed edge between two notes in the knowledge graph.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
}
