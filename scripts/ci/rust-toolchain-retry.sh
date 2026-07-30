#!/usr/bin/env bash
# Install a Rust toolchain via rustup with retries (CDN/DNS flakes on GHA runners).
# Usage: rust-toolchain-retry.sh [toolchain] [target ...]
# Env: RUST_TOOLCHAIN_ATTEMPTS (default 3), RUST_TOOLCHAIN_RETRY_DELAY_SEC (default 30)
set -euo pipefail

toolchain="${1:-stable}"
if [[ $# -gt 0 ]]; then
  shift
fi
targets=("$@")

attempts="${RUST_TOOLCHAIN_ATTEMPTS:-3}"
delay="${RUST_TOOLCHAIN_RETRY_DELAY_SEC:-30}"

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found on PATH" >&2
  exit 1
fi

install_once() {
  rustup toolchain install "$toolchain" --profile minimal
  rustup default "$toolchain"
  local t
  for t in "${targets[@]}"; do
    [[ -z "$t" ]] && continue
    rustup target add "$t" --toolchain "$toolchain"
  done
  rustc -vV
  cargo -vV
}

attempt=1
while true; do
  if install_once; then
    exit 0
  fi
  status=$?
  if (( attempt >= attempts )); then
    echo "rustup toolchain install failed after ${attempts} attempts (toolchain=${toolchain})" >&2
    exit "$status"
  fi
  echo "rustup failed (attempt ${attempt}/${attempts}); retrying in ${delay}s..." >&2
  sleep "$delay"
  ((attempt++)) || true
done
