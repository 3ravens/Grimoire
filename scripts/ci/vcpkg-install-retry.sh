#!/usr/bin/env bash
# Retry vcpkg install on transient download/DNS failures (common on GitHub macOS runners).
# Usage: vcpkg-install-retry.sh /path/to/vcpkg package:triplet [package:triplet ...]
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 /path/to/vcpkg package:triplet [...]" >&2
  exit 2
fi

vcpkg_bin="$1"
shift
packages=("$@")

max_attempts="${VCPKG_INSTALL_ATTEMPTS:-4}"
delay="${VCPKG_INSTALL_RETRY_DELAY_SEC:-45}"

attempt=1
while true; do
  if "${vcpkg_bin}" install "${packages[@]}"; then
    exit 0
  fi
  status=$?
  if (( attempt >= max_attempts )); then
    echo "vcpkg install failed after ${max_attempts} attempts: ${packages[*]}" >&2
    exit "$status"
  fi
  echo "vcpkg install failed (attempt ${attempt}/${max_attempts}); retrying in ${delay}s..." >&2
  sleep "$delay"
  ((attempt++))
done
