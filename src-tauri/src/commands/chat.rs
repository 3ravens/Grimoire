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

use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::{AppError, AppResult};

fn ollama_http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(7200))
        .build()
        .map_err(|e| AppError::OllamaUnavailable(format!("HTTP client: {e}")))
}

// ---------------------------------------------------------------------------
// Chat (Ollama)
// ---------------------------------------------------------------------------

const IMPROVE_SYSTEM_PROMPT: &str = r#"You are an expert note editor. Improve the following note according to the user's instruction. Return ONLY the complete improved note text. No explanations, no markdown fences, no commentary. Preserve all existing markdown formatting unless the instruction says otherwise."#;

const REFINE_HUNK_SYSTEM_PROMPT: &str = r#"You are an expert note editor. The user is reviewing a specific section of text that was changed by an earlier LLM suggestion. Rewrite ONLY that section according to the user's instruction. Return ONLY the rewritten section text — no explanations, no markdown fences, no commentary, no surrounding context. Preserve existing markdown formatting unless the instruction says otherwise."#;

/// A single message in a conversation. `role` is "user" or "assistant".
/// Both `Serialize` (to send to Ollama) and `Deserialize` (to receive from the frontend) are needed.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// The request body sent to Ollama's /api/chat endpoint.
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    /// Seconds to keep the model loaded after the request finishes.
    /// -1 = keep forever (keep-in-memory setting), 300 = default 5-minute timeout.
    keep_alive: i64,
    options: OllamaOptions,
}

/// Runtime options forwarded to Ollama on every request.
/// `num_thread` caps the number of CPU threads Ollama uses for inference,
/// leaving headroom for the OS and other running applications.
/// The remaining fields are user-configurable inference parameters.
#[derive(Serialize)]
struct OllamaOptions {
    num_thread: usize,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    num_ctx: i32,
}

impl OllamaOptions {
    fn new(temperature: f32, top_p: f32, top_k: i32, repeat_penalty: f32, num_ctx: i32) -> Self {
        let total = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self {
            num_thread: (total / 2).max(1),
            temperature,
            top_p,
            top_k,
            repeat_penalty,
            num_ctx,
        }
    }
}

/// One line in Ollama's NDJSON streaming response.
/// `done` is true on the final (empty) message that signals end of stream.
#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: ChatMessage,
    done: bool,
}

/// Send a chat message to a locally-running Ollama instance.
/// Tokens are emitted incrementally via the `chat:token` Tauri event as they
/// arrive from Ollama. The command resolves once the stream is complete.
/// `keep_in_memory`: when true, keep_alive is set to -1 so the model is
/// never unloaded; when false the Ollama default (300s) is used.
#[tauri::command(rename_all = "camelCase")]
pub async fn chat(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    model: String,
    messages: Vec<ChatMessage>,
    keep_in_memory: bool,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    num_ctx: i32,
) -> AppResult<()> {
    use tauri::Emitter;

    // Capture the last user message for the audit log before the messages vec is moved.
    let audit_detail: String = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| crate::audit::truncate(&m.content, 500).to_string())
        .unwrap_or_default();

    let client = ollama_http_client()?;

    let body = OllamaChatRequest {
        model,
        messages,
        stream: true,
        keep_alive: if keep_in_memory { -1 } else { 300 },
        options: OllamaOptions::new(temperature, top_p, top_k, repeat_penalty, num_ctx),
    };

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::OllamaUnavailable(format!("Could not reach Ollama — is it running? ({e})")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!("Ollama returned {status}: {body}")));
    }

    // Ollama streams NDJSON: one JSON object per line, terminated by a final
    // object with `"done": true`. We buffer bytes into lines and parse each one.
    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::OllamaUnavailable(format!("Stream read error: {e}")))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| AppError::OllamaUnavailable(format!("UTF-8 error: {e}")))?;

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                if line.is_empty() { continue; }

                let parsed: OllamaStreamChunk = serde_json::from_str(&line)
                    .map_err(|e| AppError::OllamaUnavailable(format!("Unexpected Ollama chunk: {e}\nLine: {line}")))?;

                if !parsed.done && !parsed.message.content.is_empty() {
                    app.emit("chat:token", &parsed.message.content)
                        .map_err(|e| AppError::OllamaUnavailable(format!("Event emit error: {e}")))?;
                }

                if parsed.done {
                    let _ = crate::audit::log_event(
                        pool.inner(), "llm_chat", None, None, None,
                        if audit_detail.is_empty() { None } else { Some(audit_detail.as_str()) },
                    ).await;
                    return Ok(());
                }
            } else {
                line_buf.push(ch);
            }
        }
    }

    Ok(())
}

/// Debug-only: measure semantic retrieval + Ollama time-to-first-token for a RAG-style chat.
/// Does not emit `chat:token` events; does not write audit entries for the chat call.
#[cfg(debug_assertions)]
#[derive(Serialize)]
pub struct RagChatTtftBench {
    /// Wall-clock from start until first non-empty assistant token (retrieval + chat).
    pub total_ms_to_first_token: u64,
    pub retrieval_ms: u64,
    pub note_match_count: usize,
    pub chat_model: String,
    pub embedding_model: String,
}

#[cfg(debug_assertions)]
pub async fn measure_rag_chat_ttft(
    pool: &SqlitePool,
    keys: &crate::SharedKeyStore,
    vdb: &lancedb::Connection,
    embedding_model: &str,
    query: &str,
    chat_model: &str,
) -> AppResult<RagChatTtftBench> {
    use std::time::Instant;

    let wall = Instant::now();

    let r0 = Instant::now();
    let matches = super::rag::search_notes_semantic(
        pool,
        keys,
        vdb,
        embedding_model,
        query,
        crate::vector::notes::CHUNK_FETCH_LIMIT,
        false,
    )
    .await?;
    let retrieval_ms = r0.elapsed().as_millis() as u64;

    let mut user_notes = String::new();
    for m in &matches {
        let body = m.excerpts.join("\n");
        user_notes.push_str(&format!("[Note: \"{}\"]\n{}\n\n", m.title, body));
    }
    let system = if user_notes.is_empty() {
        "You are a concise assistant. No notes matched; answer briefly from general knowledge."
            .to_string()
    } else {
        format!(
            "You are a personal knowledge assistant. Answer using the notes below.\n\nUSER NOTES:\n{}",
            user_notes.trim_end()
        )
    };

    let client = ollama_http_client()?;
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system,
        },
        ChatMessage {
            role: "user".into(),
            content: format!("{query}\n\nReply in one short sentence."),
        },
    ];
    let body = OllamaChatRequest {
        model: chat_model.to_string(),
        messages,
        stream: true,
        keep_alive: 300,
        options: OllamaOptions::new(0.2, 0.9, 40, 1.1, 2048),
    };

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            AppError::OllamaUnavailable(format!("Could not reach Ollama — is it running? ({e})"))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!(
            "Ollama returned {status}: {text}"
        )));
    }

    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut first_token_ms: Option<u64> = None;

    'stream: while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::OllamaUnavailable(format!("Stream read error: {e}")))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| AppError::OllamaUnavailable(format!("UTF-8 error: {e}")))?;

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                if line.is_empty() {
                    continue;
                }

                let parsed: OllamaStreamChunk = serde_json::from_str(&line).map_err(|e| {
                    AppError::OllamaUnavailable(format!("Unexpected Ollama chunk: {e}\nLine: {line}"))
                })?;

                if !parsed.done && !parsed.message.content.is_empty() && first_token_ms.is_none() {
                    first_token_ms = Some(wall.elapsed().as_millis() as u64);
                    break 'stream;
                }

                if parsed.done {
                    break 'stream;
                }
            } else {
                line_buf.push(ch);
            }
        }
    }

    let total_ms_to_first_token = first_token_ms.unwrap_or_else(|| wall.elapsed().as_millis() as u64);

    Ok(RagChatTtftBench {
        total_ms_to_first_token,
        retrieval_ms,
        note_match_count: matches.len(),
        chat_model: chat_model.to_string(),
        embedding_model: embedding_model.to_string(),
    })
}

#[cfg(debug_assertions)]
#[tauri::command(rename_all = "camelCase")]
pub async fn benchmark_rag_chat_ttft(
    pool: State<'_, SqlitePool>,
    keys: State<'_, crate::SharedKeyStore>,
    vdb: State<'_, crate::vector::VectorDb>,
    config: State<'_, crate::config::SharedConfig>,
    query: String,
    chat_model: Option<String>,
) -> AppResult<serde_json::Value> {
    let embedding_model = config.read().unwrap().embedding_model.clone();
    let chat_m = chat_model.unwrap_or_else(|| {
        std::env::var("PERF_CHAT_MODEL").unwrap_or_else(|_| {
            crate::perf_budget::DEFAULT_BENCHMARK_CHAT_MODEL.to_string()
        })
    });
    let out = measure_rag_chat_ttft(
        pool.inner(),
        keys.inner(),
        &vdb.0,
        &embedding_model,
        &query,
        &chat_m,
    )
    .await?;
    Ok(serde_json::to_value(out).unwrap_or_default())
}

/// Ask the LLM to improve a note's content.
/// Streams the improved text via the `note:improve-token` Tauri event.
#[tauri::command(rename_all = "camelCase")]
pub async fn suggest_note_improvement(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    model: String,
    note_id: Option<i64>,
    note_title: Option<String>,
    note_content: String,
    instruction: String,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    num_ctx: i32,
) -> AppResult<()> {
    use tauri::Emitter;

    let _ = crate::audit::log_event(
        pool.inner(), "llm_improve", Some("note"),
        note_id, note_title.as_deref(),
        Some(crate::audit::truncate(&instruction, 500)),
    ).await;
    // Using a single user message avoids issues with models that don't support the `system` role.
    let user_content = format!(
        "{}\n\nUser instruction: {}\n\nNote content:\n{}",
        IMPROVE_SYSTEM_PROMPT, instruction, note_content
    );

    let messages = vec![
        ChatMessage { role: "user".to_string(), content: user_content },
    ];

    let client = ollama_http_client()?;

    let body = OllamaChatRequest {
        model,
        messages,
        stream: true,
        keep_alive: 300,
        options: OllamaOptions::new(temperature, top_p, top_k, repeat_penalty, num_ctx),
    };

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::OllamaUnavailable(format!("Could not reach Ollama — is it running? ({e})")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!("Ollama returned {status}: {body}")));
    }

    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::OllamaUnavailable(format!("Stream read error: {e}")))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| AppError::OllamaUnavailable(format!("UTF-8 error: {e}")))?;

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                if line.is_empty() { continue; }

                let parsed: OllamaStreamChunk = serde_json::from_str(&line)
                    .map_err(|e| AppError::OllamaUnavailable(format!("Unexpected Ollama chunk: {e}\nLine: {line}")))?;

                if !parsed.done && !parsed.message.content.is_empty() {
                    app.emit("note:improve-token", &parsed.message.content)
                        .map_err(|e| AppError::OllamaUnavailable(format!("Event emit error: {e}")))?;
                }

                if parsed.done {
                    return Ok(());
                }
            } else {
                line_buf.push(ch);
            }
        }
    }

    Ok(())
}

/// Ask the LLM to refine a single diff hunk's content.
/// Streams the refined text via the `note:refine-hunk-token` Tauri event.
#[tauri::command(rename_all = "camelCase")]
pub async fn suggest_hunk_refinement(
    app: tauri::AppHandle,
    model: String,
    hunk_content: String,
    instruction: String,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    num_ctx: i32,
) -> AppResult<()> {
    use tauri::Emitter;

    let user_content = format!(
        "{}\n\nUser instruction: {}\n\nSection to rewrite:\n{}",
        REFINE_HUNK_SYSTEM_PROMPT, instruction, hunk_content
    );

    let messages = vec![
        ChatMessage { role: "user".to_string(), content: user_content },
    ];

    let client = ollama_http_client()?;

    let body = OllamaChatRequest {
        model,
        messages,
        stream: true,
        keep_alive: 300,
        options: OllamaOptions::new(temperature, top_p, top_k, repeat_penalty, num_ctx),
    };

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::OllamaUnavailable(format!("Could not reach Ollama — is it running? ({e})")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!("Ollama returned {status}: {body}")));
    }

    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::OllamaUnavailable(format!("Stream read error: {e}")))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| AppError::OllamaUnavailable(format!("UTF-8 error: {e}")))?;

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                if line.is_empty() { continue; }

                let parsed: OllamaStreamChunk = serde_json::from_str(&line)
                    .map_err(|e| AppError::OllamaUnavailable(format!("Unexpected Ollama chunk: {e}\nLine: {line}")))?;

                if !parsed.done && !parsed.message.content.is_empty() {
                    app.emit("note:refine-hunk-token", &parsed.message.content)
                        .map_err(|e| AppError::OllamaUnavailable(format!("Event emit error: {e}")))?;
                }

                if parsed.done {
                    return Ok(());
                }
            } else {
                line_buf.push(ch);
            }
        }
    }

    Ok(())
}
