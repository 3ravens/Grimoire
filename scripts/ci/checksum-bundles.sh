#!/usr/bin/env bash
# Write SHA-256 checksums for installer artifacts under a Tauri bundle directory.
# Avoid mapfile/readarray (bash 4+) so macOS default bash 3.2 works on GitHub runners.
set -euo pipefail

ROOT="${1:-src-tauri/target/release/bundle}"
OUT="${2:-checksums.txt}"

hash_file() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file"
  else
    shasum -a 256 "$file"
  fi
}

if [[ ! -d "$ROOT" ]]; then
  echo "Bundle directory not found: $ROOT" >&2
  exit 1
fi

list_file="$(mktemp)"
trap 'rm -f "$list_file"' EXIT

find "$ROOT" -type f \( \
  -name '*.msi' -o -name '*.exe' -o -name '*.deb' -o -name '*.rpm' -o \
  -name '*.AppImage' -o -name '*.dmg' -o -name '*.app.tar.gz' \
\) | sort >"$list_file"

if [[ ! -s "$list_file" ]]; then
  echo "No installer artifacts found under $ROOT" >&2
  exit 1
fi

{
  echo "# Grimoire release checksums (SHA-256)"
  echo "# Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  while IFS= read -r f; do
    (cd "$(dirname "$f")" && hash_file "$(basename "$f")")
  done <"$list_file"
} >"$OUT"

cat "$OUT"
