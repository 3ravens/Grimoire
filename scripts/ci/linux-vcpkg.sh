#!/usr/bin/env bash
# Bootstrap vcpkg and install libzim:x64-linux (matches Windows CI / release builds).
set -euo pipefail

# shellcheck source=vcpkg-bootstrap.sh
source "$(dirname "$0")/vcpkg-bootstrap.sh"

bash "$(dirname "$0")/vcpkg-install-retry.sh" "${VCPKG_ROOT}/vcpkg" libzim:x64-linux

vcpkg_save_ci_cache

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
