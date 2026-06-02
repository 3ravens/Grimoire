#!/usr/bin/env bash
# Fail if app version strings drift between npm, Cargo, and Tauri config.
# Optional: --release-tag vX.Y.Z ensures the git tag matches package.json version.
set -euo pipefail

RELEASE_TAG=""
if [[ "${1:-}" == "--release-tag" ]]; then
  RELEASE_TAG="${2:-}"
  if [[ -z "$RELEASE_TAG" ]]; then
    echo "Usage: $0 [--release-tag vX.Y.Z]" >&2
    exit 1
  fi
fi

PKG=$(node -p "require('./package.json').version")
CARGO=$(awk -F'"' '/^version = / { print $2; exit }' src-tauri/Cargo.toml)
TAURI=$(node -p "require('./src-tauri/tauri.conf.json').version")

echo "package.json:        $PKG"
echo "src-tauri/Cargo.toml: $CARGO"
echo "tauri.conf.json:     $TAURI"

MISMATCH=0
if [[ "$PKG" != "$CARGO" ]]; then
  echo "package.json ($PKG) != Cargo.toml ($CARGO)" >&2
  MISMATCH=1
fi
if [[ "$PKG" != "$TAURI" ]]; then
  echo "package.json ($PKG) != tauri.conf.json ($TAURI)" >&2
  MISMATCH=1
fi
if [[ "$CARGO" != "$TAURI" ]]; then
  echo "Cargo.toml ($CARGO) != tauri.conf.json ($TAURI)" >&2
  MISMATCH=1
fi
if [[ "$MISMATCH" -ne 0 ]]; then
  echo "Version mismatch — align package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json before tagging." >&2
  exit 1
fi

if [[ -n "$RELEASE_TAG" ]]; then
  EXPECTED="v${PKG}"
  if [[ "$RELEASE_TAG" != "$EXPECTED" ]]; then
    echo "Release tag '$RELEASE_TAG' does not match expected '$EXPECTED' (from package.json)." >&2
    exit 1
  fi
  echo "Release tag matches app version: $RELEASE_TAG"
fi

echo "Versions are in sync."
