# Performance budget harness

Grimoire’s v1.0 performance targets are defined as constants in [`src/perf_budget.rs`](src/perf_budget.rs). The `perf-budget` binary measures a subset of those targets against the same **test vault** shape as Settings → Developer → **Generate test data** ([`src/test_data.rs`](src/test_data.rs)).

**Search quality (retrieval):** run **`cargo run --bin search-quality`** from `src-tauri` for FTS + semantic benchmarks over 20 hand-authored gold notes on top of a 200-note seeded vault. Details: [`../docs/search-quality.md`](../docs/search-quality.md).

The **incremental embed** line uses the **median-sized** note in that vault (by `LENGTH(content)`), but only the **first `PERF_EMBED_BENCH_BODY_CHARS` Unicode scalars** of that body for the timed embed (save latency still rewrites the full note). That cap keeps the harness stable: indexing uses **one sentence per line chunk**, so bullet-heavy median notes can otherwise trigger hundreds of `/api/embed` slices. Override with `PERF_EMBED_BENCH_CHARS` (positive integer). **RAG TTFT** is end-to-end on the real pipeline (including Ollama model unload/reload between query embed and chat where applicable); see `perf_budget.rs` / `docs/performance-faq.md` for why sub-second TTFT is not the v1 target with default models.

## Run

From the `src-tauri` directory (debug profile — do **not** use `--release` for this binary):

```bash
cargo run --bin perf-budget
```

On **Windows**, run from `src-tauri` (there is no `Cargo.toml` in the repo root). The harness uses a **16 MiB** worker-thread stack so the test-data seeder’s large async state machine does not hit `STATUS_STACK_OVERFLOW` on the default 1 MiB main stack.

Prerequisites for **full** results (embed + RAG):

- **Ollama** running on `http://localhost:11434`
- Embedding model available (default `nomic-embed-text`, overridable with `PERF_EMBED_MODEL`)
- Chat model for TTFT (default `llama3.2`, overridable with `PERF_CHAT_MODEL`, e.g. `llama3.2:1b`)

### Environment variables

| Variable | Effect |
|----------|--------|
| `PERF_OFFLINE=1` | Seed without embedding; skip incremental embed and RAG TTFT (CI-friendly). |
| `PERF_SKIP_OLLAMA=1` | Skip RAG TTFT only (still runs embed if seed embedded). |
| `PERF_EMBED_MODEL` | Override embedding model name for the run. |
| `PERF_CHAT_MODEL` | Override chat model for TTFT. |
| `PERF_RAG_QUERY` | Override the fixed semantic query (default asks about fermentation). |
| `PERF_EMBED_BENCH_CHARS` | Override embed-benchmark body cap (default `PERF_EMBED_BENCH_BODY_CHARS` in `perf_budget.rs`). |
| `PERF_BUDGET_STRICT=1` | Exit with status 1 if save / embed / TTFT exceed roadmap targets. |

Warm-up: one discarded iteration for Ollama-backed paths (`OLLAMA_WARMUP_ITERATIONS` in `perf_budget.rs`).

## Developer UI progress

When you use **Settings → Developer → Generate Test Data**, the backend emits **`test_data:progress`** (phase, human-readable message, optional `current` / `total`). The Settings panel subscribes and shows a live status line and progress bar so long embedding runs do not look frozen.

## Cold start

Cold start (&lt; 2 s to interactive UI on reference hardware) is **not** measured by this binary. The frontend sets `window.__GRIMOIRE_PERF_READY__` after the first paint/`show()` sequence in `App.svelte`. Use the Node helper [`../scripts/perf-cold-start.mjs`](../scripts/perf-cold-start.mjs) or a manual stopwatch against that flag (see [`../docs/performance-faq.md`](../docs/performance-faq.md)).

## CI

An optional GitHub Actions workflow (`.github/workflows/perf-advisory.yml`) runs `perf-budget` with `PERF_OFFLINE=1` on `workflow_dispatch` only; it does **not** block merges.

## Dependency reduction audit

Use these commands to measure **Rust** and **frontend** dependency weight before and after trimming `Cargo.toml` / `package.json` (Phase 4 roadmap: dependency reduction audit).

### Rust release binary (`app`)

From `src-tauri`:

```bash
cargo build --release
```

Largest crates in the linked binary (install once: `cargo install cargo-bloat`):

```bash
cargo bloat --release -n 40
```

### Frontend bundle (Vite)

From the repo root:

```bash
npm install
npm run build
```

Optional treemap / sunburst of Rollup output (writes `dist/stats.html`; enable with `ANALYZE=1`):

```bash
set ANALYZE=1
npm run build
```

On Unix: `ANALYZE=1 npm run build`. Open `dist/stats.html` in a browser.

### Before / after log

Re-run the commands after each audit tier and append a short row (date, tier, `cargo bloat` top few lines or total EXE size, main JS chunk size from `dist/assets/*.js`).

| Date | Change | `app` EXE size (release) | Notes |
|------|--------|--------------------------|-------|
| 2026-05-15 | After dependency audit (this pass) | **~129 MB** (`app.exe` ≈ 134.8 MiB on Windows x64) | `cargo build --release` in `src-tauri`. Main Vite chunk `index-*.js` ≈ **409 kB** raw / **126 kB** gzip; graph code split to `Graph-*.js` ≈ **20 kB** raw / **8 kB** gzip (`npm run build`). |
| (optional) | Baseline before audit | — | Re-run the same commands on an older commit if you need a strict before/after row. |
