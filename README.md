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