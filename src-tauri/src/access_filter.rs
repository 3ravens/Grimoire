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

//! Locked-folder access filter.
//!
//! `AccessFilter` is a per-request snapshot of which folders are currently
//! inaccessible (password-protected with no session key held).  It is the
//! single place in the codebase that answers the question
//! "is this folder accessible right now?"
//!
//! Construct it once per command with `AccessFilter::load`, then call
//! `is_accessible` for each note or folder you want to check.
//!
//! ## Why a snapshot?
//! Folder lock state can only change when the user explicitly calls
//! `unlock_folder` or `lock_folder`, which never happen concurrently with a
//! read command.  Loading once at the top of a command is therefore safe and
//! avoids repeated SQL round-trips inside tight loops.

use std::collections::HashSet;
use sqlx::SqlitePool;
use crate::{AppError, AppResult, KeyStore};

/// A snapshot of currently inaccessible folders for one request.
///
/// A folder is inaccessible when it has `locked = 1` in the database AND no
/// session key for it is held in `KeyStore`.
pub struct AccessFilter {
    /// IDs of folders that are locked and have no current session key.
    locked: HashSet<i64>,
}

impl AccessFilter {
    /// Load the current access filter from the database and key store.
    ///
    /// Issues a single SQL query (`SELECT id FROM folders WHERE locked = 1`)
    /// and cross-references the set of session-unlocked folder IDs from
    /// `KeyStore`.  Only locked folders with no session key end up in the
    /// internal set, so `is_accessible` checks are O(1) lookups on a
    /// typically-empty `HashSet`.
    pub async fn load(pool: &SqlitePool, keys: &KeyStore) -> AppResult<Self> {
        let all_locked: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM folders WHERE locked = 1")
                .fetch_all(pool)
                .await?;

        let unlocked: HashSet<i64> = keys
            .folder_keys
            .lock()
            .map_err(|e| AppError::InvalidInput(e.to_string()))?
            .keys()
            .copied()
            .collect();

        let locked = all_locked
            .into_iter()
            .filter(|id| !unlocked.contains(id))
            .collect();

        Ok(Self { locked })
    }

    /// Returns `true` when the given folder (or note with no folder) is
    /// readable in the current session.
    ///
    /// Notes with no folder (`folder_id = None`) are always accessible.
    #[inline]
    pub fn is_accessible(&self, folder_id: Option<i64>) -> bool {
        folder_id.map(|fid| !self.locked.contains(&fid)).unwrap_or(true)
    }
}
