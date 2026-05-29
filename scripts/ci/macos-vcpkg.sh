#!/usr/bin/env bash
# Bootstrap vcpkg and install libzim for macOS CI (same 9.x as Windows/Linux vcpkg).
set -euo pipefail

export VCPKG_ROOT="${VCPKG_ROOT:-${GITHUB_WORKSPACE}/vcpkg}"

if [[ ! -x "${VCPKG_ROOT}/vcpkg" ]]; then
  if [[ ! -d "${VCPKG_ROOT}/.git" ]]; then
    git clone --depth 1 https://github.com/microsoft/vcpkg "${VCPKG_ROOT}"
  fi
  "${VCPKG_ROOT}/bootstrap-vcpkg.sh" -disableMetrics
fi

arch="$(uname -m)"
if [[ "${arch}" == arm64 ]]; then
  triplet=arm64-osx
else
  triplet=x64-osx
fi

"${VCPKG_ROOT}/vcpkg" install "libzim:${triplet}"

include="${VCPKG_ROOT}/installed/${triplet}/include"
lib="${VCPKG_ROOT}/installed/${triplet}/lib"

export CXXFLAGS="-I${include} ${CXXFLAGS:-}"
export LIBZIM_INCLUDE="${include}"
export LIBZIM_LIB="${lib}"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    echo "VCPKG_ROOT=${VCPKG_ROOT}"
    echo "CXXFLAGS=${CXXFLAGS}"
    echo "LIBZIM_INCLUDE=${LIBZIM_INCLUDE}"
    echo "LIBZIM_LIB=${LIBZIM_LIB}"
    echo "DYLD_LIBRARY_PATH=${lib}:${DYLD_LIBRARY_PATH:-}"
  } >> "$GITHUB_ENV"
fi

echo "VCPKG triplet=${triplet}"
echo "LIBZIM_INCLUDE=${LIBZIM_INCLUDE}"
echo "LIBZIM_LIB=${LIBZIM_LIB}"
