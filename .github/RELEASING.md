# Releasing Grimoire

Short maintainer checklist. Full packaging details: [`docs/builds-and-installers.md`](../docs/builds-and-installers.md).

## Before you tag

1. Bump version in all three places (must match):
   - [`package.json`](../package.json)
   - [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml)
   - [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json)
2. Run locally: `bash scripts/ci/check-version-sync.sh`
3. Merge to `main` with green **ci** workflow on GitHub (Actions).

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
| Linux | `.deb` or `.AppImage` launches; WebKitGTK present on host |

Confirm **unsigned** OS warnings are acceptable (SmartScreen / Gatekeeper). Verify SHA-256 files match downloaded installers.

Confirm uninstall does **not** delete the user’s note vault folder.

## Publish

1. Edit release notes on GitHub if needed.
2. **Publish** the draft release (remove draft status).
3. Update [grimoireapp.dev/download](https://grimoireapp.dev/download) with links to each platform asset.
4. Add a “Last verified” line in `docs/builds-and-installers.md` (date + tag).
5. Mark the Phase 4 cross-platform builds item in your local roadmap/checklist when all assets are validated.

## Release blocked if

- Any matrix job in **release** failed.
- Windows build logged `release build: missing DLL`.
- Linux release is missing `.deb`, `.rpm`, or `.AppImage`.
- macOS Intel or ARM artifact is missing.
- Checksum files are missing or do not match installers.
- Download page still says “coming soon” after publish.

## Not in this release pipeline

- **Ollama** and models — user installs via wizard or manually.
- **Code signing / notarization** — follow-up hardening; see builds doc.
- **Auto-update** — separate Phase 4 roadmap item.
