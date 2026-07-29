//! Opt-in, notify-only update check.
//!
//! Privacy: this command performs **no network request** unless the user has
//! explicitly enabled the `update_check_enabled` setting (default off). When
//! enabled it fetches a tiny static manifest (`{ "version": "x.y.z" }`) from the
//! Grimoire site and compares it against the running app version. Only the
//! request itself leaves the machine — no telemetry, no identifiers. The check
//! is recorded in the audit log so the user can verify the behaviour.
//!
//! There is no silent in-app apply: the UI surfaces a banner/badge linking to
//! the download page (deferred per the Phase 4 scope decision).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::audit;
use crate::error::AppError;
use crate::AppResult;

/// Static version manifest published by the release process. Stays on the
/// already-allowlisted `grimoireapp.dev` host (see `commands::help`).
const VERSION_ENDPOINT: &str = "https://grimoireapp.dev/version.json";

/// Public download page the banner links to (opened via `open_external_url`).
const DOWNLOAD_URL: &str = "https://grimoireapp.dev/download";

/// Setting key (text "true"/"false") gating whether the check may run at all.
const SETTING_UPDATE_CHECK_ENABLED: &str = "update_check_enabled";

/// Shape of the remote manifest. Extra fields are ignored.
#[derive(Debug, Deserialize)]
struct VersionManifest {
    version: String,
}

/// Result returned to the frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    /// False when the user has not opted in; in that case no network call was made.
    pub enabled: bool,
    /// The running app version (always populated).
    pub current: String,
    /// The latest advertised version, when the check ran and succeeded.
    pub latest: Option<String>,
    /// True when `latest` is strictly newer than `current`.
    pub update_available: bool,
    /// Where to send the user to download a newer build.
    pub download_url: String,
}

/// Read a boolean setting from the `settings` table (text "true"/"false").
async fn read_bool_setting(pool: &SqlitePool, key: &str, default: bool) -> bool {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ? LIMIT 1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(default)
}

/// Opt-in update check. Returns `{ enabled: false, .. }` without touching the
/// network when the user has not enabled `update_check_enabled`.
#[tauri::command]
pub async fn check_for_update(
    app_handle: tauri::AppHandle,
    db: State<'_, SqlitePool>,
) -> AppResult<UpdateCheckResult> {
    let current = app_handle.package_info().version.to_string();
    let pool = db.inner();

    if !read_bool_setting(pool, SETTING_UPDATE_CHECK_ENABLED, false).await {
        return Ok(UpdateCheckResult {
            enabled: false,
            current,
            latest: None,
            update_available: false,
            download_url: DOWNLOAD_URL.to_string(),
        });
    }

    // The one privacy-sensitive action: record the outbound check before it runs.
    audit::log_event(
        pool,
        "update_check",
        Some("network"),
        None,
        Some(VERSION_ENDPOINT),
        Some(audit::truncate(&format!("current={current}"), 64)),
    )
    .await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Io(format!("Failed to build HTTP client: {e}")))?;

    let manifest: VersionManifest = client
        .get(VERSION_ENDPOINT)
        .send()
        .await
        .map_err(|e| AppError::Io(format!("Update check failed: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Io(format!("Update check failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Io(format!("Could not parse version manifest: {e}")))?;

    let latest = manifest.version.trim().to_string();
    let update_available = is_newer(&latest, &current);

    Ok(UpdateCheckResult {
        enabled: true,
        current,
        latest: Some(latest),
        update_available,
        download_url: DOWNLOAD_URL.to_string(),
    })
}

/// A parsed semantic version: numeric `major.minor.patch` plus optional
/// prerelease identifiers (the part after `-`). Build metadata (`+...`) is ignored.
struct SemVer {
    core: [u64; 3],
    pre: Vec<PreId>,
}

/// One dot-separated prerelease identifier. Numeric identifiers compare
/// numerically and rank below alphanumeric ones (per the SemVer spec).
enum PreId {
    Num(u64),
    Text(String),
}

fn parse_semver(input: &str) -> Option<SemVer> {
    let s = input.trim();
    // Drop build metadata.
    let s = s.split('+').next().unwrap_or(s);
    let (core_str, pre_str) = match s.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (s, None),
    };

    let mut core = [0u64; 3];
    let mut parts = core_str.split('.');
    for slot in core.iter_mut() {
        let part = parts.next()?;
        *slot = part.parse::<u64>().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }

    let pre = match pre_str {
        None => Vec::new(),
        Some(p) if p.is_empty() => return None,
        Some(p) => p
            .split('.')
            .map(|id| match id.parse::<u64>() {
                Ok(n) => PreId::Num(n),
                Err(_) => PreId::Text(id.to_string()),
            })
            .collect(),
    };

    Some(SemVer { core, pre })
}

/// Order two prerelease identifier lists per SemVer §11.4.
/// An empty list (a stable release) outranks any non-empty list (a prerelease).
fn cmp_pre(a: &[PreId], b: &[PreId]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater, // stable > prerelease
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    for pair in a.iter().zip(b.iter()) {
        let ord = match pair {
            (PreId::Num(x), PreId::Num(y)) => x.cmp(y),
            (PreId::Num(_), PreId::Text(_)) => Ordering::Less,
            (PreId::Text(_), PreId::Num(_)) => Ordering::Greater,
            (PreId::Text(x), PreId::Text(y)) => x.cmp(y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.len().cmp(&b.len())
}

/// True when `latest` is a strictly newer version than `current`.
/// Unparseable versions are treated conservatively: if either side cannot be
/// parsed, returns true only when the strings differ and `latest` is non-empty,
/// so a malformed manifest never *hides* a potential update but also never
/// fabricates one for identical strings.
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => {
            let core = l.core.cmp(&c.core);
            if core != std::cmp::Ordering::Equal {
                return core == std::cmp::Ordering::Greater;
            }
            cmp_pre(&l.pre, &c.pre) == std::cmp::Ordering::Greater
        }
        _ => {
            let l = latest.trim();
            !l.is_empty() && l != current.trim()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn patch_minor_major_bumps_are_newer() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.1.0", "1.0.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
    }

    #[test]
    fn equal_or_older_is_not_newer() {
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("1.2.3", "2.0.0"));
    }

    #[test]
    fn stable_outranks_prerelease_of_same_core() {
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0"));
    }

    #[test]
    fn prerelease_ordering() {
        assert!(is_newer("1.0.0-rc.2", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0-rc.2"));
        // numeric identifiers rank below alphanumeric
        assert!(is_newer("1.0.0-rc.1.beta", "1.0.0-rc.1"));
    }

    #[test]
    fn newer_core_beats_prerelease_status() {
        assert!(is_newer("1.0.1-rc.1", "1.0.0"));
        assert!(is_newer("1.1.0-rc.1", "1.0.0-rc.9"));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert!(!is_newer("1.0.0+build5", "1.0.0+build1"));
        assert!(is_newer("1.0.1+build1", "1.0.0+build9"));
    }

    #[test]
    fn unparseable_falls_back_to_difference() {
        assert!(is_newer("nightly", "1.0.0"));
        assert!(!is_newer("garbage", "garbage"));
        assert!(!is_newer("", "1.0.0"));
    }
}
