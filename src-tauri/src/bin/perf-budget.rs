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

//! Local performance harness. Run from `src-tauri`:
//! `cargo run --bin perf-budget`
//!
//! Requires a **debug** build profile. See `README-PERF.md`.
//!
//! The Tokio runtime runs on a **16 MiB** stack thread so the large `seed_test_vault_inner`
//! future does not overflow the default Windows main-thread stack (~1 MiB).

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!("perf-budget is only built with debug_assertions (use `cargo run --bin perf-budget` without --release).");
    std::process::exit(2);
}

#[cfg(debug_assertions)]
fn main() {
    // `seed_test_vault_inner` is a large async state machine; polling it on the default
    // Windows main-thread stack (~1 MiB) can overflow. Run the runtime on a bigger stack.
    const STACK: usize = 16 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .name("perf-budget-worker".into())
        .stack_size(STACK)
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            if let Err(e) = rt.block_on(run()) {
                eprintln!("perf-budget failed: {e}");
                std::process::exit(1);
            }
        })
        .expect("spawn perf-budget worker");
    if handle.join().is_err() {
        eprintln!("perf-budget worker thread panicked");
        std::process::exit(1);
    }
}

#[cfg(debug_assertions)]
fn median_u64(xs: &mut [u64]) -> u64 {
    if xs.is_empty() {
        return 0;
    }
    xs.sort_unstable();
    let mid = xs.len() / 2;
    if xs.len() % 2 == 0 {
        (xs[mid - 1] + xs[mid]) / 2
    } else {
        xs[mid]
    }
}

#[cfg(debug_assertions)]
async fn run() -> Result<(), String> {
    use std::sync::{Arc, RwLock};
    use std::time::Instant;

    use app_lib::bench_shared_keystore;
    use app_lib::perf_budget::{
        DEFAULT_BENCHMARK_CHAT_MODEL, INCREMENTAL_EMBED_MS, NOTE_SAVE_MS,
        OLLAMA_WARMUP_ITERATIONS, PERF_EMBED_BENCH_BODY_CHARS, RAG_CHAT_TTFT_MS,
        SAVE_BENCHMARK_ITERATIONS,
    };
    use app_lib::perf_budget_bin::{
        connect_dir, index_note_vectors_for_benchmark, measure_rag_chat_ttft, open_sqlite_file,
        seed_test_vault_inner, save_note_with_version_benchmark_path, AppConfig, SeedTestVaultParams,
    };
    use tempfile::tempdir;

    let dir = tempdir().map_err(|e| e.to_string())?;
    let db_path = dir.path().join("grimoire.db");
    let pool = open_sqlite_file(&db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mut cfg = AppConfig::load(&pool).await.map_err(|e| e.to_string())?;
    if let Ok(m) = std::env::var("PERF_EMBED_MODEL") {
        cfg.embedding_model = m;
    }
    let config = Arc::new(RwLock::new(cfg));

    let lance_dir = dir.path().join("lancedb");
    let conn = connect_dir(&lance_dir)
        .await
        .map_err(|e| e.to_string())?;

    let keys = bench_shared_keystore();

    let offline = std::env::var("PERF_OFFLINE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    eprintln!("Seeding test vault (deterministic seed)…");
    seed_test_vault_inner(
        &pool,
        Some(&conn),
        &config,
        SeedTestVaultParams {
            note_count: 120,
            folder_count: 6,
            seed: Some(42),
            include_daily_notes: true,
            embed: !offline,
        },
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Median-sized note by character length — matches FAQ "typical note" better than
    // the longest notes (domain fixtures can be very large and dominate embed time).
    let row: (i64, String, String) = sqlx::query_as(
        "SELECT id, title, content FROM notes \
         ORDER BY LENGTH(content) ASC \
         LIMIT 1 OFFSET (SELECT (COUNT(*) - 1) / 2 FROM notes)",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let (note_id, title, content) = row;

    let embed_cap = std::env::var("PERF_EMBED_BENCH_CHARS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&c| c > 0)
        .unwrap_or(PERF_EMBED_BENCH_BODY_CHARS);
    let full_chars = content.chars().count();
    let content_for_embed: String = if full_chars <= embed_cap {
        content.clone()
    } else {
        eprintln!(
            "  embed bench: timing first {embed_cap} Unicode scalars of median note ({full_chars} total); list-style bodies get one chunk per sentence — raise PERF_EMBED_BENCH_CHARS to stress-test."
        );
        content.chars().take(embed_cap).collect()
    };

    let mut times: Vec<u64> = Vec::with_capacity(SAVE_BENCHMARK_ITERATIONS);
    for i in 0..SAVE_BENCHMARK_ITERATIONS {
        let body = format!("{content}\n<!--bench {i}-->");
        let t0 = Instant::now();
        save_note_with_version_benchmark_path(&pool, &keys, note_id, &title, &body)
            .await
            .map_err(|e| e.to_string())?;
        times.push(t0.elapsed().as_millis() as u64);
    }
    let max_save = *times.iter().max().unwrap_or(&0);
    let med_save = median_u64(&mut times);
    eprintln!(
        "save_note_with_version+FTS: max={max_save}ms median={med_save}ms (target ≤{NOTE_SAVE_MS}ms)"
    );

    let em = config.read().unwrap().embedding_model.clone();
    let mut embed_ms: u64 = 0;
    if offline {
        eprintln!("PERF_OFFLINE — skipping incremental embed benchmark (requires Ollama).");
    } else {
        for _ in 0..OLLAMA_WARMUP_ITERATIONS {
            index_note_vectors_for_benchmark(&pool, &conn, &em, note_id, &title, &content_for_embed)
                .await
                .map_err(|e| e.to_string())?;
        }
        let t0 = Instant::now();
        index_note_vectors_for_benchmark(&pool, &conn, &em, note_id, &title, &content_for_embed)
            .await
            .map_err(|e| e.to_string())?;
        embed_ms = t0.elapsed().as_millis() as u64;
        eprintln!("incremental re-embed one note: {embed_ms}ms (target ≤{INCREMENTAL_EMBED_MS}ms)");
        if embed_ms > INCREMENTAL_EMBED_MS {
            eprintln!(
                "  Note: the ≤{INCREMENTAL_EMBED_MS}ms embed target is for roadmap reference hardware; your time is model- and GPU/CPU-dependent. See docs/performance-faq.md."
            );
        }
    }

    let chat_model = std::env::var("PERF_CHAT_MODEL")
        .unwrap_or_else(|_| DEFAULT_BENCHMARK_CHAT_MODEL.to_string());
    let query = std::env::var("PERF_RAG_QUERY")
        .unwrap_or_else(|_| "What did I write about fermentation?".to_string());

    let skip_ollama = offline
        || std::env::var("PERF_SKIP_OLLAMA")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    let mut ttft_ms: Option<u64> = None;
    if skip_ollama {
        if offline {
            eprintln!("PERF_OFFLINE — skipping RAG TTFT (no localhost Ollama).");
        } else {
            eprintln!("PERF_SKIP_OLLAMA — skipping RAG TTFT.");
        }
    } else {
        for _ in 0..OLLAMA_WARMUP_ITERATIONS {
            let _ = measure_rag_chat_ttft(&pool, &keys, &conn, &em, &query, &chat_model).await;
        }
        match measure_rag_chat_ttft(&pool, &keys, &conn, &em, &query, &chat_model).await {
            Ok(b) => {
                eprintln!(
                    "RAG TTFT: total={}ms retrieval={}ms note_hits={} (target total ≤{RAG_CHAT_TTFT_MS}ms; retrieval = query embed + LanceDB + excerpts)",
                    b.total_ms_to_first_token, b.retrieval_ms, b.note_match_count
                );
                ttft_ms = Some(b.total_ms_to_first_token);
                if b.total_ms_to_first_token > RAG_CHAT_TTFT_MS {
                    eprintln!(
                        "  Note: TTFT includes Ollama unloading/reloading between embed and chat models (see perf_budget.rs). CPU-only or very large prompts often exceed the reference target; see docs/performance-faq.md."
                    );
                }
            }
            Err(e) => {
                let msg = e.to_string();
                eprintln!("RAG TTFT skipped: {msg}");
                if msg.contains("not found") || msg.contains("404") {
                    eprintln!(
                        "  Hint: `ollama pull {DEFAULT_BENCHMARK_CHAT_MODEL}` or set PERF_CHAT_MODEL to a chat model you already have (e.g. mistral, phi3)."
                    );
                } else {
                    eprintln!("  Hint: ensure Ollama is running on http://localhost:11434 or set PERF_SKIP_OLLAMA=1.");
                }
            }
        }
    }

    let strict = std::env::var("PERF_BUDGET_STRICT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if strict {
        let mut fail = false;
        if max_save > NOTE_SAVE_MS {
            eprintln!("STRICT FAIL: note save max {max_save}ms > {NOTE_SAVE_MS}ms");
            fail = true;
        }
        if embed_ms > INCREMENTAL_EMBED_MS && !offline {
            eprintln!("STRICT FAIL: embed {embed_ms}ms > {INCREMENTAL_EMBED_MS}ms");
            fail = true;
        }
        if let Some(t) = ttft_ms {
            if t > RAG_CHAT_TTFT_MS {
                eprintln!("STRICT FAIL: RAG TTFT {t}ms > {RAG_CHAT_TTFT_MS}ms");
                fail = true;
            }
        }
        if fail {
            return Err("performance budget exceeded (see PERF_BUDGET_STRICT)".into());
        }
    }

    eprintln!("Done. Targets: docs/performance-faq.md — how to run: README-PERF.md");
    Ok(())
}
