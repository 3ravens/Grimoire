#!/usr/bin/env bash
# Fail if app version strings drift between npm, Cargo, and Tauri config.
set -euo pipefail

PKG=$(node -p "require('./package.json').version")
CARGO=$(awk -F'"' '/^version = / { print $2; exit }' src-tauri/Cargo.toml)
TAURI=$(node -p "require('./src-tauri/tauri.conf.json').version")

echo "package.json:        $PKG"
echo "src-tauri/Cargo.toml: $CARGO"
echo "tauri.conf.json:     $TAURI"

if [[ "$PKG" != "$CARGO" || "$PKG" != "$TAURI" ]]; then
  echo "Version mismatch — align package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json before tagging." >&2
  exit 1
fi

echo "Versions are in sync."
