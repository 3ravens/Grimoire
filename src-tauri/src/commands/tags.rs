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
use crate::{AppError, AppResult, EncryptedNoteStore};
use super::{Note, LinkedNote, GraphNode, GraphEdge};

// ---------------------------------------------------------------------------
// Tags and wiki-links
// ---------------------------------------------------------------------------

/// Extract `#tag` mentions from note content.
/// A tag is `#` immediately followed by one or more word characters (letters,
/// digits, `-`, `_`), and must be preceded by whitespace or start-of-text so
/// that URLs like `https://example.com/#section` are not treated as tags.
fn parse_tags(content: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' {
            let preceded_ok = i == 0 || chars[i - 1].is_whitespace();
            let followed_ok = chars
                .get(i + 1)
                .map(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .unwrap_or(false);
            if preceded_ok && followed_ok {
                let start = i + 1;
                let mut end = start;
                while end < chars.len()
                    && (chars[end].is_alphanumeric() || chars[end] == '_' || chars[end] == '-')
                {
                    end += 1;
                }
                let tag: String = chars[start..end].iter().collect::<String>().to_lowercase();
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    tags
}

/// Extract `[[note title]]` wiki-link targets from note content.
fn parse_wiki_links(content: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    let mut rest = content;
    while let Some(open) = rest.find("[[") {
        rest = &rest[open + 2..];
        if let Some(close) = rest.find("]]") {
            let title = rest[..close].trim().to_string();
            if !title.is_empty() && !links.contains(&title) {
                links.push(title);
            }
            rest = &rest[close + 2..];
        } else {
            break;
        }
    }
    links
}

/// Persist the parsed tags for a note. Replaces all existing note→tag rows,
/// but leaves the `tags` table rows in place (tags are shared across notes).
async fn sync_tags(pool: &SqlitePool, note_id: i64, tags: &[String]) -> AppResult<()> {
    sqlx::query("DELETE FROM note_tags WHERE note_id = ?")
        .bind(note_id)
        .execute(pool)
        .await
        ?;

    for tag in tags {
        // Ensure the tag name exists in the tags table.
        sqlx::query("INSERT OR IGNORE INTO tags (name) VALUES (?)")
            .bind(tag)
            .execute(pool)
            .await
            ?;

        let tag_id: i64 = sqlx::query_scalar("SELECT id FROM tags WHERE name = ?")
            .bind(tag)
            .fetch_one(pool)
            .await
            ?;

        sqlx::query("INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?, ?)")
            .bind(note_id)
            .bind(tag_id)
            .execute(pool)
            .await
            ?;
    }
    Ok(())
}

/// Persist the parsed wiki-links for a note. Replaces all existing outgoing
/// links. Link targets that don't match an existing note title are silently
/// skipped — they'll be picked up on the next save if the target is created.
async fn sync_links(
    pool: &SqlitePool,
    note_id: i64,
    link_titles: &[String],
) -> AppResult<()> {
    sqlx::query("DELETE FROM note_links WHERE source_id = ?")
        .bind(note_id)
        .execute(pool)
        .await
        ?;

    for title in link_titles {
        let target: Option<i64> =
            sqlx::query_scalar("SELECT id FROM notes WHERE title = ? LIMIT 1")
                .bind(title)
                .fetch_optional(pool)
                .await
                ?;

        if let Some(target_id) = target {
            if target_id != note_id {
                sqlx::query(
                    "INSERT OR IGNORE INTO note_links (source_id, target_id) VALUES (?, ?)",
                )
                .bind(note_id)
                .bind(target_id)
                .execute(pool)
                .await
                ?;
            }
        }
    }
    Ok(())
}

/// Return all tags with a count of how many notes use each one, sorted by
/// name. Used by the sidebar tag browser.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

/// Parse and persist all `#tags` and `[[wiki-links]]` found in `content` for `note_id`.
/// Used by the Tauri command and by internal bulk fixtures (e.g. test data generator).
pub(crate) async fn sync_note_relations_pool(
    pool: &SqlitePool,
    note_id: i64,
    content: &str,
) -> AppResult<()> {
    let tags = parse_tags(content);
    let links = parse_wiki_links(content);
    sync_tags(pool, note_id, &tags).await?;
    sync_links(pool, note_id, &links).await?;
    Ok(())
}

#[tauri::command]
pub async fn sync_note_relations(
    pool: State<'_, SqlitePool>,
    note_id: i64,
    content: String,
) -> AppResult<()> {
    sync_note_relations_pool(pool.inner(), note_id, &content).await
}

/// Return the tag names attached to a note, alphabetically sorted.
#[tauri::command]
pub async fn get_note_tags(
    pool: State<'_, SqlitePool>,
    note_id: i64,
) -> AppResult<Vec<String>> {
    let tags: Vec<String> = sqlx::query_scalar(
        "SELECT t.name FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         WHERE nt.note_id = ?
         ORDER BY t.name ASC",
    )
    .bind(note_id)
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(tags)
}

/// Return notes that this note links to via `[[title]]`, alphabetically sorted.
#[tauri::command]
pub async fn get_note_links(
    pool: State<'_, SqlitePool>,
    note_id: i64,
) -> AppResult<Vec<LinkedNote>> {
    let links = sqlx::query_as::<_, LinkedNote>(
        "SELECT n.id, n.title FROM notes n
         JOIN note_links nl ON nl.target_id = n.id
         WHERE nl.source_id = ?
         ORDER BY n.title ASC",
    )
    .bind(note_id)
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(links)
}

/// Return notes that link to this note via `[[title]]` (backlinks), alphabetically sorted.
#[tauri::command]
pub async fn get_backlinks(
    pool: State<'_, SqlitePool>,
    note_id: i64,
) -> AppResult<Vec<LinkedNote>> {
    let links = sqlx::query_as::<_, LinkedNote>(
        "SELECT n.id, n.title FROM notes n
         JOIN note_links nl ON nl.source_id = n.id
         WHERE nl.target_id = ?
         ORDER BY n.title ASC",
    )
    .bind(note_id)
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(links)
}

/// List all notes that carry a given tag, sorted by most recently updated.
#[tauri::command]
pub async fn list_notes_by_tag(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    tag: String,
) -> AppResult<Vec<Note>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    store.notes_for_tag(&tag.to_lowercase()).await
}

/// Return all tags with a count of how many notes use each one, sorted by
/// name. Used by the sidebar tag browser.
#[tauri::command]
pub async fn list_all_tags(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<TagCount>> {
    let tags = sqlx::query_as::<_, TagCount>(
        "SELECT t.name, COUNT(nt.note_id) AS count
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         GROUP BY t.id
         ORDER BY t.name ASC",
    )
    .fetch_all(pool.inner())
    .await
    ?;
    Ok(tags)
}

/// Return all notes and all wiki-links as a graph dataset.
/// The frontend uses this to build a force-directed graph.
#[tauri::command]
pub async fn get_graph_data(
    pool: State<'_, SqlitePool>,
) -> AppResult<(Vec<GraphNode>, Vec<GraphEdge>)> {
    let nodes = sqlx::query_as::<_, GraphNode>(
        "SELECT id, title, folder_id FROM notes ORDER BY id ASC",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let edges = sqlx::query_as::<_, GraphEdge>(
        "SELECT source_id AS source, target_id AS target FROM note_links",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    Ok((nodes, edges))
}

/// Return notes that mention the given title as plain text but do not already
/// link to this note via [[wiki-link]]. These are "unlinked mentions" — the
/// user can convert them to proper links from the note footer.
///
/// Notes in locked folders are excluded (their content is ciphertext and would
/// produce false positives or negatives).
#[tauri::command]
pub async fn get_unlinked_mentions(
    pool: State<'_, SqlitePool>,
    note_id: i64,
    title: String,
) -> AppResult<Vec<LinkedNote>> {
    // Plain-text pattern: title appears in content but NOT as [[title]].
    // We do a LIKE pre-filter in SQL (cheap) and post-filter in Rust for the
    // wiki-link exclusion — this avoids complex SQL escaping of bracket chars.
    let plain_pattern = format!("%{}%", title);

    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        content: String,
        folder_locked: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT n.id, n.title, n.content, COALESCE(f.locked, 0) AS folder_locked
         FROM notes n
         LEFT JOIN folders f ON f.id = n.folder_id
         WHERE n.id != ?
           AND n.content LIKE ?
           AND n.id NOT IN (
               SELECT source_id FROM note_links WHERE target_id = ?
           )
         ORDER BY n.title ASC",
    )
    .bind(note_id)
    .bind(&plain_pattern)
    .bind(note_id)
    .fetch_all(pool.inner())
    .await
    ?;

    let wiki_link = format!("[[{}]]", title);

    let mentions: Vec<LinkedNote> = rows
        .into_iter()
        .filter(|r| {
            // Skip locked folders — their content is ciphertext.
            if r.folder_locked != 0 {
                return false;
            }
            // The LIKE pre-filter confirmed the title appears somewhere in the
            // content. Now check that not every occurrence is already a wiki-link.
            // We accept the note as an unlinked mention if the raw title string
            // appears at least once outside of [[...]].
            let stripped = r.content.replace(&wiki_link, "");
            stripped.contains(title.as_str())
        })
        .map(|r| LinkedNote { id: r.id, title: r.title })
        .collect();

    Ok(mentions)
}

/// Replace the first plain-text occurrence of `title` in the given note's
/// content with `[[title]]`, then persist the change and re-sync link relations.
/// Returns the updated content so the frontend can refresh an open tab.
#[tauri::command]
pub async fn convert_mention_to_link(
    pool: State<'_, SqlitePool>,
    note_id: i64,
    title: String,
) -> AppResult<String> {
    let content: String = sqlx::query_scalar("SELECT content FROM notes WHERE id = ?")
        .bind(note_id)
        .fetch_optional(pool.inner())
        .await
        ?
        .ok_or_else(|| AppError::NotFound(format!("Note {note_id} not found")))?;

    let wiki_link = format!("[[{title}]]");

    // Replace only the first plain occurrence that is not already bracketed.
    // Strategy: scan for `title`, check it is not preceded by `[[` and followed by `]]`.
    let updated = replace_first_plain_mention(&content, &title, &wiki_link);

    if updated == content {
        // No plain occurrence found — nothing to do.
        return Ok(content);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    sqlx::query("UPDATE notes SET content = ?, updated_at = ? WHERE id = ?")
        .bind(&updated)
        .bind(now)
        .bind(note_id)
        .execute(pool.inner())
        .await
        ?;

    // Re-sync wiki-links so the new [[title]] link appears in note_links immediately.
    let links = parse_wiki_links(&updated);
    sync_links(pool.inner(), note_id, &links).await?;

    Ok(updated)
}

/// Replace the first occurrence of `needle` in `haystack` that is NOT already
/// surrounded by `[[` and `]]`. Returns the original string if no such
/// occurrence exists.
fn replace_first_plain_mention(haystack: &str, needle: &str, replacement: &str) -> String {
    let needle_len = needle.len();
    let bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut i = 0;

    while i + needle_len <= haystack.len() {
        if &bytes[i..i + needle_len] == needle_bytes {
            // Check it is not already inside [[...]]:
            // preceded by "[[" means bytes[i-2..i] == b"[["
            // followed by "]]" means bytes[i+needle_len..i+needle_len+2] == b"]]"
            let preceded_by_brackets = i >= 2 && &bytes[i - 2..i] == b"[[";
            let followed_by_brackets = i + needle_len + 2 <= haystack.len()
                && &bytes[i + needle_len..i + needle_len + 2] == b"]]";

            if !(preceded_by_brackets && followed_by_brackets) {
                let mut result = String::with_capacity(haystack.len() + replacement.len());
                result.push_str(&haystack[..i]);
                result.push_str(replacement);
                result.push_str(&haystack[i + needle_len..]);
                return result;
            }
        }
        i += 1;
    }
    haystack.to_string()
}
