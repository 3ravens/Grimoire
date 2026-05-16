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
use sqlx::SqlitePool;
use tauri::State;
use crate::SharedKeyStore;
use crate::{AppError, AppResult};
use crate::EncryptedNoteStore;

/// Returns true when `t` is a supported property definition type.
fn property_type_allowed(t: &str) -> bool {
    ["text", "number", "date", "boolean", "select"].contains(&t)
}

// ---------------------------------------------------------------------------
// Note properties / databases
// ---------------------------------------------------------------------------

/// A property definition — one per "column" in a folder's database schema.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PropertyDef {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
    pub options: Option<String>,  // JSON array, only for 'select' type
    pub position: i64,
}

/// A single property value attached to a note.
/// Denormalised: includes the def's name, type, and options so the frontend
/// can render the correct input without a second round-trip.
/// `value` is `None` when the note has no row for this property (LEFT JOIN miss
/// in the table view) and `Some(...)` when the note owns the property.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct NoteProperty {
    pub def_id: i64,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
    pub options: Option<String>,
    pub value: Option<String>,
}

/// Return all property definitions for a folder, ordered by position.
#[tauri::command]
pub async fn get_property_defs(
    pool: State<'_, SqlitePool>,
    folder_id: i64,
) -> AppResult<Vec<PropertyDef>> {
    let defs = sqlx::query_as::<_, PropertyDef>(
        "SELECT id, folder_id, name, type, options, position
         FROM property_defs
         WHERE folder_id = ?
         ORDER BY position ASC, id ASC",
    )
    .bind(folder_id)
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(defs)
}

/// Create a new property definition for a folder and return it.
#[tauri::command]
pub async fn create_property_def(
    pool: State<'_, SqlitePool>,
    folder_id: i64,
    name: String,
    r#type: String,
    options: Option<String>,
) -> AppResult<PropertyDef> {
    // Validate type
    if !property_type_allowed(&r#type) {
        return Err(AppError::InvalidInput(format!("Invalid property type: {}", r#type)));
    }

    // Auto-assign position as max+1
    let max_pos: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(position) FROM property_defs WHERE folder_id = ?",
    )
    .bind(folder_id)
    .fetch_one(pool.inner())
    .await
    ?;

    let position = max_pos.unwrap_or(-1) + 1;

    let def = sqlx::query_as::<_, PropertyDef>(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, ?, ?, ?, ?)
         RETURNING id, folder_id, name, type, options, position",
    )
    .bind(folder_id)
    .bind(&name)
    .bind(&r#type)
    .bind(&options)
    .bind(position)
    .fetch_one(pool.inner())
    .await
    ?;

    Ok(def)
}

/// Update a property definition (name, type, options).
#[tauri::command]
pub async fn update_property_def(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
    r#type: String,
    options: Option<String>,
) -> AppResult<PropertyDef> {
    if !["text", "number", "date", "boolean", "select"].contains(&r#type.as_str()) {
        return Err(AppError::InvalidInput(format!("Invalid property type: {}", r#type)));
    }

    let def = sqlx::query_as::<_, PropertyDef>(
        "UPDATE property_defs SET name = ?, type = ?, options = ? WHERE id = ?
         RETURNING id, folder_id, name, type, options, position",
    )
    .bind(&name)
    .bind(&r#type)
    .bind(&options)
    .bind(id)
    .fetch_one(pool.inner())
    .await
    ?;

    Ok(def)
}

/// Delete a property definition. Cascades to all note_properties rows.
#[tauri::command]
pub async fn delete_property_def(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM property_defs WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;
    Ok(())
}

/// Reorder a property definition to a new position.
/// Shifts other defs in the same folder to make room.
#[tauri::command]
pub async fn reorder_property_def(
    pool: State<'_, SqlitePool>,
    id: i64,
    new_position: i64,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    let (folder_id, current_position): (i64, i64) = sqlx::query_as(
        "SELECT folder_id, position FROM property_defs WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if new_position > current_position {
        sqlx::query(
            "UPDATE property_defs SET position = position - 1
             WHERE folder_id = ?1 AND position > ?2 AND position <= ?3",
        )
        .bind(folder_id)
        .bind(current_position)
        .bind(new_position)
        .execute(&mut *tx)
        .await?;
    } else if new_position < current_position {
        sqlx::query(
            "UPDATE property_defs SET position = position + 1
             WHERE folder_id = ?1 AND position >= ?2 AND position < ?3",
        )
        .bind(folder_id)
        .bind(new_position)
        .bind(current_position)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE property_defs SET position = ? WHERE id = ?")
        .bind(new_position)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Return property values for a note — only defs for which an explicit
/// note_properties row exists (INNER JOIN). Notes that have never had a
/// property set will return an empty list, keeping blank notes clean.
#[tauri::command]
pub async fn get_note_properties(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
) -> AppResult<Vec<NoteProperty>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(note_id).await?;
    if note.locked {
        return Ok(vec![]);
    }

    let props = sqlx::query_as::<_, NoteProperty>(
        "SELECT pd.id AS def_id, pd.name, pd.type, pd.options, np.value
         FROM note_properties np
         INNER JOIN property_defs pd ON pd.id = np.def_id
         WHERE np.note_id = ?
         ORDER BY pd.position ASC, pd.id ASC",
    )
    .bind(note_id)
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(props)
}

/// Set (upsert) a single property value for a note.
#[tauri::command]
pub async fn set_note_property(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    def_id: i64,
    value: String,
) -> AppResult<()> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(note_id).await?;
    if note.locked {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    let ok: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM notes n
         INNER JOIN property_defs pd ON pd.id = ?1 AND pd.folder_id = n.folder_id
         WHERE n.id = ?2",
    )
    .bind(def_id)
    .bind(note_id)
    .fetch_optional(pool.inner())
    .await?;

    if ok.is_none() {
        return Err(AppError::InvalidInput(
            "Property definition does not belong to this note's folder.".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
         ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
    )
    .bind(note_id)
    .bind(def_id)
    .bind(&value)
    .execute(pool.inner())
    .await
    ?;
    Ok(())
}

/// Return all notes in a folder with their property values, formatted for the
/// table/database view. Returns a list of objects, each containing the note
/// plus a map of def_id → value.
#[derive(Debug, Serialize)]
pub struct NoteWithProperties {
    pub id: i64,
    pub title: String,
    pub folder_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub properties: Vec<NoteProperty>,
}

#[tauri::command]
pub async fn list_notes_with_properties(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    folder_id: i64,
) -> AppResult<Vec<NoteWithProperties>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let notes = store.list_notes(Some(folder_id), false).await?;

    // Build per-note property values.
    let mut result = Vec::new();
    for note in notes {
        if note.locked {
            continue;
        }

        let props = sqlx::query_as::<_, NoteProperty>(
            "SELECT pd.id AS def_id, pd.name, pd.type, pd.options, np.value
             FROM property_defs pd
             LEFT JOIN note_properties np ON np.def_id = pd.id AND np.note_id = ?
             WHERE pd.folder_id = ?
             ORDER BY pd.position ASC, pd.id ASC",
        )
        .bind(note.id)
        .bind(folder_id)
        .fetch_all(pool.inner())
        .await
        ?;

        result.push(NoteWithProperties {
            id: note.id,
            title: note.title,
            folder_id: note.folder_id,
            created_at: note.created_at,
            updated_at: note.updated_at,
            properties: props,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_type_allowed_accepts_known_types() {
        for t in ["text", "number", "date", "boolean", "select"] {
            assert!(property_type_allowed(t), "{t}");
        }
    }

    #[test]
    fn property_type_allowed_rejects_unknown() {
        assert!(!property_type_allowed("json"));
        assert!(!property_type_allowed(""));
    }
}
