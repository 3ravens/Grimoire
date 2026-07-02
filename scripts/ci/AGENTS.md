# CI scripts — agent notes

## `linuxdeploy-plugin-gtk.sh`

This file is a **vendored copy** of Tauri's AppImage GTK plugin script, with one Grimoire-specific patch appended at the end.

### What it does

The `release` workflow pre-places this script at `~/.cache/tauri/linuxdeploy-plugin-gtk.sh` **before** `tauri-apps/tauri-action` runs. Tauri's bundler only writes its own copy when that path does not already exist, so our version is used for Linux AppImage builds.

### Why we vendor it

The only Grimoire change is the trailing block that removes bundled `libwayland-*` libraries from the AppDir before the AppImage is packaged. Without this, AppImages built on CI bundle older Wayland client libs that clash with the host compositor on Wayland sessions, causing:

```
Could not create default EGL display: EGL_BAD_PARAMETER.  Aborting...
```

The app already forces `GDK_BACKEND=x11` (see [tauri#8541](https://github.com/tauri-apps/tauri/issues/8541)) and never talks to Wayland directly, so these bundled libs serve no purpose and only introduce ABI-mismatch risk.

### Sync rule (required on Tauri bumps)

Whenever `@tauri-apps/cli` in [`package.json`](../../package.json) is bumped, re-vendor the script body from the matching Tauri tag:

```text
https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v<version>/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy-plugin-gtk.sh
```

Steps:

1. Download the upstream script for the new Tauri version.
2. Replace the body of `scripts/ci/linuxdeploy-plugin-gtk.sh` with that content (verbatim).
3. Re-apply **only** the trailing "Grimoire patch" block (the `libwayland-*` removal loop at the end of the file).
4. Ensure the file remains executable (`chmod +x`).

Current upstream source used: **Tauri v2.10.3** (matches `@tauri-apps/cli@^2.10.1`).

### Safety net

The `Verify AppImage has no bundled Wayland libs` step in [`.github/workflows/release.yml`](../../.github/workflows/release.yml) extracts the built AppImage and fails the build if any `libwayland-*` file remains. A broken or missing patch after a Tauri bump should surface there.
