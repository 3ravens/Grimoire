#!/usr/bin/env bash
# Clone microsoft/vcpkg and optionally merge CI cache dirs (see ci.yml actions/cache).
# Cache must NOT live at $VCPKG_ROOT — a partial restore blocks `git clone`.
set -euo pipefail

export VCPKG_ROOT="${VCPKG_ROOT:-${GITHUB_WORKSPACE}/vcpkg}"
VCPKG_CI_CACHE="${VCPKG_CI_CACHE:-${GITHUB_WORKSPACE}/.ci-vcpkg-cache}"

vcpkg_ci_cache_subdirs=(downloads installed buildtrees packages)

vcpkg_restore_ci_cache() {
  [[ -n "${GITHUB_WORKSPACE:-}" ]] || return 0
  local sub src dst
  for sub in "${vcpkg_ci_cache_subdirs[@]}"; do
    src="${VCPKG_CI_CACHE}/${sub}"
    dst="${VCPKG_ROOT}/${sub}"
    if [[ -d "$src" ]]; then
      mkdir -p "$dst"
      cp -a "${src}/." "$dst/"
    fi
  done
}

vcpkg_save_ci_cache() {
  [[ -n "${GITHUB_WORKSPACE:-}" ]] || return 0
  local sub src dst
  for sub in "${vcpkg_ci_cache_subdirs[@]}"; do
    src="${VCPKG_ROOT}/${sub}"
    dst="${VCPKG_CI_CACHE}/${sub}"
    if [[ -d "$src" ]]; then
      mkdir -p "$dst"
      cp -a "${src}/." "$dst/"
    fi
  done
}

if [[ ! -x "${VCPKG_ROOT}/vcpkg" ]]; then
  if [[ -e "${VCPKG_ROOT}" && ! -d "${VCPKG_ROOT}/.git" ]]; then
    rm -rf "${VCPKG_ROOT}"
  fi
  if [[ ! -d "${VCPKG_ROOT}/.git" ]]; then
    git clone --depth 1 https://github.com/microsoft/vcpkg "${VCPKG_ROOT}"
  fi
  "${VCPKG_ROOT}/bootstrap-vcpkg.sh" -disableMetrics
fi

vcpkg_restore_ci_cache
