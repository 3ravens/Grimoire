// Copyright (C) 2026 Wim Palland
//
// Test-data generator for debug builds.
//
// Invocable from Settings → Developer in dev builds.  Populates the vault
// with 100+ realistic notes across multiple folders, writing styles, tags,
// wiki-links, properties, templates, and daily notes, so that benchmarks
// and manual QA work against data that resembles real usage.
//
// Compiled out of release builds entirely via #[cfg(debug_assertions)].

mod test_data_content;

use crate::commands::NoteRow;
use crate::config::{AppConfig, SharedConfig};
use crate::error::{AppError, AppResult};
use crate::SharedKeyStore;
use crate::vector::VectorDb;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashSet;
use lancedb::Connection;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// Public command
// ---------------------------------------------------------------------------

/// Summary returned to the frontend after generation completes.
#[derive(Debug, Serialize)]
pub struct TestDataSummary {
    pub notes: usize,
    pub folders: usize,
    pub templates: usize,
    pub tags: usize,
    pub links: usize,
    pub daily_notes: usize,
    pub embedded: usize,
    pub errors: Vec<String>,
}

/// Parameters for [`seed_test_vault_inner`] (shared by the Tauri command and `perf-budget`).
#[derive(Debug, Clone)]
pub struct SeedTestVaultParams {
    pub note_count: usize,
    pub folder_count: usize,
    pub seed: Option<u64>,
    pub include_daily_notes: bool,
    pub embed: bool,
}

/// Payload for the `test_data:progress` event (Settings → Developer generator).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDataProgress {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

fn emit_test_data_progress(
    app: Option<&AppHandle>,
    phase: &str,
    message: impl Into<String>,
    current: Option<u32>,
    total: Option<u32>,
) {
    let Some(handle) = app else {
        return;
    };
    let payload = TestDataProgress {
        phase: phase.to_string(),
        message: message.into(),
        current,
        total,
    };
    let _ = handle.emit("test_data:progress", &payload);
}

/// Populate SQLite + optionally LanceDB using the same logic as Settings → Developer
/// "Generate test data".
///
/// `progress_app`: when set, emits `test_data:progress` so the UI can show status (embed is slow).
pub async fn seed_test_vault_inner(
    pool: &SqlitePool,
    lance: Option<&Connection>,
    config: &SharedConfig,
    params: SeedTestVaultParams,
    progress_app: Option<&AppHandle>,
) -> AppResult<TestDataSummary> {
    let SeedTestVaultParams {
        note_count,
        folder_count,
        seed,
        include_daily_notes,
        embed,
    } = params;

    if embed && lance.is_none() {
        return Err(AppError::Io(
            "seed_test_vault_inner: embed=true requires a LanceDB connection".into(),
        ));
    }

    let mut errors: Vec<String> = Vec::new();

    let rng_seed = seed.unwrap_or_else(|| rand::thread_rng().gen());
    let mut rng = StdRng::seed_from_u64(rng_seed);
    let domains = test_data_content::domain_pools();

    let nc = note_count.clamp(10, 500);
    let fc = folder_count.clamp(1, 20).min(domains.len());

    emit_test_data_progress(
        progress_app,
        "starting",
        format!("Generating test vault ({nc} notes, {fc} topic folders)…"),
        None,
        None,
    );
    tokio::task::yield_now().await;

    // ── 1. Folders ────────────────────────────────────────────────────
    let mut folder_ids: Vec<i64> = Vec::with_capacity(fc);
    let chosen_domains: Vec<usize> = rand_domain_indices(&mut rng, &domains, fc);

    for &di in &chosen_domains {
        match sqlx::query_scalar("INSERT INTO folders (name) VALUES (?) RETURNING id")
            .bind(domains[di].name)
            .fetch_one(pool)
            .await
        {
            Ok(id) => folder_ids.push(id),
            Err(e) => errors.push(format!("folder '{}': {e}", domains[di].name)),
        }
    }

    emit_test_data_progress(progress_app, "folders", "Created topic folders.", None, None);
    tokio::task::yield_now().await;

    // Fixed Kanban, Database, and weekly-recap fixture folders (not part of random domain clusters).
    let mut fixture_note_ids: Vec<i64> = Vec::new();
    let (kanban_folder_id, kanban_note_ids) = seed_test_kanban_folder(pool, &mut errors).await;
    if let Some(fid) = kanban_folder_id {
        folder_ids.push(fid);
        fixture_note_ids.extend(kanban_note_ids);
    }
    let (database_folder_id, database_note_ids) =
        seed_test_database_folder(pool, &mut rng, &mut errors).await;
    if let Some(fid) = database_folder_id {
        folder_ids.push(fid);
        fixture_note_ids.extend(database_note_ids);
    }
    let (weekly_folder_id, weekly_note_ids) =
        seed_test_weekly_review_fixture_folder(pool, &mut errors).await;
    if let Some(fid) = weekly_folder_id {
        folder_ids.push(fid);
        fixture_note_ids.extend(weekly_note_ids);
    }

    emit_test_data_progress(
        progress_app,
        "fixtures",
        "Added Kanban, database, and weekly-review fixture folders.",
        None,
        None,
    );
    tokio::task::yield_now().await;

    // ── 2. Templates ──────────────────────────────────────────────────
    let template_ids = seed_templates(pool, &mut rng)
        .await
        .unwrap_or_else(|e| {
            errors.push(format!("templates: {e}"));
            vec![]
        });

    emit_test_data_progress(progress_app, "templates", "Seeded note templates.", None, None);
    tokio::task::yield_now().await;

    // ── 3. Notes ──────────────────────────────────────────────────────
    let mut note_ids: Vec<i64> = Vec::with_capacity(nc);
    let mut note_titles: Vec<String> = Vec::with_capacity(nc);
    let mut title_set: HashSet<String> = HashSet::new();

    // Pre-generate unique titles.
    for _ in 0..nc {
        let di = chosen_domains[rng.gen_range(0..fc)];
        let dp = &domains[di];
        loop {
            let frag = dp.title_fragments[rng.gen_range(0..dp.title_fragments.len())];
            let noun = pick_noun(&mut rng, dp);
            let title = format!("{frag} {noun}");
            if title_set.insert(title.clone()) {
                note_titles.push(title);
                break;
            }
        }
    }

    // Assign folders before content so [[wiki-links]] can prefer same-cluster targets.
    let mut note_folder: Vec<usize> = Vec::with_capacity(nc);
    for _ in 0..nc {
        note_folder.push(rng.gen_range(0..fc));
    }

    let note_goal = note_titles.len() as u32;
    for (i, title) in note_titles.iter().enumerate() {
        if i == 0 || (i + 1) % 10 == 0 || i + 1 == note_titles.len() {
            emit_test_data_progress(
                progress_app,
                "notes",
                format!("Writing notes ({}/{})…", i + 1, note_titles.len()),
                Some((i + 1) as u32),
                Some(note_goal),
            );
            tokio::task::yield_now().await;
        }
        let fi = note_folder[i];
        let folder_id = folder_ids[fi];
        let dp = &domains[chosen_domains[fi]];

        let style: &str = match rng.gen_range(0u32..10) {
            0..=3 => "structured",
            4..=6 => "prose",
            7..=8 => "bullet",
            _ => "journal",
        };

        let same_folder_titles: Vec<&str> = note_titles
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i && note_folder[*j] == fi)
            .map(|(_, t)| t.as_str())
            .collect();

        let content = assemble_note(
            style,
            &mut rng,
            dp,
            title.as_str(),
            &same_folder_titles,
            &note_titles,
        );

        let template_id = if !template_ids.is_empty() && rng.gen_bool(0.15) {
            Some(template_ids[rng.gen_range(0..template_ids.len())])
        } else {
            None
        };

        match sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (title, content, folder_id, template_id) VALUES (?, ?, ?, ?)
             RETURNING id, title, content, folder_id, created_at, updated_at",
        )
        .bind(title)
        .bind(&content)
        .bind(folder_id)
        .bind(template_id)
        .fetch_one(pool)
        .await
        {
            Ok(row) => note_ids.push(row.id),
            Err(e) => errors.push(format!("note '{title}': {e}")),
        }
    }

    emit_test_data_progress(progress_app, "tags", "Assigning tags to notes…", None, None);
    tokio::task::yield_now().await;

    // ── 4. Tags ───────────────────────────────────────────────────────
    let mut tag_count = 0usize;
    for (i, note_id) in note_ids.iter().enumerate() {
        let fi = note_folder[i];
        let dp = &domains[chosen_domains[fi]];
        let n = rng.gen_range(1..=3).min(dp.tags.len());
        let mut picked: HashSet<&str> = HashSet::new();
        while picked.len() < n {
            picked.insert(dp.tags[rng.gen_range(0..dp.tags.len())]);
        }
        for tag_name in picked {
            if let Err(e) = sqlx::query(
                "INSERT OR IGNORE INTO tags (name) VALUES (?); \
                 INSERT OR IGNORE INTO note_tags (note_id, tag_id) \
                 SELECT ?, id FROM tags WHERE name = ?",
            )
            .bind(tag_name)
            .bind(note_id)
            .bind(tag_name)
            .execute(pool)
            .await
            {
                errors.push(format!("tag '{tag_name}' nid={note_id}: {e}"));
            } else {
                tag_count += 1;
            }
        }
    }

    emit_test_data_progress(progress_app, "wiki_links", "Creating wiki-links between notes…", None, None);
    tokio::task::yield_now().await;

    // ── 5. Wiki-links (mostly intra-folder clusters + sparse bridges) ──
    let mut link_count = 0usize;
    let n_notes = note_ids.len();

    // Intra-folder: each note links to a few others in the same folder only.
    for i in 0..n_notes {
        let same: Vec<usize> = (0..n_notes)
            .filter(|&j| j != i && note_folder[j] == note_folder[i])
            .collect();
        if same.is_empty() {
            continue;
        }
        let k = rng.gen_range(1..=3).min(same.len());
        let mut picked: HashSet<usize> = HashSet::new();
        let mut guard = 0usize;
        while picked.len() < k && guard < k * 20 {
            guard += 1;
            picked.insert(same[rng.gen_range(0..same.len())]);
        }
        for &tidx in &picked {
            let sid = note_ids[i];
            let tid = note_ids[tidx];
            if insert_link_if_new(pool, sid, tid, &mut errors).await {
                link_count += 1;
            }
        }
    }

    // Sparse bridges across folders (keeps lobes visually separated).
    let mut bridge_budget = (3 * fc).min(n_notes / 15);
    if fc > 1 && n_notes >= 8 && bridge_budget < 2 {
        bridge_budget = 2;
    }
    for _ in 0..bridge_budget {
        let mut placed = false;
        for _ in 0..80 {
            let a = rng.gen_range(0..n_notes);
            let b = rng.gen_range(0..n_notes);
            if a == b || note_folder[a] == note_folder[b] {
                continue;
            }
            let sid = note_ids[a];
            let tid = note_ids[b];
            if insert_link_if_new(pool, sid, tid, &mut errors).await {
                link_count += 1;
                placed = true;
                break;
            }
        }
        if !placed {
            break;
        }
    }

    emit_test_data_progress(progress_app, "properties", "Adding sample database properties…", None, None);
    tokio::task::yield_now().await;

    // ── 6. Properties ─────────────────────────────────────────────────
    for (idx, &fi) in chosen_domains.iter().enumerate() {
        if rng.gen_bool(0.5) {
            if let Err(e) = seed_folder_properties(pool, folder_ids[idx]).await {
                errors.push(format!("properties for folder {}: {e}", domains[fi].name));
            }
        }
    }

    // ── 7. Daily notes ────────────────────────────────────────────────
    let mut daily_count = 0usize;
    if include_daily_notes {
        emit_test_data_progress(
            progress_app,
            "daily_notes",
            "Generating daily notes (last ~120 days)…",
            None,
            None,
        );
        tokio::task::yield_now().await;
        daily_count = seed_daily_notes(pool, &mut rng, &mut errors).await;
    }

    // ── 8. Embedding (best-effort) ────────────────────────────────────
    // Use the same batched path as normal indexing (`index_note_vectors_inner`), not
    // one Ollama round-trip per sentence — the latter can take tens of minutes for 100+ notes.
    let mut embedded = 0usize;
    if embed {
        let conn = lance.expect("checked above");
        let (model, max_retries) = {
            let c = config.read().unwrap();
            (c.embedding_model.clone(), c.background_max_retries)
        };
        let embed_total = note_ids.len() + fixture_note_ids.len();
        emit_test_data_progress(
            progress_app,
            "embedding",
            format!(
                "Embedding {embed_total} notes for semantic search (Ollama, model `{model}`)…"
            ),
            Some(0),
            Some(embed_total as u32),
        );
        tokio::task::yield_now().await;
        for (ei, &note_id) in note_ids.iter().chain(fixture_note_ids.iter()).enumerate() {
            if ei == 0 || (ei + 1) % 3 == 0 || ei + 1 == embed_total {
                emit_test_data_progress(
                    progress_app,
                    "embedding",
                    format!(
                        "Embedding notes ({}/{embed_total}) — Ollama may take several seconds per note…",
                        ei + 1
                    ),
                    Some((ei + 1) as u32),
                    Some(embed_total as u32),
                );
                tokio::task::yield_now().await;
            }
            let content: Option<String> =
                sqlx::query_scalar("SELECT content FROM notes WHERE id = ?")
                    .bind(note_id)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);
            if let Some(body) = content {
                let title: String = sqlx::query_scalar("SELECT title FROM notes WHERE id = ?")
                    .bind(note_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or_default();
                match crate::commands::rag::index_note_vectors_inner(
                    pool,
                    conn,
                    &model,
                    max_retries,
                    note_id,
                    &title,
                    &body,
                    crate::vector::EmbedBatchOptions::default(),
                )
                .await
                {
                    Ok(()) => embedded += 1,
                    Err(e) => errors.push(format!("embed note {note_id}: {e}")),
                }
            }
        }
    }

    emit_test_data_progress(
        progress_app,
        "done",
        "Test data generation finished.",
        None,
        None,
    );
    tokio::task::yield_now().await;

    Ok(TestDataSummary {
        notes: note_ids.len() + fixture_note_ids.len(),
        folders: folder_ids.len(),
        templates: template_ids.len(),
        tags: tag_count,
        links: link_count,
        daily_notes: daily_count,
        embedded,
        errors,
    })
}

/// Wipes all vault metadata from SQLite and drops LanceDB semantic indexes.
/// Dev builds only — intended for a clean QA slate (destructive).
#[tauri::command]
#[cfg(debug_assertions)]
pub async fn clean_developer_database(
    pool: tauri::State<'_, SqlitePool>,
    vdb: tauri::State<'_, VectorDb>,
    config: tauri::State<'_, SharedConfig>,
    keys: tauri::State<'_, SharedKeyStore>,
) -> AppResult<()> {
    // Match `clear_notes_index` / `clear_wiki_index` / `clear_scanned_index` behaviour.
    crate::commands::rag::clear_vault_reindex_checkpoint(pool.inner()).await?;
    crate::vector::clear_notes_index(&vdb.0)
        .await
        .map_err(AppError::VectorStore)?;
    crate::vector::clear_wiki_index(&vdb.0)
        .await
        .map_err(AppError::VectorStore)?;
    crate::vector::clear_scanned_index(&vdb.0)
        .await
        .map_err(AppError::VectorStore)?;

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM note_versions")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM note_properties")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM note_tags").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM note_links").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM bookmarks").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM notes").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM property_defs")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM templates").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM tags").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM folders").execute(&mut *tx).await?;

    sqlx::query("DELETE FROM scanned_files")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM scanned_paths")
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM wikipedia_highlights")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM wikipedia_index_checkpoint")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM wikipedia_bundles")
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM vault_reindex_queue")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM vault_reindex_state")
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM audit_log").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM settings").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM vault_lock").execute(&mut *tx).await?;

    let _ = sqlx::query("DELETE FROM wikipedia_articles_fts")
        .execute(&mut *tx)
        .await;

    let _ = sqlx::query("DELETE FROM sqlite_sequence")
        .execute(&mut *tx)
        .await;

    tx.commit().await?;

    let fresh = AppConfig::load(pool.inner()).await?;
    *config.write().unwrap() = fresh;

    keys.vault_key.lock().unwrap().take();
    keys.folder_keys.lock().unwrap().clear();

    Ok(())
}

#[tauri::command]
#[cfg(debug_assertions)]
pub async fn generate_test_data(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    vdb: tauri::State<'_, VectorDb>,
    config: tauri::State<'_, SharedConfig>,
    note_count: usize,
    folder_count: usize,
    seed: Option<u64>,
    include_daily_notes: bool,
    embed: bool,
) -> AppResult<TestDataSummary> {
    seed_test_vault_inner(
        pool.inner(),
        Some(&vdb.0),
        config.inner(),
        SeedTestVaultParams {
            note_count,
            folder_count,
            seed,
            include_daily_notes,
            embed,
        },
        Some(&app),
    )
    .await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Kanban-ready folder (Status + Priority). Distinct from `seed_notes`' "Kanban Demo".
async fn seed_test_kanban_folder(
    pool: &SqlitePool,
    errors: &mut Vec<String>,
) -> (Option<i64>, Vec<i64>) {
    let folder_id: i64 = match sqlx::query_scalar(
        "INSERT INTO folders (name) VALUES ('Capstone — Project board') RETURNING id",
    )
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("kanban fixture folder: {e}"));
            return (None, Vec::new());
        }
    };

    let status_options = r#"["Todo","In Progress","Review","Done"]"#;
    let status_def_id: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Status', 'select', ?, 0) RETURNING id",
    )
    .bind(folder_id)
    .bind(status_options)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("kanban Status def: {e}"));
            return (None, Vec::new());
        }
    };

    let priority_options = r#"["Low","Medium","High"]"#;
    let priority_def_id: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Priority', 'select', ?, 1) RETURNING id",
    )
    .bind(folder_id)
    .bind(priority_options)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("kanban Priority def: {e}"));
            return (None, Vec::new());
        }
    };

    let notes: &[(&str, &str, &str, &str)] = &[
        (
            "Literature review synthesis",
            "Summarize related work for chapter 2: themes, gaps, and how our contribution differs.",
            "Todo",
            "High",
        ),
        (
            "Research ethics checklist",
            "Confirm consent forms and data retention policy with the faculty advisor.",
            "Todo",
            "Medium",
        ),
        (
            "Prototype survey instrument",
            "Draft Likert-scale questions; pilot with three classmates before wider deployment.",
            "Todo",
            "Low",
        ),
        (
            "Dataset cleaning pipeline",
            "Normalize timestamps, drop duplicates, document exclusions in an appendix.",
            "In Progress",
            "High",
        ),
        (
            "Evaluation harness",
            "Automate baseline comparisons and plot precision/recall curves for each experiment.",
            "In Progress",
            "Medium",
        ),
        (
            "User study recruitment copy",
            "Write recruitment email and eligibility criteria; route through IRB template.",
            "In Progress",
            "Low",
        ),
        (
            "Chapter 3 structural edit",
            "Tighten transitions and move proofs to supplementary material.",
            "Review",
            "High",
        ),
        (
            "Poster draft for symposium",
            "One-page layout with QR code linking to the repo; feedback from lab group.",
            "Review",
            "Medium",
        ),
        (
            "Tooling bootstrap",
            "Repo skeleton, README, and reproducible devcontainer setup.",
            "Done",
            "Low",
        ),
        (
            "Mid-term advisor meeting",
            "Captured milestones through winter break and revised timeline.",
            "Done",
            "Medium",
        ),
    ];

    let mut ids = Vec::with_capacity(notes.len());
    for (title, content, status, priority) in notes {
        let note_id: i64 = match sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(content)
        .bind(folder_id)
        .fetch_one(pool)
        .await
        {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("kanban note '{title}': {e}"));
                continue;
            }
        };
        ids.push(note_id);

        for (def_id, val) in [(status_def_id, status), (priority_def_id, priority)] {
            if let Err(e) = sqlx::query(
                "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
                 ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
            )
            .bind(note_id)
            .bind(def_id)
            .bind(val)
            .execute(pool)
            .await
            {
                errors.push(format!("kanban property nid={note_id}: {e}"));
            }
        }
    }

    (Some(folder_id), ids)
}

/// Weekly recap fixtures: many short history notes plus one detailed recap; wiki-links synced.
async fn seed_test_weekly_review_fixture_folder(
    pool: &SqlitePool,
    errors: &mut Vec<String>,
) -> (Option<i64>, Vec<i64>) {
    const FOLDER_NAME: &str = "Weekly recaps";
    const FLAGSHIP_TITLE: &str = "Weekly review — 12 May";
    // Older stubs: body stays minimal so only the flagship reads "full" in screenshots.
    const STUB_BACKLINK: &str = "[[Weekly review — 12 May]]";
    // Oldest → newest by calendar week (insert order + timestamps below).
    const PRIOR_CHRONOLOGICAL: &[&str] = &[
        "Weekly review — 18 February",
        "Weekly review — 25 February",
        "Weekly review — 3 March",
        "Weekly review — 10 March",
        "Weekly review — 17 March",
        "Weekly review — 24 March",
        "Weekly review — 31 March",
        "Weekly review — 7 April",
        "Weekly review — 14 April",
        "Weekly review — 21 April",
        "Weekly review — 28 April",
        "Weekly review — 5 May",
    ];
    const FLAGSHIP_CONTENT: &str = r#"# Weekly review — 12 May

Snapshot before planning next week. Demo content for screenshots and chat-over-vault QA.

## Wins this week

- Shipped search panel tweaks; fewer noisy hits on short queries.
- Documented the local embedding retry path for Ollama blips.

## Blockers

- Cold GPU benchmarks still jitter — need stable timeouts.
- Windows installer signing blocked on external credentials.

## Next week (top 3)

1. Triage vault reindex edge cases after bulk imports.
2. Polish chat empty state when no model is selected.
3. Draft release notes for the first public build.

## Do not forget

- Answer the design thread before **Friday**.
- Back up the test vault before destructive developer runs.

## Earlier recaps

Threads carried from [[Weekly review — 5 May]], [[Weekly review — 28 April]], [[Weekly review — 21 April]], [[Weekly review — 14 April]], and [[Weekly review — 31 March]].

#review #weekly
"#;

    let folder_id: i64 = match sqlx::query_scalar(
        "INSERT INTO folders (name) VALUES (?) RETURNING id",
    )
    .bind(FOLDER_NAME)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("weekly recap fixture folder: {e}"));
            return (None, Vec::new());
        }
    };

    let mut prior_ids: Vec<i64> = Vec::with_capacity(PRIOR_CHRONOLOGICAL.len());
    for title in PRIOR_CHRONOLOGICAL {
        let note_id: i64 = match sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, '', ?) RETURNING id",
        )
        .bind(title)
        .bind(folder_id)
        .fetch_one(pool)
        .await
        {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("weekly recap stub '{title}': {e}"));
                continue;
            }
        };
        prior_ids.push(note_id);
    }

    let flagship_id: i64 = match sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(FLAGSHIP_TITLE)
    .bind(FLAGSHIP_CONTENT)
    .bind(folder_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("weekly recap flagship: {e}"));
            return (Some(folder_id), prior_ids);
        }
    };

    if let Err(e) =
        crate::commands::tags::sync_note_relations_pool(pool, flagship_id, FLAGSHIP_CONTENT).await
    {
        errors.push(format!("weekly recap flagship relations: {e}"));
    }

    for &pid in &prior_ids {
        if let Err(e) = sqlx::query("UPDATE notes SET content = ? WHERE id = ?")
            .bind(STUB_BACKLINK)
            .bind(pid)
            .execute(pool)
            .await
        {
            errors.push(format!("weekly recap stub body id={pid}: {e}"));
            continue;
        }
        if let Err(e) =
            crate::commands::tags::sync_note_relations_pool(pool, pid, STUB_BACKLINK).await
        {
            errors.push(format!("weekly recap stub relations id={pid}: {e}"));
        }
    }

    // `created_at` defaults to the same second for bulk inserts, so NoteList "Created" sort
    // (desc) ties and keeps DB order; stub `UPDATE`s also make `updated_at` newer than the
    // flagship. Use one-second steps in calendar order so descending sorts list 12 May → … →
    // 18 Feb (newest week first).
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let span = (prior_ids.len() + 1) as i64;
    let base = now_secs.saturating_sub(span + 60);
    for (i, &pid) in prior_ids.iter().enumerate() {
        let ts = base + i as i64;
        if let Err(e) = sqlx::query(
            "UPDATE notes SET created_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(ts)
        .bind(ts)
        .bind(pid)
        .execute(pool)
        .await
        {
            errors.push(format!("weekly recap stub timestamps id={pid}: {e}"));
        }
    }
    let flagship_ts = base + prior_ids.len() as i64;
    if let Err(e) = sqlx::query(
        "UPDATE notes SET created_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(flagship_ts)
    .bind(flagship_ts)
    .bind(flagship_id)
    .execute(pool)
    .await
    {
        errors.push(format!("weekly recap flagship timestamps: {e}"));
    }

    let mut all_ids = prior_ids;
    all_ids.push(flagship_id);
    (Some(folder_id), all_ids)
}

/// Database/table-view folder with one column per property type.
async fn seed_test_database_folder(
    pool: &SqlitePool,
    rng: &mut StdRng,
    errors: &mut Vec<String>,
) -> (Option<i64>, Vec<i64>) {
    let folder_id: i64 = match sqlx::query_scalar(
        "INSERT INTO folders (name) VALUES ('Reading & sources') RETURNING id",
    )
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database fixture folder: {e}"));
            return (None, Vec::new());
        }
    };

    let source_def: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Source', 'text', NULL, 0) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database Source def: {e}"));
            return (None, Vec::new());
        }
    };

    let medium_opts = r#"["Paper","Book","Article","Video"]"#;
    let medium_def: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Medium', 'select', ?, 1) RETURNING id",
    )
    .bind(folder_id)
    .bind(medium_opts)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database Medium def: {e}"));
            return (None, Vec::new());
        }
    };

    let year_def: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Year', 'number', NULL, 2) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database Year def: {e}"));
            return (None, Vec::new());
        }
    };

    let deadline_def: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Deadline', 'date', NULL, 3) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database Deadline def: {e}"));
            return (None, Vec::new());
        }
    };

    let reviewed_def: i64 = match sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Reviewed', 'boolean', NULL, 4) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            errors.push(format!("database Reviewed def: {e}"));
            return (None, Vec::new());
        }
    };

    let rows: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "Attention mechanisms survey",
            "Vaswani et al. lineage and transformer variants",
            "Paper",
            "2017",
            "2026-06-01",
            "true",
        ),
        (
            "Deep Learning textbook — optimisation chapter",
            "Goodfellow et al.; focus on momentum and adaptive rates",
            "Book",
            "2016",
            "2026-05-20",
            "true",
        ),
        (
            "Literature notes — citation graphs",
            "Survey tools for mapping citation networks",
            "Article",
            "2023",
            "2026-07-15",
            "false",
        ),
        (
            "Recorded seminar — retrieval augmented generation",
            "University guest lecture on grounding LLMs",
            "Video",
            "2024",
            "2026-04-30",
            "true",
        ),
        (
            "BERT and masked language modelling",
            "Short technical summary for thesis background",
            "Paper",
            "2019",
            "2026-05-10",
            "false",
        ),
        (
            "Note-taking methodology",
            "Ahrens on evergreen notes (comparison with Zettelkasten)",
            "Book",
            "2017",
            "2026-08-01",
            "false",
        ),
        (
            "SQLite FTS5 reference",
            "Official docs excerpt on external content tables",
            "Article",
            "2025",
            "2026-03-22",
            "true",
        ),
        (
            "Rust async ecosystem overview",
            "Tokio tutorial playlist — chapters on cancellation",
            "Video",
            "2022",
            "2026-06-18",
            "false",
        ),
        (
            "Probabilistic graphical models primer",
            "Koller & Friedman selected sections",
            "Book",
            "2009",
            "2026-09-01",
            "false",
        ),
        (
            "Contrastive learning self-supervised vision",
            "SimCLR paper summary",
            "Paper",
            "2020",
            "2026-05-05",
            "true",
        ),
        (
            "HCI study design checklist",
            "ACM guideline summary for user studies",
            "Article",
            "2021",
            "2026-04-12",
            "true",
        ),
        (
            "GPU profiling workshop",
            "Vendor tooling walkthrough for RDNA",
            "Video",
            "2025",
            "2026-07-01",
            "false",
        ),
        (
            "Information retrieval textbook",
            "Manning et al. — BM25 and evaluation metrics",
            "Book",
            "2008",
            "2026-10-01",
            "false",
        ),
        (
            "LoRA low-rank adaptation",
            "Parameter-efficient fine-tuning summary",
            "Paper",
            "2021",
            "2026-06-12",
            "false",
        ),
        (
            "Accessible UI patterns",
            "Deque article series on focus management",
            "Article",
            "2024",
            "2026-05-28",
            "true",
        ),
        (
            "Column-oriented databases explained",
            "Conference talk on LanceDB-style workloads",
            "Video",
            "2023",
            "2026-08-20",
            "false",
        ),
        (
            "Ethics of synthetic data",
            "ACM Queue long-read",
            "Article",
            "2025",
            "2026-07-30",
            "false",
        ),
        (
            "Classic lexical search survey",
            "Historical overview before neural retrieval",
            "Paper",
            "2015",
            "2026-04-08",
            "true",
        ),
    ];

    let mut ids = Vec::with_capacity(rows.len());
    for (title, source, medium, year, deadline, reviewed_base) in rows {
        let reviewed = if rng.gen_bool(0.15) {
            if *reviewed_base == "true" {
                "false"
            } else {
                "true"
            }
        } else {
            reviewed_base
        };

        let note_id: i64 = match sqlx::query_scalar(
            "INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(format!(
            "# {title}\n\nPrimary source field: **Source**. Medium: {medium}. Target deadline: {deadline}.\n"
        ))
        .bind(folder_id)
        .fetch_one(pool)
        .await
        {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!("database note '{title}': {e}"));
                continue;
            }
        };
        ids.push(note_id);

        let props: [(i64, &str); 5] = [
            (source_def, source),
            (medium_def, medium),
            (year_def, year),
            (deadline_def, deadline),
            (reviewed_def, reviewed),
        ];
        for (def_id, val) in props {
            if let Err(e) = sqlx::query(
                "INSERT INTO note_properties (note_id, def_id, value) VALUES (?, ?, ?)
                 ON CONFLICT(note_id, def_id) DO UPDATE SET value = excluded.value",
            )
            .bind(note_id)
            .bind(def_id)
            .bind(val)
            .execute(pool)
            .await
            {
                errors.push(format!("database property nid={note_id} def={def_id}: {e}"));
            }
        }
    }

    (Some(folder_id), ids)
}

async fn insert_link_if_new(
    pool: &SqlitePool,
    sid: i64,
    tid: i64,
    errors: &mut Vec<String>,
) -> bool {
    match sqlx::query(
        "INSERT OR IGNORE INTO note_links (source_id, target_id) VALUES (?, ?)",
    )
    .bind(sid)
    .bind(tid)
    .execute(pool)
    .await
    {
        Ok(r) => r.rows_affected() > 0,
        Err(e) => {
            errors.push(format!("link {sid}->{tid}: {e}"));
            false
        }
    }
}

fn rand_domain_indices(
    rng: &mut StdRng,
    domains: &[test_data_content::DomainPool],
    count: usize,
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..domains.len()).collect();
    for i in 0..count {
        let j = rng.gen_range(i..indices.len());
        indices.swap(i, j);
    }
    indices.truncate(count);
    indices
}

fn pick_noun(rng: &mut StdRng, pool: &test_data_content::DomainPool) -> String {
    let para = pool.paragraphs[rng.gen_range(0..pool.paragraphs.len())];
    let mut words = para.split_whitespace();
    let w1 = words.next().unwrap_or("Notes");
    let w2 = words.next().unwrap_or("");
    let raw = format!("{w1} {w2}").trim().to_string();
    let clean: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let mut title = String::new();
    for (i, c) in clean.chars().enumerate() {
        if i == 0 {
            title.push(c.to_ascii_uppercase());
        } else {
            title.push(c);
        }
    }
    if title.is_empty() {
        "Notes".to_string()
    } else {
        title
    }
}

fn assemble_note(
    style: &str,
    rng: &mut StdRng,
    pool: &test_data_content::DomainPool,
    current_title: &str,
    same_folder_titles: &[&str],
    all_titles: &[String],
) -> String {
    let n = rng.gen_range(2..=4).min(pool.paragraphs.len());
    let mut picks: Vec<&str> = Vec::with_capacity(n);
    let mut seen: HashSet<usize> = HashSet::new();
    while picks.len() < n {
        let idx = rng.gen_range(0..pool.paragraphs.len());
        if seen.insert(idx) {
            picks.push(pool.paragraphs[idx]);
        }
    }

    let mut link_targets: Vec<&str> = Vec::new();
    let link_count = rng.gen_range(1..=3);
    let other_titles: Vec<&String> = all_titles
        .iter()
        .filter(|t| t.as_str() != current_title)
        .collect();

    for _ in 0..link_count {
        if other_titles.is_empty() {
            break;
        }
        let title_ref = if same_folder_titles.is_empty() {
            other_titles[rng.gen_range(0..other_titles.len())].as_str()
        } else if rng.gen_bool(0.92) {
            same_folder_titles[rng.gen_range(0..same_folder_titles.len())]
        } else {
            other_titles[rng.gen_range(0..other_titles.len())].as_str()
        };
        link_targets.push(title_ref);
    }

    let mut inline_tags: Vec<&str> = Vec::new();
    let tag_count = rng.gen_range(0..=2);
    for _ in 0..tag_count {
        inline_tags.push(pool.tags[rng.gen_range(0..pool.tags.len())]);
    }

    match style {
        "structured" => {
            let mut out = String::new();
            out.push_str(&format!(
                "# {}\n\n",
                picks[0]
                    .split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
            out.push_str("## Overview\n\n");
            out.push_str(picks[0]);
            out.push_str("\n\n## Details\n\n");
            for (i, p) in picks.iter().enumerate().skip(1) {
                out.push_str(&format!("- {}\n", p));
                if i == 1 && !inline_tags.is_empty() {
                    let tags_str = inline_tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    out.push_str(&format!("  Tags: {}\n", tags_str));
                }
            }
            out.push_str("\n## See Also\n\n");
            for lt in &link_targets {
                out.push_str(&format!("- [[{}]]\n", lt));
            }
            if !inline_tags.is_empty() && picks.len() <= 2 {
                let tags_str = inline_tags
                    .iter()
                    .map(|t| format!("#{t}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("\n{}\n", tags_str));
            }
            out
        }
        "prose" => {
            let mut out = String::new();
            for (i, p) in picks.iter().enumerate() {
                out.push_str(p);
                out.push_str("\n\n");
                if i == 0 && !inline_tags.is_empty() {
                    let tags_str = inline_tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    out.push_str(&format!("{}\n\n", tags_str));
                }
            }
            out.push_str("---\n\n**See also:** ");
            let links: Vec<String> = link_targets.iter().map(|t| format!("[[{t}]]")).collect();
            out.push_str(&links.join(", "));
            out.push('\n');
            out
        }
        "bullet" => {
            let mut out = String::new();
            for p in &picks {
                let sentences: Vec<&str> = p.split(". ").collect();
                for (si, s) in sentences.iter().enumerate() {
                    let trimmed = s.trim().trim_end_matches('.');
                    if trimmed.is_empty() {
                        continue;
                    }
                    if si == 0 {
                        out.push_str(&format!("- {trimmed}\n"));
                    } else {
                        out.push_str(&format!("  - {trimmed}\n"));
                    }
                }
            }
            if !inline_tags.is_empty() {
                let tags_str = inline_tags
                    .iter()
                    .map(|t| format!("#{t}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("\n{}\n", tags_str));
            }
            for lt in &link_targets {
                out.push_str(&format!("- \u{2192} [[{}]]\n", lt));
            }
            out
        }
        "journal" => {
            let mut out = String::new();
            let first = picks[0].split('.').next().unwrap_or("").trim();
            out.push_str(&format!(
                "Today I was thinking about {}. ",
                first.to_lowercase()
            ));
            out.push_str(&picks[0].split('.').skip(1).collect::<Vec<_>>().join(". "));
            out.push_str("\n\n");
            for p in picks.iter().skip(1) {
                out.push_str(p);
                out.push_str("\n\n");
            }
            out.push_str("I should write more about this later.\n");
            if !link_targets.is_empty() {
                out.push_str(&format!("\nRelated: [[{}]]\n", link_targets[0]));
            }
            out
        }
        _ => picks.join("\n\n"),
    }
}

// ── Templates ───────────────────────────────────────────────────────────

fn random_template_title(rng: &mut StdRng) -> &'static str {
    const TITLES: &[&str] = &[
        "Meeting Notes",
        "Project Overview",
        "Weekly Review",
        "Book Notes",
        "Research Log",
        "Lecture Notes",
        "Idea Scratchpad",
        "Daily Standup",
    ];
    TITLES[rng.gen_range(0..TITLES.len())]
}

fn random_template_content(rng: &mut StdRng) -> &'static str {
    const CONTENT: &[&str] = &[
        "# Meeting Notes\n\n**Date:** \n**Attendees:** \n\n## Agenda\n- \n\n## Notes\n\n## Action Items\n- [ ] \n",
        "# Project Overview\n\n**Goal:** \n**Deadline:** \n\n## Key Milestones\n- \n\n## Risks\n- \n\n## Resources\n- \n",
        "# Weekly Review\n\n## Wins\n- \n\n## Challenges\n- \n\n## Next Week\n- \n\n## Notes\n",
        "# Book Notes\n\n**Title:** \n**Author:** \n\n## Key Takeaways\n- \n\n## Quotes\n> \n\n## Questions\n- \n",
        "# Research Log\n\n**Topic:** \n**Date:** \n\n## Hypothesis\n\n## Method\n\n## Results\n\n## Next Steps\n",
        "# Lecture Notes\n\n**Course:** \n**Lecturer:** \n**Date:** \n\n## Key Points\n- \n\n## Questions\n- \n\n## Further Reading\n- \n",
        "# Idea Scratchpad\n\n## The Idea\n\n## Why It Matters\n\n## Open Questions\n- \n\n## Related\n- \n",
        "# Daily Standup\n\n**Yesterday:** \n**Today:** \n**Blockers:** \n",
    ];
    CONTENT[rng.gen_range(0..CONTENT.len())]
}

async fn seed_templates(pool: &SqlitePool, rng: &mut StdRng) -> AppResult<Vec<i64>> {
    let count = rng.gen_range(3..=6);
    let mut ids = Vec::with_capacity(count);
    let mut used: HashSet<&str> = HashSet::new();

    for _ in 0..count {
        let name = loop {
            let n = random_template_title(rng);
            if used.insert(n) {
                break n;
            }
        };
        let content = random_template_content(rng);
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO templates (name, title, content) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(name)
        .bind(name)
        .bind(content)
        .fetch_one(pool)
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

async fn seed_folder_properties(pool: &SqlitePool, folder_id: i64) -> AppResult<()> {
    let options = r#"["Todo","In Progress","Review","Done"]"#;
    let _def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Status', 'select', ?, 0) RETURNING id",
    )
    .bind(folder_id)
    .bind(options)
    .fetch_one(pool)
    .await?;
    Ok(())
}

/// Root folder name must match [`crate::commands::calendar`] (`get_or_create_daily_note`).
async fn ensure_daily_notes_folder_id(pool: &SqlitePool, errors: &mut Vec<String>) -> Option<i64> {
    match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM folders WHERE name = 'Daily Notes' AND parent_id IS NULL LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => Some(id),
        Ok(None) => match sqlx::query_scalar::<_, i64>(
            "INSERT INTO folders (name, parent_id) VALUES ('Daily Notes', NULL) RETURNING id",
        )
        .fetch_one(pool)
        .await
        {
            Ok(id) => Some(id),
            Err(e) => {
                errors.push(format!("daily notes folder: {e}"));
                None
            }
        },
        Err(e) => {
            errors.push(format!("daily notes folder lookup: {e}"));
            None
        }
    }
}

async fn seed_daily_notes(pool: &SqlitePool, rng: &mut StdRng, errors: &mut Vec<String>) -> usize {
    let Some(folder_id) = ensure_daily_notes_folder_id(pool, errors).await else {
        return 0;
    };

    let mut count = 0usize;
    let now = chrono::Utc::now();
    for days_ago in 0..120 {
        if !rng.gen_bool(0.5) {
            continue;
        }
        let date = now - chrono::Duration::days(days_ago);
        let date_str = date.format("%Y-%m-%d").to_string();
        let title = date.format("%d-%m-%Y").to_string();

        let entries: &[&str] = &[
            "Reviewed the open tasks and made progress on the top priority.",
            "Spent the morning reading and taking notes. Productive afternoon.",
            "Long walk in the afternoon cleared my head. Jotted down a few new ideas.",
            "Quiet day. Caught up on admin and replied to pending messages.",
            "Had a good conversation about the project direction. Need to follow up.",
            "Focused deep-work session. Made significant progress on the main deliverable.",
            "Took the day off to recharge. Read a novel and cooked a new recipe.",
            "Reviewed notes from the past week. Several patterns emerging.",
            "Team sync went well. Action items captured and delegated.",
            "Spent the evening organising notes and updating links. The vault is getting better.",
            "Wrote a draft of the proposal. Needs revision but the core argument is solid.",
            "Learned something new today and added it to the knowledge base.",
        ];
        let entry = entries[rng.gen_range(0..entries.len())];
        let content = format!("# {date_str}\n\n{entry}\n");

        match sqlx::query("INSERT INTO notes (title, content, folder_id) VALUES (?, ?, ?)")
            .bind(&title)
            .bind(&content)
            .bind(folder_id)
            .execute(pool)
            .await
        {
            Ok(_) => count += 1,
            Err(e) => errors.push(format!("daily note {date_str}: {e}")),
        }
    }
    count
}
