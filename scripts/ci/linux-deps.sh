#!/usr/bin/env bash
# Native packages for Tauri (WebKitGTK) + libzim + protoc on Ubuntu CI runners.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  libfuse2 \
  build-essential \
  libstdc++-12-dev \
  g++ \
  curl \
  wget \
  file \
  libssl-dev \
  pkg-config \
  clang \
  libzim-dev \
  protobuf-compiler

zim_ver="$(pkg-config --modversion libzim)"
echo "libzim version (system): ${zim_ver}"
pkg-config --libs libzim

built_from_source=0
zim_major="${zim_ver%%.*}"
if [[ "${zim_major}" -lt 9 ]]; then
  echo "System libzim ${zim_ver} is older than 9.x; building libzim from source..."
  bash "$(dirname "$0")/linux-libzim-from-source.sh"
  export PKG_CONFIG_PATH="${GITHUB_WORKSPACE}/deps/libzim/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
  built_from_source=1
  zim_ver="$(pkg-config --modversion libzim)"
  echo "libzim version (built): ${zim_ver}"
  pkg-config --libs libzim
fi

libdir="$(pkg-config --variable=libdir libzim)"
incdir="$(pkg-config --variable=includedir libzim)"

# Ensure test/bundle binaries find libzim.so at runtime and embed rpath when linking.
if [[ -n "${GITHUB_ENV:-}" ]]; then
  if [[ "${built_from_source}" -eq 1 ]]; then
    echo "PKG_CONFIG_PATH=${PKG_CONFIG_PATH}" >> "$GITHUB_ENV"
    echo "LIBZIM_INCLUDE=${incdir}" >> "$GITHUB_ENV"
    echo "LIBZIM_LIB=${libdir}" >> "$GITHUB_ENV"
  fi
  echo "LD_LIBRARY_PATH=${libdir}:${LD_LIBRARY_PATH:-}" >> "$GITHUB_ENV"
  echo "RUSTFLAGS=-C link-arg=-Wl,-rpath,${libdir} ${RUSTFLAGS:-}" >> "$GITHUB_ENV"
fi
