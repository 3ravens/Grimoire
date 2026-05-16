# Search quality benchmark

This document describes the **search quality** harness for Grimoire (Phase 4 roadmap). It complements the [performance budget](performance-faq.md) work: it scores **retrieval** (semantic + FTS) against hand-authored queries with known gold notes.

## What it measures

| Lane | Rule |
|------|------|
| **FTS** | For each of 20 cases, the gold note must appear in full-text search results for that case’s unique token query (see `fts_search_inner` in the Rust backend). **100%** pass required when FTS runs. |
| **Semantic** | For each case, the gold note must appear in the **top 3** results of `search_notes_semantic` (same path as in-app RAG retrieval). **≥ 85%** (17/20) pass when Ollama is available and anchors are embedded. |

Cases live in [`src-tauri/search_quality_cases.json`](../src-tauri/search_quality_cases.json) (versioned JSON: `id`, `title`, `fts_query`, `semantic_query`, `body`). Each anchor body contains a **globally unique** token so FTS is unambiguous.

## How to run

From the **`src-tauri`** directory, **debug** profile (same convention as `perf-budget`):

```bash
cd src-tauri
cargo run --bin search-quality
```

Prerequisites for the **full** benchmark (semantic + FTS):

- **Ollama** on `http://localhost:11434` with the vault’s embedding model pulled (default `nomic-embed-text`, overridable via app config before seed or `SEARCH_QUALITY_EMBED_MODEL` in the harness).

The harness:

1. Creates a **temporary** SQLite DB + LanceDB directory.
2. Seeds **200** filler notes via [`seed_test_vault_inner`](../src-tauri/src/test_data.rs) (deterministic seed `42`).
3. Inserts **20** anchor notes, updates `notes_fts`, then embeds anchors into LanceDB (unless offline).
4. Runs FTS + semantic checks and prints a per-case table.

### Environment variables

| Variable | Effect |
|----------|--------|
| `SEARCH_QUALITY_OFFLINE=1` | Seed filler **without** embedding; skip Lance indexing for anchors and **skip the semantic lane** (FTS only). Useful for CI or machines without Ollama. |
| `SEARCH_QUALITY_EMBED_MODEL` | Override embedding model name for the run (same idea as `PERF_EMBED_MODEL`). |
| `SEARCH_QUALITY_CASES_PATH` | Path to a custom JSON file instead of the bundled `search_quality_cases.json`. |
| `SEARCH_QUALITY_VERBOSE=1` | On semantic miss, print top-3 note ids for debugging. |
| `SEARCH_QUALITY_STRICT=1` | Exit with status **1** if thresholds fail (FTS must be 100%; semantic must meet ≥85% when not offline). |

### Strict mode with offline

With `SEARCH_QUALITY_OFFLINE=1`, only **FTS** is enforced under `SEARCH_QUALITY_STRICT=1` (semantic is skipped and treated as satisfied for exit status).

## Windows stack note

The binary runs the async harness on a **16 MiB** worker thread (same pattern as `perf-budget`) to avoid stack overflow when seeding large vaults on Windows.

## Adding or editing cases

1. Edit [`src-tauri/search_quality_cases.json`](../src-tauri/search_quality_cases.json).
2. Keep **`version": 1`** until the schema changes.
3. Keep exactly **20** cases (constants `SEARCH_QUALITY_ANCHOR_COUNT` / `SEMANTIC_TOP3_PASS_MIN` in [`src-tauri/src/search_quality.rs`](../src-tauri/src/search_quality.rs) must stay in sync if you change the count).
4. Each **`fts_query`** should be a **single distinctive token** present verbatim in that case’s `body` (and not reused elsewhere).
5. **`semantic_query`** should be natural language that matches the anchor’s topic **without** copying the FTS token, so the semantic test is independent of lexical grep.

## Privacy

The harness is **local-only**: temp DB, localhost Ollama for embeddings, no telemetry.
