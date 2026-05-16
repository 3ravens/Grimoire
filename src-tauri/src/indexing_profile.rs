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

//! Hardware-adaptive throughput limits for bulk indexing (embed batches,
//! Wikipedia ZIM scan windows, HTML parse parallelism, LanceDB append chunking).
//!
//! Distinct from [`crate::hardware::LlmCapability`], which gates default LLM UX.
//! Tiers are derived from RAM, CPU core count, and best-effort GPU memory.

use std::sync::{Arc, OnceLock};

use rayon::ThreadPool;
use serde::Serialize;

use crate::hardware::{GpuInfo, HardwareInfo, LlmCapability};

// ---------------------------------------------------------------------------
// Tier selection thresholds (MB unless noted)
// ---------------------------------------------------------------------------
//
// Reference: roadmap "high-end" dev box (16 GB VRAM class, 32 GB RAM).
// Low tier avoids CPU/RAM spikes from 1024-wide ZIM windows + full-core Rayon
// and reduces Ollama batch payload on memory-constrained hosts.

/// Below this RAM, always use low indexing tier.
const LOW_RAM_MB: u64 = 8192;
/// With at least this much RAM and a strong GPU, high tier is allowed.
const HIGH_RAM_MB: u64 = 16384;
/// Discrete GPU total VRAM (MB) at or above this → contributes to "high".
const HIGH_DISCRETE_VRAM_MB: u64 = 10240;
/// Apple / unified memory (MB) at or above this → contributes to "high".
const HIGH_UNIFIED_VRAM_MB: u64 = 20480;
/// Minimum logical CPU cores for high tier.
const HIGH_MIN_CPU_CORES: usize = 6;

static GLOBAL_PLAN: OnceLock<Arc<IndexingThroughputPlan>> = OnceLock::new();

/// Throughput tier for background indexing (Wikipedia, file scanner, vault re-embed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexingThroughputTier {
    Low,
    Mid,
    High,
}

/// Concrete limits selected for a tier. Cloned into background tasks via `Arc`.
#[derive(Debug)]
pub struct IndexingThroughputPlan {
    pub tier: IndexingThroughputTier,
    /// Max texts per Ollama `/api/embed` request (before model-specific clamp).
    pub embed_inputs_per_request: usize,
    /// ZIM entries read per outer Wikipedia indexing iteration.
    pub wiki_scan_window: u32,
    /// Upper bound when ramping dynamic embed batch size after successful windows.
    pub wiki_dynamic_embed_ceiling: usize,
    /// `0` = use Rayon default pool; `>0` = dedicated pool for HTML→text parse.
    #[allow(dead_code)]
    pub wiki_parse_threads: usize,
    /// Max rows per proactive Lance append (`usize::MAX` = single append per window).
    pub wiki_lance_initial_chunk_rows: usize,
    parse_pool: Option<Arc<ThreadPool>>,
}

impl IndexingThroughputPlan {
    /// Optional dedicated parse pool (only when `wiki_parse_threads > 0`).
    pub fn wiki_parse_pool(&self) -> Option<&ThreadPool> {
        self.parse_pool.as_deref()
    }

    /// Effective `/api/embed` slice size: `min(plan cap, per-model hard cap)`.
    pub fn embed_cap_for_model(&self, model: &str) -> usize {
        let tier_cap = self.embed_inputs_per_request.max(1);
        let model_cap = model_max_embed_batch(model).max(1);
        tier_cap.min(model_cap).max(1)
    }

    /// One-line description for Settings → Hardware.
    pub fn summary_label(&self) -> &'static str {
        match self.tier {
            IndexingThroughputTier::Low => {
                "Conservative indexing (smaller batches, less parallelism)"
            }
            IndexingThroughputTier::Mid => {
                "Balanced indexing (default throughput)"
            }
            IndexingThroughputTier::High => {
                "Performance indexing (larger windows and batches)"
            }
        }
    }
}

/// Best-effort largest detectable VRAM pool in MB (unified counts as reported total).
fn best_gpu_memory_mb(gpus: &[GpuInfo]) -> u64 {
    gpus
        .iter()
        .filter_map(|g| g.vram_total_mb)
        .max()
        .unwrap_or(0)
}

/// True if any GPU meets the "strong" threshold for high-tier indexing.
fn has_strong_indexing_gpu(gpus: &[GpuInfo]) -> bool {
    gpus.iter().any(|g| {
        let Some(v) = g.vram_total_mb else {
            return false;
        };
        if g.is_unified_memory {
            v >= HIGH_UNIFIED_VRAM_MB
        } else {
            v >= HIGH_DISCRETE_VRAM_MB
        }
    })
}

/// Classify host for indexing (independent of persisted user LLM overrides).
pub fn tier_from_hardware(hw: &HardwareInfo) -> IndexingThroughputTier {
    if hw.capability == LlmCapability::Insufficient || hw.ram_total_mb < LOW_RAM_MB {
        return IndexingThroughputTier::Low;
    }

    // Embedding-only with modest RAM: keep CPU/RAM pressure low.
    if hw.capability == LlmCapability::EmbeddingOnly && hw.ram_total_mb < HIGH_RAM_MB {
        return IndexingThroughputTier::Low;
    }

    let vram_best = best_gpu_memory_mb(&hw.gpus);
    // CPU-only or unknown GPU on 8–16 GB RAM: prefer conservative indexing.
    if hw.ram_total_mb < HIGH_RAM_MB && vram_best < 6144 {
        return IndexingThroughputTier::Low;
    }

    if hw.ram_total_mb >= HIGH_RAM_MB
        && hw.cpu_cores >= HIGH_MIN_CPU_CORES
        && has_strong_indexing_gpu(&hw.gpus)
    {
        return IndexingThroughputTier::High;
    }

    IndexingThroughputTier::Mid
}

/// Optional override: `GRIMOIRE_INDEXING_TIER=low|mid|high` (explicit opt-in for dev/CI).
pub fn tier_from_env() -> Option<IndexingThroughputTier> {
    let raw = std::env::var("GRIMOIRE_INDEXING_TIER").ok()?;
    match raw.to_ascii_lowercase().as_str() {
        "low" => Some(IndexingThroughputTier::Low),
        "mid" => Some(IndexingThroughputTier::Mid),
        "high" => Some(IndexingThroughputTier::High),
        _ => None,
    }
}

/// Build plan + optional Rayon parse pool for `tier`.
pub fn plan_for_tier(tier: IndexingThroughputTier) -> IndexingThroughputPlan {
    let (
        embed_inputs_per_request,
        wiki_scan_window,
        wiki_dynamic_embed_ceiling,
        wiki_parse_threads,
        wiki_lance_initial_chunk_rows,
    ) = match tier {
        IndexingThroughputTier::Low => (16u32, 256u32, 32usize, 4usize, 512usize),
        IndexingThroughputTier::Mid => (64, 1024, 128, 0, usize::MAX),
        IndexingThroughputTier::High => (96, 1536, 192, 0, usize::MAX),
    };

    let parse_pool = if wiki_parse_threads > 0 {
        Some(Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(wiki_parse_threads)
                .thread_name(|i| format!("wiki-parse-{i}"))
                .build()
                .expect("wiki parse thread pool"),
        ))
    } else {
        None
    };

    IndexingThroughputPlan {
        tier,
        embed_inputs_per_request: embed_inputs_per_request as usize,
        wiki_scan_window,
        wiki_dynamic_embed_ceiling,
        wiki_parse_threads,
        wiki_lance_initial_chunk_rows,
        parse_pool,
    }
}

/// Per-model ceiling on `/api/embed` inputs (token/context safety).
fn model_max_embed_batch(model: &str) -> usize {
    if model.contains("mxbai") {
        // Short inputs per `content_chars_for_model`; still avoid huge batches.
        48
    } else {
        128
    }
}

/// Install the process-wide indexing plan (call once from Tauri `setup`).
pub fn init_global(plan: Arc<IndexingThroughputPlan>) {
    let _ = GLOBAL_PLAN.set(plan);
}

/// Effective `/api/embed` slice size for `model`, using the global plan when set
/// or **Mid** tier defaults (for harnesses before [`init_global`]).
pub fn effective_embed_slice_cap(model: &str) -> usize {
    try_global()
        .map(|p| p.embed_cap_for_model(model))
        .unwrap_or_else(|| plan_for_tier(IndexingThroughputTier::Mid).embed_cap_for_model(model))
}

/// Returns installed plan, or `None` before init (unit tests / partial harness).
pub fn try_global() -> Option<Arc<IndexingThroughputPlan>> {
    GLOBAL_PLAN.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{GpuInfo, LlmCapability};

    fn hw(
        ram_mb: u64,
        cores: usize,
        capability: LlmCapability,
        gpus: Vec<GpuInfo>,
    ) -> HardwareInfo {
        HardwareInfo {
            cpu_name: "test".into(),
            cpu_cores: cores,
            ram_total_mb: ram_mb,
            ram_used_mb: 1024,
            ram_grimoire_mb: 100,
            gpus,
            capability,
        }
    }

    fn discrete(vram_mb: u64) -> GpuInfo {
        GpuInfo {
            name: "GPU".into(),
            vram_total_mb: Some(vram_mb),
            vram_used_mb: None,
            is_unified_memory: false,
        }
    }

    fn apple(unified_mb: u64) -> GpuInfo {
        GpuInfo {
            name: "Apple M".into(),
            vram_total_mb: Some(unified_mb),
            vram_used_mb: None,
            is_unified_memory: true,
        }
    }

    #[test]
    fn insufficient_ram_is_low() {
        let t = tier_from_hardware(&hw(
            4096,
            8,
            LlmCapability::Insufficient,
            vec![discrete(16384)],
        ));
        assert_eq!(t, IndexingThroughputTier::Low);
    }

    #[test]
    fn high_ram_strong_gpu_is_high() {
        let t = tier_from_hardware(&hw(
            32 * 1024,
            12,
            LlmCapability::Full,
            vec![discrete(16384)],
        ));
        assert_eq!(t, IndexingThroughputTier::High);
    }

    #[test]
    fn mid_tier_typical_laptop() {
        let t = tier_from_hardware(&hw(
            16 * 1024,
            8,
            LlmCapability::Full,
            vec![discrete(8192)],
        ));
        assert_eq!(t, IndexingThroughputTier::Mid);
    }

    #[test]
    fn apple_unified_high() {
        let t = tier_from_hardware(&hw(
            32 * 1024,
            8,
            LlmCapability::Full,
            vec![apple(24 * 1024)],
        ));
        assert_eq!(t, IndexingThroughputTier::High);
    }

    #[test]
    fn low_embed_cap_clamps_mxbai() {
        let plan = plan_for_tier(IndexingThroughputTier::Low);
        assert!(plan.embed_cap_for_model("mxbai-embed-large") <= 16);
    }

    #[test]
    fn mid_plan_matches_legacy_defaults() {
        let p = plan_for_tier(IndexingThroughputTier::Mid);
        assert_eq!(p.embed_inputs_per_request, 64);
        assert_eq!(p.wiki_scan_window, 1024);
        assert_eq!(p.wiki_dynamic_embed_ceiling, 128);
        assert!(p.parse_pool.is_none());
    }
}
