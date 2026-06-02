#!/usr/bin/env bash
# protoc (LanceDB) via Homebrew; libzim 9.x via vcpkg (matches Windows/Linux CI).
set -euo pipefail

brew update
brew install protobuf pkg-config

bash "$(dirname "$0")/macos-vcpkg.sh"
