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

use tauri::State;
use crate::{AppResult};
use crate::hardware::{detect, HardwareInfo, LlmCapability};
use crate::config::SharedConfig;

/// Returned to the frontend — extends HardwareInfo with the persisted override flag.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareReport {
    #[serde(flatten)]
    pub info: HardwareInfo,
    /// Whether the user has opted in to LLM features despite insufficient hardware.
    pub llm_force_enabled: bool,
}

/// Detect hardware capabilities and return a full report including the
/// persisted override setting from the database.
#[tauri::command]
pub async fn get_hardware_info(config: State<'_, SharedConfig>) -> AppResult<HardwareReport> {
    let info = detect().await;
    let force_enabled = config.read().unwrap().llm_force_enabled;
    Ok(HardwareReport { info, llm_force_enabled: force_enabled })
}

/// Returns true if LLM features should be active:
/// either the hardware is capable, or the user has force-enabled.
#[tauri::command]
pub async fn get_llm_enabled(config: State<'_, SharedConfig>) -> AppResult<bool> {
    let info = detect().await;
    if info.capability == LlmCapability::Full {
        return Ok(true);
    }
    let force = config.read().unwrap().llm_force_enabled;
    Ok(force)
}

/// A single model currently loaded in Ollama, from /api/ps.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningModel {
    pub name: String,
    /// VRAM occupied by this model in megabytes, or None if Ollama didn't report it.
    pub vram_mb: Option<u64>,
    /// True when the model is pinned (keep_alive = -1 → expires at Go zero time).
    pub pinned: bool,
}

/// Return the list of models currently loaded in Ollama.
/// Returns an empty Vec when Ollama is not running or reports no models.
#[tauri::command]
pub async fn get_running_models() -> AppResult<Vec<RunningModel>> {
    #[derive(serde::Deserialize)]
    struct OllamaModel {
        name: String,
        #[serde(default)]
        size_vram: u64,
        #[serde(default)]
        expires_at: String,
    }
    #[derive(serde::Deserialize)]
    struct PsResp { models: Vec<OllamaModel> }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| crate::AppError::OllamaUnavailable(e.to_string()))?;

    let resp = match client.get("http://localhost:11434/api/ps").send().await {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // Ollama not running — not an error
    };

    let ps: PsResp = resp.json().await.map_err(|e| crate::AppError::OllamaUnavailable(e.to_string()))?;

    let models = ps.models.into_iter().map(|m| {
        let vram_mb = if m.size_vram > 0 { Some(m.size_vram / (1024 * 1024)) } else { None };
        // Ollama represents "never expire" (keep_alive = -1) with Go's zero time.
        let pinned = m.expires_at.starts_with("0001-");
        RunningModel { name: m.name, vram_mb, pinned }
    }).collect();

    Ok(models)
}
