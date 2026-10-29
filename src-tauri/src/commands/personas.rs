//! Chat personas: settings-backed prompts for conversational chat (T-005).

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::{AppError, AppResult};

const ACTIVE_SETTING_KEY: &str = "chat_active_persona";
const MAX_TITLE_CHARS: usize = 80;
const MAX_BODY_CHARS: usize = 4000;

const ID_BLUNT: &str = "builtin:blunt-editor";
const ID_SOCRATIC: &str = "builtin:socratic-tutor";
const ID_CITE: &str = "builtin:cite-notes-only";

/// A persona as returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub title: String,
    pub body: String,
    pub also_apply_verbosity: bool,
    pub builtin: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct PersonaRow {
    id: i64,
    title: String,
    body: String,
    also_apply_verbosity: i64,
}

fn user_id(row_id: i64) -> String {
    format!("user:{row_id}")
}

fn parse_user_id(id: &str) -> Option<i64> {
    id.strip_prefix("user:")?.parse().ok()
}

fn row_to_persona(row: PersonaRow) -> Persona {
    Persona {
        id: user_id(row.id),
        title: row.title,
        body: row.body,
        also_apply_verbosity: row.also_apply_verbosity != 0,
        builtin: false,
    }
}

fn builtin_personas() -> Vec<Persona> {
    vec![
        Persona {
            id: ID_BLUNT.to_string(),
            title: "Blunt editor".to_string(),
            body: "You are a blunt editor. Be short and direct. Prioritise clarity over politeness. Call out weak phrasing, fluff, and vagueness. Do not pad answers with praise.".to_string(),
            also_apply_verbosity: false,
            builtin: true,
        },
        Persona {
            id: ID_SOCRATIC.to_string(),
            title: "Socratic tutor".to_string(),
            body: "You are a Socratic tutor. Ask guiding questions that help the user think. Prefer questions over dumping the full answer first. When you must state facts from sources, keep them brief and return to questions.".to_string(),
            also_apply_verbosity: false,
            builtin: true,
        },
        Persona {
            id: ID_CITE.to_string(),
            title: "Cite notes only".to_string(),
            body: "Prefer the user's notes when they are relevant. Do not lead with general knowledge or Wikipedia when the notes cover the question. If no accessible notes apply, follow the app's normal source rules.".to_string(),
            also_apply_verbosity: false,
            builtin: true,
        },
    ]
}

fn find_builtin(id: &str) -> Option<Persona> {
    builtin_personas().into_iter().find(|p| p.id == id)
}

fn validate_fields(title: &str, body: &str) -> AppResult<()> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() {
        return Err(AppError::InvalidInput(
            "Persona title cannot be empty.".to_string(),
        ));
    }
    if body.is_empty() {
        return Err(AppError::InvalidInput(
            "Persona prompt body cannot be empty.".to_string(),
        ));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::InvalidInput(format!(
            "Persona title must be at most {MAX_TITLE_CHARS} characters."
        )));
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(AppError::InvalidInput(format!(
            "Persona prompt body must be at most {MAX_BODY_CHARS} characters."
        )));
    }
    Ok(())
}

/// List built-in starters first, then user-managed personas.
pub async fn list_personas_impl(pool: &SqlitePool) -> AppResult<Vec<Persona>> {
    let rows = sqlx::query_as::<_, PersonaRow>(
        "SELECT id, title, body, also_apply_verbosity FROM chat_personas ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut result = builtin_personas();
    result.extend(rows.into_iter().map(row_to_persona));
    Ok(result)
}

#[tauri::command]
pub async fn list_personas(pool: State<'_, SqlitePool>) -> AppResult<Vec<Persona>> {
    list_personas_impl(pool.inner()).await
}

/// Create a user-managed persona.
pub async fn create_persona_impl(
    pool: &SqlitePool,
    title: String,
    body: String,
    also_apply_verbosity: bool,
) -> AppResult<Persona> {
    validate_fields(&title, &body)?;
    let title = title.trim().to_string();
    let body = body.trim().to_string();
    let flag: i64 = if also_apply_verbosity { 1 } else { 0 };

    let row = sqlx::query_as::<_, PersonaRow>(
        "INSERT INTO chat_personas (title, body, also_apply_verbosity) VALUES (?, ?, ?)
         RETURNING id, title, body, also_apply_verbosity",
    )
    .bind(&title)
    .bind(&body)
    .bind(flag)
    .fetch_one(pool)
    .await?;

    Ok(row_to_persona(row))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn create_persona(
    pool: State<'_, SqlitePool>,
    title: String,
    body: String,
    also_apply_verbosity: bool,
) -> AppResult<Persona> {
    create_persona_impl(pool.inner(), title, body, also_apply_verbosity).await
}

/// Update a user-managed persona. Built-ins cannot be edited.
pub async fn update_persona_impl(
    pool: &SqlitePool,
    id: String,
    title: String,
    body: String,
    also_apply_verbosity: bool,
) -> AppResult<Persona> {
    let Some(row_id) = parse_user_id(&id) else {
        return Err(AppError::InvalidInput(
            "Built-in personas cannot be edited. Duplicate one to customise it.".to_string(),
        ));
    };

    validate_fields(&title, &body)?;
    let title = title.trim().to_string();
    let body = body.trim().to_string();
    let flag: i64 = if also_apply_verbosity { 1 } else { 0 };

    let row = sqlx::query_as::<_, PersonaRow>(
        "UPDATE chat_personas SET title = ?, body = ?, also_apply_verbosity = ? WHERE id = ?
         RETURNING id, title, body, also_apply_verbosity",
    )
    .bind(&title)
    .bind(&body)
    .bind(flag)
    .bind(row_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Persona {id} not found")))?;

    Ok(row_to_persona(row))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_persona(
    pool: State<'_, SqlitePool>,
    id: String,
    title: String,
    body: String,
    also_apply_verbosity: bool,
) -> AppResult<Persona> {
    update_persona_impl(pool.inner(), id, title, body, also_apply_verbosity).await
}

/// Delete a user-managed persona. Clears active setting in the same transaction when it matched.
pub async fn delete_persona_impl(pool: &SqlitePool, id: String) -> AppResult<()> {
    let Some(row_id) = parse_user_id(&id) else {
        return Err(AppError::InvalidInput(
            "Built-in personas cannot be deleted.".to_string(),
        ));
    };

    let mut tx = pool.begin().await?;

    let deleted = sqlx::query("DELETE FROM chat_personas WHERE id = ?")
        .bind(row_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    if deleted == 0 {
        return Err(AppError::NotFound(format!("Persona {id} not found")));
    }

    let active: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = ?1 LIMIT 1",
    )
    .bind(ACTIVE_SETTING_KEY)
    .fetch_optional(&mut *tx)
    .await?;

    if active.as_deref() == Some(id.as_str()) {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(ACTIVE_SETTING_KEY)
        .bind("")
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_persona(pool: State<'_, SqlitePool>, id: String) -> AppResult<()> {
    delete_persona_impl(pool.inner(), id).await
}

/// Resolve the active persona from settings, or `None` if unset/unknown.
/// Clears a stale active id so the next read is clean.
pub async fn resolve_active_persona_impl(pool: &SqlitePool) -> AppResult<Option<Persona>> {
    let active = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = ?1 LIMIT 1",
    )
    .bind(ACTIVE_SETTING_KEY)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();

    let active = active.trim();
    if active.is_empty() {
        return Ok(None);
    }

    if let Some(p) = find_builtin(active) {
        return Ok(Some(p));
    }

    if let Some(row_id) = parse_user_id(active) {
        let row = sqlx::query_as::<_, PersonaRow>(
            "SELECT id, title, body, also_apply_verbosity FROM chat_personas WHERE id = ?",
        )
        .bind(row_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            return Ok(Some(row_to_persona(row)));
        }
    }

    // Stale / unknown id → treat as None and clear the setting.
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(ACTIVE_SETTING_KEY)
    .bind("")
    .execute(pool)
    .await?;

    Ok(None)
}

#[tauri::command]
pub async fn resolve_active_persona(
    pool: State<'_, SqlitePool>,
) -> AppResult<Option<Persona>> {
    resolve_active_persona_impl(pool.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_id_round_trip() {
        assert_eq!(parse_user_id("user:42"), Some(42));
        assert_eq!(parse_user_id("builtin:blunt-editor"), None);
        assert_eq!(parse_user_id("user:x"), None);
    }

    #[test]
    fn builtin_ids_are_stable() {
        let ids: Vec<_> = builtin_personas().into_iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![
                ID_BLUNT.to_string(),
                ID_SOCRATIC.to_string(),
                ID_CITE.to_string()
            ]
        );
    }

    #[test]
    fn validate_rejects_empty_and_oversize() {
        assert!(validate_fields("ok", "body").is_ok());
        assert!(validate_fields("  ", "body").is_err());
        assert!(validate_fields("t", "  ").is_err());
        let long_title: String = "a".repeat(MAX_TITLE_CHARS + 1);
        assert!(validate_fields(&long_title, "body").is_err());
        let long_body: String = "b".repeat(MAX_BODY_CHARS + 1);
        assert!(validate_fields("t", &long_body).is_err());
    }
}
