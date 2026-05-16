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

//! List locally installed Ollama models and stream `ollama pull` progress to the UI.

use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::{AppError, AppResult};

const OLLAMA_BASE: &str = "http://localhost:11434";

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

/// Split an Ollama model id into `(base, optional_tag)` after trimming.
/// `"phi3"` → `("phi3", None)`, `"gemma2:2b"` → `("gemma2", Some("2b"))`.
fn split_ollama_model_name(s: &str) -> (&str, Option<&str>) {
    let s = s.trim();
    if s.is_empty() {
        return ("", None);
    }
    match s.split_once(':') {
        Some((b, t)) if !t.is_empty() => (b, Some(t)),
        _ => (s, None),
    }
}

/// True if `installed_full` satisfies the user's `requested` id (exact match or base/tag rules).
pub fn ollama_installed_matches_request(installed_full: &str, requested: &str) -> bool {
    let requested = requested.trim();
    if requested.is_empty() {
        return false;
    }
    if installed_full == requested {
        return true;
    }
    let (req_base, req_tag) = split_ollama_model_name(requested);
    let (ins_base, ins_tag) = split_ollama_model_name(installed_full);
    if req_base != ins_base {
        return false;
    }
    match req_tag {
        None => true,
        Some(rt) => ins_tag == Some(rt),
    }
}

pub fn is_ollama_model_installed(requested: &str, installed: &[String]) -> bool {
    installed
        .iter()
        .any(|i| ollama_installed_matches_request(i, requested))
}

async fn fetch_installed_model_names() -> AppResult<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::OllamaUnavailable(format!("HTTP client: {e}")))?;

    let response = client
        .get(format!("{OLLAMA_BASE}/api/tags"))
        .send()
        .await
        .map_err(|e| {
            AppError::OllamaUnavailable(format!(
                "Could not reach Ollama — is it running? ({e})"
            ))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!(
            "Ollama returned {status}: {body}"
        )));
    }

    let tags: TagsResponse = response
        .json()
        .await
        .map_err(|e| AppError::OllamaUnavailable(format!("Invalid /api/tags JSON: {e}")))?;

    Ok(tags.models.into_iter().map(|m| m.name).collect())
}

/// Return all locally installed model names (e.g. `llama3.2:latest`).
#[tauri::command]
pub async fn list_ollama_installed_models() -> AppResult<Vec<String>> {
    fetch_installed_model_names().await
}

/// Whether a model matching `model` is already present locally.
#[tauri::command]
pub async fn ollama_model_installed(model: String) -> AppResult<bool> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err(AppError::InvalidInput(
            "Model name must not be empty.".to_string(),
        ));
    }
    let installed = fetch_installed_model_names().await?;
    Ok(is_ollama_model_installed(&model, &installed))
}

#[derive(Serialize)]
struct PullRequestBody {
    name: String,
    stream: bool,
}

/// Pull a model via Ollama (`POST /api/pull`). Streams NDJSON progress as `ollama:pull_progress` events.
#[tauri::command]
pub async fn pull_ollama_model(app: tauri::AppHandle, model: String) -> AppResult<()> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err(AppError::InvalidInput(
            "Model name must not be empty.".to_string(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(7200))
        .build()
        .map_err(|e| AppError::OllamaUnavailable(format!("HTTP client: {e}")))?;

    let body = PullRequestBody {
        name: model.clone(),
        stream: true,
    };

    let response = client
        .post(format!("{OLLAMA_BASE}/api/pull"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            AppError::OllamaUnavailable(format!(
                "Could not reach Ollama — is it running? ({e})"
            ))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!(
            "Ollama pull returned {status}: {text}"
        )));
    }

    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::OllamaUnavailable(format!("Pull stream error: {e}")))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| AppError::OllamaUnavailable(format!("UTF-8 error: {e}")))?;

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                if line.is_empty() {
                    continue;
                }

                let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
                    AppError::OllamaUnavailable(format!("Unexpected pull chunk: {e}\nLine: {line}"))
                })?;

                if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                    return Err(AppError::OllamaUnavailable(err.to_string()));
                }

                app.emit("ollama:pull_progress", &v).map_err(|e| {
                    AppError::OllamaUnavailable(format!("Event emit error: {e}"))
                })?;
            } else {
                line_buf.push(ch);
            }
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct DeleteRequestBody {
    model: String,
}

/// Delete a locally installed model via Ollama (`DELETE /api/delete`).
/// Resolves `model` against `/api/tags` the same way as install checks (base / tag rules).
#[tauri::command]
pub async fn delete_ollama_model(model: String) -> AppResult<()> {
    let requested = model.trim().to_string();
    if requested.is_empty() {
        return Err(AppError::InvalidInput(
            "Model name must not be empty.".to_string(),
        ));
    }

    let installed = fetch_installed_model_names().await?;
    let to_delete = installed
        .iter()
        .find(|i| ollama_installed_matches_request(i, &requested))
        .cloned()
        .ok_or_else(|| {
            AppError::InvalidInput("That model is not installed locally.".to_string())
        })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::OllamaUnavailable(format!("HTTP client: {e}")))?;

    let response = client
        .delete(format!("{OLLAMA_BASE}/api/delete"))
        .json(&DeleteRequestBody {
            model: to_delete,
        })
        .send()
        .await
        .map_err(|e| {
            AppError::OllamaUnavailable(format!(
                "Could not reach Ollama — is it running? ({e})"
            ))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::OllamaUnavailable(format!(
            "Ollama delete returned {status}: {text}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_without_tag_matches_any_variant() {
        let installed = vec!["phi3:latest".to_string(), "mistral:7b".to_string()];
        assert!(is_ollama_model_installed("phi3", &installed));
        assert!(!is_ollama_model_installed("phi3", &vec!["phi3.5:latest".to_string()]));
    }

    #[test]
    fn tag_must_match_when_specified() {
        let installed = vec!["phi3:latest".to_string()];
        assert!(is_ollama_model_installed("phi3:latest", &installed));
        assert!(!is_ollama_model_installed("phi3:3.8b", &installed));
    }

    #[test]
    fn llama3_is_not_llama3_2() {
        let installed = vec!["llama3.2:latest".to_string()];
        assert!(!is_ollama_model_installed("llama3", &installed));
        assert!(is_ollama_model_installed("llama3.2", &installed));
    }

    #[test]
    fn exact_string_match() {
        let installed = vec!["gemma2:2b".to_string()];
        assert!(is_ollama_model_installed("gemma2:2b", &installed));
    }
}
