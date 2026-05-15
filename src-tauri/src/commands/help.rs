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

//! User-initiated links to the public website (help, bug reports).

use std::fmt::Write as _;

use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;
use crate::AppResult;

/// Public bug report page. Query keys are stable for the site form:
/// `version`, `os`, `arch`, `app` (bundle identifier), `name` (product display name).
const BUG_REPORT_PAGE: &str = "https://grimoireapp.dev/bug-report";

/// Hosts the UI may ask the shell browser to open (user-initiated only).
fn is_allowlisted_public_https(url: &str) -> bool {
    const PREFIXES: &[&str] = &["https://grimoireapp.dev", "https://docs.grimoireapp.dev"];
    for p in PREFIXES {
        if let Some(rest) = url.strip_prefix(p) {
            let ok = rest.is_empty()
                || rest.starts_with('/')
                || rest.starts_with('?')
                || rest.starts_with('#');
            if ok {
                return true;
            }
        }
    }
    false
}

fn open_https_in_browser(
    app_handle: &tauri::AppHandle,
    url: String,
) -> AppResult<()> {
    app_handle
        .opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

/// Opens a known public Grimoire website URL in the default browser.
/// Restricted to `grimoireapp.dev` and `docs.grimoireapp.dev` so arbitrary URLs
/// cannot be opened from the frontend.
#[tauri::command]
pub fn open_external_url(app_handle: tauri::AppHandle, url: String) -> AppResult<()> {
    let url = url.trim();
    if !is_allowlisted_public_https(url) {
        return Err(AppError::InvalidInput(format!(
            "URL is not an allowed public site: {url}"
        )));
    }
    open_https_in_browser(&app_handle, url.to_string())
}

/// RFC 3986 query value encoding (percent-encode every byte outside unreserved).
fn encode_query_value(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char)
            }
            _ => write!(&mut out, "%{b:02X}").expect("fmt to String"),
        }
    }
    out
}

/// Opens the Grimoire bug report page in the system browser with build metadata
/// in the query string so the web form can pre-fill fields. No note content or
/// logs are sent — only what is encoded in the URL the user explicitly opens.
#[tauri::command]
pub fn open_bug_report(app_handle: tauri::AppHandle) -> AppResult<()> {
    let pkg = app_handle.package_info();
    let version = pkg.version.to_string();
    let cfg = app_handle.config();
    let name = cfg
        .product_name
        .clone()
        .unwrap_or_else(|| pkg.name.to_string());
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let app_id = cfg.identifier.as_str().to_string();

    let url = format!(
        "{}?version={}&os={}&arch={}&app={}&name={}",
        BUG_REPORT_PAGE,
        encode_query_value(&version),
        encode_query_value(os),
        encode_query_value(arch),
        encode_query_value(&app_id),
        encode_query_value(&name),
    );

    open_https_in_browser(&app_handle, url)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{encode_query_value, is_allowlisted_public_https};

    #[test]
    fn encodes_space_and_plus() {
        assert_eq!(encode_query_value("1.0 beta"), "1.0%20beta");
        assert_eq!(encode_query_value("a+b"), "a%2Bb");
    }

    #[test]
    fn allowlist_accepts_main_and_docs() {
        assert!(is_allowlisted_public_https("https://grimoireapp.dev"));
        assert!(is_allowlisted_public_https("https://grimoireapp.dev/forum"));
        assert!(is_allowlisted_public_https("https://docs.grimoireapp.dev"));
        assert!(is_allowlisted_public_https("https://docs.grimoireapp.dev/getting-started"));
    }

    #[test]
    fn allowlist_rejects_subdomain_spoof() {
        assert!(!is_allowlisted_public_https("https://grimoireapp.dev.attacker.com"));
    }
}
