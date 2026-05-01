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

//! Structured application error type returned by all Tauri commands.
//!
//! Serialized over IPC as `{"kind": "<Variant>", "message": "<human text>"}` so
//! the frontend can pattern-match on `kind` and show actionable hints.

use std::fmt;
use serde::Serialize;

/// All failure categories that can be returned by a Tauri command.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    /// SQLite query or connection failure.
    Database(String),
    /// A requested record does not exist.
    NotFound(String),
    /// Ollama is not reachable (process not running, wrong port, etc.).
    OllamaUnavailable(String),
    /// Embedding model returned an error or an unusable result.
    EmbeddingFailed(String),
    /// File-system I/O failure (read, write, path resolution).
    Io(String),
    /// The caller supplied invalid or inconsistent arguments.
    InvalidInput(String),
    /// Authentication failure (wrong password, key not present).
    Auth(String),
    /// LanceDB vector store failure (open, insert, query).
    VectorStore(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(m)         => write!(f, "{m}"),
            AppError::NotFound(m)         => write!(f, "{m}"),
            AppError::OllamaUnavailable(m) => write!(f, "{m}"),
            AppError::EmbeddingFailed(m)  => write!(f, "{m}"),
            AppError::Io(m)               => write!(f, "{m}"),
            AppError::InvalidInput(m)     => write!(f, "{m}"),
            AppError::Auth(m)             => write!(f, "{m}"),
            AppError::VectorStore(m)      => write!(f, "{m}"),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("Record not found".to_string()),
            other => AppError::Database(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

/// Convenience alias used by all command return types.
pub type AppResult<T> = Result<T, AppError>;
