#!/usr/bin/env bash
# Build libzim 9.x into $GITHUB_WORKSPACE/deps/libzim when distro packages are too old (< 9).
# Used by linux-deps.sh after probing pkg-config version.
set -euo pipefail

version=9.6.0
prefix="${GITHUB_WORKSPACE}/deps/libzim"
src="${prefix}/src"

if [[ -f "${prefix}/lib/pkgconfig/libzim.pc" ]]; then
  echo "libzim ${version} already installed at ${prefix}"
  exit 0
fi

sudo apt-get install -y \
  meson \
  ninja-build \
  libicu-dev \
  liblzma-dev \
  libzstd-dev \
  libxapian-dev

mkdir -p "$src"
cd "$src"
if [[ ! -d libzim-${version} ]]; then
  curl -fsSL "https://github.com/openzim/libzim/archive/refs/tags/${version}.tar.gz" \
    | tar xz
fi
cd "libzim-${version}"

meson setup build --prefix="$prefix" -Dstatic=false -Dtests=false
meson compile -C build
meson install -C build

echo "Installed libzim ${version} to ${prefix}"
