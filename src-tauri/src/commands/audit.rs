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

//! Tauri commands for reading and clearing the audit log.

use sqlx::SqlitePool;
use tauri::State;
use crate::audit::AuditEntry;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a frontend filter group name to a SQL literal list for an IN clause.
///
/// The returned string is safe to embed directly in SQL because it is built
/// from a fixed set of known-good string literals — it contains no user input.
fn action_group_to_in(filter: Option<&str>) -> &'static str {
    match filter.unwrap_or("all") {
        "notes"        => "'note_open','note_create','note_update','note_delete','note_export'",
        "folders"      => "'folder_create','folder_rename','folder_delete'",
        "search"       => "'search_fts','search_semantic','search_combined'",
        "llm"          => "'llm_chat','llm_improve'",
        "file_scanner" => "'file_scan','file_import'",
        "wikipedia"    => "'wikipedia_read'",
        // "all" and any unknown value → every action type
        _              => concat!(
            "'note_open','note_create','note_update','note_delete','note_export',",
            "'folder_create','folder_rename','folder_delete',",
            "'search_fts','search_semantic','search_combined',",
            "'llm_chat','llm_improve',",
            "'file_scan','file_import',",
            "'wikipedia_read'"
        ),
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Fetch a page of audit log entries, newest first.
///
/// Parameters:
/// - `page`          — 1-based page number (default: 1)
/// - `page_size`     — rows per page, clamped 1–100 (default: 25)
/// - `action_filter` — one of "all" | "notes" | "folders" | "search" |
///                     "llm" | "file_scanner" | "wikipedia" (default: "all")
/// - `search`        — substring matched against `resource_name` and `detail`
///                     (case-insensitive, empty/null means no filter)
#[tauri::command]
pub async fn get_audit_log(
    pool: State<'_, SqlitePool>,
    page: Option<i64>,
    page_size: Option<i64>,
    action_filter: Option<String>,
    search: Option<String>,
) -> Result<Vec<AuditEntry>, String> {
    let page      = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(25).clamp(1, 100);
    let offset    = (page - 1) * page_size;

    let action_in = action_group_to_in(action_filter.as_deref());

    // Build the LIKE pattern once; None means "no text filter".
    let like_pat: Option<String> = search
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    // `? IS NULL` short-circuits when like_pat is None, skipping the LIKE checks.
    let sql = format!(
        "SELECT id, action, resource_type, resource_id, resource_name, detail, created_at
         FROM audit_log
         WHERE action IN ({action_in})
           AND (? IS NULL OR resource_name LIKE ? OR detail LIKE ?)
         ORDER BY created_at DESC
         LIMIT ? OFFSET ?"
    );

    sqlx::query_as::<_, AuditEntry>(&sql)
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

/// Count the total number of entries matching the same filters as `get_audit_log`.
/// Used by the frontend to compute the total page count.
#[tauri::command]
pub async fn get_audit_log_count(
    pool: State<'_, SqlitePool>,
    action_filter: Option<String>,
    search: Option<String>,
) -> Result<i64, String> {
    let action_in = action_group_to_in(action_filter.as_deref());

    let like_pat: Option<String> = search
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let sql = format!(
        "SELECT COUNT(*) FROM audit_log
         WHERE action IN ({action_in})
           AND (? IS NULL OR resource_name LIKE ? OR detail LIKE ?)"
    );

    sqlx::query_scalar::<_, i64>(&sql)
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

/// Permanently delete all audit log entries.
/// This action is irreversible — the frontend must ask for confirmation first.
#[tauri::command]
pub async fn clear_audit_log(pool: State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM audit_log")
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
