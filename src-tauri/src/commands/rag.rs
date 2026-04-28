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

use sqlx::SqlitePool;
use tauri::State;
use crate::KeyStore;
use super::{NoteRow, map_note_row};

async fn get_embedding_model(db: &SqlitePool) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'embedding_model' LIMIT 1")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "nomic-embed-text".to_string())
}

/// Public alias used by `wikipedia.rs` to read the configured embedding model.
pub(crate) async fn get_embedding_model_pub(db: &SqlitePool) -> String {
    get_embedding_model(db).await
}

// ---------------------------------------------------------------------------
// RAG commands (vector index + semantic search)
// ---------------------------------------------------------------------------

// The embed model is configurable via the 'embedding_model' setting.
// nomic-embed-text requires asymmetric prefixes: documents are prefixed with
// "search_document: " and queries with "search_query: " for accurate retrieval.
// Other models may require different prefixes or none at all.

pub(crate) async fn embed_document(text: &str, model: &str) -> Result<Vec<f32>, String> {
    crate::vector::embed(&format!("search_document: {text}"), model).await
}

pub(crate) async fn embed_query(text: &str, model: &str) -> Result<Vec<f32>, String> {
    crate::vector::embed(&format!("search_query: {text}"), model).await
}

/// Split a long line into smaller pieces at sentence-ending punctuation.
/// Only splits where `. `, `! `, or `? ` is followed by an uppercase letter,
/// which avoids false splits on abbreviations like "e.g. " or "TLS 1.3 ".
fn split_at_punctuation(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        buf.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let next = chars.get(i + 1).copied();
            let after = chars.get(i + 2).copied();
            if matches!(next, Some(' ') | Some('\t'))
                && matches!(after, Some(c) if c.is_uppercase())
            {
                let s = buf.trim().to_string();
                if !s.is_empty() {
                    parts.push(s);
                }
                buf.clear();
            }
        }
    }

    let tail = buf.trim().to_string();
    if !tail.is_empty() {
        parts.push(tail);
    }
    if parts.is_empty() {
        parts.push(text.trim().to_string());
    }
    parts
}

/// Split `text` into individual sentences.
/// First splits on newlines (one idea per line is common in notes), then
/// further splits long lines at sentence-ending punctuation.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.split_whitespace().count() > 30 {
            sentences.extend(split_at_punctuation(line));
        } else {
            sentences.push(line.to_string());
        }
    }

    if sentences.is_empty() {
        let s = text.trim().to_string();
        if !s.is_empty() {
            sentences.push(s);
        }
    }
    sentences
}

/// Group a flat list of sentences into overlapping chunks.
/// `per_chunk` is the number of sentences per chunk; `overlap` is how many
/// sentences the next chunk re-uses from the end of the previous one.
pub fn chunk_sentences(
    sentences: Vec<String>,
    per_chunk: usize,
    overlap: usize,
) -> Vec<String> {
    if sentences.is_empty() {
        return vec![String::new()];
    }
    if sentences.len() <= per_chunk {
        return vec![sentences.join(" ")];
    }
    let step = per_chunk.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + per_chunk).min(sentences.len());
        chunks.push(sentences[start..end].join(" "));
        if end == sentences.len() {
            break;
        }
        start += step;
    }
    chunks
}

/// Build a "Properties: key=value, …" suffix for a note, to be appended to
/// the note content before embedding. Returns an empty string if the note has
/// no properties or no folder.
pub(crate) async fn build_properties_suffix(pool: &SqlitePool, note_id: i64) -> String {
    #[derive(sqlx::FromRow)]
    struct PV {
        name: String,
        value: String,
    }

    let pairs: Vec<PV> = sqlx::query_as(
        "SELECT pd.name, COALESCE(np.value, '') AS value
         FROM property_defs pd
         LEFT JOIN note_properties np ON np.def_id = pd.id AND np.note_id = ?
         WHERE pd.folder_id = (SELECT folder_id FROM notes WHERE id = ?)
         ORDER BY pd.position ASC",
    )
    .bind(note_id)
    .bind(note_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let filled: Vec<String> = pairs
        .into_iter()
        .filter(|p| !p.value.is_empty())
        .map(|p| format!("{}={}", p.name, p.value))
        .collect();

    if filled.is_empty() {
        String::new()
    } else {
        format!("\nProperties: {}", filled.join(", "))
    }
}

/// Embed a note and store it in the vector index.
#[tauri::command]
pub async fn index_note(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    note_id: i64,
    title: String,
    content: String,
) -> Result<(), String> {
    let props_suffix = build_properties_suffix(pool.inner(), note_id).await;
    let full_content = if props_suffix.is_empty() {
        content
    } else {
        format!("{content}{props_suffix}")
    };

    let sentences = split_sentences(&full_content);
    let raw_chunks = chunk_sentences(sentences, 1, 0);

    if raw_chunks.iter().all(|c| c.trim().is_empty()) {
        return crate::vector::remove(&vdb.0, note_id).await;
    }

    let model = get_embedding_model(pool.inner()).await;

    // Prefix every chunk with the document search prefix expected by nomic-embed-text.
    let doc_texts: Vec<String> = raw_chunks
        .iter()
        .map(|chunk| format!("search_document: {title}\n{chunk}"))
        .collect();

    // Embed all chunks in a single batch request instead of one-by-one.
    // For large files this is orders of magnitude faster.
    let embeddings = match crate::vector::embed_batch(&doc_texts, &model).await {
        Ok(e) => e,
        Err(_) => {
            // Fall back to sequential embedding if batch fails.
            let mut fallback = Vec::with_capacity(raw_chunks.len());
            for chunk in &raw_chunks {
                let emb = embed_document(chunk, &model).await?;
                fallback.push(emb);
            }
            fallback
        }
    };

    let chunks: Vec<(i32, String, Vec<f32>)> = raw_chunks
        .into_iter()
        .enumerate()
        .zip(embeddings)
        .map(|((i, chunk_text), embedding)| (i as i32, chunk_text, embedding))
        .collect();

    crate::vector::upsert(&vdb.0, note_id, &title, chunks).await
}

/// Remove a note from the vector index. Called when a note is deleted.
#[tauri::command]
pub async fn remove_note_index(
    vdb: State<'_, crate::vector::VectorDb>,
    note_id: i64,
) -> Result<(), String> {
    crate::vector::remove(&vdb.0, note_id).await
}

/// Embed the query text and return the most semantically similar notes.
///
/// After the LanceDB search, results are cross-referenced with SQLite to:
/// 1. Filter out notes in folders that are currently locked (no session key).
/// 2. Replace the title stored in LanceDB with the current, decrypted title
///    from SQLite — this fixes stale ciphertext titles left over from notes
///    that were indexed before encryption was applied (or before migration 0009).
#[tauri::command]
pub async fn search_notes(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<crate::vector::NoteMatch>, String> {
    let model = get_embedding_model(pool.inner()).await;
    let embedding = embed_query(&query, &model).await?;
    let mut matches = crate::vector::search(
        &vdb.0,
        embedding,
        limit.unwrap_or(crate::vector::CHUNK_FETCH_LIMIT),
    )
    .await?;

    if matches.is_empty() {
        return Ok(matches);
    }

    // Batch-fetch the current title and lock state for all returned note IDs.
    let ids: Vec<i64> = matches.iter().map(|m| m.note_id).collect();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT n.id, n.title, n.folder_id, COALESCE(f.locked, 0) AS folder_locked
         FROM notes n
         LEFT JOIN folders f ON f.id = n.folder_id
         WHERE n.id IN ({placeholders})"
    );
    let mut q = sqlx::query_as::<_, (i64, String, Option<i64>, i64)>(&sql);
    for id in &ids {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;

    // Build a map from note_id → decrypted title, skipping locked notes.
    let mut accessible: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for (id, raw_title, folder_id, folder_locked_col) in rows {
        // Skip notes in currently-locked folders.
        let locked = folder_locked_col != 0
            && folder_id
                .map(|fid| super::folder_is_locked(fid, true, &keys))
                .unwrap_or(false);
        if locked {
            continue;
        }

        // Decrypt the title if an active key is available for this note.
        let title = if let Some(key) = super::resolve_key(folder_id, &keys) {
            crate::crypto::decrypt(&key, &raw_title)
                .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                .unwrap_or(raw_title)
        } else {
            raw_title
        };
        accessible.insert(id, title);
    }

    // Drop locked/missing results; update titles with current decrypted values.
    matches.retain(|m| accessible.contains_key(&m.note_id));
    for m in &mut matches {
        if let Some(title) = accessible.get(&m.note_id) {
            m.title = title.clone();
        }
    }

    let _ = crate::audit::log_event(
        pool.inner(), "search_semantic", None, None, None, Some(&query),
    ).await;
    Ok(matches)
}

/// Re-index every note currently in SQLite into LanceDB from scratch.
#[tauri::command]
pub async fn reindex_all(
    pool: State<'_, SqlitePool>,
    keys: State<'_, KeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
) -> Result<String, String> {
    let vault_key_absent = keys.vault_key.lock()
        .map(|vk| vk.is_none())
        .unwrap_or(true);
    if vault_key_absent {
        let has_pw: bool = sqlx::query_scalar(
            "SELECT COUNT(*) FROM vault_lock WHERE id = 1",
        )
        .fetch_one(pool.inner())
        .await
        .map(|n: i64| n > 0)
        .unwrap_or(false);
        if has_pw {
            return Ok("0 notes indexed (vault is locked)".to_string());
        }
    }

    let raw_notes = sqlx::query_as::<_, NoteRow>(
        "SELECT id, title, content, folder_id, created_at, updated_at FROM notes",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let folder_lock_states: std::collections::HashMap<i64, bool> = {
        let locked_rows: Vec<(i64, i64)> =
            sqlx::query_as("SELECT id, locked FROM folders")
                .fetch_all(pool.inner())
                .await
                .unwrap_or_default();
        locked_rows.into_iter().map(|(id, lk)| (id, lk != 0)).collect()
    };

    let model = get_embedding_model(pool.inner()).await;
    let mut count = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for raw in raw_notes {
        let fl = raw.folder_id
            .and_then(|fid| folder_lock_states.get(&fid).copied())
            .unwrap_or(false);
        let note = map_note_row(raw, fl, &keys);
        if note.locked {
            continue;
        }
        super::search::fts_upsert(pool.inner(), note.id, &note.title, &note.content).await;
        let props_suffix = build_properties_suffix(pool.inner(), note.id).await;
        let full_content = if props_suffix.is_empty() {
            note.content.clone()
        } else {
            format!("{}{props_suffix}", note.content)
        };
        let sentences = split_sentences(&full_content);
        let raw_chunks = chunk_sentences(sentences, 1, 0);
        if raw_chunks.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let mut chunks: Vec<(i32, String, Vec<f32>)> = Vec::new();
        let mut note_ok = true;
        for (i, chunk_text) in raw_chunks.into_iter().enumerate() {
            match embed_document(&chunk_text, &model).await {
                Ok(embedding) => chunks.push((i as i32, chunk_text, embedding)),
                Err(e) => {
                    failed.push(format!("\"{}\" — {}", note.title, e));
                    note_ok = false;
                    break;
                }
            }
        }
        if note_ok {
            if let Err(e) = crate::vector::upsert(&vdb.0, note.id, &note.title, chunks).await {
                failed.push(format!("\"{}\" (upsert) — {}", note.title, e));
            } else {
                count += 1;
            }
        }
    }

    let summary = if failed.is_empty() {
        format!("{count} notes indexed")
    } else {
        format!("{count} notes indexed, {} failed:\n{}", failed.len(), failed.join("\n"))
    };
    Ok(summary)
}

/// Debug command: returns the top 10 vector search hits with raw distance scores.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_search(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    query: String,
) -> Result<Vec<crate::vector::RawMatch>, String> {
    let model = get_embedding_model(pool.inner()).await;
    let embedding = embed_query(&query, &model).await?;
    crate::vector::raw_search(&vdb.0, embedding, 10).await
}

/// Debug command: returns top Wikipedia search hits with raw distance scores, no filtering.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_search_wikipedia(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
    query: String,
) -> Result<Vec<crate::vector::RawMatch>, String> {
    let model = get_embedding_model(pool.inner()).await;
    let embedding = embed_query(&query, &model).await?;
    crate::vector::raw_wikipedia_search(&vdb.0, embedding, 10).await
}

/// Insert a set of varied seed notes and index them all.
/// Intended for development/testing only.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn seed_notes(
    pool: State<'_, SqlitePool>,
    vdb: State<'_, crate::vector::VectorDb>,
) -> Result<String, String> {
    let seeds: &[(&str, &str)] = &[
        (
            "Rust ownership and borrowing",
            "Rust enforces memory safety through a system of ownership with rules that the \
            compiler checks at compile time. Every value in Rust has a single owner. When the \
            owner goes out of scope, the value is dropped. References allow you to refer to a \
            value without taking ownership. The borrow checker ensures references are always \
            valid. Mutable references (&mut T) are exclusive — only one can exist at a time. \
            This prevents data races at compile time, without needing a garbage collector.",
        ),
        (
            "Sleep and cognitive performance",
            "Sleep plays a critical role in memory consolidation. During slow-wave sleep, the \
            hippocampus replays experiences to the neocortex for long-term storage. REM sleep \
            is associated with procedural memory and emotional regulation. Chronic sleep \
            deprivation impairs prefrontal cortex function, reducing decision-making ability, \
            working memory, and attention span. Adults generally need 7–9 hours. Even one night \
            of under-sleeping measurably reduces cognitive performance the following day.",
        ),
        (
            "How Transformer models work",
            "Transformers use self-attention to weigh the relevance of each token in a sequence \
            relative to every other token. Unlike RNNs, they process all tokens in parallel, \
            making them highly amenable to GPU acceleration. The attention mechanism computes \
            query, key, and value matrices and produces weighted sums of values. Positional \
            encodings are added to embeddings to preserve sequence order. Models like GPT are \
            decoder-only (autoregressive); BERT is encoder-only (masked language modeling). \
            LLMs are transformer-based models trained on large corpora to predict the next token.",
        ),
        (
            "Fermentation basics",
            "Fermentation is a metabolic process in which microorganisms like bacteria, yeast, \
            or fungi convert sugars into acids, gases, or alcohol. Lactic acid fermentation \
            (used in yoghurt, kimchi, sauerkraut) produces lactic acid. Alcoholic fermentation \
            (used in beer, wine, bread) produces ethanol and CO2. Temperature, salt concentration, \
            and pH all affect which microorganisms thrive. Fermented foods are rich in probiotics \
            and have been linked to improved gut microbiome diversity. Starter cultures can be \
            used to ensure consistent results.",
        ),
        (
            "The Stoic practice of negative visualisation",
            "Negative visualisation (premeditatio malorum) is a Stoic technique where you \
            deliberately imagine losing things you value — health, relationships, property. The \
            goal is not to induce anxiety but to cultivate gratitude and reduce attachment. Seneca \
            wrote that we should rehearse poverty, illness, and death periodically so that fortune \
            cannot catch us off guard. The practice counteracts hedonic adaptation, the tendency \
            to take good things for granted over time. It pairs well with the dichotomy of \
            control: focusing only on what is within your power.",
        ),
        (
            "How HTTPS and TLS work",
            "HTTPS is HTTP over TLS (Transport Layer Security). A TLS handshake establishes a \
            secure channel before any HTTP data is sent. The client sends a ClientHello with \
            supported cipher suites. The server responds with its certificate (signed by a CA) \
            and the chosen cipher. Key exchange uses asymmetric cryptography (e.g. ECDH) to \
            derive a shared secret without transmitting it. All subsequent traffic is encrypted \
            with symmetric keys derived from that secret. TLS 1.3 removed weak cipher suites and \
            reduced handshake round-trips from two to one.",
        ),
        (
            "Compound interest and long-term investing",
            "Compound interest is interest calculated on both the initial principal and the \
            accumulated interest. Over long periods, the effect is exponential. A 7% annual \
            return doubles money roughly every 10 years (rule of 72). Starting early matters \
            more than the amount invested: £5,000 invested at 25 grows more than £10,000 \
            invested at 35 at the same return rate. Index funds provide broad market exposure \
            with low fees, historically outperforming most actively managed funds over 20+ year \
            windows. Fee drag compounds just as returns do — a 1% annual fee has a significant \
            long-term cost.",
        ),
        (
            "Zettelkasten note-taking method",
            "# Zettelkasten\n\n\
            Zettelkasten is a note-taking method developed by sociologist **Niklas Luhmann**, who \
            used it to write over 70 books.\n\n\
            ## Core idea\n\n\
            Each note (*zettel*) contains a **single atomic idea** and is linked to related notes \
            by reference. Notes are not organised into folders or topics — meaning emerges from \
            the link structure.\n\n\
            ## Note types\n\n\
            1. **Fleeting notes** — quick captures, disposable\n\
            2. **Literature notes** — summaries from sources, in your own words\n\
            3. **Permanent notes** — processed ideas, written as if to a reader\n\n\
            ## Why it works\n\n\
            > The method is designed to build a personal knowledge graph over time, with the \
            network of links surfacing non-obvious connections between ideas.\n\n\
            Links between notes create a `web of knowledge` rather than a hierarchy. \
            Over years, the network becomes a reliable thinking partner.\n\n\
            ---\n\n\
            **See also:** [[How Transformer models work]], [[Sleep and cognitive performance]]",
        ),
    ];

    let model = get_embedding_model(pool.inner()).await;
    let mut count = 0usize;
    let mut indexed = 0usize;
    for (title, content) in seeds {
        let row = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, content) VALUES (?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(title)
        .bind(content)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        count += 1;

        // Embedding is best-effort: seeding should work even without Ollama
        let sentences = split_sentences(&row.content);
        let raw_chunks = chunk_sentences(sentences, 1, 0);
        let mut chunks: Vec<(i32, String, Vec<f32>)> = Vec::new();
        let mut embed_ok = true;
        for (i, chunk_text) in raw_chunks.into_iter().enumerate() {
            match embed_document(&chunk_text, &model).await {
                Ok(embedding) => chunks.push((i as i32, chunk_text, embedding)),
                Err(_) => { embed_ok = false; break; }
            }
        }
        if embed_ok {
            if let Ok(()) = crate::vector::upsert(&vdb.0, row.id, &row.title, chunks).await {
                indexed += 1;
            }
        }
    }

    // Seed the kanban demo folder (best-effort — don't fail seeding if this errors).
    let _ = seed_kanban_folder(pool.inner()).await;

    if indexed == count {
        Ok(format!("{count}"))
    } else {
        Ok(format!("{count}:{indexed}"))
    }
}

/// Seed a "Kanban Demo" folder with a Status select property, a Priority select
/// property, and a set of notes spread across the Status columns.
/// Called at the end of seed_notes; extracted for readability.
#[cfg(debug_assertions)]
async fn seed_kanban_folder(pool: &SqlitePool) -> Result<(), String> {
    // Create the folder.
    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name) VALUES ('Kanban Demo') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Create "Status" select property (position 0).
    let status_options = r#"["Todo","In Progress","Review","Done"]"#;
    let status_def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Status', 'select', ?, 0) RETURNING id",
    )
    .bind(folder_id)
    .bind(status_options)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Create "Priority" select property (position 1).
    let priority_options = r#"["Low","Medium","High"]"#;
    let priority_def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Priority', 'select', ?, 1) RETURNING id",
    )
    .bind(folder_id)
    .bind(priority_options)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Notes: (title, content, status, priority)
    let notes: &[(&str, &str, &str, &str)] = &[
        (
            "Design landing page",
            "Sketch wireframes and decide on the hero section layout. Review with the team before moving to high-fidelity.",
            "Todo", "High",
        ),
        (
            "Write API documentation",
            "Document all public endpoints with request/response examples. Include authentication flow and error codes.",
            "Todo", "Medium",
        ),
        (
            "Set up CI pipeline",
            "Configure GitHub Actions to run tests and linting on every pull request. Add a deploy step for staging.",
            "Todo", "Low",
        ),
        (
            "Implement search indexing",
            "Integrate the FTS5 virtual table with the Rust backend. Ensure encrypted notes are indexed with decrypted content.",
            "In Progress", "High",
        ),
        (
            "Refactor authentication module",
            "Replace the hand-rolled session handling with a proper state machine. Add rate limiting to the unlock endpoint.",
            "In Progress", "Medium",
        ),
        (
            "Add dark mode support",
            "Audit all CSS variables and ensure they respond correctly to the prefers-color-scheme media query.",
            "In Progress", "Low",
        ),
        (
            "Write unit tests for crypto module",
            "Cover all encryption and decryption paths. Include edge cases: wrong password, corrupted sentinel, empty content.",
            "Review", "High",
        ),
        (
            "Optimise vector embeddings",
            "Profile the embedding pipeline and reduce latency. Consider batching requests to the Ollama API.",
            "Review", "Medium",
        ),
        (
            "Fix calendar date offset bug",
            "Daily notes created near midnight were being assigned the wrong date in some time zones. Patched and tested.",
            "Done", "High",
        ),
        (
            "Update dependencies",
            "Bumped sqlx to 0.8, tauri to 2.x, and svelte to 5. Resolved all breaking changes and deprecation warnings.",
            "Done", "Low",
        ),
        (
            "Add tag autocomplete",
            "Implemented #tag suggestions in the editor using a floating dropdown. Filters on the current word after #.",
            "Done", "Medium",
        ),
    ];

    for (title, content, status, priority) in notes {
        let note_id: i64 = sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(content)
        .bind(folder_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Set Status property.
        sqlx::query(
            "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
             ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
        )
        .bind(note_id)
        .bind(status_def_id)
        .bind(status)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Set Priority property.
        sqlx::query(
            "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
             ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
        )
        .bind(note_id)
        .bind(priority_def_id)
        .bind(priority)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
