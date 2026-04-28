#![allow(unused_imports)]
use std::env;
use std::path::{Path, PathBuf};

// *************************************************
// Damn simple configuration if we are on Windows (not tested)
#[cfg(not(target_family = "unix"))]
fn configure_lib() {
    println!("cargo:rustc-link-lib=zim");
}

#[cfg(not(target_family = "unix"))]
fn find_libzim() -> Vec<PathBuf> {
    configure_lib();
    vec![]
}

// *************************************************
// Real unix configuration starts here

#[cfg(target_family = "unix")]
fn configure_lib(link_path: &PathBuf) {
    println!("cargo:rustc-link-search={}", link_path.display());
}

/// Find libzim binary using pkg_config
#[cfg(target_family = "unix")]
fn find_system_lib() -> Vec<PathBuf> {
    let libzim = pkg_config::probe_library("libzim").unwrap();

    let include_paths: Vec<&Path> = libzim.include_paths.iter().map(PathBuf::as_path).collect();
    let link_path = &libzim.link_paths[0];
    println!("Linking to {link_path:?} and includes {include_paths:?}");

    configure_lib(link_path);
    libzim.include_paths
}

/// Find libzim using env var `LIBZIM_INCLUDE`, `LIBZIM_LIB`.
///
/// Can be used to use a specific library and don't use system one.
#[cfg(target_family = "unix")]
fn find_local_lib() -> Result<Vec<PathBuf>, ()> {
    let include_path: PathBuf = if let Ok(p) = env::var("LIBZIM_INCLUDE") {
        p.into()
    } else {
        return Err(());
    };

    let lib_dir: PathBuf = if let Ok(p) = env::var("LIBZIM_LIB") {
        p.into()
    } else {
        return Err(());
    };
    configure_lib(&lib_dir);
    Ok(vec![include_path])
}

#[cfg(target_family = "unix")]
fn find_libzim() -> Vec<PathBuf> {
    match find_local_lib() {
        Ok(p) => return p,
        Err(_) => find_system_lib(),
    }
}

fn main() {
    let include_dirs = find_libzim();

    let sources = ["src/binding.rs"];
    cxx_build::bridges(sources)
        .file("zim-bind.cc")
        .includes(include_dirs)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-Wno-deprecated-declarations")
        .compile("zim-sys");

    println!("cargo:rustc-link-lib=zim"); // if this doesn't go after cxx_build.(...).compile() we get a link error
    println!("cargo:rerun-if-env-changed=LIBZIM_INCLUDE");
    println!("cargo:rerun-if-env-changed=LIBZIM_LIB");
    println!("cargo:rerun-if-env-changed=LD_LIBRARY_PATH");
    println!("cargo:rerun-if-changed=build.rs");
}
