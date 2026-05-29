#!/usr/bin/env bash
# Homebrew packages for libzim (zim-sys) and protoc (LanceDB) on macOS CI runners.
set -euo pipefail

brew update
brew install libzim protobuf pkg-config

echo "libzim version: $(brew list --versions libzim)"

# Help pkg-config find Homebrew prefixes on Apple Silicon and Intel.
if [[ -n "${GITHUB_ENV:-}" ]]; then
  paths=()
  dylib_paths=()
  [[ -d /opt/homebrew/lib/pkgconfig ]] && paths+=("/opt/homebrew/lib/pkgconfig")
  [[ -d /usr/local/lib/pkgconfig ]] && paths+=("/usr/local/lib/pkgconfig")
  if [[ ${#paths[@]} -gt 0 ]]; then
    IFS=:
    echo "PKG_CONFIG_PATH=${paths[*]}" >> "$GITHUB_ENV"
  fi

  for prefix in /opt/homebrew /usr/local; do
    if [[ -d "${prefix}/opt/libzim/lib" ]]; then
      dylib_paths+=("${prefix}/opt/libzim/lib")
    elif [[ -d "${prefix}/lib" ]]; then
      dylib_paths+=("${prefix}/lib")
    fi
  done
  if [[ ${#dylib_paths[@]} -gt 0 ]]; then
    IFS=:
    echo "DYLD_LIBRARY_PATH=${dylib_paths[*]}:${DYLD_LIBRARY_PATH:-}" >> "$GITHUB_ENV"
  fi
fi
