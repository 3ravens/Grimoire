# Releasing Grimoire

Short maintainer checklist. Full packaging details: [`docs/builds-and-installers.md`](../docs/builds-and-installers.md).

## Before you tag

1. Bump version in all three places (must match):
   - [`package.json`](../package.json)
   - [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml)
   - [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json)
2. Run locally: `bash scripts/ci/check-version-sync.sh`
3. License audit is green (also enforced by the `license-audit` CI job):
   - `cargo deny --manifest-path src-tauri/Cargo.toml check licenses`
   - `npm run license:check`
4. Merge to `main` with green **ci** workflow on GitHub (Actions).

## Create a release candidate

**Option A — git tag (normal release):**

```bash
git tag v0.1.0-rc.1
git push origin v0.1.0-rc.1
```

**Option B — manual dispatch (no tag yet):** GitHub → Actions → **release** → Run workflow → enter `v0.1.0-rc.1` as `release_tag`.

Pushing a `v*` tag or running the workflow starts [`.github/workflows/release.yml`](workflows/release.yml). It builds Windows, macOS (Intel + Apple Silicon), and Linux (`.deb`, `.rpm`, `.AppImage`), then creates a **draft** GitHub Release with installers and per-platform `checksums-*.txt` files.

## Review the draft release

Download each artifact and smoke-test:

| Platform | Check |
|----------|--------|
| Windows | Installer runs; Wikipedia reader opens a ZIM; no missing-DLL errors |
| macOS Intel | `.dmg` opens on Intel Mac (or Rosetta if testing on ARM) |
| macOS ARM | `.dmg` opens on Apple Silicon |
| Linux | `.deb` or `.AppImage` launches on a **Wayland session** (not just X11); WebKitGTK present on host |

Confirm **unsigned** OS warnings are acceptable (SmartScreen / Gatekeeper). Verify SHA-256 files match downloaded installers.

Confirm uninstall does **not** delete user data when the Windows NSIS **“Delete app data”** checkbox is **left unchecked** (notes live in `%APPDATA%\com.grimoire.app\grimoire.db`). On Windows, also confirm the uninstaller checkbox label reads **“Permanently delete all notes, settings, and history (cannot be undone)”**; checking it must remove that folder intentionally.

## Publish

1. Edit release notes on GitHub if needed.
2. **Publish** the draft release (remove draft status).
3. Update [grimoireapp.dev/download](https://grimoireapp.dev/download) with links to each platform asset.
4. **Update `version.json` on grimoireapp.dev** to the new version number. This static file
   (`https://grimoireapp.dev/version.json`, shape `{ "version": "x.y.z" }`) is what every
   opted-in client's update check reads. If it is not bumped, users who enabled "Check for
   updates" are never notified of this release. This is automated by the website-repo
   workflow drafted in [`website-version-json.workflow.yml`](website-version-json.workflow.yml)
   (copy it into the website repo); the manual fallback is to edit the file directly.
5. Add a “Last verified” line in `docs/builds-and-installers.md` (date + tag).
6. Mark the Phase 4 cross-platform builds item in your local roadmap/checklist when all assets are validated.

## Release blocked if

- Any matrix job in **release** failed (includes `version-sync` and `license-audit`).
- Windows build logged `release build: missing DLL`.
- Linux release is missing `.deb`, `.rpm`, or `.AppImage`.
- macOS Intel or ARM artifact is missing.
- Checksum files are missing or do not match installers.
- Download page still says “coming soon” after publish.
- `version.json` on grimoireapp.dev was not bumped to the published version.

## Updater signing (required for In-app Update)

Release builds use `bundle.createUpdaterArtifacts` and publish `latest.json` + `.sig` files via
`tauri-action`. Set these GitHub Actions secrets before cutting a release that should support
**Update** on Windows:

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the minisign private key (pair with the pubkey in `src-tauri/tauri.conf.json`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Key password (if any) |
| `WINDOWS_CERTIFICATE` | Optional Base64 `.pfx` for Authenticode |
| `WINDOWS_CERTIFICATE_PASSWORD` | Optional PFX password |

Windows release jobs pass `--features updater-apply` so the app shows **Update**. macOS/Linux stay
notify-only (**View download** / **Not now**) until those platforms are signed for apply.

Notify path remains opt-in `version.json` on grimoireapp.dev (no silent download, no auto-restart).

## Not in this release pipeline

- **Ollama** and models — user installs via wizard or manually.
- **macOS notarization / Linux signed In-app Update** — later per-platform follow-up.
- **Silent / background apply** — never; user must click **Update** and confirm.
