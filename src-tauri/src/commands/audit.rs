//! Tauri commands for reading and clearing the audit log.

use std::collections::{HashMap, HashSet};
use std::fs::File;

use serde::Serialize;
use sqlx::{QueryBuilder, SqlitePool};
use tauri::State;

use crate::audit::AuditEntry;
use crate::{AppError, AppResult, SharedKeyStore};

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

fn audit_like_pattern(search: &Option<String>) -> Option<String> {
    search
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"))
}

/// Folder IDs that are password-locked and have no session key this request.
/// Audit rows referencing notes in these folders are withheld (same rule as export).
async fn audit_locked_folder_blocklist(
    pool: &SqlitePool,
    keys: &SharedKeyStore,
) -> AppResult<Vec<i64>> {
    let locked: Vec<i64> = sqlx::query_scalar("SELECT id FROM folders WHERE locked = 1")
        .fetch_all(pool)
        .await?;

    let unlocked = keys
        .folder_keys
        .lock()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    Ok(locked
        .into_iter()
        .filter(|id| !unlocked.contains_key(&id))
        .collect())
}

/// Map note_id → (folder_id, folder_has_password) for export batching.
async fn note_folder_locked_map(
    pool: &SqlitePool,
    note_ids: &HashSet<i64>,
) -> AppResult<HashMap<i64, (Option<i64>, bool)>> {
    const CHUNK: usize = 400;
    let mut note_folder_locked: HashMap<i64, (Option<i64>, bool)> = HashMap::new();
    if note_ids.is_empty() {
        return Ok(note_folder_locked);
    }
    let ids: Vec<i64> = note_ids.iter().copied().collect();
    for chunk in ids.chunks(CHUNK) {
        let mut qb = QueryBuilder::new(
            "SELECT n.id, n.folder_id, COALESCE(f.locked, 0) AS folder_locked \
             FROM notes n LEFT JOIN folders f ON n.folder_id = f.id WHERE n.id IN (",
        );
        {
            let mut sep = qb.separated(", ");
            for id in chunk {
                sep.push_bind(id);
            }
        }
        qb.push(")");
        let pairs: Vec<(i64, Option<i64>, i64)> =
            qb.build_query_as().fetch_all(pool).await?;
        for (nid, folder_id, locked_flag) in pairs {
            note_folder_locked.insert(nid, (folder_id, locked_flag != 0));
        }
    }
    Ok(note_folder_locked)
}

fn created_at_iso_utc(secs: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| secs.to_string())
}

/// Rows written to an export file and how many were withheld (locked-folder notes).
#[derive(Debug, Serialize)]
pub struct AuditExportResult {
    pub exported: i64,
    pub skipped_locked: i64,
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
    keys: State<'_, SharedKeyStore>,
    page: Option<i64>,
    page_size: Option<i64>,
    action_filter: Option<String>,
    search: Option<String>,
) -> AppResult<Vec<AuditEntry>> {
    let page      = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(25).clamp(1, 100);
    let offset    = (page - 1) * page_size;

    let blocklist = audit_locked_folder_blocklist(pool.inner(), keys.inner()).await?;

    let action_in = action_group_to_in(action_filter.as_deref());
    let like_pat = audit_like_pattern(&search);

    let mut sql = format!(
        "SELECT al.id, al.action, al.resource_type, al.resource_id, al.resource_name, al.detail, al.created_at
         FROM audit_log al
         WHERE al.action IN ({action_in})
           AND (? IS NULL OR al.resource_name LIKE ? OR al.detail LIKE ?)",
    );
    if !blocklist.is_empty() {
        sql.push_str(
            " AND NOT (LOWER(COALESCE(al.resource_type, '')) = 'note' \
               AND al.resource_id IS NOT NULL \
               AND EXISTS (SELECT 1 FROM notes n WHERE n.id = al.resource_id AND n.folder_id IN (",
        );
        sql.push_str(&vec!["?"; blocklist.len()].join(", "));
        sql.push_str(")))");
    }
    sql.push_str(" ORDER BY al.created_at DESC, al.id DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, AuditEntry>(&sql);
    q = q
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref());
    for id in &blocklist {
        q = q.bind(id);
    }
    q = q.bind(page_size).bind(offset);

    q.fetch_all(pool.inner()).await.map_err(Into::into)
}

/// Count the total number of entries matching the same filters as `get_audit_log`.
/// Used by the frontend to compute the total page count.
#[tauri::command]
pub async fn get_audit_log_count(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    action_filter: Option<String>,
    search: Option<String>,
) -> AppResult<i64> {
    let blocklist = audit_locked_folder_blocklist(pool.inner(), keys.inner()).await?;

    let action_in = action_group_to_in(action_filter.as_deref());

    let like_pat = audit_like_pattern(&search);

    let mut sql = format!(
        "SELECT COUNT(*) FROM audit_log al
         WHERE al.action IN ({action_in})
           AND (? IS NULL OR al.resource_name LIKE ? OR al.detail LIKE ?)",
    );
    if !blocklist.is_empty() {
        sql.push_str(
            " AND NOT (LOWER(COALESCE(al.resource_type, '')) = 'note' \
               AND al.resource_id IS NOT NULL \
               AND EXISTS (SELECT 1 FROM notes n WHERE n.id = al.resource_id AND n.folder_id IN (",
        );
        sql.push_str(&vec!["?"; blocklist.len()].join(", "));
        sql.push_str(")))");
    }

    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    q = q
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref());
    for id in &blocklist {
        q = q.bind(id);
    }

    q.fetch_one(pool.inner()).await.map_err(Into::into)
}

/// Permanently delete all audit log entries.
/// This action is irreversible — the frontend must ask for confirmation first.
#[tauri::command]
pub async fn clear_audit_log(pool: State<'_, SqlitePool>) -> AppResult<()> {
    sqlx::query("DELETE FROM audit_log")
        .execute(pool.inner())
        .await
        ?;
    Ok(())
}

/// Export all audit rows matching the same filters as the list view (no pagination).
/// Skips rows that reference a note in a password-protected folder that is not unlocked this session.
#[tauri::command]
pub async fn export_audit_log(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    format: String,
    action_filter: Option<String>,
    search: Option<String>,
    dest_path: String,
) -> AppResult<AuditExportResult> {
    let fmt = format.trim().to_ascii_lowercase();
    if fmt != "csv" && fmt != "json" {
        return Err(AppError::InvalidInput(
            "format must be 'csv' or 'json'".into(),
        ));
    }

    let action_in = action_group_to_in(action_filter.as_deref());
    let like_pat = audit_like_pattern(&search);

    let sql = format!(
        "SELECT id, action, resource_type, resource_id, resource_name, detail, created_at
         FROM audit_log
         WHERE action IN ({action_in})
           AND (? IS NULL OR resource_name LIKE ? OR detail LIKE ?)
         ORDER BY created_at DESC, id DESC"
    );

    let rows: Vec<AuditEntry> = sqlx::query_as::<_, AuditEntry>(&sql)
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .bind(like_pat.as_deref())
        .fetch_all(pool.inner())
        .await?;

    let note_ids: HashSet<i64> = rows
        .iter()
        .filter(|r| {
            r.resource_type
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("note"))
                && r.resource_id.is_some()
        })
        .filter_map(|r| r.resource_id)
        .collect();

    let note_folder_locked: HashMap<i64, (Option<i64>, bool)> =
        note_folder_locked_map(pool.inner(), &note_ids).await?;

    let folder_keys = keys.folder_keys.lock().map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let mut skipped_locked = 0i64;
    let mut filtered: Vec<AuditEntry> = Vec::with_capacity(rows.len());
    for r in rows {
        let skip = r
            .resource_type
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case("note"))
            && r.resource_id.is_some_and(|_nid| {
                if let Some((folder_id, has_password)) = r
                    .resource_id
                    .and_then(|note_id| note_folder_locked.get(&note_id).copied())
                {
                    if let Some(fid) = folder_id {
                        has_password && !folder_keys.contains_key(&fid)
                    } else {
                        false
                    }
                } else {
                    false
                }
            });
        if skip {
            skipped_locked += 1;
        } else {
            filtered.push(r);
        }
    }
    drop(folder_keys);

    let exported_len = filtered.len() as i64;

    match fmt.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_path(&dest_path)
                .map_err(|e| AppError::Io(e.to_string()))?;
            wtr.write_record([
                "id",
                "created_at_unix",
                "created_at_iso",
                "action",
                "resource_type",
                "resource_id",
                "resource_name",
                "detail",
            ])
            .map_err(|e| AppError::Io(e.to_string()))?;
            for e in &filtered {
                wtr.write_record([
                    e.id.to_string(),
                    e.created_at.to_string(),
                    created_at_iso_utc(e.created_at),
                    e.action.clone(),
                    e.resource_type.clone().unwrap_or_default(),
                    e.resource_id.map(|x| x.to_string()).unwrap_or_default(),
                    e.resource_name.clone().unwrap_or_default(),
                    e.detail.clone().unwrap_or_default(),
                ])
                .map_err(|e| AppError::Io(e.to_string()))?;
            }
            wtr.flush().map_err(|e| AppError::Io(e.to_string()))?;
        }
        "json" => {
            let f = File::create(&dest_path).map_err(|e| AppError::Io(e.to_string()))?;
            serde_json::to_writer_pretty(f, &filtered).map_err(|e| AppError::Io(e.to_string()))?;
        }
        _ => unreachable!(),
    }

    Ok(AuditExportResult {
        exported: exported_len,
        skipped_locked,
    })
}

/// Count audit rows older than `days` (based on `created_at`). Returns 0 when `days <= 0`.
#[tauri::command]
pub async fn preview_audit_retention_prune(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> AppResult<i64> {
    if days <= 0 {
        return Ok(0);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = now.saturating_sub(days.saturating_mul(86400));
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE created_at < ?")
        .bind(cutoff)
        .fetch_one(pool.inner())
        .await?;
    Ok(n)
}

/// Delete audit rows with `created_at` older than `now - days * 86400`. Returns rows removed.
#[tauri::command]
pub async fn prune_audit_log(pool: State<'_, SqlitePool>, days: i64) -> AppResult<i64> {
    if days < 0 {
        return Err(AppError::InvalidInput(
            "Retention days cannot be negative".into(),
        ));
    }
    if days == 0 {
        return Ok(0);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = now.saturating_sub(days.saturating_mul(86400));
    let r = sqlx::query("DELETE FROM audit_log WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool.inner())
        .await?;
    Ok(r.rows_affected() as i64)
}
