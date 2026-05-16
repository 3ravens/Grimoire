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

//! Search quality harness (debug). Run from `src-tauri`:
//! `cargo run --bin search-quality`
//!
//! See `docs/search-quality.md`.

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!(
        "search-quality is only built with debug_assertions (use `cargo run --bin search-quality` without --release)."
    );
    std::process::exit(2);
}

#[cfg(debug_assertions)]
fn main() {
    const STACK: usize = 16 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .name("search-quality-worker".into())
        .stack_size(STACK)
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            if let Err(e) = rt.block_on(run()) {
                eprintln!("search-quality failed: {e}");
                std::process::exit(1);
            }
        })
        .expect("spawn search-quality worker");
    if handle.join().is_err() {
        eprintln!("search-quality worker thread panicked");
        std::process::exit(1);
    }
}

#[cfg(debug_assertions)]
async fn run() -> Result<(), String> {
    use std::sync::{Arc, RwLock};

    use app_lib::bench_shared_keystore;
    use app_lib::search_quality_bin::{
        cases_json_embedded, connect_dir, fts_search_inner, index_note_vectors_for_benchmark,
        insert_anchor_notes, load_cases_from_str, open_sqlite_file, search_notes_semantic,
        seed_test_vault_inner, AppConfig, SeedTestVaultParams, CHUNK_FETCH_LIMIT,
        SEARCH_QUALITY_ANCHOR_COUNT, SEMANTIC_TOP3_PASS_MIN,
    };
    use tempfile::tempdir;

    use app_lib::indexing_profile::{
        init_global, plan_for_tier, tier_from_env, IndexingThroughputTier,
    };
    let tier = tier_from_env().unwrap_or(IndexingThroughputTier::Mid);
    init_global(std::sync::Arc::new(plan_for_tier(tier)));

    let offline = std::env::var("SEARCH_QUALITY_OFFLINE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let verbose = std::env::var("SEARCH_QUALITY_VERBOSE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let strict = std::env::var("SEARCH_QUALITY_STRICT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let cases_json = if let Ok(path) = std::env::var("SEARCH_QUALITY_CASES_PATH") {
        std::fs::read_to_string(&path).map_err(|e| format!("read SEARCH_QUALITY_CASES_PATH: {e}"))?
    } else {
        cases_json_embedded().to_string()
    };

    let file = load_cases_from_str(&cases_json).map_err(|e| e.to_string())?;
    if file.version != 1 {
        return Err(format!("unsupported cases version {}", file.version));
    }
    if file.cases.len() != SEARCH_QUALITY_ANCHOR_COUNT {
        return Err(format!(
            "expected {} cases, got {}",
            SEARCH_QUALITY_ANCHOR_COUNT,
            file.cases.len()
        ));
    }

    let dir = tempdir().map_err(|e| e.to_string())?;
    let db_path = dir.path().join("grimoire.db");
    let pool = open_sqlite_file(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mut cfg = AppConfig::load(&pool).await.map_err(|e| e.to_string())?;
    if let Ok(m) = std::env::var("SEARCH_QUALITY_EMBED_MODEL") {
        cfg.embedding_model = m;
    }
    let config = Arc::new(RwLock::new(cfg));
    let embed_model = config.read().unwrap().embedding_model.clone();

    let lance_dir = dir.path().join("lancedb");
    let conn = connect_dir(&lance_dir)
        .await
        .map_err(|e| e.to_string())?;

    let keys = bench_shared_keystore();

    eprintln!(
        "Seeding filler vault ({} notes, deterministic seed)…",
        200u32
    );
    seed_test_vault_inner(
        &pool,
        Some(&conn),
        &config,
        SeedTestVaultParams {
            note_count: 200,
            folder_count: 6,
            seed: Some(42),
            include_daily_notes: true,
            embed: !offline,
        },
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    eprintln!("Inserting {} anchor notes + FTS…", file.cases.len());
    let gold = insert_anchor_notes(&pool, &file.cases)
        .await
        .map_err(|e| e.to_string())?;

    if !offline {
        eprintln!("Embedding anchor notes into LanceDB ({embed_model})…");
        for c in &file.cases {
            let note_id = gold
                .iter()
                .find(|(id, _)| id == &c.id)
                .map(|(_, nid)| *nid)
                .ok_or_else(|| format!("missing gold id for case {}", c.id))?;
            index_note_vectors_for_benchmark(&pool, &conn, &embed_model, note_id, &c.title, &c.body)
                .await
                .map_err(|e| e.to_string())?;
        }
    } else {
        eprintln!("SEARCH_QUALITY_OFFLINE — anchor notes not embedded; semantic lane skipped.");
    }

    let keys_ref = keys.as_ref();
    let mut fts_pass = 0usize;
    let mut sem_pass = 0usize;

    eprintln!();
    eprintln!("{:-<100}", "");
    eprintln!(
        "{:<28} {:>6} {:>6} {:>6}  {}",
        "case_id", "FTS", "sem", "rank", "notes"
    );
    eprintln!("{:-<100}", "");

    for c in &file.cases {
        let gold_id = gold
            .iter()
            .find(|(id, _)| id == &c.id)
            .map(|(_, nid)| *nid)
            .ok_or_else(|| format!("missing gold id for case {}", c.id))?;

        let fts_ok = fts_search_inner(&pool, keys_ref, c.fts_query.trim(), 32)
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .any(|r| r.note_id == gold_id);
        if fts_ok {
            fts_pass += 1;
        }

        let (sem_ok, rank_str, note) = if offline {
            (true, "-".to_string(), "skipped offline".to_string())
        } else {
            let matches = search_notes_semantic(
                &pool,
                &keys,
                &conn,
                &embed_model,
                c.semantic_query.trim(),
                CHUNK_FETCH_LIMIT,
                false,
            )
            .await
            .map_err(|e| e.to_string())?;

            let top3: Vec<i64> = matches.iter().take(3).map(|m| m.note_id).collect();
            let in_top3 = top3.contains(&gold_id);
            let rank = matches.iter().position(|m| m.note_id == gold_id);
            let rank_str = rank
                .map(|i| format!("{}", i + 1))
                .unwrap_or_else(|| ">5".to_string());
            let note = if in_top3 {
                "ok"
            } else if matches.is_empty() {
                "no semantic results"
            } else {
                "miss top-3"
            };

            if verbose && !in_top3 {
                eprintln!(
                    "  [{}] semantic_query={:?} gold={} top={:?}",
                    c.id, c.semantic_query, gold_id, top3
                );
            }

            if in_top3 {
                sem_pass += 1;
            }
            (in_top3, rank_str, note.to_string())
        };

        eprintln!(
            "{:<28} {:>6} {:>6} {:>6}  {}",
            c.id,
            if fts_ok { "pass" } else { "FAIL" },
            if offline {
                "skip"
            } else if sem_ok {
                "pass"
            } else {
                "FAIL"
            },
            rank_str,
            note
        );
    }

    eprintln!("{:-<100}", "");
    eprintln!(
        "FTS:        {fts_pass}/{} (required {})",
        file.cases.len(),
        file.cases.len()
    );
    if offline {
        eprintln!("Semantic:   skipped (SEARCH_QUALITY_OFFLINE)");
    } else {
        eprintln!(
            "Semantic:   {sem_pass}/{} (required ≥{SEMANTIC_TOP3_PASS_MIN} for 85% top-3)",
            file.cases.len()
        );
    }
    eprintln!(
        "Thresholds: FTS 100%; semantic gold in top-3 for ≥85% of cases when Ollama available."
    );
    eprintln!("Docs: docs/search-quality.md");

    let fts_ok = fts_pass == file.cases.len();
    let sem_ok = offline || sem_pass >= SEMANTIC_TOP3_PASS_MIN;

    if strict && (!fts_ok || !sem_ok) {
        return Err(format!(
            "SEARCH_QUALITY_STRICT: fts_ok={fts_ok} sem_ok={sem_ok} (see output above)"
        ));
    }

    Ok(())
}
