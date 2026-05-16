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

//! Hardware capability detection.
//!
//! Called once on startup to classify the host machine into one of three
//! capability tiers that control whether LLM features are offered by default:
//!
//!   Full           — 8 GB+ RAM and a GPU with 4 GB+ VRAM (or Apple Silicon
//!                    with 16 GB+ unified memory). Chat + RAG work well.
//!   EmbeddingOnly  — 4–8 GB RAM. RAG indexing works; chat is slow / not recommended.
//!   Insufficient   — < 4 GB RAM. LLM features hidden by default.
//!
//! GPU detection is best-effort via subprocess; it never panics. The rest of
//! the app functions normally even if detection returns no GPU data.

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};
use tokio::process::Command;

/// Avoid brief console flashes when spawning helper tools on Windows.
#[cfg(target_os = "windows")]
fn command_no_console(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut std_cmd = std::process::Command::new(program);
    std_cmd.creation_flags(CREATE_NO_WINDOW);
    Command::from(std_cmd)
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// How capable the host machine is for LLM workloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LlmCapability {
    /// Chat + RAG fully supported.
    Full,
    /// RAG indexing works; chat not recommended.
    EmbeddingOnly,
    /// Below minimum — LLM features should be disabled by default.
    Insufficient,
}

/// Information about a single GPU/display adapter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    /// Total VRAM in megabytes, if detectable.
    pub vram_total_mb: Option<u64>,
    /// Currently used VRAM in megabytes, if detectable.
    pub vram_used_mb: Option<u64>,
    /// True for Apple Silicon — VRAM is the same pool as system RAM.
    pub is_unified_memory: bool,
}

/// Full snapshot of host hardware capabilities.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    /// Total system RAM in megabytes.
    pub ram_total_mb: u64,
    /// Currently used system RAM in megabytes.
    pub ram_used_mb: u64,
    /// RAM currently used by the Grimoire process in megabytes.
    pub ram_grimoire_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub capability: LlmCapability,
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Run the full hardware detection and return a [`HardwareInfo`] snapshot.
/// This is async because GPU detection spawns subprocesses.
pub async fn detect() -> HardwareInfo {
    let (cpu_name, cpu_cores, ram_total_mb, ram_used_mb, ram_grimoire_mb) =
        tokio::task::spawn_blocking(collect_cpu_ram)
            .await
            .unwrap_or_else(|e| {
                log::warn!("CPU/RAM detection task failed: {e}");
                ("Unknown CPU".to_string(), 0usize, 0u64, 0u64, 0u64)
            });
    let gpus = collect_gpus().await;
    let capability = classify(ram_total_mb, &gpus);

    HardwareInfo {
        cpu_name,
        cpu_cores,
        ram_total_mb,
        ram_used_mb,
        ram_grimoire_mb,
        gpus,
        capability,
    }
}

// ---------------------------------------------------------------------------
// CPU + RAM
// ---------------------------------------------------------------------------

fn collect_cpu_ram() -> (String, usize, u64, u64, u64) {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU name — take the first CPU's brand string; fall back to "Unknown CPU".
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cpu_cores = sys.physical_core_count().unwrap_or(sys.cpus().len());

    // RAM — sysinfo returns bytes; convert to MB.
    let ram_total_mb = sys.total_memory() / (1024 * 1024);
    let ram_used_mb  = sys.used_memory()  / (1024 * 1024);

    // Current process memory — Tauri apps spawn WebView2 renderer, GPU, and
    // utility subprocesses as separate PIDs. Sum all processes that share the
    // same executable name so the figure matches what Task Manager shows for
    // "Grimoire (N)".
    let ram_grimoire_mb = {
        let pid = sysinfo::get_current_pid().ok();
        if let Some(pid) = pid {
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let exe_name = sys.process(pid)
                .and_then(|p| p.exe().map(|e| e.to_path_buf()));
            if let Some(exe) = exe_name {
                sys.processes().values()
                    .filter(|p| p.exe().map(|e| e == exe).unwrap_or(false))
                    .map(|p| p.memory())
                    .sum::<u64>() / (1024 * 1024)
            } else {
                sys.process(pid)
                    .map(|p| p.memory() / (1024 * 1024))
                    .unwrap_or(0)
            }
        } else {
            0
        }
    };

    (cpu_name, cpu_cores, ram_total_mb, ram_used_mb, ram_grimoire_mb)
}

// ---------------------------------------------------------------------------
// GPU detection — subprocess-based, best-effort
// ---------------------------------------------------------------------------

async fn collect_gpus() -> Vec<GpuInfo> {
    // On Windows, DXGI is the primary source — it provides name, total VRAM,
    // and live VRAM usage from a single API (same as Task Manager).
    #[cfg(target_os = "windows")]
    if let Some(gpus) = try_windows_dxgi().await {
        return gpus;
    }

    // Try each strategy in order; return as soon as one succeeds.
    if let Some(gpus) = try_nvidia_smi().await {
        if !gpus.is_empty() {
            return gpus;
        }
    }

    #[cfg(target_os = "macos")]
    if let Some(gpus) = try_macos_system_profiler().await {
        if !gpus.is_empty() {
            return gpus;
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(gpus) = try_windows_registry().await {
        return gpus;
    }

    // WMIC is kept as a last-resort fallback on Windows (driver-dependent).
    #[cfg(target_os = "windows")]
    if let Some(gpus) = try_windows_wmic().await {
        if !gpus.is_empty() {
            return gpus;
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(gpus) = try_linux_drm().await {
        if !gpus.is_empty() {
            return gpus;
        }
    }

    Vec::new()
}

// ── NVIDIA-SMI (works on Windows, Linux, macOS with NVIDIA drivers) ────────

async fn try_nvidia_smi() -> Option<Vec<GpuInfo>> {
    #[cfg(target_os = "windows")]
    let output = command_no_console("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await
        .ok()?;
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let gpus = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ',').collect();
            if parts.len() < 3 {
                return None;
            }
            let name          = parts[0].trim().to_string();
            let vram_total_mb = parts[1].trim().parse::<u64>().ok();
            let vram_used_mb  = parts[2].trim().parse::<u64>().ok();
            Some(GpuInfo { name, vram_total_mb, vram_used_mb, is_unified_memory: false })
        })
        .collect();

    Some(gpus)
}

// ── macOS: system_profiler ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn try_macos_system_profiler() -> Option<Vec<GpuInfo>> {
    // First check if this is Apple Silicon (unified memory).
    let is_apple_silicon = is_apple_silicon().await;

    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let displays = json.get("SPDisplaysDataType")?.as_array()?;

    let mut gpus = Vec::new();
    for entry in displays {
        let name = entry
            .get("sppci_model")
            .or_else(|| entry.get("_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown GPU")
            .to_string();

        // VRAM for Apple Silicon: use total system RAM as the shared pool.
        // For discrete GPUs, look for spdisplays_vram or spdisplays_vram_shared.
        let (vram_total_mb, is_unified) = if is_apple_silicon {
            let total = unified_memory_mb().await;
            (total, true)
        } else {
            let mb = parse_macos_vram(entry);
            (mb, false)
        };

        gpus.push(GpuInfo {
            name,
            vram_total_mb,
            vram_used_mb: None, // macOS doesn't easily expose per-process VRAM usage
            is_unified_memory: is_unified,
        });
    }

    Some(gpus)
}

#[cfg(target_os = "macos")]
async fn is_apple_silicon() -> bool {
    let output = Command::new("system_profiler")
        .args(["SPHardwareDataType", "-json"])
        .output()
        .await;

    if let Ok(output) = output {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(hw) = json
                .get("SPHardwareDataType")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
            {
                // chip_type or cpu_type starts with "Apple" on M-series
                let chip = hw
                    .get("chip_type")
                    .or_else(|| hw.get("cpu_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                return chip.to_ascii_lowercase().contains("apple");
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
async fn unified_memory_mb() -> Option<u64> {
    // sysctl hw.memsize returns total RAM in bytes — this is the unified memory pool.
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .await
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout);
    let bytes: u64 = s.trim().parse().ok()?;
    Some(bytes / (1024 * 1024))
}

#[cfg(target_os = "macos")]
fn parse_macos_vram(entry: &serde_json::Value) -> Option<u64> {
    // The field is something like "4096 MB" or "1 GB".
    let raw = entry
        .get("spdisplays_vram")
        .or_else(|| entry.get("spdisplays_vram_shared"))
        .and_then(|v| v.as_str())?;

    let raw = raw.trim().to_ascii_uppercase();
    if let Some(s) = raw.strip_suffix(" MB") {
        return s.trim().parse().ok();
    }
    if let Some(s) = raw.strip_suffix(" GB") {
        let gb: u64 = s.trim().parse().ok()?;
        return Some(gb * 1024);
    }
    None
}

// ── Windows: registry (64-bit VRAM, primary method) ───────────────────────

/// Read GPU names and VRAM from the Windows registry.
///
/// `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-...}\000N` contains:
///   - `DriverDesc`                      — human-readable GPU name
///   - `HardwareInformation.MemorySize`  — VRAM as an 8-byte little-endian QWORD
///
/// This is the most reliable source on Windows because the WMI field
/// `Win32_VideoController.AdapterRAM` is a 32-bit DWORD and overflows for
/// any GPU with ≥ 4 GB VRAM.
#[cfg(target_os = "windows")]
async fn try_windows_registry() -> Option<Vec<GpuInfo>> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    const DISPLAY_CLASS: &str =
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(DISPLAY_CLASS, KEY_READ).ok()?;

    let mut gpus = Vec::new();

    for i in 0u32.. {
        let subkey_name = format!("{i:04}");
        let subkey = match class_key.open_subkey_with_flags(&subkey_name, KEY_READ) {
            Ok(k) => k,
            Err(_) => break, // No more numbered subkeys.
        };

        let name: String = match subkey.get_value("DriverDesc") {
            Ok(n) => n,
            Err(_) => continue, // Not a GPU adapter entry.
        };
        if name.trim().is_empty() {
            continue;
        }

        // Some drivers (AMD RDNA 3/4, Intel) write a proper 64-bit key.
        // Older drivers fall back to a 32-bit MemorySize that overflows at 4 GB,
        // identical to the WMIC issue — we apply the same overflow correction.
        let vram_total_mb: Option<u64> =
            read_registry_vram(&subkey, "HardwareInformation.qwMemorySize", false)
                .or_else(|| read_registry_vram(&subkey, "HardwareInformation.MemorySize", true));

        gpus.push(GpuInfo {
            name,
            vram_total_mb,
            vram_used_mb: None,
            is_unified_memory: false,
        });
    }

    // Only return registry data if at least one GPU has VRAM info.
    // If all VRAMs are None (driver doesn't write this key), fall through to
    // WMIC so it can provide the clamped value.
    if gpus.iter().any(|g| g.vram_total_mb.is_some()) {
        Some(gpus)
    } else {
        None
    }
}

/// Read a REG_BINARY VRAM value from a display-class registry subkey.
///
/// - 8+ bytes: treated as a real 64-bit little-endian value; no overflow possible.
/// - 4 bytes:  32-bit value; same overflow issue as WMIC — apply correction when
///   `apply_overflow_correction` is true.
#[cfg(target_os = "windows")]
fn read_registry_vram(subkey: &winreg::RegKey, value_name: &str, apply_overflow_correction: bool) -> Option<u64> {
    let rv = subkey.get_raw_value(value_name).ok()?;
    let bytes = &rv.bytes;
    match bytes.len() {
        n if n >= 8 => {
            let arr: [u8; 8] = bytes[..8].try_into().ok()?;
            let raw = u64::from_le_bytes(arr);
            if raw == 0 { return None; }
            Some(raw / (1024 * 1024))
        }
        4 => {
            let arr: [u8; 4] = bytes[..4].try_into().ok()?;
            let raw = u32::from_le_bytes(arr) as u64;
            if raw == 0 { return None; }
            if apply_overflow_correction {
                Some(wmic_adapter_ram_to_mb(raw))
            } else {
                Some(raw / (1024 * 1024))
            }
        }
        _ => None,
    }
}

// ── Windows: WMIC ──────────────────────────────────────────────────────────

/// Convert a raw `AdapterRAM` byte value from WMIC to megabytes.
///
/// `Win32_VideoController.AdapterRAM` is a 32-bit DWORD. GPUs with ≥ 4 GB
/// VRAM overflow the field; Windows typically reports 4,294,967,295
/// (0xFFFF_FFFF) as a sentinel. We treat any value ≥ 4,000,000,000 bytes
/// (~3.7 GB) as "at least 4,096 MB", which is the minimum for `Full`
/// capability classification.
#[cfg(target_os = "windows")]
fn wmic_adapter_ram_to_mb(bytes: u64) -> u64 {
    const OVERFLOW_FLOOR: u64 = 4_000_000_000; // bytes; below this the value is trustworthy
    if bytes >= OVERFLOW_FLOOR {
        4096
    } else {
        bytes / (1024 * 1024)
    }
}

#[cfg(target_os = "windows")]
async fn try_windows_wmic() -> Option<Vec<GpuInfo>> {
    let output = command_no_console("wmic")
        .args(["path", "win32_VideoController", "get", "Name,AdapterRAM", "/value"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // wmic /value output looks like:
    //   AdapterRAM=4293918720
    //   Name=NVIDIA GeForce RTX 3060
    // Entries are separated by blank lines.
    let mut gpus = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_vram: Option<u64> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            if let Some(name) = current_name.take() {
                gpus.push(GpuInfo {
                    name,
                    vram_total_mb: current_vram.take().map(wmic_adapter_ram_to_mb),
                    vram_used_mb: None,
                    is_unified_memory: false,
                });
            } else {
                current_vram = None;
            }
            continue;
        }
        if let Some(val) = line.strip_prefix("Name=") {
            let v = val.trim().to_string();
            if !v.is_empty() { current_name = Some(v); }
        } else if let Some(val) = line.strip_prefix("AdapterRAM=") {
            current_vram = val.trim().parse::<u64>().ok();
        }
    }
    // Flush the last entry if not already flushed.
    if let Some(name) = current_name {
        gpus.push(GpuInfo {
            name,
            vram_total_mb: current_vram.map(wmic_adapter_ram_to_mb),
            vram_used_mb: None,
            is_unified_memory: false,
        });
    }

    Some(gpus)
}

// ── Linux: /sys/class/drm (AMD / Intel via kernel driver) ─────────────────

#[cfg(target_os = "linux")]
async fn try_linux_drm() -> Option<Vec<GpuInfo>> {
    use tokio::fs;

    // Discover cards by listing /sys/class/drm/card*/device/
    let mut entries = fs::read_dir("/sys/class/drm").await.ok()?;
    let mut gpus = Vec::new();

    while let Some(entry) = entries.next_entry().await.ok().flatten() {
        let name_os = entry.file_name();
        let card_name = name_os.to_string_lossy();
        // Only "cardN" directories (not "cardN-*" connector entries).
        if !card_name.starts_with("card") || card_name.contains('-') {
            continue;
        }

        let base = entry.path().join("device");

        if fs::metadata(&base).await.is_err() {
            continue;
        }

        // GPU display name from the vendor/device uevent or modalias.
        let gpu_name = read_drm_name(&base).await.unwrap_or_else(|| card_name.to_string());

        // VRAM total (AMD: mem_info_vram_total, Intel: not exposed this way).
        let vram_total_mb = fs::read_to_string(base.join("mem_info_vram_total"))
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|b| b / (1024 * 1024));

        let vram_used_mb = fs::read_to_string(base.join("mem_info_vram_used"))
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|b| b / (1024 * 1024));

        gpus.push(GpuInfo {
            name: gpu_name,
            vram_total_mb,
            vram_used_mb,
            is_unified_memory: false,
        });
    }

    Some(gpus)
}

#[cfg(target_os = "linux")]
async fn read_drm_name(base: &std::path::Path) -> Option<String> {
    use tokio::fs;
    // Try reading "label" from product file, fall back to uevent DRIVER= line.
    if let Ok(s) = fs::read_to_string(base.join("product_name")).await {
        let s = s.trim().to_string();
        if !s.is_empty() { return Some(s); }
    }
    // Parse DRIVER from uevent as a minimal fallback.
    if let Ok(uevent) = fs::read_to_string(base.join("uevent")).await {
        for line in uevent.lines() {
            if let Some(val) = line.strip_prefix("DRIVER=") {
                return Some(val.trim().to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// DXGI + WMI Performance Counters — primary GPU detection on Windows
// ---------------------------------------------------------------------------

/// Enumerate DXGI adapters for name + total VRAM, then query Windows
/// Performance Counters for **system-wide** dedicated VRAM usage per adapter.
///
/// `IDXGIAdapter3::QueryVideoMemoryInfo` only reports the *calling process's*
/// VRAM — useless for a process that doesn't use the GPU directly. Task
/// Manager reads `Win32_PerfFormattedData_GPUPerformanceCounters_GPUAdapterMemory`
/// instead, which reports system-wide usage. We do the same here, matching
/// counter instances to DXGI adapters by their LUID.
#[cfg(target_os = "windows")]
async fn try_windows_dxgi() -> Option<Vec<GpuInfo>> {
    use windows::Win32::Graphics::Dxgi::*;

    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1() }.ok()?;

    // Step 1: enumerate DXGI adapters — name, total VRAM, LUID.
    struct DxgiAdapter {
        name: String,
        total_mb: u64,
        luid_prefix: String, // "luid_0xHHHHHHHH_0xLLLLLLLL"
    }

    let mut adapters = Vec::new();
    let mut idx: u32 = 0;
    loop {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(idx) } {
            Ok(a) => a,
            Err(_) => break,
        };
        idx += 1;

        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(d) => d,
            Err(_) => continue,
        };

        let name = String::from_utf16_lossy(
            &desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len())]
        );

        // Skip software / virtual adapters.
        if name.contains("Microsoft") || name.contains("Basic Render") {
            continue;
        }

        let total_mb = desc.DedicatedVideoMemory as u64 / (1024 * 1024);
        let luid_prefix = format!(
            "luid_0x{:08x}_0x{:08x}",
            desc.AdapterLuid.HighPart as u32,
            desc.AdapterLuid.LowPart
        );

        adapters.push(DxgiAdapter { name, total_mb, luid_prefix });
    }

    if adapters.is_empty() {
        return None;
    }

    // Step 2: query system-wide dedicated VRAM usage via WMI performance counters.
    let usage_map = query_gpu_vram_usage().await;

    // Step 3: build GpuInfo list, matching usage by LUID prefix.
    let mut gpus: Vec<GpuInfo> = adapters.into_iter().filter_map(|a| {
        // Skip iGPUs with tiny VRAM (< 1 GB) — not useful for LLM workloads.
        if a.total_mb > 0 && a.total_mb < 1024 {
            return None;
        }

        let luid_lower = a.luid_prefix.to_lowercase();
        let used_mb = if usage_map.is_empty() {
            None // WMI query failed — no usage data available
        } else {
            Some(usage_map.iter()
                .filter(|(name, _)| name.to_lowercase().starts_with(&luid_lower))
                .map(|(_, mb)| *mb)
                .sum())
        };

        Some(GpuInfo {
            name: a.name,
            vram_total_mb: if a.total_mb > 0 { Some(a.total_mb) } else { None },
            vram_used_mb: used_mb,
            is_unified_memory: false,
        })
    }).collect();

    // Deduplicate: DXGI can enumerate the same physical GPU twice.
    // Keep the entry with actual usage data, or the first one if neither has it.
    gpus.dedup_by(|b, a| {
        if a.name != b.name {
            return false;
        }
        // Merge: prefer whichever has usage; take the higher value for both.
        a.vram_used_mb = match (a.vram_used_mb, b.vram_used_mb) {
            (Some(x), Some(y)) => Some(x.max(y)),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            _ => None,
        };
        if let Some(bt) = b.vram_total_mb {
            a.vram_total_mb = Some(a.vram_total_mb.map(|at| at.max(bt)).unwrap_or(bt));
        }
        true // b is merged into a; remove b
    });

    Some(gpus)
}

/// Query WMI performance counters for per-adapter dedicated VRAM usage.
/// Returns `(instance_name, used_mb)` pairs. Instance names contain the
/// adapter LUID: `luid_0xHHHHHHHH_0xLLLLLLLL_phys_N`.
#[cfg(target_os = "windows")]
async fn query_gpu_vram_usage() -> Vec<(String, u64)> {
    let output = command_no_console("wmic")
        .args([
            "path", "Win32_PerfFormattedData_GPUPerformanceCounters_GPUAdapterMemory",
            "get", "DedicatedUsage,Name", "/value",
        ])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    let mut current_usage: Option<u64> = None;
    let mut current_name: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            if let (Some(name), Some(usage)) = (current_name.take(), current_usage.take()) {
                results.push((name, usage / (1024 * 1024)));
            }
            continue;
        }
        if let Some(val) = line.strip_prefix("DedicatedUsage=") {
            current_usage = val.trim().parse::<u64>().ok();
        } else if let Some(val) = line.strip_prefix("Name=") {
            current_name = Some(val.trim().to_string());
        }
    }
    // Flush last entry.
    if let (Some(name), Some(usage)) = (current_name, current_usage) {
        results.push((name, usage / (1024 * 1024)));
    }

    results
}

// ---------------------------------------------------------------------------
// Capability classification
// ---------------------------------------------------------------------------

fn classify(ram_total_mb: u64, gpus: &[GpuInfo]) -> LlmCapability {
    if ram_total_mb < 4096 {
        return LlmCapability::Insufficient;
    }

    // Check for a capable GPU.
    let has_capable_gpu = gpus.iter().any(|g| {
        if g.is_unified_memory {
            // Apple Silicon: treat total unified memory as VRAM.
            g.vram_total_mb.map(|v| v >= 16384).unwrap_or(false)
        } else {
            g.vram_total_mb.map(|v| v >= 4096).unwrap_or(false)
        }
    });

    if ram_total_mb >= 8192 && has_capable_gpu {
        LlmCapability::Full
    } else {
        LlmCapability::EmbeddingOnly
    }
}
