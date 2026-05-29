# Bootstrap vcpkg and install libzim for Windows release/CI builds.
# Sets VCPKG_ROOT and CXXFLAGS for the current GitHub Actions job when GITHUB_ENV is set.
$ErrorActionPreference = "Stop"

if (-not $env:VCPKG_ROOT) {
  $env:VCPKG_ROOT = Join-Path $env:GITHUB_WORKSPACE "vcpkg"
}

if (-not (Test-Path (Join-Path $env:VCPKG_ROOT "vcpkg.exe"))) {
  if (-not (Test-Path $env:VCPKG_ROOT)) {
    git clone --depth 1 https://github.com/microsoft/vcpkg $env:VCPKG_ROOT
  }
  & (Join-Path $env:VCPKG_ROOT "bootstrap-vcpkg.bat") -disableMetrics
}

& (Join-Path $env:VCPKG_ROOT "vcpkg.exe") install libzim:x64-windows

$include = Join-Path $env:VCPKG_ROOT "installed\x64-windows\include"
$env:CXXFLAGS = "/I$include"

if ($env:GITHUB_ENV) {
  Add-Content -Path $env:GITHUB_ENV -Value "VCPKG_ROOT=$($env:VCPKG_ROOT)"
  Add-Content -Path $env:GITHUB_ENV -Value "CXXFLAGS=$($env:CXXFLAGS)"
}

Write-Host "VCPKG_ROOT=$($env:VCPKG_ROOT)"
Write-Host "CXXFLAGS=$($env:CXXFLAGS)"
