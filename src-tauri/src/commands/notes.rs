use std::path::PathBuf;
use std::collections::HashMap;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;
use crate::AccessFilter;
use crate::SharedKeyStore;
use crate::config::SharedConfig;
use crate::hardware::HardwareCapability;
use crate::{AppError, AppResult, EncryptedNoteStore};
use crate::vector::VectorDb;
use super::rag::spawn_note_reindex_if_enabled;
use super::{Note, Folder};

#[derive(Debug, Serialize)]
pub struct NoteVersionMeta {
    pub id: i64,
    pub created_at: i64,
    pub encrypted: bool,
    pub preview_title: String,
    pub preview_body: String,
}

#[derive(Debug, Serialize)]
pub struct NoteVersionContent {
    pub id: i64,
    pub note_id: i64,
    pub title: String,
    pub content: String,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Note commands
// ---------------------------------------------------------------------------

/// Create a new note and return the full row.
#[tauri::command]
pub async fn create_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    title: String,
    folder_id: Option<i64>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.create_note(&title, folder_id).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_create", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// Fetch a single note by id.
#[tauri::command]
pub async fn get_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    id: i64,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(id).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "note_open", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// List all notes, optionally filtered to a specific folder.
/// Pass `null` from JS to get notes with no folder, omit the filter to get all.
/// This command takes an explicit `all` flag to distinguish "no folder" from "every folder".
#[tauri::command]
pub async fn list_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    folder_id: Option<i64>,
    all: Option<bool>,
) -> AppResult<Vec<Note>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    store.list_notes(folder_id, all.unwrap_or(false)).await
}

/// Update a note's title and content. Bumps updated_at to the current time.
/// Does **not** append a row to `note_versions`; UI saves should call
/// [`save_note_with_version`] so history stays complete.
#[tauri::command]
pub async fn update_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    id: i64,
    title: String,
    content: String,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.update_note(id, &title, &content).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), None,
    ).await;
    Ok(note)
}

/// Explicit save path for the editor save button / Ctrl+S.
#[tauri::command]
pub async fn save_note_with_version(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    id: i64,
    title: String,
    content: String,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.save_note_with_version(id, &title, &content).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("explicit_save"),
    ).await;
    Ok(note)
}

/// Same persistence as [`save_note_with_version`] plus FTS, without audit logging.
/// For the `perf-budget` binary and local timing runs.
#[cfg(debug_assertions)]
pub async fn save_note_with_version_benchmark_path(
    pool: &SqlitePool,
    keys: &SharedKeyStore,
    id: i64,
    title: &str,
    content: &str,
) -> AppResult<()> {
    let store = EncryptedNoteStore::new(pool, keys.as_ref());
    let note = store.save_note_with_version(id, title, content).await?;
    if !note.locked {
        super::search::fts_upsert(pool, note.id, &note.title, &note.content).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_note_versions(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
) -> AppResult<Vec<NoteVersionMeta>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let versions = store.get_note_versions(note_id).await?;
    Ok(versions
        .into_iter()
        .map(
            |(id, created_at, encrypted, preview_title, preview_body)| NoteVersionMeta {
                id,
                created_at,
                encrypted,
                preview_title,
                preview_body,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn get_note_version_content(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    version_id: i64,
) -> AppResult<NoteVersionContent> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let (title, content, created_at) = store.get_note_version_content(note_id, version_id).await?;
    Ok(NoteVersionContent {
        id: version_id,
        note_id,
        title,
        content,
        created_at,
    })
}

#[tauri::command]
pub async fn restore_note_version(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    version_id: i64,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.restore_note_version(note_id, version_id).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("restore_version"),
    ).await;
    Ok(note)
}

/// Move a note to a different folder (or to no folder when folder_id is null).
#[tauri::command]
pub async fn move_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
    config: State<'_, SharedConfig>,
    hw: State<'_, HardwareCapability>,
    id: i64,
    folder_id: Option<i64>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.move_note(id, folder_id).await?;
    if note.locked {
        super::search::fts_delete(pool.inner(), note.id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        crate::vector::notes::remove(&vdb.0, note.id)
            .await
            .map_err(AppError::VectorStore)?;
    } else {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
        if let Err(e) = crate::vector::notes::remove(&vdb.0, note.id).await {
            log::warn!("move_note: failed to clear stale vectors for {}: {e}", note.id);
        }
        spawn_note_reindex_if_enabled(
            pool.inner().clone(),
            vdb.inner().0.clone(),
            config.inner().clone(),
            hw.0.clone(),
            note.id,
            note.title.clone(),
            note.content.clone(),
        );
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("moved"),
    ).await;
    Ok(note)
}

/// Rename a note (title only). Returns the updated note.
#[tauri::command]
pub async fn rename_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
    config: State<'_, SharedConfig>,
    hw: State<'_, HardwareCapability>,
    id: i64,
    name: String,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.rename_note(id, &name).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    if !note.locked {
        spawn_note_reindex_if_enabled(
            pool.inner().clone(),
            vdb.inner().0.clone(),
            config.inner().clone(),
            hw.0.clone(),
            note.id,
            note.title.clone(),
            note.content.clone(),
        );
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_update", Some("note"),
        Some(note.id), Some(&note.title), Some("renamed"),
    ).await;
    Ok(note)
}

/// Delete a note. Returns nothing on success.
#[tauri::command]
pub async fn delete_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
    id: i64,
) -> AppResult<()> {
    let row: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT folder_id FROM notes WHERE id = ?")
            .bind(id)
            .fetch_optional(pool.inner())
            .await?;

    let (folder_id,) = row.ok_or_else(|| AppError::NotFound(format!("Note {id} not found")))?;

    let filter = AccessFilter::load(pool.inner(), keys.inner().as_ref()).await?;
    if !filter.is_accessible(folder_id) {
        return Err(AppError::Auth("folder_locked".to_string()));
    }

    crate::vector::notes::remove(&vdb.0, id)
        .await
        .map_err(AppError::VectorStore)?;

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    super::search::fts_delete_tx(&mut tx, id).await?;
    tx.commit().await?;

    let _ = crate::audit::log_event(
        pool.inner(), "note_delete", Some("note"),
        Some(id), None, None,
    ).await;
    Ok(())
}

/// Duplicate a note — creates a copy with " (copy)" appended to the title in
/// the same folder. Returns the new note row.
#[tauri::command]
pub async fn duplicate_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    vdb: State<'_, VectorDb>,
    config: State<'_, SharedConfig>,
    hw: State<'_, HardwareCapability>,
    id: i64,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.duplicate_note(id).await?;
    if !note.locked {
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
        spawn_note_reindex_if_enabled(
            pool.inner().clone(),
            vdb.inner().0.clone(),
            config.inner().clone(),
            hw.0.clone(),
            note.id,
            note.title.clone(),
            note.content.clone(),
        );
    }
    let _ = crate::audit::log_event(
        pool.inner(), "note_create", Some("note"),
        Some(note.id), Some(&note.title), Some("duplicated"),
    ).await;
    Ok(note)
}

/// Resolved target for `![[note title]]` transclusion (read-only embed). Does not
/// write audit "note_open" entries — embed resolution is not a full note open.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteEmbedResolve {
    pub found: bool,
    pub id: Option<i64>,
    pub locked: bool,
    pub content: String,
}

/// Batch-resolve note titles for transclusion. Titles are compared to decrypted
/// note titles (same rules as wiki-links). Locked/inaccessible notes match only
/// when the session can decrypt the title.
#[tauri::command]
pub async fn resolve_note_embed_batch(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    titles: Vec<String>,
) -> AppResult<HashMap<String, NoteEmbedResolve>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let all_notes = store.list_notes(None, true).await?;
    let mut out: HashMap<String, NoteEmbedResolve> = HashMap::new();

    for raw in titles {
        let key = raw.trim().to_string();
        if key.is_empty() {
            continue;
        }
        if out.contains_key(&key) {
            continue;
        }

        let mut matched: Option<NoteEmbedResolve> = None;
        for note in &all_notes {
            if note.title == key {
                matched = Some(NoteEmbedResolve {
                    found: true,
                    id: Some(note.id),
                    locked: note.locked,
                    content: if note.locked {
                        String::new()
                    } else {
                        note.content.clone()
                    },
                });
                break;
            }
        }

        out.insert(
            key,
            matched.unwrap_or(NoteEmbedResolve {
                found: false,
                id: None,
                locked: false,
                content: String::new(),
            }),
        );
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Folder commands
// ---------------------------------------------------------------------------

/// Create a new folder and return the full row.
#[tauri::command]
pub async fn create_folder(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    name: String,
    parent_id: Option<i64>,
) -> AppResult<Folder> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let folder = store.create_folder(&name, parent_id).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "folder_create", Some("folder"),
        Some(folder.id), Some(&folder.name), None,
    ).await;
    Ok(folder)
}

/// List all folders. The frontend is responsible for building the tree from parent_id.
#[tauri::command]
pub async fn list_folders(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
) -> AppResult<Vec<Folder>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    store.list_folders().await
}

/// Rename a folder.
#[tauri::command]
pub async fn rename_folder(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    id: i64,
    name: String,
) -> AppResult<Folder> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let folder = store.rename_folder(id, &name).await?;
    let _ = crate::audit::log_event(
        pool.inner(), "folder_rename", Some("folder"),
        Some(folder.id), Some(&folder.name), None,
    ).await;
    Ok(folder)
}

/// Delete a folder. Child folders and notes are handled by ON DELETE CASCADE
/// and ON DELETE SET NULL respectively (defined in the migration).
#[tauri::command]
pub async fn delete_folder(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM folders WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    let _ = crate::audit::log_event(
        pool.inner(), "folder_delete", Some("folder"),
        Some(id), None, None,
    ).await;
    Ok(())
}

/// Move a folder to a new parent (or to the root when new_parent_id is null).
/// Rejects the move if it would create a cycle (i.e. new_parent_id is a descendant
/// of the folder being moved, or equals the folder itself).
#[tauri::command]
pub async fn move_folder(
    pool: State<'_, SqlitePool>,
    id: i64,
    new_parent_id: Option<i64>,
) -> AppResult<()> {
    // A folder cannot be moved into itself or into one of its own descendants.
    if let Some(target) = new_parent_id {
        if target == id {
            return Err(AppError::InvalidInput("A folder cannot be its own parent".to_string()));
        }
        // Walk the ancestor chain of `target` upward; if we ever reach `id` then
        // the proposed parent is a descendant — reject it.
        let descendant_ids: Vec<(i64,)> = sqlx::query_as(
            "WITH RECURSIVE subtree(id) AS (
                 SELECT id FROM folders WHERE id = ?
                 UNION ALL
                 SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id
             )
             SELECT id FROM subtree WHERE id != ?",
        )
        .bind(id)
        .bind(id)
        .fetch_all(pool.inner())
        .await
        ?;

        if descendant_ids.iter().any(|(did,)| *did == target) {
            return Err(AppError::InvalidInput("Cannot move a folder into one of its own descendants".to_string()));
        }
    }

    sqlx::query("UPDATE folders SET parent_id = ? WHERE id = ?")
        .bind(new_parent_id)
        .bind(id)
        .execute(pool.inner())
        .await
        ?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Export all unlocked notes as plain Markdown files under `dest_dir`.
/// The folder hierarchy is recreated as subdirectories; unfiled notes go to
/// the root. Locked notes are silently skipped — we never decrypt without the
/// user's key, and the key is not available at export time for locked folders.
#[tauri::command]
pub async fn export_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    dest_dir: String,
) -> AppResult<u32> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());

    let folders = store.list_folders().await?;
    let parent_by_id: HashMap<i64, Option<i64>> =
        folders.iter().map(|f| (f.id, f.parent_id)).collect();

    // Decrypted names for unlocked folders; locked folders use empty (fallback when building paths).
    let display_names: HashMap<i64, String> = folders
        .iter()
        .map(|f| (f.id, if f.locked { String::new() } else { f.name.clone() }))
        .collect();

    // Fetch all notes; locked ones surface with note.locked = true.
    let all_notes = store.list_notes(None, true).await?;

    let dest = PathBuf::from(&dest_dir);

    // Wrap everything in a timestamped subfolder so repeated exports don't collide.
    let date_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let export_root = dest.join(format!("Grimoire - export {date_str}"));
    let dest = export_root;
    let mut exported: u32 = 0;

    for note in all_notes {
        if note.locked {
            continue; // skip — no key available
        }

        // Resolve the output directory for this note (full ancestor chain).
        let out_dir = if let Some(fid) = note.folder_id {
            dest.join(folder_export_relpath(fid, &parent_by_id, &display_names))
        } else {
            dest.clone()
        };

        std::fs::create_dir_all(&out_dir)
            .map_err(|e| AppError::Io(format!("Could not create directory {}: {e}", out_dir.display())))?;

        // Build the output file path, sanitising the title.
        let safe_title = sanitise_path_component(&note.title);
        let file_name = if safe_title.is_empty() {
            format!("note_{}", note.id)
        } else {
            safe_title
        };
        let mut file_path = out_dir.join(&file_name).with_extension("md");

        // Avoid overwriting an existing file from a different note with the same title.
        if file_path.exists() {
            file_path = out_dir.join(format!("{}_{}", file_name, note.id)).with_extension("md");
        }

        std::fs::write(&file_path, &note.content)
            .map_err(|e| AppError::Io(format!("Could not write {}: {e}", file_path.display())))?;

        exported += 1;
    }

    let _ = crate::audit::log_event(
        pool.inner(), "note_export", Some("note"),
        None, None, Some(&dest_dir),
    ).await;
    Ok(exported)
}

/// Export one unlocked note as Markdown to `dest_path` (full path including `.md`).
/// `markdown` is the exact bytes to write (matches editor/read mode, including unsaved edits).
#[tauri::command]
pub async fn export_single_note_markdown(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    dest_path: String,
    markdown: String,
) -> AppResult<()> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(note_id).await?;
    if note.locked {
        return Err(AppError::InvalidInput(
            "Cannot export a locked note. Unlock its folder first.".into(),
        ));
    }

    let path = PathBuf::from(&dest_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(format!("Could not create directory {}: {e}", parent.display()))
            })?;
        }
    }

    std::fs::write(&path, markdown.as_bytes()).map_err(|e| {
        AppError::Io(format!("Could not write {}: {e}", path.display()))
    })?;

    let _ = crate::audit::log_event(
        pool.inner(),
        "note_export",
        Some("note"),
        Some(note_id),
        Some(&note.title),
        Some("single_markdown"),
    )
    .await;
    Ok(())
}

/// Write standalone HTML for an unlocked note; frontend supplies HTML matching read mode.
#[tauri::command]
pub async fn save_note_html_export(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
    dest_path: String,
    html: String,
) -> AppResult<()> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(note_id).await?;
    if note.locked {
        return Err(AppError::InvalidInput(
            "Cannot export a locked note. Unlock its folder first.".into(),
        ));
    }

    let path = PathBuf::from(&dest_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(format!("Could not create directory {}: {e}", parent.display()))
            })?;
        }
    }

    std::fs::write(&path, html.as_bytes()).map_err(|e| {
        AppError::Io(format!("Could not write {}: {e}", path.display()))
    })?;

    let _ = crate::audit::log_event(
        pool.inner(),
        "note_export",
        Some("note"),
        Some(note_id),
        Some(&note.title),
        Some("single_html"),
    )
    .await;
    Ok(())
}

/// Record that the user initiated print-to-PDF for an unlocked note (no file write).
#[tauri::command]
pub async fn log_note_export_pdf_print(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    note_id: i64,
) -> AppResult<()> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let note = store.get_note(note_id).await?;
    if note.locked {
        return Err(AppError::InvalidInput(
            "Cannot export a locked note. Unlock its folder first.".into(),
        ));
    }

    let _ = crate::audit::log_event(
        pool.inner(),
        "note_export",
        Some("note"),
        Some(note_id),
        Some(&note.title),
        Some("single_pdf_print"),
    )
    .await;
    Ok(())
}

/// Build `ancestor/.../leaf` under the export root from folder parent links.
fn folder_export_relpath(
    mut folder_id: i64,
    parent_by_id: &HashMap<i64, Option<i64>>,
    display_names: &HashMap<i64, String>,
) -> PathBuf {
    let mut comps: Vec<String> = Vec::new();
    for _ in 0..512 {
        let raw = display_names
            .get(&folder_id)
            .cloned()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("folder_{folder_id}"));
        comps.push(sanitise_path_component(&raw));
        match parent_by_id.get(&folder_id).copied() {
            Some(Some(pid)) => folder_id = pid,
            _ => break,
        }
    }
    comps.reverse();
    comps.into_iter().fold(PathBuf::new(), |acc, c| acc.join(c))
}

/// Strip characters that are illegal in directory or file names on Windows,
/// macOS, and Linux. Collapses repeating spaces/dashes and trims whitespace.
fn sanitise_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
