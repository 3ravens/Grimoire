# Grimoire

A local-first note-taking app with built-in LLM assistance. Everything runs on your machine — no internet required, no data leaves your device.

Your notes work with the LLM immediately — no configuration, no API keys, no cloud accounts, and nothing leaves your machine. Open the app, write a note, ask a question. That's it.

## Stack

| Layer | Technology |
|---|---|
| UI | Svelte + Vite |
| Desktop shell | Tauri |
| Backend | Rust |
| Database | SQLite (sqlx) |
| Vector search | LanceDB |
| LLM runtime | Ollama |

## Development

```bash
npm install
npm run tauri dev
```

## Building installers

Pre-built installers are produced by GitHub Actions when a `v*` tag is pushed (see [`.github/RELEASING.md`](.github/RELEASING.md)). For local packaging, native prerequisites (libzim, WebKitGTK, vcpkg on Windows), commands, and troubleshooting, see **[`docs/builds-and-installers.md`](docs/builds-and-installers.md)**.

```bash
npm run tauri:build   # release bundles under src-tauri/target/release/bundle/
```

## Where app data lives

- SQLite (`grimoire.db`), LanceDB (`lancedb/`), encrypted note content, settings, and migration markers live under the OS app data directory for the bundle id **`com.grimoire.app`** (for example `%APPDATA%\com.grimoire.app` on Windows). Notes are stored in SQLite, not a separate vault folder. Default uninstall preserves this directory; on Windows, checking **Permanently delete all notes, settings, and history** during uninstall removes it.
- Preview builds may have used **`com.tauri.dev`**, **`dev.grimoireapp.grimoire`**, or **`app.grimoire.grimoire`** under the same kinds of paths. On first launch after upgrading the bundle identifier, Grimoire **copies** the database and vector index from a matching preview folder if the new location is still empty; old preview folders are left on disk.

## Tests

- **Rust** (library + `src-tauri/tests/` integration): from `src-tauri`, run `cargo test`. Optional ignored tests (e.g. LanceDB smoke): `cargo test -- --ignored`.
- **JavaScript** (Vitest, `src/**/*.test.js`): from the repo root, run `npm test`. Use `npm run test:watch` while editing.

## Benchmark Baseline (Wikipedia Indexing)

- In **dev builds**, open Settings → **Developer**. Enter a ZIM path, optional max entries cap, then **Run indexing benchmark**. Use **Copy result JSON** to save the run.
- Or call the debug command `benchmark_wikipedia_indexing` from code with the same parameters.
- **Baseline file** [`benchmarks/wikipedia_index_baseline.json`](benchmarks/wikipedia_index_baseline.json) uses **schema v2**: one JSON object with `version`, `captured_at`, `notes`, `degrade_thresholds`, plus **the same fields** as the benchmark output (`model`, `total_entries_in_zim`, `benchmark_entries`, `scanned_entries`, `accepted_articles`, `embedded_articles`, `windows`, `total_ms`, `read_ms`, `parse_ms`, `embed_ms`, `entries_per_sec`, `accepted_per_sec`, `embedded_per_sec`). Optionally wrap a pasted benchmark under `"benchmark": { ... }` instead of merging into the root.
- Compare current vs baseline (`--verbose` prints every field):

```bash
python scripts/compare_wiki_benchmark.py --current path/to/current_benchmark.json
python scripts/compare_wiki_benchmark.py --current path/to/current_benchmark.json --verbose
```

- The script exits with code `1` when configured regression thresholds are exceeded.


## License

Grimoire is free software released under the [GNU General Public License v3.0](LICENSE). You are free to use, modify, and distribute it under the terms of that license.

For commercial use cases that cannot comply with the GPL (e.g. embedding Grimoire in a proprietary product), a separate commercial license is available on request.
