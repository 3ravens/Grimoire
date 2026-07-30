#!/usr/bin/env bash
# Retry a command on failure (transient CI flakes: DNS, hdiutil, etc.).
# Usage:
#   with-retry.sh [--attempts N] [--delay SEC] [--pre-retry 'cmd'] -- command [args...]
# Env defaults: ATTEMPTS=3, DELAY_SEC=30
set -euo pipefail

attempts="${WITH_RETRY_ATTEMPTS:-3}"
delay="${WITH_RETRY_DELAY_SEC:-30}"
pre_retry=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --attempts)
      attempts="$2"
      shift 2
      ;;
    --delay)
      delay="$2"
      shift 2
      ;;
    --pre-retry)
      pre_retry="$2"
      shift 2
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
    *)
      break
      ;;
  esac
done

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 [--attempts N] [--delay SEC] [--pre-retry CMD] -- command [args...]" >&2
  exit 2
fi

attempt=1
while true; do
  if "$@"; then
    exit 0
  fi
  status=$?
  if (( attempt >= attempts )); then
    echo "Command failed after ${attempts} attempts: $*" >&2
    exit "$status"
  fi
  echo "Command failed (attempt ${attempt}/${attempts}); retrying in ${delay}s..." >&2
  if [[ -n "$pre_retry" ]]; then
    bash -c "$pre_retry" || true
  fi
  sleep "$delay"
  ((attempt++)) || true
done
