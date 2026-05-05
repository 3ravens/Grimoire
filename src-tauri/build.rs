// Copyright (C) 2026 Wim Palland
//
// This file is part of Grimoire.
//
// Grimoire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Grimoire is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Grimoire. If not, see <https://www.gnu.org/licenses/>.

fn main() {
    // Ensure protoc (Protocol Buffers compiler) is on the PATH for prost-build,
    // which LanceDB's encoding layer requires. If the PROTOC env var is already
    // set or protoc is on the system PATH, this is a no-op.
    #[cfg(target_os = "windows")]
    set_protoc_windows();

    // Tell the linker where to find zim.lib installed via vcpkg.
    // zim-sys emits `cargo:rustc-link-lib=zim` on Windows but NOT the search
    // path (its Windows support is "not tested"). We emit it here instead.
    #[cfg(target_os = "windows")]
    add_vcpkg_libzim_search_path();

    // Copy libzim runtime DLLs next to the compiled binary so the exe can
    // find them without requiring C:\vcpkg\... on the system PATH.
    #[cfg(target_os = "windows")]
    copy_zim_dlls_to_out();

    tauri_build::build()
}

/// Copy the libzim DLLs from vendor-dlls/ into the Cargo output directory so
/// the compiled exe finds them at runtime without a PATH change.
///
/// Cargo sets OUT_DIR to something like `target/debug/build/<pkg>/out/`, but
/// the exe lives in `target/debug/`. We walk up three levels to reach it.
#[cfg(target_os = "windows")]
fn copy_zim_dlls_to_out() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let src_dir = manifest_dir.join("vendor-dlls");
    if !src_dir.exists() {
        return; // vendor-dlls not set up — skip silently
    }

    // OUT_DIR = target/<profile>/build/<pkg>/out — we need target/<profile>/
    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR not set"),
    );
    // out_dir / .. / .. == target/<profile>/
    let bin_dir = out_dir
        .parent().unwrap() // out
        .parent().unwrap() // <pkg>
        .parent().unwrap(); // target/<profile>

    let dlls = [
        "zim-9.dll", "zstd.dll", "liblzma.dll",
        "icudt78.dll", "icuin78.dll", "icuio78.dll", "icutu78.dll", "icuuc78.dll",
    ];
    for dll in &dlls {
        let src = src_dir.join(dll);
        let dst = bin_dir.join(dll);
        if src.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
        // Rerun if the source DLL changes.
        println!("cargo:rerun-if-changed={}", src.display());
    }
}

/// Emit the vcpkg lib directory as a linker search path so `zim.lib` is found.
#[cfg(target_os = "windows")]
fn add_vcpkg_libzim_search_path() {
    let lib_dir = std::path::Path::new(r"C:\vcpkg\installed\x64-windows\lib");
    if lib_dir.exists() {
        println!("cargo:rustc-link-search={}", lib_dir.display());
    }
}

/// On Windows, winget installs protoc under the user's WinGet packages folder.
/// Detect it and set PROTOC so prost-build can find it without the developer
/// having to restart their shell or set the variable manually.
#[cfg(target_os = "windows")]
fn set_protoc_windows() {
    if std::env::var("PROTOC").is_ok() {
        return; // already set — nothing to do
    }

    let local_appdata = match std::env::var("LOCALAPPDATA") {
        Ok(v) => std::path::PathBuf::from(v),
        Err(_) => return,
    };

    let winget_base = local_appdata.join("Microsoft\\WinGet\\Packages");
    let pattern = "Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe";
    let candidate = winget_base.join(pattern).join("bin").join("protoc.exe");

    if candidate.exists() {
        unsafe {
            std::env::set_var("PROTOC", candidate);
        }
    }
}
