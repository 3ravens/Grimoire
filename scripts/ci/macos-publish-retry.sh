#!/usr/bin/env bash
# Build macOS Tauri bundles with one retry (hdiutil / bundle_dmg flakes), then upload
# installer artifacts to the draft GitHub release.
#
# Required env:
#   RELEASE_TAG, GH_TOKEN (or GITHUB_TOKEN), BUNDLE_ROOT
# Optional env:
#   TAURI_ARGS (e.g. --target aarch64-apple-darwin)
#   RELEASE_NAME, RELEASE_BODY, RELEASE_PRERELEASE (true/false)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RELEASE_TAG="${RELEASE_TAG:?RELEASE_TAG is required}"
BUNDLE_ROOT="${BUNDLE_ROOT:?BUNDLE_ROOT is required}"
TAURI_ARGS="${TAURI_ARGS:-}"
RELEASE_NAME="${RELEASE_NAME:-Grimoire ${RELEASE_TAG}}"
RELEASE_PRERELEASE="${RELEASE_PRERELEASE:-false}"

if [[ -z "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]]; then
  echo "GH_TOKEN or GITHUB_TOKEN is required" >&2
  exit 1
fi
export GH_TOKEN="${GH_TOKEN:-$GITHUB_TOKEN}"

# shellcheck disable=SC2086
bash scripts/ci/with-retry.sh \
  --attempts 2 \
  --delay 60 \
  --pre-retry 'find src-tauri/target -type f \( -name "*.dmg" -o -name "*.dmg.sha256" \) -delete 2>/dev/null || true' \
  -- npm run tauri -- build ${TAURI_ARGS}

notes_file="$(mktemp)"
list_file="$(mktemp)"
trap 'rm -f "$notes_file" "$list_file"' EXIT

if ! gh release view "$RELEASE_TAG" >/dev/null 2>&1; then
  echo "Creating draft release ${RELEASE_TAG}..."
  if [[ -n "${RELEASE_BODY:-}" ]]; then
    printf '%s\n' "$RELEASE_BODY" >"$notes_file"
  else
    printf '%s\n' "Unsigned installers. See docs/builds-and-installers.md." >"$notes_file"
  fi
  if [[ "$RELEASE_PRERELEASE" == "true" ]]; then
    gh release create "$RELEASE_TAG" \
      --draft \
      --prerelease \
      --title "$RELEASE_NAME" \
      --notes-file "$notes_file"
  else
    gh release create "$RELEASE_TAG" \
      --draft \
      --title "$RELEASE_NAME" \
      --notes-file "$notes_file"
  fi
fi

if [[ ! -d "$BUNDLE_ROOT" ]]; then
  echo "Bundle directory not found: $BUNDLE_ROOT" >&2
  exit 1
fi

find "$BUNDLE_ROOT" -type f \( -name '*.dmg' -o -name '*.app.tar.gz' \) | sort >"$list_file"

if [[ ! -s "$list_file" ]]; then
  echo "No macOS installer artifacts found under $BUNDLE_ROOT" >&2
  exit 1
fi

upload_args=()
while IFS= read -r f; do
  upload_args+=("$f")
done <"$list_file"

echo "Uploading macOS artifacts to ${RELEASE_TAG}:"
printf '%s\n' "${upload_args[@]}"
gh release upload "$RELEASE_TAG" "${upload_args[@]}" --clobber
