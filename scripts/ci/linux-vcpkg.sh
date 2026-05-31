#!/usr/bin/env bash
# Bootstrap vcpkg and install libzim:x64-linux (matches Windows CI / release builds).
set -euo pipefail

export VCPKG_ROOT="${VCPKG_ROOT:-${GITHUB_WORKSPACE}/vcpkg}"

if [[ ! -x "${VCPKG_ROOT}/vcpkg" ]]; then
  if [[ ! -d "${VCPKG_ROOT}/.git" ]]; then
    git clone --depth 1 https://github.com/microsoft/vcpkg "${VCPKG_ROOT}"
  fi
  "${VCPKG_ROOT}/bootstrap-vcpkg.sh" -disableMetrics
fi

"${VCPKG_ROOT}/vcpkg" install libzim:x64-linux

triplet=x64-linux
include="${VCPKG_ROOT}/installed/${triplet}/include"
lib="${VCPKG_ROOT}/installed/${triplet}/lib"

export CXXFLAGS="-I${include} ${CXXFLAGS:-}"
export LIBZIM_INCLUDE="${include}"
export LIBZIM_LIB="${lib}"
export PKG_CONFIG_PATH="${lib}/pkgconfig:${PKG_CONFIG_PATH:-}"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    echo "VCPKG_ROOT=${VCPKG_ROOT}"
    echo "CXXFLAGS=${CXXFLAGS}"
    echo "LIBZIM_INCLUDE=${LIBZIM_INCLUDE}"
    echo "LIBZIM_LIB=${LIBZIM_LIB}"
    echo "PKG_CONFIG_PATH=${PKG_CONFIG_PATH}"
    echo "LD_LIBRARY_PATH=${lib}:${LD_LIBRARY_PATH:-}"
  } >> "$GITHUB_ENV"
fi

echo "VCPKG_ROOT=${VCPKG_ROOT}"
echo "LIBZIM_INCLUDE=${LIBZIM_INCLUDE}"
echo "LIBZIM_LIB=${LIBZIM_LIB}"
echo "libzim pc: $(pkg-config --modversion libzim)"
pkg-config --libs libzim
