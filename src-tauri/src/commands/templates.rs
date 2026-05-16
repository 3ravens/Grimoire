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

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use crate::{AppError, AppResult};
use super::properties::{get_property_defs, PropertyDef};

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

/// A single property spec carried by a template.
/// When a note is created from a template in a folder, each spec is inserted
/// into property_defs for that folder (INSERT OR IGNORE — existing columns
/// with the same name are left unchanged).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplatePropertySpec {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub options: Option<String>, // JSON array string, only for 'select' type
}

/// A note template. Built-in templates use negative IDs and are hardcoded here;
/// user-created templates are stored in SQLite with positive AUTOINCREMENT IDs.
/// `builtin: true` means the template cannot be deleted or edited.
#[derive(Debug, Serialize)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub content: String,
    pub builtin: bool,
    /// Seed property definitions carried by this template.
    /// Empty for built-in templates.
    pub properties: Vec<TemplatePropertySpec>,
}

/// Raw template row from SQLite.
#[derive(Debug, sqlx::FromRow)]
struct TemplateRow {
    id: i64,
    name: String,
    title: String,
    content: String,
    properties: String, // JSON, e.g. '[{"name":"Status","type":"select","options":"[\"Draft\",\"Done\"]"}]'
}

impl TemplateRow {
    fn parse_properties(&self) -> Vec<TemplatePropertySpec> {
        serde_json::from_str(&self.properties).unwrap_or_default()
    }
}

/// The hardcoded built-in templates returned alongside user-created ones.
/// Negative IDs ensure they never clash with SQLite AUTOINCREMENT values.
fn builtin_templates() -> Vec<Template> {
    vec![
        Template {
            id: -1,
            name: "Blank".to_string(),
            title: String::new(),
            content: String::new(),
            builtin: true,
            properties: vec![],
        },
        Template {
            id: -2,
            name: "Meeting Notes".to_string(),
            title: "Meeting Notes".to_string(),
            content: "# Meeting Notes\n\n**Date:** \n**Attendees:** \n\n## Agenda\n\n- \n\n## Notes\n\n## Action Items\n\n- [ ] ".to_string(),
            builtin: true,
            properties: vec![],
        },
        Template {
            id: -3,
            name: "Daily Journal".to_string(),
            title: "Journal".to_string(),
            content: "# \n\n**Mood:** \n**Energy:** \n\n## Today\n\n## Goals\n\n- ".to_string(),
            builtin: true,
            properties: vec![],
        },
        Template {
            id: -4,
            name: "Book Notes".to_string(),
            title: "Book Notes".to_string(),
            content: "# \n\n**Author:** \n\n## Key Ideas\n\n## Quotes\n\n## Takeaways\n\n".to_string(),
            builtin: true,
            properties: vec![],
        },
    ]
}

/// Return all templates: built-ins first, then user-created ones from SQLite.
#[tauri::command]
pub async fn list_templates(pool: State<'_, SqlitePool>) -> AppResult<Vec<Template>> {
    let rows = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, name, title, content, properties FROM templates ORDER BY name ASC",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let mut result = builtin_templates();
    result.extend(rows.into_iter().map(|r| {
        let properties = r.parse_properties();
        Template {
            id: r.id,
            name: r.name,
            title: r.title,
            content: r.content,
            builtin: false,
            properties,
        }
    }));

    Ok(result)
}

/// Create a new user-defined template and return it.
#[tauri::command]
pub async fn create_template(
    pool: State<'_, SqlitePool>,
    name: String,
    title: String,
    content: String,
    properties: Vec<TemplatePropertySpec>,
) -> AppResult<Template> {
    let props_json = serde_json::to_string(&properties)
        .map_err(|e| AppError::InvalidInput(format!("Failed to serialize properties: {e}")))?;

    let row = sqlx::query_as::<_, TemplateRow>(
        "INSERT INTO templates (name, title, content, properties) VALUES (?, ?, ?, ?)
         RETURNING id, name, title, content, properties",
    )
    .bind(&name)
    .bind(&title)
    .bind(&content)
    .bind(&props_json)
    .fetch_one(pool.inner())
    .await
    ?;

    let props = row.parse_properties();
    Ok(Template {
        id: row.id,
        name: row.name,
        title: row.title,
        content: row.content,
        builtin: false,
        properties: props,
    })
}

/// Update a user-created template's name, title, content, and property specs.
/// Returns an error if `id` is negative (built-in templates cannot be edited).
#[tauri::command]
pub async fn update_template(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
    title: String,
    content: String,
    properties: Vec<TemplatePropertySpec>,
) -> AppResult<Template> {
    if id <= 0 {
        return Err(AppError::InvalidInput("Built-in templates cannot be edited.".to_string()));
    }

    let props_json = serde_json::to_string(&properties)
        .map_err(|e| AppError::InvalidInput(format!("Failed to serialize properties: {e}")))?;

    let row = sqlx::query_as::<_, TemplateRow>(
        "UPDATE templates SET name = ?, title = ?, content = ?, properties = ? WHERE id = ?
         RETURNING id, name, title, content, properties",
    )
    .bind(&name)
    .bind(&title)
    .bind(&content)
    .bind(&props_json)
    .bind(id)
    .fetch_one(pool.inner())
    .await
    ?;

    let props = row.parse_properties();
    Ok(Template {
        id: row.id,
        name: row.name,
        title: row.title,
        content: row.content,
        builtin: false,
        properties: props,
    })
}

/// Delete a user-created template by id.
/// Returns an error if `id` is negative (built-in templates cannot be deleted).
#[tauri::command]
pub async fn delete_template(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    if id <= 0 {
        return Err(AppError::InvalidInput("Built-in templates cannot be deleted.".to_string()));
    }

    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    Ok(())
}

/// Apply a template's property specs to a specific note.
/// For each spec:
///   1. INSERT OR IGNORE the def into the folder schema (so the database/table
///      view has a column for it).
///   2. INSERT OR IGNORE an empty note_properties row for this note specifically,
///      so the note's properties panel shows it immediately.
/// Blank notes created without a template are never touched and remain property-free.
#[tauri::command]
pub async fn apply_template_to_note(
    pool: State<'_, SqlitePool>,
    note_id: i64,
    folder_id: i64,
    template_id: i64,
) -> AppResult<Vec<PropertyDef>> {
    // Built-in templates (negative IDs) carry no property specs — nothing to do.
    if template_id <= 0 {
        return get_property_defs(pool, folder_id).await;
    }

    let row: Option<TemplateRow> = sqlx::query_as(
        "SELECT id, name, title, content, properties FROM templates WHERE id = ?",
    )
    .bind(template_id)
    .fetch_optional(pool.inner())
    .await
    ?;

    let specs = row.map(|r| r.parse_properties()).unwrap_or_default();

    let mut tx = pool.begin().await?;

    for spec in &specs {
        if !["text", "number", "date", "boolean", "select"].contains(&spec.r#type.as_str()) {
            continue; // skip malformed specs silently
        }

        // Get current max position so the new def lands at the end.
        let max_pos: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(position) FROM property_defs WHERE folder_id = ?",
        )
        .bind(folder_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(None);

        let position = max_pos.unwrap_or(-1) + 1;

        // INSERT OR IGNORE: if a def with this name already exists, leave it untouched.
        // Also carry the template_id so future syncs can find this column.
        sqlx::query(
            "INSERT OR IGNORE INTO property_defs (folder_id, name, type, options, position, template_id)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(folder_id)
        .bind(&spec.name)
        .bind(&spec.r#type)
        .bind(&spec.options)
        .bind(position)
        .bind(template_id)
        .execute(&mut *tx)
        .await
        ?;

        // Find the def id (just inserted or pre-existing).
        let def_id: i64 = sqlx::query_scalar(
            "SELECT id FROM property_defs WHERE folder_id = ? AND name = ?",
        )
        .bind(folder_id)
        .bind(&spec.name)
        .fetch_one(&mut *tx)
        .await
        ?;

        // If the def existed before tracking was introduced (template_id IS NULL),
        // stamp it now — retroactively linking it to this template.
        sqlx::query(
            "UPDATE property_defs SET template_id = ? WHERE id = ? AND template_id IS NULL",
        )
        .bind(template_id)
        .bind(def_id)
        .execute(&mut *tx)
        .await
        ?;

        // Seed an empty note_properties row so this note "owns" the property.
        sqlx::query(
            "INSERT OR IGNORE INTO note_properties (note_id, def_id, value) VALUES (?, ?, '')",
        )
        .bind(note_id)
        .bind(def_id)
        .execute(&mut *tx)
        .await
        ?;
    }

    // Stamp the note so future auto-syncs can find it.
    sqlx::query("UPDATE notes SET template_id = ? WHERE id = ?")
        .bind(template_id)
        .bind(note_id)
        .execute(&mut *tx)
        .await
        ?;

    tx.commit().await?;

    get_property_defs(pool, folder_id).await
}

// ---------------------------------------------------------------------------
// Template sync
// ---------------------------------------------------------------------------

/// Auto-sync a template's current property specs to all notes/folders that
/// were previously created from it (i.e. that have a matching template_id).
///
/// For each folder that owns at least one tracked def from this template:
///   - Updates `options` on existing defs whose name matches the current spec
///   - Inserts missing defs (new properties added to the template since creation)
///   - Seeds empty note_properties rows for template-tracked notes in that folder
///
/// Called automatically after `update_template` saves.
/// Returns a human-readable summary string.
#[tauri::command]
pub async fn sync_template_to_notes(
    pool: State<'_, SqlitePool>,
    template_id: i64,
) -> AppResult<String> {
    if template_id <= 0 {
        return Ok("Built-in templates cannot be synced.".to_string());
    }

    // Load the template's current specs.
    let row: Option<TemplateRow> = sqlx::query_as(
        "SELECT id, name, title, content, properties FROM templates WHERE id = ?",
    )
    .bind(template_id)
    .fetch_optional(pool.inner())
    .await
    ?;

    let specs = match row {
        Some(r) => r.parse_properties(),
        None => return Ok("Template not found.".to_string()),
    };

    // Find every folder that already has at least one def tracked to this template.
    let folder_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT folder_id FROM property_defs WHERE template_id = ? AND folder_id IS NOT NULL",
    )
    .bind(template_id)
    .fetch_all(pool.inner())
    .await
    ?;

    let mut total_folders = 0i64;
    let mut total_notes = 0i64;

    for folder_id in &folder_ids {
        let seeded = sync_specs_to_folder(pool.inner(), &specs, *folder_id, template_id).await?;
        total_folders += 1;
        total_notes += seeded;
    }

    Ok(format!(
        "Synced {} folder(s), {} note(s) updated.",
        total_folders, total_notes
    ))
}

/// Force-apply a template's current property specs to a specific folder,
/// regardless of whether the notes or defs have a template_id stamped.
///
/// This is the manual "Sync from template" action in the database view.
/// It covers notes created before tracking was introduced.
///
/// After syncing:
///   - All notes in the folder have `template_id` stamped (so future auto-syncs
///     will find them too)
///   - All newly-created defs have `template_id` set
///   - Existing same-named defs get their `options` updated and `template_id`
///     stamped if it was NULL
///
/// Returns the number of note_properties rows seeded.
#[tauri::command]
pub async fn apply_template_to_folder(
    pool: State<'_, SqlitePool>,
    template_id: i64,
    folder_id: i64,
) -> AppResult<String> {
    if template_id <= 0 {
        return Err(AppError::InvalidInput("Built-in templates cannot be synced to a folder.".to_string()));
    }

    let row: Option<TemplateRow> = sqlx::query_as(
        "SELECT id, name, title, content, properties FROM templates WHERE id = ?",
    )
    .bind(template_id)
    .fetch_optional(pool.inner())
    .await
    ?;

    let specs = match row {
        Some(r) => r.parse_properties(),
        None => return Err(AppError::NotFound("Template not found.".to_string())),
    };

    let seeded = sync_specs_to_folder(pool.inner(), &specs, folder_id, template_id).await?;

    // Stamp all unstamped notes in this folder so future auto-syncs include them.
    sqlx::query(
        "UPDATE notes SET template_id = ? WHERE folder_id = ? AND template_id IS NULL",
    )
    .bind(template_id)
    .bind(folder_id)
    .execute(pool.inner())
    .await
    ?;

    Ok(format!("Applied template to folder. {} note(s) updated.", seeded))
}

/// Shared helper: diff a template's specs against a folder's existing property_defs,
/// insert missing defs, update options on existing ones, and seed empty
/// note_properties rows for notes that are missing a value for each new def.
///
/// Returns the number of note_properties rows inserted (i.e. note×def pairs seeded).
async fn sync_specs_to_folder(
    pool: &SqlitePool,
    specs: &[TemplatePropertySpec],
    folder_id: i64,
    template_id: i64,
) -> AppResult<i64> {
    let mut seeded_total: i64 = 0;

    for spec in specs {
        if !["text", "number", "date", "boolean", "select"].contains(&spec.r#type.as_str()) {
            continue;
        }

        // Does a def with this name already exist in this folder?
        let existing: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT id, options FROM property_defs WHERE folder_id = ? AND name = ?",
        )
        .bind(folder_id)
        .bind(&spec.name)
        .fetch_optional(pool)
        .await
        ?;

        let def_id: i64 = if let Some((id, _)) = existing {
            // Def exists — update its options (e.g. new select options added to the template)
            // and stamp template_id if it was untracked.
            sqlx::query(
                "UPDATE property_defs SET options = ?, template_id = COALESCE(template_id, ?)
                 WHERE id = ?",
            )
            .bind(&spec.options)
            .bind(template_id)
            .bind(id)
            .execute(pool)
            .await
            ?;
            id
        } else {
            // Def is new — insert it at the end of this folder's column list.
            let max_pos: Option<i64> =
                sqlx::query_scalar("SELECT MAX(position) FROM property_defs WHERE folder_id = ?")
                    .bind(folder_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(None);

            let position = max_pos.unwrap_or(-1) + 1;

            sqlx::query(
                "INSERT INTO property_defs (folder_id, name, type, options, position, template_id)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(folder_id)
            .bind(&spec.name)
            .bind(&spec.r#type)
            .bind(&spec.options)
            .bind(position)
            .bind(template_id)
            .execute(pool)
            .await
            ?;

            sqlx::query_scalar("SELECT id FROM property_defs WHERE folder_id = ? AND name = ?")
                .bind(folder_id)
                .bind(&spec.name)
                .fetch_one(pool)
                .await
                ?
        };

        // Seed empty note_properties rows for every note in this folder that
        // doesn't already have a value for this def.
        let result = sqlx::query(
            "INSERT OR IGNORE INTO note_properties (note_id, def_id, value)
             SELECT n.id, ?, ''
             FROM notes n
             WHERE n.folder_id = ?
             AND NOT EXISTS (
                 SELECT 1 FROM note_properties np WHERE np.note_id = n.id AND np.def_id = ?
             )",
        )
        .bind(def_id)
        .bind(folder_id)
        .bind(def_id)
        .execute(pool)
        .await
        ?;

        seeded_total += result.rows_affected() as i64;
    }

    Ok(seeded_total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_row_parse_properties_reads_json_specs() {
        let row = TemplateRow {
            id: 1,
            name: "Custom".into(),
            title: "T".into(),
            content: "C".into(),
            properties: r#"[{"name":"Status","type":"select","options":"[\"A\",\"B\"]"}]"#
                .into(),
        };
        let specs = row.parse_properties();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "Status");
        assert_eq!(specs[0].r#type, "select");
        assert!(specs[0].options.as_ref().is_some_and(|o| o.contains('A')));
    }

    #[test]
    fn template_row_parse_properties_malformed_json_yields_empty() {
        let row = TemplateRow {
            id: 2,
            name: "Bad".into(),
            title: "".into(),
            content: "".into(),
            properties: "not-json".into(),
        };
        assert!(row.parse_properties().is_empty());
    }

    #[test]
    fn builtin_templates_include_blank() {
        let templates = builtin_templates();
        assert!(templates.into_iter().any(|t| t.name == "Blank"));
    }
}
