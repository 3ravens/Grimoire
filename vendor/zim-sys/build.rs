#![allow(unused_imports)]
use std::env;
use std::path::{Path, PathBuf};

// *************************************************
// Windows configuration
#[cfg(target_os = "windows")]
fn vcpkg_root() -> PathBuf {
    env::var("VCPKG_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\vcpkg"))
}

#[cfg(target_os = "windows")]
fn configure_lib() {
    let lib_dir = vcpkg_root()
        .join("installed")
        .join("x64-windows")
        .join("lib");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=zim");
}

#[cfg(target_os = "windows")]
fn find_libzim() -> (Vec<PathBuf>, bool) {
    configure_lib();

    let include_dir = env::var("LIBZIM_INCLUDE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            vcpkg_root()
                .join("installed")
                .join("x64-windows")
                .join("include")
        });

    (vec![include_dir], false)
}

// *************************************************
// Real unix configuration starts here

#[cfg(target_family = "unix")]
fn configure_lib(link_path: &PathBuf) {
    println!("cargo:rustc-link-search={}", link_path.display());
}

/// Find libzim binary using pkg_config
#[cfg(target_family = "unix")]
fn probe_pkg_config(include_override: Option<PathBuf>) -> Option<Vec<PathBuf>> {
    let libzim = pkg_config::Config::new().probe("libzim").ok()?;

    let include_paths: Vec<&Path> = libzim.include_paths.iter().map(PathBuf::as_path).collect();
    let link_path = libzim.link_paths.first()?;
    println!("Linking to {link_path:?} and includes {include_paths:?}");

    for path in &libzim.link_paths {
        configure_lib(path);
    }

    let includes = match include_override {
        Some(p) => vec![p],
        None => libzim.include_paths,
    };
    Some(includes)
}

/// Find libzim using env var `LIBZIM_INCLUDE`, `LIBZIM_LIB`.
///
/// Can be used to use a specific library and don't use system one.
#[cfg(target_family = "unix")]
fn find_local_lib(include_override: Option<PathBuf>) -> Result<Vec<PathBuf>, ()> {
    let include_path = include_override.or_else(|| env::var("LIBZIM_INCLUDE").ok().map(PathBuf::from))
        .ok_or(())?;

    let lib_dir: PathBuf = env::var("LIBZIM_LIB").ok().map(PathBuf::from).ok_or(())?;
    configure_lib(&lib_dir);
    Ok(vec![include_path])
}

#[cfg(target_family = "unix")]
fn find_libzim() -> (Vec<PathBuf>, bool) {
    let include_override = env::var("LIBZIM_INCLUDE").ok().map(PathBuf::from);

    // Prefer pkg-config so ICU/zstd/lzma etc. are linked (vcpkg and distro .pc files).
    if let Some(includes) = probe_pkg_config(include_override.clone()) {
        return (includes, true);
    }

    if let Ok(includes) = find_local_lib(include_override) {
        return (includes, false);
    }

    let includes = probe_pkg_config(None).expect(
        "libzim not found: install libzim-dev (Linux), brew/vcpkg libzim (macOS), or set LIBZIM_INCLUDE and LIBZIM_LIB",
    );
    (includes, true)
}

fn main() {
    let (include_dirs, linked_by_pkg_config) = find_libzim();

    let sources = ["src/binding.rs"];
    cxx_build::bridges(sources)
        .file("zim-bind.cc")
        .includes(include_dirs)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-Wno-deprecated-declarations")
        .compile("zim-sys");

    if !linked_by_pkg_config {
        println!("cargo:rustc-link-lib=zim");
    }
    println!("cargo:rerun-if-env-changed=LIBZIM_INCLUDE");
    println!("cargo:rerun-if-env-changed=LIBZIM_LIB");
    println!("cargo:rerun-if-env-changed=LD_LIBRARY_PATH");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-changed=build.rs");
}
