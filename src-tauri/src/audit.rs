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

//! Audit log helpers.
//!
//! `log_event` is the single write entry point.  It is intentionally
//! fire-and-forget: callers do `let _ = audit::log_event(...).await;` so a
//! logging failure never breaks the primary operation.
//!
//! The log is always local and never exported unless the user opts in.

use serde::Serialize;
use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single row from `audit_log`, returned to the frontend.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id:            i64,
    pub action:        String,
    pub resource_type: Option<String>,
    pub resource_id:   Option<i64>,
    pub resource_name: Option<String>,
    pub detail:        Option<String>,
    pub created_at:    i64,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Read a boolean setting from the `settings` table.
/// Returns `default` when the key is absent or the value cannot be parsed.
async fn read_bool_setting(pool: &SqlitePool, key: &str, default: bool) -> bool {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ? LIMIT 1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Append one entry to the audit log.
///
/// Respects the `audit_enabled` setting (default: true).  For `file_scan` and
/// `file_import` actions, also checks the `log_file_access` sub-toggle.
///
/// This function is intentionally infallible from the caller's perspective:
/// a failed write is silently swallowed so it can never break a note save,
/// search, or any other primary operation.
pub async fn log_event(
    pool: &SqlitePool,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<i64>,
    resource_name: Option<&str>,
    detail: Option<&str>,
) {
    if !read_bool_setting(pool, "audit_enabled", true).await {
        return;
    }

    // File-scanner actions are additionally gated by the file-access sub-toggle.
    if matches!(action, "file_scan" | "file_import") {
        if !read_bool_setting(pool, "log_file_access", true).await {
            return;
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let _ = sqlx::query(
        "INSERT INTO audit_log
             (action, resource_type, resource_id, resource_name, detail, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(resource_name)
    .bind(detail)
    .bind(now)
    .execute(pool)
    .await;
}

/// Truncate a string to at most `max_chars` Unicode scalar values.
/// Used to cap LLM query text stored in the detail field.
pub fn truncate(s: &str, max_chars: usize) -> &str {
    s.char_indices()
        .nth(max_chars)
        .map(|(i, _)| &s[..i])
        .unwrap_or(s)
}
