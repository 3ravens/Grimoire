
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

// Default embedding dimensions for nomic-embed-text.
pub const DIMS: i32 = 768;

// Runtime telemetry for batch embedding degradation. This helps the UI surface
// when we had to split batches or fall back to single-item embeds.
static EMBED_BATCH_SPLIT_COUNT: AtomicU64 = AtomicU64::new(0);
static EMBED_BATCH_SINGLE_FALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn reset_embed_batch_telemetry() {
    EMBED_BATCH_SPLIT_COUNT.store(0, Ordering::Relaxed);
    EMBED_BATCH_SINGLE_FALLBACK_COUNT.store(0, Ordering::Relaxed);
}

pub fn snapshot_embed_batch_telemetry() -> (u64, u64) {
    (
        EMBED_BATCH_SPLIT_COUNT.load(Ordering::Relaxed),
        EMBED_BATCH_SINGLE_FALLBACK_COUNT.load(Ordering::Relaxed),
    )
}

/// Return the embedding dimension for a given model name.
#[allow(dead_code)]
pub fn dims_for_model(model: &str) -> i32 {
    if model.contains("mxbai") { 1024 } else { 768 }
}

/// Maximum content characters to send to the embedding model for a single input.
/// mxbai-embed-large has a 512-token context window (~350 chars safe budget after
/// accounting for the "search_document: {title}\n" prefix). nomic-embed-text
/// supports 8192 tokens so 1500 chars is well within budget.
pub fn content_chars_for_model(model: &str) -> usize {
    if model.contains("mxbai") { 350 } else { 1500 }
}

/// Batch size for `/api/embed` requests: host indexing tier plus per-model clamp
/// (see [`crate::indexing_profile::effective_embed_slice_cap`]).
pub fn batch_size_for_model(model: &str) -> usize {
    crate::indexing_profile::effective_embed_slice_cap(model)
}

/// Evict all currently loaded Ollama models *except* the one we're about to use.
/// On AMD RDNA4 with Vulkan, running two models simultaneously causes GPU crashes.
/// Skipping the target model avoids the cost of unloading and reloading it when
/// it is already resident from the previous call (e.g. during a bulk reindex).
async fn evict_other_models(client: &reqwest::Client, keep_model: &str) {
    #[derive(Deserialize)]
    struct RunningModel { name: String }
    #[derive(Deserialize)]
    struct PsResp { models: Vec<RunningModel> }
    #[derive(Serialize)]
    struct UnloadReq<'a> { model: &'a str, keep_alive: i32, stream: bool }

    let Ok(resp) = client.get("http://localhost:11434/api/ps").send().await else { return };
    let Ok(ps) = resp.json::<PsResp>().await else { return };
    for m in ps.models {
        if m.name == keep_model { continue; }
        let _ = client
            .post("http://localhost:11434/api/generate")
            .json(&UnloadReq { model: &m.name, keep_alive: 0, stream: false })
            .send()
            .await;
    }
}

/// Unload every Ollama runner except `keep_model` (RDNA4/Vulkan safety).
/// Bulk indexers should call this once at job start and periodically instead of
/// relying on per-batch eviction inside [`embed_batch_with_options`].
pub async fn evict_ollama_models_except(keep_model: &str) {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    else {
        return;
    };
    evict_other_models(&client, keep_model).await;
}

/// Controls Ollama eviction and optional progress around [`embed_batch_with_options`].
#[derive(Clone, Default)]
pub struct EmbedBatchOptions {
    /// When `false` (default), evict competing models once before `/api/embed`.
    /// When `true`, the caller must run [`evict_ollama_models_except`] (e.g. start +
    /// periodic during Wikipedia / file-scan indexing).
    pub skip_ollama_entry_eviction: bool,
    /// After each successful `/api/embed` slice, called with `(chunks_ready, total_chunks)`.
    /// `chunks_ready` counts inputs that have an embedding in memory so far (monotonic).
    /// Used by vault re-index so very large notes do not look frozen at `N/M notes`.
    pub on_slice_progress: Option<std::sync::Arc<dyn Fn(usize, usize) + Send + Sync>>,
}

/// Call Ollama's /api/embeddings endpoint and return the embedding vector.
/// Evicts all running models first to prevent Vulkan GPU context conflicts.
/// Retries once on failure — evicting again before the second attempt clears
/// any GPU state that was corrupted by the first crash.
///
/// `keep_alive_secs`: seconds Ollama keeps the embedding model loaded after the request.
/// Use `0` for interactive RAG queries so VRAM is freed quickly; use `300` (or similar)
/// when embedding many chunks in sequence so the model stays warm between calls.
pub async fn embed_with_keep_alive(
    text: &str,
    model: &str,
    keep_alive_secs: i32,
) -> Result<Vec<f32>, String> {
    log::info!(
        "[embed] model={model} text_len={} keep_alive={keep_alive_secs}s",
        text.len()
    );
    #[derive(Serialize)]
    struct Req<'a> {
        model: &'a str,
        prompt: &'a str,
        keep_alive: i32,
    }

    #[derive(Deserialize)]
    struct Resp {
        embedding: Option<Vec<f32>>,
        error: Option<String>,
    }

    // 120-second timeout — embedding a single chunk should never take this long.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut last_err = String::new();
    for attempt in 1u32..=2 {
        // Evict competing models before every attempt. Skips the embed model itself
        // so it stays resident across consecutive calls (e.g. during bulk reindex).
        evict_other_models(&client, model).await;

        let result: Result<Vec<f32>, String> = async {
            let response = client
                .post("http://localhost:11434/api/embeddings")
                .json(&Req {
                    model,
                    prompt: text,
                    keep_alive: keep_alive_secs,
                })
                .send()
                .await
                .map_err(|e| format!("Could not reach Ollama (embedding): {e}"))?;

            let text_body = response
                .text()
                .await
                .map_err(|e| format!("Could not read embed response: {e}"))?;

            let resp: Resp = serde_json::from_str(&text_body)
                .map_err(|e| format!("Unexpected embed response: {e}\nBody: {text_body}"))?;

            if let Some(err) = resp.error {
                return Err(format!("Ollama embedding error: {err}"));
            }

            let embedding = resp.embedding.ok_or_else(|| {
                format!("Embed response missing embedding\nBody: {text_body}")
            })?;

            if embedding.is_empty() {
                return Err("Empty embedding response from Ollama".to_string());
            }

            // Normalize to unit length before storing. Different Ollama versions and
            // task prefixes (search_document:, search_query:) can return vectors with
            // varying norms (observed: 1.0–20+). Normalizing here ensures consistent
            // L2² distances in the range [0, 4] regardless of model or configuration.
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 0.1 {
                return Err(format!("Degenerate embedding vector (norm={norm:.4}) — Ollama inference likely crashed"));
            }
            let normalized: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

            Ok(normalized)
        }.await;

        match result {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = e;
                if attempt < 2 {
                    // Wait for Ollama to finish cleaning up the crashed runner.
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_err)
}

/// Same as [`embed_with_keep_alive`] with `keep_alive_secs = 0` (unload after each call).
pub async fn embed(text: &str, model: &str) -> Result<Vec<f32>, String> {
    embed_with_keep_alive(text, model, 0).await
}

/// Embed a batch of texts in a single Ollama request using `/api/embed`.
/// 5–10× faster than calling `embed()` per-text: one HTTP round-trip per batch,
/// model stays resident (`keep_alive=300`). By default evicts competing Ollama
/// models once before `/api/embed`; use [`embed_batch_with_options`] with
/// [`EmbedBatchOptions::skip_ollama_entry_eviction`] when the caller runs eviction
/// separately (bulk indexing).
/// Returns one vector per input in the same order.
pub async fn embed_batch(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>, String> {
    embed_batch_with_options(texts, model, EmbedBatchOptions::default()).await
}

/// Same as [`embed_batch`] with explicit eviction policy.
pub async fn embed_batch_with_options(
    texts: &[String],
    model: &str,
    opts: EmbedBatchOptions,
) -> Result<Vec<Vec<f32>>, String> {
    log::info!("[embed_batch] model={model} chunks={}", texts.len());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    async fn embed_batch_once(
        client: &reqwest::Client,
        texts: &[String],
        model: &str,
    ) -> Result<Vec<Vec<f32>>, String> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            input: &'a [String],
            keep_alive: i32,
            truncate: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            embeddings: Option<Vec<Vec<f32>>>,
            embedding: Option<Vec<f32>>,
            error: Option<String>,
        }

        let response = client
            .post("http://localhost:11434/api/embed")
            .json(&Req { model, input: texts, keep_alive: 300, truncate: true })
            .send()
            .await
            .map_err(|e| format!("Could not reach Ollama (batch embed): {e}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Could not read batch embed response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Batch embed HTTP {}: {body}", status));
        }

        let resp: Resp = serde_json::from_str(&body)
            .map_err(|e| format!("Unexpected batch embed response: {e}\\nBody: {body}"))?;

        if let Some(err) = resp.error {
            return Err(format!("Batch embed error: {err}"));
        }

        if let Some(vs) = resp.embeddings {
            return Ok(vs);
        }

        if let Some(v) = resp.embedding {
            return Ok(vec![v]);
        }

        Err(format!("Batch embed response missing embeddings\\nBody: {body}"))
    }

    if texts.is_empty() {
        return Ok(vec![]);
    }

    let slice_cb = opts.on_slice_progress.clone();

    // `/api/embed` can fail sporadically when Ollama's internal runner drops the
    // connection (HTTP 400 with embedded TCP errors). Single-chunk work is common
    // during incremental rescans; use the same `/api/embeddings` path as `embed()`
    // and skip the batch endpoint entirely.
    if texts.len() == 1 {
        // Match multi-chunk batch behavior: keep the model warm (see /api/embed keep_alive).
        let v = embed_with_keep_alive(&texts[0], model, 300).await?;
        if let Some(cb) = slice_cb.as_ref() {
            cb(1, 1);
        }
        return Ok(vec![v]);
    }

    if !opts.skip_ollama_entry_eviction {
        evict_other_models(&client, model).await;
    }

    // Split into API-sized batches up front. Sending thousands of inputs in one `/api/embed`
    // request can stall Ollama for minutes with no feedback (common for large EPUBs).
    let bs = batch_size_for_model(model).max(1);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut s = 0usize;
    while s < texts.len() {
        let e = (s + bs).min(texts.len());
        ranges.push((s, e));
        s = e;
    }

    let mut out: Vec<Option<Vec<f32>>> = vec![None; texts.len()];

    if let Some(cb) = slice_cb.as_ref() {
        if texts.len() > 1 {
            cb(0, texts.len());
        }
    }

    let started_at = std::time::Instant::now();
    const MAX_SPLIT_OPS: u32 = 64;
    const MAX_BATCH_DEGRADE_SECS: u64 = 180;
    let mut split_ops: u32 = 0;
    while let Some((start, end)) = ranges.pop() {
        if started_at.elapsed().as_secs() > MAX_BATCH_DEGRADE_SECS {
            return Err(format!(
                "Batch embedding exceeded degrade time budget ({}s)",
                MAX_BATCH_DEGRADE_SECS
            ));
        }
        let slice = &texts[start..end];
        match embed_batch_once(&client, slice, model).await {
            Ok(embs) if embs.len() == slice.len() => {
                if texts.len() >= 128 {
                    log::info!(
                        "[embed_batch] slice {}..{} / {} chunks",
                        start,
                        end,
                        texts.len()
                    );
                }
                for (i, emb) in embs.into_iter().enumerate() {
                    out[start + i] = Some(emb);
                }
                if let Some(cb) = slice_cb.as_ref() {
                    let ready = out.iter().filter(|o| o.is_some()).count();
                    cb(ready, texts.len());
                }
            }
            Ok(embs) if embs.len() == 1 && slice.len() == 1 => {
                out[start] = embs.into_iter().next();
                if let Some(cb) = slice_cb.as_ref() {
                    let ready = out.iter().filter(|o| o.is_some()).count();
                    cb(ready, texts.len());
                }
            }
            Ok(embs) => {
                let err = format!(
                    "Batch embed returned {} vectors for {} inputs",
                    embs.len(),
                    slice.len()
                );
                if slice.len() == 1 {
                    log::warn!("[embed_batch] single-item batch mismatch: {err}");
                    EMBED_BATCH_SINGLE_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
                    out[start] = Some(embed_with_keep_alive(&texts[start], model, 300).await?);
                    if let Some(cb) = slice_cb.as_ref() {
                        let ready = out.iter().filter(|o| o.is_some()).count();
                        cb(ready, texts.len());
                    }
                } else {
                    let mid = start + (slice.len() / 2);
                    log::warn!("[embed_batch] splitting batch ({start}..{end}) after mismatch: {err}");
                    split_ops = split_ops.saturating_add(1);
                    EMBED_BATCH_SPLIT_COUNT.fetch_add(1, Ordering::Relaxed);
                    if split_ops > MAX_SPLIT_OPS {
                        return Err(format!(
                            "Batch embedding exceeded split budget (>{MAX_SPLIT_OPS})"
                        ));
                    }
                    ranges.push((mid, end));
                    ranges.push((start, mid));
                }
            }
            Err(e) => {
                if slice.len() == 1 {
                    log::warn!("[embed_batch] single-item batch failed, falling back to /api/embeddings: {e}");
                    EMBED_BATCH_SINGLE_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
                    out[start] = Some(embed_with_keep_alive(&texts[start], model, 300).await?);
                    if let Some(cb) = slice_cb.as_ref() {
                        let ready = out.iter().filter(|o| o.is_some()).count();
                        cb(ready, texts.len());
                    }
                } else {
                    let mid = start + (slice.len() / 2);
                    log::warn!("[embed_batch] splitting batch ({start}..{end}) after failure: {e}");
                    split_ops = split_ops.saturating_add(1);
                    EMBED_BATCH_SPLIT_COUNT.fetch_add(1, Ordering::Relaxed);
                    if split_ops > MAX_SPLIT_OPS {
                        return Err(format!(
                            "Batch embedding exceeded split budget (>{MAX_SPLIT_OPS})"
                        ));
                    }
                    ranges.push((mid, end));
                    ranges.push((start, mid));
                }
            }
        }
    }

    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for (i, maybe_emb) in out.into_iter().enumerate() {
        embeddings.push(maybe_emb.ok_or_else(|| format!("Missing embedding for batch index {i}"))?);
    }

    // Normalize each vector to unit length — see embed() for rationale.
    let normalized: Vec<Vec<f32>> = embeddings.into_iter().enumerate()
        .map(|(i, emb)| {
            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 0.1 {
                return Err(format!("Degenerate embedding at index {i} (norm={norm:.4})"));
            }
            Ok(emb.into_iter().map(|x| x / norm).collect())
        })
        .collect::<Result<_, _>>()?;
    Ok(normalized)
}
