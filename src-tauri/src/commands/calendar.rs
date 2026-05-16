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

use std::collections::HashMap;
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;
use crate::SharedKeyStore;
use crate::{AppResult, EncryptedNoteStore};
use super::Note;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Re-format an ISO date string (`YYYY-MM-DD`) into the user's preferred display
/// format. Returns the input unchanged for `YYYY-MM-DD` or any unrecognised format.
pub(crate) fn format_display_date(iso: &str, fmt: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() != 3 {
        return iso.to_string();
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    match fmt {
        "DD-MM-YYYY" => format!("{}-{}-{}", d, m, y),
        "MM-DD-YYYY" => format!("{}-{}-{}", m, d, y),
        _ => iso.to_string(),
    }
}

/// Parse an ISO date from a display-formatted token (e.g. `05-05-2026` with DD-MM-YYYY).
fn parse_display_date_token(token: &str, date_format: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = token.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let (a, b, c): (u32, u32, i32) = (parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?);
    if c < 1900 || c > 2100 {
        return None;
    }
    let (y, m, d) = match date_format {
        "DD-MM-YYYY" => (c, b, a),
        "MM-DD-YYYY" => (c, a, b),
        "YYYY-MM-DD" if parts[0].len() == 4 => return NaiveDate::parse_from_str(token, "%Y-%m-%d").ok(),
        _ => return None,
    };
    NaiveDate::from_ymd_opt(y, m, d)
}

const MONTH_NAMES: &[(&str, u32)] = &[
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("sept", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

/// Day number immediately before a month name (e.g. "5th" or "5" in "… 5th of ").
fn extract_trailing_day(before: &str) -> Option<u32> {
    for token in before.split_whitespace().rev() {
        let digits: String = token.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            continue;
        }
        let day: u32 = digits.parse().ok()?;
        if (1..=31).contains(&day) {
            return Some(day);
        }
    }
    None
}

fn extract_leading_day(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let day: u32 = digits.parse().ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn extract_leading_year(s: &str) -> Option<i32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let y: i32 = digits.parse().ok()?;
    (1900..=2100).contains(&y).then_some(y)
}

/// Best-effort natural-language date, e.g. "5th of may 2026" or "may 5 2026".
/// Returns the chosen date and the byte offset of the month token (earliest valid match).
fn parse_natural_language_date(text: &str) -> Option<(NaiveDate, usize)> {
    let lower = text.to_lowercase();
    let mut best: Option<(usize, NaiveDate)> = None;

    for &(month_name, month) in MONTH_NAMES {
        for (mpos, _) in lower.match_indices(month_name) {
            let before = &lower[..mpos];
            let after = lower[mpos + month_name.len()..].trim_start();

            let try_date = |day: u32, year: i32| {
                NaiveDate::from_ymd_opt(year, month, day).map(|d| (mpos, d))
            };

            if let Some(day) = extract_trailing_day(before) {
                if let Some(year) = extract_leading_year(after) {
                    if let Some(pair) = try_date(day, year) {
                        best = Some(match best {
                            None => pair,
                            Some((bp, _)) if pair.0 < bp => pair,
                            Some(b) => b,
                        });
                    }
                }
            }
            if let Some(day) = extract_leading_day(after) {
                let rest = after
                    .chars()
                    .skip_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .trim_start()
                    .to_string();
                if let Some(year) = extract_leading_year(&rest) {
                    if let Some(pair) = try_date(day, year) {
                        best = Some(match best {
                            None => pair,
                            Some((bp, _)) if pair.0 < bp => pair,
                            Some(b) => b,
                        });
                    }
                }
            }
        }
    }

    best.map(|(p, d)| (d, p))
}

struct DateMatch {
    date: NaiveDate,
    start: usize,
}

fn scan_iso_dates(text: &str) -> Vec<DateMatch> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 10 <= bytes.len() {
        if bytes[i + 4] == b'-' && bytes[i + 7] == b'-' {
            let slice = &text[i..i + 10];
            if slice.chars().all(|c| c.is_ascii_digit() || c == '-') {
                if let Ok(date) = NaiveDate::parse_from_str(slice, "%Y-%m-%d") {
                    out.push(DateMatch { date, start: i });
                }
            }
        }
        i += 1;
    }
    out
}

fn scan_display_dates(text: &str, date_format: &str) -> Vec<DateMatch> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i;
        let mut dashes = 0usize;
        while j < chars.len() {
            if chars[j].is_ascii_digit() {
                j += 1;
            } else if chars[j] == '-' {
                if dashes < 2 {
                    dashes += 1;
                    j += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if dashes == 2 && j > start {
            let token: String = chars[start..j].iter().collect();
            if let Some(date) = parse_display_date_token(&token, date_format) {
                out.push(DateMatch { date, start });
            }
        }
        i = if j > i { j } else { i + 1 };
    }
    out
}

/// Extract calendar dates from free text, ordered by first appearance.
pub(crate) fn parse_dates_from_text(text: &str, date_format: &str) -> Vec<String> {
    let mut matches = scan_iso_dates(text);
    matches.extend(scan_display_dates(text, date_format));
    let lower = text.to_lowercase();
    if let Some((date, start)) = parse_natural_language_date(&lower) {
        matches.push(DateMatch { date, start });
    }
    matches.sort_by_key(|m| m.start);
    let mut seen = HashMap::new();
    let mut ordered = Vec::new();
    for m in matches {
        let iso = m.date.format("%Y-%m-%d").to_string();
        if seen.insert(iso.clone(), ()).is_none() {
            ordered.push(iso);
        }
    }
    ordered
}

async fn find_daily_note_in_folder(
    store: &EncryptedNoteStore<'_>,
    folder_id: i64,
    iso_date: &str,
    date_format: &str,
) -> AppResult<Option<Note>> {
    let display_title = format_display_date(iso_date, date_format);
    let notes = store.list_notes(Some(folder_id), false).await?;
    Ok(notes
        .into_iter()
        .find(|n| n.title == display_title || n.title == iso_date))
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Result of resolving a calendar day mentioned in chat text to a daily note title.
#[derive(Debug, Serialize)]
pub struct ResolvedDailyNote {
    pub iso_date: String,
    pub display_title: String,
    /// Present when a matching daily note exists and is accessible (not locked).
    pub note: Option<Note>,
}

/// Per-day activity counts returned to the frontend for the heatmap.
///
/// `created`  = number of notes whose `created_at` falls on this day.
/// `modified` = number of notes whose `updated_at` falls on this day AND on a
///              different calendar day than their `created_at` (so a note is not
///              double-counted if it was created and saved on the same day).
#[derive(Debug, Serialize)]
pub struct ActivityDay {
    pub date: String, // YYYY-MM-DD (UTC)
    pub created: i64,
    pub modified: i64,
}

/// Internal SQL aggregate row — not exposed to the frontend.
#[derive(sqlx::FromRow)]
struct DateCount {
    date: String,
    cnt: i64,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Return per-day activity counts for the past 365 days.
///
/// Two queries are run: one counting created notes per day, one counting notes
/// that were modified on a different day than they were created. The results
/// are merged in Rust and returned sorted oldest-first.
///
/// Timestamps are stored as UTC Unix epoch integers, so all date grouping uses
/// SQLite's `date(ts, 'unixepoch')` which also returns UTC dates.
#[tauri::command]
pub async fn get_activity_heatmap(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<ActivityDay>> {
    let created: Vec<DateCount> = sqlx::query_as(
        "SELECT date(created_at, 'unixepoch') AS date, COUNT(*) AS cnt
         FROM notes
         WHERE created_at >= unixepoch('now', '-365 days')
         GROUP BY date
         ORDER BY date",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let modified: Vec<DateCount> = sqlx::query_as(
        "SELECT date(updated_at, 'unixepoch') AS date, COUNT(*) AS cnt
         FROM notes
         WHERE updated_at >= unixepoch('now', '-365 days')
           AND date(updated_at, 'unixepoch') != date(created_at, 'unixepoch')
         GROUP BY date
         ORDER BY date",
    )
    .fetch_all(pool.inner())
    .await
    ?;

    let mut map: HashMap<String, ActivityDay> = HashMap::new();

    for row in created {
        let entry = map.entry(row.date.clone()).or_insert(ActivityDay {
            date: row.date,
            created: 0,
            modified: 0,
        });
        entry.created += row.cnt;
    }

    for row in modified {
        let entry = map.entry(row.date.clone()).or_insert(ActivityDay {
            date: row.date,
            created: 0,
            modified: 0,
        });
        entry.modified += row.cnt;
    }

    let mut days: Vec<ActivityDay> = map.into_values().collect();
    days.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(days)
}

/// Return all notes that were created or last modified on the given calendar day.
///
/// `date_str` must be a UTC date in `YYYY-MM-DD` format.
/// Locked-folder notes are returned as locked stubs (no title/content), matching
/// the behaviour of `list_notes`.
#[tauri::command]
pub async fn get_notes_for_day(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_str: String,
) -> AppResult<Vec<Note>> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    store.list_notes_for_day(&date_str).await
}

/// Find the daily note for `date_str` inside the "Daily Notes" folder, creating
/// both the folder and the note if they do not yet exist.
///
/// The stored note title is the date formatted according to `date_format` (e.g.
/// `DD-MM-YYYY` → `"06-04-2026"`), encrypted with the vault key when one is active.
/// Legacy notes stored as raw ISO (`YYYY-MM-DD`) are matched as a fallback.
///
/// Because AES-GCM is non-deterministic we cannot query by encrypted title directly.
/// Instead, all notes in the folder are fetched and decrypted in Rust to find the
/// match — at most ~365 notes for a year of daily use, so this is acceptable.
#[tauri::command]
pub async fn get_or_create_daily_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_str: String,
    date_format: Option<String>,
) -> AppResult<Note> {
    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let fmt = date_format.as_deref().unwrap_or("DD-MM-YYYY");

    let folder_id: i64 = if let Some(id) = store.find_root_folder_by_name("Daily Notes").await? {
        id
    } else {
        store.create_folder("Daily Notes", None).await?.id
    };

    if let Some(note) = find_daily_note_in_folder(&store, folder_id, &date_str, fmt).await? {
        return Ok(note);
    }

    let display_title = format_display_date(&date_str, fmt);
    let note = store.create_note(&display_title, Some(folder_id)).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    Ok(note)
}

/// Always create a new daily note for today. If a note with today's title already
/// exists, append ` (2)`, ` (3)`, etc. until a free title is found.
///
/// Unlike `get_or_create_daily_note` (which opens the existing note), this command
/// is bound to the activity bar button and never re-opens an existing note.
#[tauri::command]
pub async fn create_daily_note(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    date_format: Option<String>,
) -> AppResult<Note> {
    // Get today's date in ISO format from SQLite (UTC).
    let (iso_date,): (String,) = sqlx::query_as("SELECT date('now')")
        .fetch_one(pool.inner())
        .await
        ?;

    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let fmt = date_format.as_deref().unwrap_or("DD-MM-YYYY");
    let base_title = format_display_date(&iso_date, fmt);

    let folder_id: i64 = if let Some(id) = store.find_root_folder_by_name("Daily Notes").await? {
        id
    } else {
        store.create_folder("Daily Notes", None).await?.id
    };

    let existing_titles: Vec<String> = store
        .list_notes(Some(folder_id), false).await?
        .into_iter()
        .map(|n| n.title)
        .collect();

    let final_title = if !existing_titles.contains(&base_title) {
        base_title
    } else {
        let mut n = 2u32;
        loop {
            let candidate = format!("{} ({})", base_title, n);
            if !existing_titles.contains(&candidate) {
                break candidate;
            }
            n += 1;
        }
    };

    let note = store.create_note(&final_title, Some(folder_id)).await?;
    super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
    Ok(note)
}

/// Parse calendar dates from chat text and look up matching daily notes (read-only).
///
/// Used before RAG to pin the correct daily note and expose the display title format.
/// Does not create notes or folders.
#[tauri::command]
pub async fn resolve_daily_note_from_query(
    pool: State<'_, SqlitePool>,
    keys: State<'_, SharedKeyStore>,
    query: String,
    date_format: Option<String>,
) -> AppResult<Option<ResolvedDailyNote>> {
    let fmt = date_format.as_deref().unwrap_or("DD-MM-YYYY");
    let dates = parse_dates_from_text(query.trim(), fmt);
    let Some(first_iso) = dates.first() else {
        return Ok(None);
    };

    let store = EncryptedNoteStore::new(pool.inner(), keys.inner().as_ref());
    let mut iso_date = first_iso.clone();
    let mut note = None;

    if let Some(folder_id) = store.find_root_folder_by_name("Daily Notes").await? {
        for iso in &dates {
            if let Some(n) = find_daily_note_in_folder(&store, folder_id, iso, fmt).await? {
                iso_date = iso.clone();
                note = Some(n);
                break;
            }
        }
    }

    let display_title = format_display_date(&iso_date, fmt);
    Ok(Some(ResolvedDailyNote {
        iso_date,
        display_title,
        note,
    }))
}

#[cfg(test)]
mod tests {
    use super::{format_display_date, parse_dates_from_text, parse_natural_language_date};

    #[test]
    fn format_display_date_dd_mm_yyyy() {
        assert_eq!(
            format_display_date("2026-03-05", "DD-MM-YYYY"),
            "05-03-2026"
        );
    }

    #[test]
    fn format_display_date_mm_dd_yyyy() {
        assert_eq!(
            format_display_date("2026-03-05", "MM-DD-YYYY"),
            "03-05-2026"
        );
    }

    #[test]
    fn format_display_date_iso_default_returns_input() {
        assert_eq!(format_display_date("2026-03-05", "YYYY-MM-DD"), "2026-03-05");
    }

    #[test]
    fn format_display_date_non_iso_passthrough() {
        assert_eq!(format_display_date("hello", "DD-MM-YYYY"), "hello");
    }

    #[test]
    fn parse_natural_fifth_of_may_2026() {
        let (d, _) = parse_natural_language_date("what did i write on the 5th of may 2026")
            .expect("date");
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-05-05");
    }

    #[test]
    fn parse_natural_may_5_2026() {
        let (d, _) = parse_natural_language_date("notes from may 5 2026").expect("date");
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-05-05");
    }

    /// When a month substring appears more than once, every occurrence is considered; the
    /// first invalid "may …" must not block a later valid "may 5 2026".
    #[test]
    fn parse_natural_second_month_token_when_first_is_incomplete() {
        let text = "noise may xxxxx may 5 2026";
        let (d, mpos) = parse_natural_language_date(text).expect("date");
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-05-05");
        assert_eq!(mpos, "noise may xxxxx ".len());
    }

    /// Among multiple valid parses for the same month name, the earliest byte offset wins.
    #[test]
    fn parse_natural_earliest_month_match_wins_when_both_valid() {
        let (d, mpos) =
            parse_natural_language_date("may 5 2026 and later may 10 2026").expect("date");
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-05-05");
        assert_eq!(mpos, 0);
    }

    #[test]
    fn parse_dates_from_text_ordinal_and_display() {
        let dates = parse_dates_from_text(
            "what did i write on the 5th of may 2026",
            "DD-MM-YYYY",
        );
        assert_eq!(dates.first().map(String::as_str), Some("2026-05-05"));

        let dates = parse_dates_from_text("see 05-05-2026 for details", "DD-MM-YYYY");
        assert!(dates.contains(&"2026-05-05".to_string()));
    }

    #[test]
    fn parse_dates_iso_in_text() {
        let dates = parse_dates_from_text("meeting on 2026-05-05", "DD-MM-YYYY");
        assert_eq!(dates, vec!["2026-05-05".to_string()]);
    }
}
