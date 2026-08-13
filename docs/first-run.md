# First-run and installation wizard

User-facing guide to Grimoire’s first launch, installation wizard, and where your data lives. Maintainer packaging details: [`builds-and-installers.md`](./builds-and-installers.md).

**Last verified:** 2026-08-13 — installation wizard polish (guided Ollama setup, skip-AI persistence, empty-vault copy, preview migration WAL copy, backfill tests).

---

## Where app data lives

Notes, settings, the SQLite database, and the LanceDB vector index are stored under the OS app data directory for bundle id **`com.grimoire.app`**:

| OS | Typical path |
|----|----------------|
| **Windows** | `%APPDATA%\com.grimoire.app` (e.g. `C:\Users\<you>\AppData\Roaming\com.grimoire.app`) |
| **macOS** | `~/Library/Application Support/com.grimoire.app` |
| **Linux** | `~/.local/share/com.grimoire.app` or `$XDG_DATA_HOME/com.grimoire.app` |

Inside that folder:

- `grimoire.db` — SQLite database (note bodies, folders, settings, FTS)
- `lancedb/` — semantic search vectors
- `logs/` — optional diagnostic logs
- `app_data_migrated_from.txt` — written once if data was copied from a preview install

Notes are **not** a separate vault folder on disk; they live in SQLite. Wikipedia ZIM files are stored at a path you choose in Settings → Wikipedia (not necessarily under app data).

---

## Uninstall and reinstall

- **Default uninstall** preserves app data. Your notes and settings remain on disk.
- **Windows:** the uninstaller shows an opt-in checkbox: **“Permanently delete all notes, settings, and history (cannot be undone)”**. Only when checked does it remove `%APPDATA%\com.grimoire.app` and `%LOCALAPPDATA%\com.grimoire.app`.
- **Reinstall without deleting data:** Grimoire opens your existing vault; the installation wizard does **not** run again (`wizard_v1_completed` is already set).
- **Reinstall after opting to delete data:** you get a fresh database and the installation wizard runs again.

---

## Preview / development bundle migration

Older preview builds may have used:

- `com.tauri.dev`
- `dev.grimoireapp.grimoire`
- `app.grimoire.grimoire`

On first launch of a release build, if `com.grimoire.app` has no `grimoire.db` yet, Grimoire **copies** the database (including WAL sidecar files when present), `lancedb/`, and optional `logs/` from the first matching legacy folder. **Legacy folders are never deleted.**

If multiple legacy folders exist, the first match in that list wins — data is not merged from two preview installs.

A one-time banner explains that your vault was copied. Dismiss it from the banner or in Settings.

If migration fails, Grimoire logs an error and starts with an empty app data folder; preview data remains in the old location.

---

## Installation wizard flow

The wizard runs once on first launch (unless backfill detects an existing vault — see below). Every step is also available later in **Settings**.

1. **Tour (skippable)** — four short text slides about notes, chat, search, and settings. Skipping the tour still requires choosing a workspace layout.
2. **Workspace starter (required)** — Empty, PKM, Bullet journal, or PARA. Creates local folders/templates only; nothing leaves your machine.
3. **Local AI runtime** — Checks for Ollama. Grimoire does **not** install Ollama for you. Guided steps: download from ollama.com → start the service → **Check again**. **Next** is disabled until Ollama responds or you choose **Continue without AI features**.
4. **Hardware** — RAM/CPU/GPU scan and tier hint (AMD Vulkan note when relevant).
5. **Models** — Curated chat + embedding pulls (skipped if you continued without AI or models are already present).
6. **Wikipedia (optional)** — Enable the reader; actual ZIM download is a separate, user-initiated action in Settings → Wikipedia.

**Continue without AI:** chat and semantic search stay off until you install Ollama, pull models, and configure Settings → LLM (or enable override in Settings → Hardware).

---

## Legacy vault backfill

If you already had notes or folders before the wizard shipped (e.g. upgraded from an early preview), Grimoire marks setup complete automatically and sets starter pack id to `legacy`. The wizard does not appear.

---

## First-run test matrix (maintainers)

| Scenario | Expected |
|----------|----------|
| Fresh install | Wizard appears; empty starter → empty folder list + “create note” hint |
| Skip tour → empty → finish without Ollama | Notes work; chat shows “skipped AI setup” banner |
| Ollama missing on deps step | Numbered guide; Next disabled until check passes or skip AI |
| Ollama OK, models pulled | Models step may be skipped; chat works after finish |
| Reinstall, data preserved | No wizard; existing notes visible |
| Reinstall after Windows “delete all data” | Wizard runs; empty vault |
| Preview `com.tauri.dev` → release `com.grimoire.app` | Data copied; banner once; old folder remains |
| Uninstall, checkbox **unchecked** | `%APPDATA%\com.grimoire.app\grimoire.db` still exists |

---

## Local testing without touching your real vault

Your normal install uses `%APPDATA%\com.grimoire.app` (see above). **Do not delete that folder** to re-test the wizard.

Instead, use the wizard sandbox — it sets `GRIMOIRE_APP_DATA_DIR` to an isolated folder under `scripts/.local-sandboxes/` (gitignored). Your production notes and settings are never read or written.

```powershell
# From repo root — first-run wizard (empty sandbox)
npm run tauri:wizard-sandbox:fresh

# Continue testing in the same sandbox (wizard already completed, etc.)
npm run tauri:wizard-sandbox

# Preview migration banner (fake legacy vault copied into sandbox only)
npm run tauri:wizard-sandbox:migration
```

On startup, the Rust log should include:

`GRIMOIRE_APP_DATA_DIR is set — using isolated app data at ...`

Automated coverage (no UI): `cd src-tauri && cargo test --test wizard_flow --test app_data_migration_flow`

To wipe only the sandbox: delete `scripts/.local-sandboxes/` or run `:fresh` again.

---

## SHIPPED checklist (Phase 4 — Installation wizard)

Verified against current code and the matrix above:

- [x] First-start guide (skippable text slides)
- [x] Workspace starter choice required even when tour skipped
- [x] Dependency check — Ollama detected; guided install (no silent auto-install)
- [x] Hardware detection and tier messaging
- [x] LLM selection (~5 curated + custom id + disclaimer)
- [x] AMD / Vulkan driver hint
- [x] Wikipedia enable without in-wizard download
- [x] Settings parity for all wizard capabilities
- [x] Legacy vault backfill + Windows reinstall/migration edge cases
