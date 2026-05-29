#!/usr/bin/env bash
# Homebrew packages for libzim (zim-sys) and protoc (LanceDB) on macOS CI runners.
set -euo pipefail

brew update
brew install libzim protobuf pkg-config

# Help pkg-config find Homebrew prefixes on Apple Silicon and Intel.
if [[ -n "${GITHUB_ENV:-}" ]]; then
  paths=()
  [[ -d /opt/homebrew/lib/pkgconfig ]] && paths+=("/opt/homebrew/lib/pkgconfig")
  [[ -d /usr/local/lib/pkgconfig ]] && paths+=("/usr/local/lib/pkgconfig")
  if [[ ${#paths[@]} -gt 0 ]]; then
    IFS=:
    echo "PKG_CONFIG_PATH=${paths[*]}" >> "$GITHUB_ENV"
  fi
fi
