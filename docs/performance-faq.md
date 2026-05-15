# Performance FAQ

This page documents what Grimoire means by “performance budget” for v1.0 and how numbers are produced. It is intended for [grimoireapp.dev](https://grimoireapp.dev) and can be copied verbatim into the site’s FAQ section.

## What we measure

| Area | Target (reference hardware) | What is included |
|------|-----------------------------|------------------|
| Cold start | Under 2 seconds | From process launch until the UI is usable: the shell loads, the WebView runs the bundled frontend, and `window.__GRIMOIRE_PERF_READY__` is set after the window is shown. **Ollama is assumed already running**; model download time is not part of this budget. |
| RAG chat (time to first token) | Under 5 seconds | From the moment a RAG-backed chat is submitted: semantic search (embedding the query + LanceDB retrieval + SQLite title/decrypt for excerpts) **plus** streaming from the local Ollama `/api/chat` endpoint until the **first non-empty** assistant token. Grimoire keeps **one** Ollama model loaded at a time on many GPUs, so this path includes **unloading the chat model before query embed** and loading it again for generation — budgets assume **GPU-backed** Ollama on the reference machine. |
| Note save | Under 100 ms | The same path as **Ctrl+S**: SQLite write with version snapshot rules, FTS upsert for unlocked notes, and persistence. **Embedding runs asynchronously** and is **not** part of this budget. |
| Incremental embed (one note) | Under 2.5 seconds | Re-indexing a **single** typical note after the system is warm: sentence chunking, embedding all chunks, LanceDB upsert. The `perf-budget` harness uses the **median-sized** note in the generated vault (by character length) but **caps how much of that body is timed** (see `PERF_EMBED_BENCH_BODY_CHARS` in `perf_budget.rs`): list-style notes can have one chunk per short line, so uncapped median-by-char notes made wall time unstable. Dominated by Ollama and GPU/CPU; model choice matters. |

## Reference hardware

Targets are stated for roughly a **five-year-old mid-range laptop**: Core i5 or Ryzen 5 class CPU, **8 GB RAM**, **SSD**, with Ollama already running and **both** the default embedding model (`nomic-embed-text`) and default chat model (`llama3.2`) running on **GPU** (integrated or discrete). Pure CPU inference is expected to exceed the Ollama-backed rows while the app remains usable — the in-app **hardware** settings explain LLM defaults.

## How to reproduce (developers)

1. Install Ollama and pull the models you want to test (defaults: `nomic-embed-text`, `llama3.2` via `ollama pull llama3.2`, or set `PERF_EMBED_MODEL` / `PERF_CHAT_MODEL` — e.g. `llama3.2:1b` if you prefer the smaller tag).
2. From the **`src-tauri`** directory (the workspace root has no `Cargo.toml`; `cargo run` must be run from `src-tauri`), run:

   ```bash
   cd src-tauri
   cargo run --bin perf-budget
   ```

3. For machines without Ollama (e.g. CI smoke), run:

   ```bash
   PERF_OFFLINE=1 cargo run --bin perf-budget
   ```

   That run still measures **note save** latency against a generated vault; embed and RAG steps are skipped.

4. Optional strict gate (fails the process if a measured metric exceeds the constant target):

   ```bash
   PERF_BUDGET_STRICT=1 cargo run --bin perf-budget
   ```

Details: [`src-tauri/README-PERF.md`](../src-tauri/README-PERF.md).

## Cold start measurement

The `perf-budget` binary does **not** launch the full Tauri window. For cold start, use a stopwatch or automation against a **debug** build and wait until `window.__GRIMOIRE_PERF_READY__ === true` in the WebView console (set in `App.svelte` after `show()`). See [`scripts/perf-cold-start.mjs`](../scripts/perf-cold-start.mjs) for a scripted outline (optional Playwright).

## Privacy

All benchmarks are **local-only**. The harness talks to **localhost** Ollama; no telemetry or cloud endpoints are used.
