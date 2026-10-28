
use tauri::State;
use crate::AppResult;
use crate::hardware::{llm_features_enabled, HardwareCapability, HardwareInfo, SharedHardware};
use crate::config::SharedConfig;
use crate::indexing_profile::{SharedIndexingPlan, IndexingThroughputTier};

/// Returned to the frontend — extends HardwareInfo with the persisted override flag.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareReport {
    #[serde(flatten)]
    pub info: HardwareInfo,
    /// Whether the user has opted in to LLM features despite insufficient hardware.
    pub llm_force_enabled: bool,
    /// Indexing throughput tier selected at startup (matches bulk indexers).
    pub indexing_throughput_tier: IndexingThroughputTier,
    /// Short human-readable summary of indexing behaviour for this session.
    pub indexing_throughput_summary: String,
}

fn report_from(
    info: HardwareInfo,
    force_enabled: bool,
    indexing_plan: &SharedIndexingPlan,
) -> HardwareReport {
    let plan = indexing_plan.current();
    HardwareReport {
        info,
        llm_force_enabled: force_enabled,
        indexing_throughput_tier: plan.tier,
        indexing_throughput_summary: plan.summary_label().to_string(),
    }
}

/// Detect hardware capabilities and return a full report including the
/// persisted override setting from the database.
///
/// When `refresh` is false/omitted, returns the cached startup snapshot (awaiting
/// the in-flight first detect if needed). When `refresh` is true, re-runs detect
/// and updates capability + indexing tier (used by Settings live polling).
#[tauri::command]
pub async fn get_hardware_info(
    refresh: Option<bool>,
    config: State<'_, SharedConfig>,
    shared_hw: State<'_, SharedHardware>,
    hw_cap: State<'_, HardwareCapability>,
    indexing_plan: State<'_, SharedIndexingPlan>,
) -> AppResult<HardwareReport> {
    let force_enabled = config.read().unwrap().llm_force_enabled;
    let info = if refresh.unwrap_or(false) {
        shared_hw
            .run_detect_and_publish(hw_cap.inner(), indexing_plan.inner())
            .await
    } else {
        shared_hw.wait_info().await
    };
    Ok(report_from(info, force_enabled, indexing_plan.inner()))
}

/// Returns true if LLM features should be active:
/// either the hardware is capable, or the user has force-enabled.
#[tauri::command]
pub async fn get_llm_enabled(
    config: State<'_, SharedConfig>,
    hw: State<'_, HardwareCapability>,
) -> AppResult<bool> {
    Ok(llm_features_enabled(&config.read().unwrap(), hw.get()))
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
