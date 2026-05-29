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
  protobuf-compiler \
  git \
  libzim-dev

LIB_DIR="$(pkg-config --variable=libdir libzim)"
echo "libzim version: $(pkg-config --modversion libzim)"
echo "libzim libdir: ${LIB_DIR}"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  echo "LD_LIBRARY_PATH=${LIB_DIR}:${LD_LIBRARY_PATH:-}" >> "$GITHUB_ENV"
fi
