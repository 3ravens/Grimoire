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

echo "libzim version: $(pkg-config --modversion libzim)"
pkg-config --libs libzim

# Ensure test/bundle binaries find libzim.so at runtime (not always on default loader path).
if [[ -n "${GITHUB_ENV:-}" ]]; then
  libdir="$(pkg-config --variable=libdir libzim)"
  echo "LD_LIBRARY_PATH=${libdir}:${LD_LIBRARY_PATH:-}" >> "$GITHUB_ENV"
fi
