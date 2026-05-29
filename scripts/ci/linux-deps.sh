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
  curl \
  wget \
  file \
  libssl-dev \
  pkg-config \
  clang \
  libzim-dev \
  protobuf-compiler
