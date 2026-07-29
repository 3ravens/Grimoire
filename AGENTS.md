# AGENTS.md — Grimoire

Grimoire is a **local-first desktop note app** (Tauri v2 + Svelte 5 + Rust) with built-in chat and RAG over the user's own notes via a local Ollama instance. Everything runs on the user's machine; nothing leaves it.

## Non-negotiable invariants

Violating any of these is a bug, regardless of what the task asks for:

1. **Privacy.** No silent network calls, telemetry, or cloud sync. The only permitted network use is explicit and user-initiated (Ollama on `http://localhost:11434`, and the opt-in Wikipedia/Kiwix ZIM download in settings).
2. **User data safety.** Uninstall/updates must never delete user notes by default. Notes live in SQLite under the OS app data dir for bundle id `com.grimoire.app`. The Windows uninstaller has an explicit opt-in checkbox to delete everything — keep it opt-in.
3. **Locked content must not leak.** Notes in password-locked folders must never appear in FTS, LanceDB, RAG context, chat context, or search results until unlocked and re-indexed. Defense in depth is intentional: content is skipped at index time *and* filtered at read time (`access_filter.rs`, `KeyStore` session semantics). Never remove one layer because the other exists.
4. **Storage boundaries.**
   - SQLite (sqlx): structured metadata, settings, FTS. Note bodies live here (encrypted at rest when protection is on).
   - LanceDB (`src-tauri/src/vector/`): semantic vectors only — no note metadata, no FTS.
   - Ollama: local chat/embedding inference only.
   - Tauri IPC: the only Rust ↔ Svelte boundary.
5. **Index consistency.** Any change that mutates note content, encryption, or folder lock state must keep SQLite FTS and LanceDB aligned (including bulk `reindex_all` paths).
6. If docs conflict with code, follow the code and note the migration intent. Prefer minimal, behavior-preserving diffs.

## Repo map

```
src/                      Svelte 5 frontend
  App.svelte              Root component (large; most layout/orchestration lives here)
  lib/*.svelte            Feature components (Chat, NoteEditor, FolderSidebar, ...)
  lib/services/*.svelte.js  Reactive services (noteService, tabService, ...) — runes in .svelte.js
  lib/stores/*.svelte.js  Small reactive stores (settings, bookmarks, panelLayout)
  lib/settings/           Settings panel sub-pages
  lib/utils/              Pure JS helpers + their Vitest tests (colocated *.test.js)
  lib/styles/             Shared CSS, one file per feature; themes/ and variables.css
src-tauri/                Rust backend (run all cargo commands from here — no root Cargo.toml)
  src/lib.rs              App setup, state, generate_handler! command registration
  src/commands/           All #[tauri::command] handlers, one module per feature
  src/vector/             LanceDB + Ollama embedding layer (embedder, notes, scanned, wiki)
  src/auth.rs, crypto.rs  Argon2id, AES-256-GCM, SQLCipher, zeroize — folder lock crypto
  src/access_filter.rs    Locked-folder filtering for read paths
  src/error.rs            AppError / AppResult (serialized as {kind, message} over IPC)
  src/bin/                perf-budget and search-quality harnesses (debug-only tools)
  migrations/             Numbered forward-only SQLite migrations, run at startup
  tests/                  Rust integration tests (lock/search/RAG/migration flows)
  installer/installer.nsi Custom NSIS template (Windows data-deletion opt-in lives here)
vendor/                   Vendored zim-rs / zim-sys (libzim bindings) — do not review or churn
scripts/ci/               CI helper scripts; has its own AGENTS.md (read it before touching CI)
docs/                     builds-and-installers.md, performance-faq.md, search-quality.md
benchmarks/               Wikipedia indexing baseline JSON (schema v2)
```

## Commands

Run `npm` from the repo root and `cargo` from `src-tauri/`.

| Task | Command |
|---|---|
| Dev app | `npm run tauri dev` |
| Frontend build | `npm run build` |
| Frontend tests (Vitest) | `npm test` (watch: `npm run test:watch`) |
| Frontend build + tests | `npm run check:frontend` |
| Rust tests | `cd src-tauri && cargo test` (ignored/LanceDB smoke: `cargo test -- --ignored`) |
| Release bundles | `npm run tauri:build` |
| Perf harness | `cd src-tauri && cargo run --bin perf-budget` (debug profile only; see `src-tauri/README-PERF.md`) |
| Search quality | `cd src-tauri && cargo run --bin search-quality` (see `docs/search-quality.md`) |
| npm license audit | `npm run license:check` |

Notes:
- CI (`.github/workflows/ci.yml`) runs: frontend build, `npm test`, `cargo build --locked --tests`, `cargo test --locked` on Windows/macOS/Linux, plus version-sync and license audits (`cargo-deny` with `src-tauri/deny.toml`).
- Building the full app requires native deps (libzim, protoc, vcpkg on Windows) — see `docs/builds-and-installers.md`. `cargo test` and `npm test` work without a running Ollama; the perf/search-quality harnesses degrade gracefully via `PERF_OFFLINE=1` / `SEARCH_QUALITY_OFFLINE=1`.
- There is no ESLint/Prettier/rustfmt config; match the style of surrounding code.

## Backend conventions (Rust)

- New IPC handlers: `#[tauri::command] async fn` in the right `src-tauri/src/commands/*.rs` module, re-exported via `commands/mod.rs`, registered in `generate_handler![]` in `lib.rs`.
- Return `AppResult<T>`; pick the right `AppError` variant with an actionable, user-facing message. Never leak raw internal errors to the frontend.
- No `.unwrap()` / `.expect()` on production paths (fine in tests and `#[cfg(test)]`).
- Parameterized SQL only (sqlx binds or `QueryBuilder`); never interpolate strings into SQL.
- Schema changes require a new numbered migration in `src-tauri/migrations/` (forward-only, run at startup). No ad-hoc schema mutation in command handlers. Watch for migrations/triggers that could write plaintext of locked/encrypted notes into FTS.
- Don't hold locks across `.await` points; don't block the async runtime.
- Long-running indexing supports cancellation (`CancelMap`, `FolderUnlockReindexCoordinator`, `VaultReindexGate` in `commands/mod.rs`) — wire new bulk operations into these patterns.

### Search / RAG (highest scrutiny: `commands/rag.rs`, `commands/search.rs`, `vector/`)

Must preserve unless the task is explicitly about changing them:

- Embedding prefixes for nomic-embed-text asymmetric retrieval: `search_query: ` for queries, `search_document: ` for note chunks.
- Hybrid search: FTS fast path plus combined FTS + LanceDB; the semantic half degrades gracefully when Ollama is down.
- FTS is a secondary index: an `fts_upsert` failure must not fail a note save.
- Top-chunk retrieval, best-chunk-per-note ranking, embedding normalization, and degenerate-vector guards.
- Ollama stability mitigations for RDNA4/Vulkan: model eviction before embed (`evict_other_models`), batch split / single-item fallback, and `with_retries` cancellation-aware retry paths.

## Frontend conventions (Svelte 5)

- Use runes (`$state`, `$derived`, `$effect`). Reusable reactive logic goes in `*.svelte.js` services/stores, not components.
- Call Rust via `invoke()` from `@tauri-apps/api/core` and events via `listen()` — there is no IPC wrapper layer; follow the existing direct-call pattern in services.
- Business logic belongs in Rust commands; don't duplicate it in JS.
- Rendering note content from search/RAG must respect lock UI state.
- Sanitize rendered markdown/HTML (`marked` + `dompurify` via `lib/utils/sanitize.js`).
- Persistence is mixed: most data goes through Rust/SQLite, but several UI preferences intentionally live in `localStorage`. Check which path existing code uses before changing it.
- CSS: shared styles in `src/lib/styles/` (one file per feature), design tokens in `variables.css`. Prefer logical properties (`margin-inline-start`) over physical ones for new work; update old physical properties opportunistically, not as churn.
- New `fetch()`/WebSocket to non-local hosts is a privacy violation unless it's a clear user-facing, opt-in feature with a settings entry.

## Testing expectations

- Pure JS helpers get colocated Vitest tests (`src/lib/utils/foo.js` + `foo.test.js`, jsdom environment).
- Backend features get Rust integration tests in `src-tauri/tests/`, especially for lock/search/RAG edge cases. Never delete tests guarding locked-folder leakage or index consistency unless the behavior intentionally changed.
- Before finishing: run `npm test` and, if you touched Rust, `cargo test` from `src-tauri`. If you touched indexing/search, also sanity-check with the search-quality harness when feasible.

## Other agent-facing docs

- `.cursor/rules/*.mdc` — scoped rules (core invariants, Rust backend, SQL migrations, Svelte frontend, CSS/RTL); this file consolidates them but they remain authoritative for their globs.
- `.coderabbit.yaml` — the PR reviewer enforces the same invariants; keep it in sync when conventions change.
- `scripts/ci/AGENTS.md` — required reading before touching CI scripts (vendored linuxdeploy GTK plugin has a sync rule on Tauri bumps).
- `.github/RELEASING.md`, `docs/builds-and-installers.md` — release/tag flow and native build prerequisites.

## Final checklist before finishing any change

- [ ] No privacy regression (no new network behavior that isn't explicit and opt-in).
- [ ] No locked-content leak into FTS, LanceDB, search, chat, or RAG.
- [ ] SQLite and LanceDB stay aligned for any indexing-affecting change.
- [ ] Schema changes have a numbered migration; SQL is parameterized.
- [ ] Relevant tests pass (`npm test`, `cargo test` from `src-tauri`).
