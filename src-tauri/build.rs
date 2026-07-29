fn main() {
    #[cfg(target_os = "windows")]
    if let Err(e) = copy_zim_dlls_to_out() {
        panic!(
            "Windows DLL staging failed (nsis-dll-staging / vendor-dlls copy): {e}"
        );
    }

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

    tauri_build::build()
}

/// DLL names listed in `tauri.conf.json` → `bundle.resources` (NSIS staging).
/// Tauri validates these paths during `cargo build` / `cargo check`; they must exist.
#[cfg(target_os = "windows")]
const NSIS_STAGED_DLLS: &[&str] = &[
    "zim-9.dll",
    "zstd.dll",
    "liblzma.dll",
    "icudt78.dll",
    "icuin78.dll",
    "icuio78.dll",
    "icutu78.dll",
    "icuuc78.dll",
];

/// `installed/<triplet>/bin` for runtime DLLs (same tree as `add_vcpkg_libzim_search_path` lib dir).
#[cfg(target_os = "windows")]
fn vcpkg_root() -> std::path::PathBuf {
    std::env::var("VCPKG_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(r"C:\vcpkg"))
}

#[cfg(target_os = "windows")]
fn vcpkg_installed_bin_dir() -> std::path::PathBuf {
    vcpkg_root()
        .join("installed")
        .join("x64-windows")
        .join("bin")
}

/// Copy the libzim DLLs from vendor-dlls/ into the Cargo output directory so
/// the compiled exe finds them at runtime without a PATH change.
///
/// Cargo sets OUT_DIR to something like `target/debug/build/<pkg>/out/`, but
/// the exe lives in `target/debug/`. We walk up three levels to reach it.
#[cfg(target_os = "windows")]
fn copy_zim_dlls_to_out() -> std::io::Result<()> {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let src_dir = manifest_dir.join("vendor-dlls");

    // NSIS bundles do not auto-include `target/<profile>/*.dll` (WiX/MSI does).
    // Stage copies here so `tauri.conf.json > bundle.resources` can list them.
    let nsis_staging = manifest_dir.join("nsis-dll-staging");
    match std::fs::remove_dir_all(&nsis_staging) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(std::io::Error::new(
                e.kind(),
                format!("remove_dir_all({}): {e}", nsis_staging.display()),
            ));
        }
    }
    std::fs::create_dir_all(&nsis_staging).map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("create_dir_all({}): {e}", nsis_staging.display()),
        )
    })?;

    // OUT_DIR = target/<profile>/build/<pkg>/out — we need target/<profile>/
    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR not set"),
    );
    // out_dir / .. / .. / .. == target/<profile>/
    let bin_dir = out_dir
        .parent().unwrap() // out
        .parent().unwrap() // <pkg>
        .parent().unwrap(); // target/<profile>

    let vcpkg_bin = vcpkg_installed_bin_dir();
    let vcpkg_bin_ok = vcpkg_bin.is_dir();

    if !src_dir.is_dir() {
        println!(
            "cargo:warning=Missing {} — optional: mirror DLLs from vcpkg (see README there). Trying {} next for `cargo test` runtime.",
            src_dir.display(),
            vcpkg_bin.display()
        );
    }

    for dll in NSIS_STAGED_DLLS {
        let vendor_src = src_dir.join(dll);
        let vcpkg_src = vcpkg_bin.join(dll);

        let resolved: Option<(&std::path::Path, bool)> = if vendor_src.is_file() {
            Some((vendor_src.as_path(), true))
        } else if vcpkg_bin_ok && vcpkg_src.is_file() {
            if src_dir.is_dir() && !vendor_src.is_file() {
                println!(
                    "cargo:warning=Using `{}` from vcpkg bin for local runs ({} has no `{}`).",
                    dll,
                    src_dir.display(),
                    dll
                );
            }
            Some((vcpkg_src.as_path(), false))
        } else {
            None
        };

        if let Some((src, from_vendor)) = resolved {
            let dst = bin_dir.join(dll);
            std::fs::copy(src, &dst).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("copy {} -> {}: {e}", src.display(), dst.display()),
                )
            })?;
            let nsis_dst = nsis_staging.join(dll);
            std::fs::copy(src, &nsis_dst).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("copy {} -> {}: {e}", src.display(), nsis_dst.display()),
                )
            })?;
            if from_vendor {
                println!("cargo:rerun-if-changed={}", vendor_src.display());
            } else {
                println!("cargo:rerun-if-changed={}", src.display());
            }
        } else {
            if std::env::var("PROFILE").unwrap_or_default() == "release" {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "release build: missing DLL `{dll}` (checked {} and {}). \
Place real DLLs in vendor-dlls or vcpkg installed/x64-windows/bin before bundling.",
                        src_dir.display(),
                        vcpkg_bin.display(),
                    ),
                ));
            }
            println!(
                "cargo:warning=No `{dll}` in {} or {} — writing zero-byte file only under {} so Tauri resource paths resolve. `cargo test` / runtime need a real DLL in {} or PATH.",
                src_dir.display(),
                vcpkg_bin.display(),
                nsis_staging.display(),
                bin_dir.display()
            );
            let staged = nsis_staging.join(dll);
            std::fs::write(&staged, []).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("write placeholder {}: {e}", staged.display()),
                )
            })?;
        }
    }

    Ok(())
}

/// Emit the vcpkg lib directory as a linker search path so `zim.lib` is found.
#[cfg(target_os = "windows")]
fn add_vcpkg_libzim_search_path() {
    let lib_dir = vcpkg_root().join("installed").join("x64-windows").join("lib");
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
