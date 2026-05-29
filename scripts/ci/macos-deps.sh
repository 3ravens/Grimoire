#!/usr/bin/env bash
# Homebrew packages for libzim (zim-sys) and protoc (LanceDB) on macOS CI runners.
set -euo pipefail

brew update
brew install libzim protobuf pkg-config

prefix="$(brew --prefix libzim)"
echo "libzim brew prefix: ${prefix}"
echo "libzim version: $(brew list --versions libzim)"

# Homebrew libzim often has no libzim.pc; zim-sys honors LIBZIM_INCLUDE / LIBZIM_LIB.
if [[ -n "${GITHUB_ENV:-}" ]]; then
  echo "LIBZIM_INCLUDE=${prefix}/include" >> "$GITHUB_ENV"
  echo "LIBZIM_LIB=${prefix}/lib" >> "$GITHUB_ENV"
  echo "DYLD_LIBRARY_PATH=${prefix}/lib:${DYLD_LIBRARY_PATH:-}" >> "$GITHUB_ENV"
  echo "RUSTFLAGS=-C link-arg=-Wl,-rpath,${prefix}/lib ${RUSTFLAGS:-}" >> "$GITHUB_ENV"

  paths=()
  [[ -d "${prefix}/lib/pkgconfig" ]] && paths+=("${prefix}/lib/pkgconfig")
  [[ -d /opt/homebrew/lib/pkgconfig ]] && paths+=("/opt/homebrew/lib/pkgconfig")
  [[ -d /usr/local/lib/pkgconfig ]] && paths+=("/usr/local/lib/pkgconfig")
  if [[ ${#paths[@]} -gt 0 ]]; then
    IFS=:
    echo "PKG_CONFIG_PATH=${paths[*]}" >> "$GITHUB_ENV"
  fi
fi
