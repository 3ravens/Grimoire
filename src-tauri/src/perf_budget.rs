
//! v1.0 performance budget targets (see project roadmap).
//!
//! Reference profile: roughly a 5-year-old mid-range laptop (Core i5 or Ryzen 5,
//! **8 GB RAM**, **SSD**), with **Ollama already running** and **GPU inference enabled**
//! for both embedding and chat (CPU-only often misses Ollama-backed budgets).
//!
//! Benchmark runs should use a **fixed embedding model** (default app setting:
//! `nomic-embed-text`) and a **small chat model** (default `llama3.2` — the usual
//! `ollama pull llama3.2` tag; override with `PERF_CHAT_MODEL`, e.g. `llama3.2:1b`)
//! so results are comparable across machines. Warm up with one discarded
//! iteration before recording Ollama-backed timings.

/// Maximum time from process start until the main window is shown and the UI
/// has mounted (`window.__GRIMOIRE_PERF_READY__`), milliseconds.
pub const COLD_START_MS: u64 = 2000;

/// Maximum time from submitting a RAG-backed chat until the first streamed
/// assistant token arrives (semantic search: query embed + LanceDB + excerpts,
/// then Ollama chat until first token), milliseconds.
///
/// This is **end-to-end** on the real path: the app unloads competing Ollama
/// runners before query embed and again before chat (VRAM safety), so the chat
/// model cold-starts each turn relative to the embed model. **500 ms was not
/// realistic** with default `nomic-embed-text` + `llama3.2` on the reference
/// profile; **5000 ms** is the v1.0 calibration target for that stack on GPU
/// (stricter goals need a smaller chat model, fused pipeline, or different hardware).
pub const RAG_CHAT_TTFT_MS: u64 = 5000;

/// Maximum time for an explicit save path: SQLite note write + version snapshot
/// rules + FTS upsert + audit (embedding is async and excluded), milliseconds.
pub const NOTE_SAVE_MS: u64 = 100;

/// Maximum time to fully embed and upsert **one** typical note after the vault
/// is warm (chunk → embed → LanceDB), milliseconds.
///
/// Set above naive **2 s** after median-vault-note runs still landed ~2.1–2.3 s
/// on common dev hardware with `nomic-embed-text`; **2500 ms** matches GPU-backed
/// reference intent without treating small overruns as regressions.
pub const INCREMENTAL_EMBED_MS: u64 = 2500;

/// Maximum Unicode scalars (`.chars()`) of note **body** used for `perf-budget`
/// incremental embed timing only. The save benchmark still uses the full note from SQLite.
///
/// Vector indexing uses **one sentence per chunk** (`chunking::chunk_sentences` with
/// `per_chunk = 1`). List-heavy notes can have moderate character counts but hundreds
/// of sentences, which makes Ollama time unstable versus prose of similar length.
pub const PERF_EMBED_BENCH_BODY_CHARS: usize = 5500;

/// Default warmup iterations to discard before measuring Ollama-backed paths.
pub const OLLAMA_WARMUP_ITERATIONS: usize = 1;

/// Default measured iterations for save latency (p99-style uses max of these).
pub const SAVE_BENCHMARK_ITERATIONS: usize = 50;

/// Recommended chat model for TTFT benchmarks (`ollama pull llama3.2`).
/// Override with env `PERF_CHAT_MODEL` (e.g. `llama3.2:1b`, `phi3`, `mistral`).
pub const DEFAULT_BENCHMARK_CHAT_MODEL: &str = "llama3.2";

/// Recommended embedding model (must match settings / Ollama install).
pub const DEFAULT_BENCHMARK_EMBED_MODEL: &str = "nomic-embed-text";
