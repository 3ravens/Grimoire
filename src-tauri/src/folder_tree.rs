//! Folder hierarchy helpers shared by auth and indexing.

use sqlx::SqlitePool;

use crate::{AppError, AppResult};

/// Return the IDs of all folders in the subtree rooted at `folder_id`,
/// including `folder_id` itself.
pub async fn folder_subtree_ids(pool: &SqlitePool, folder_id: i64) -> AppResult<Vec<i64>> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "WITH RECURSIVE sub(id) AS (
             SELECT id FROM folders WHERE id = ?
             UNION
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
