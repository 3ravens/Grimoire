#!/usr/bin/env bash
# Write SHA-256 checksums for installer artifacts under src-tauri/target/release/bundle.
set -euo pipefail

ROOT="${1:-src-tauri/target/release/bundle}"
OUT="${2:-checksums.txt}"

if [[ ! -d "$ROOT" ]]; then
  echo "Bundle directory not found: $ROOT" >&2
  exit 1
fi

mapfile -t FILES < <(
  find "$ROOT" -type f \( \
    -name '*.msi' -o -name '*.exe' -o -name '*.deb' -o -name '*.rpm' -o \
    -name '*.AppImage' -o -name '*.dmg' -o -name '*.app.tar.gz' \
  \) | sort
)

if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "No installer artifacts found under $ROOT" >&2
  exit 1
fi

{
  echo "# Grimoire release checksums (SHA-256)"
  echo "# Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  for f in "${FILES[@]}"; do
    (cd "$(dirname "$f")" && sha256sum "$(basename "$f")")
  done
} > "$OUT"

cat "$OUT"
