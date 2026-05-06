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

//! Folder hierarchy helpers shared by auth and indexing.

use sqlx::SqlitePool;

use crate::{AppError, AppResult};

/// Return the IDs of all folders in the subtree rooted at `folder_id`,
/// including `folder_id` itself.
pub async fn folder_subtree_ids(pool: &SqlitePool, folder_id: i64) -> AppResult<Vec<i64>> {
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
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}
