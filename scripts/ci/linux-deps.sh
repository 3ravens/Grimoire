#!/usr/bin/env bash
# WebKitGTK/Tauri deps via apt; libzim 9.x via vcpkg (matches Windows/macOS CI and zim-sys).
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
  zip \
  unzip \
  tar

bash "$(dirname "$0")/linux-vcpkg.sh"

echo "libzim version: $(pkg-config --modversion libzim)"
pkg-config --libs libzim
