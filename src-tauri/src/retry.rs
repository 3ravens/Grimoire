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

//! Cancellation-aware exponential backoff for transient background operations.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Run `op` up to `max_retries + 1` times (so `max_retries = 0` is a single attempt).
/// Between failed attempts, sleeps with exponential backoff capped at ~3.2s.
///
/// If `cancel` is `Some((flag, err))` and `flag` is set before an attempt, returns `Err(err.clone())`.
pub async fn with_retries<E, F, Fut, T>(
    max_retries: i64,
    cancel: Option<(&Arc<AtomicBool>, E)>,
    mut op: F,
) -> Result<T, E>
where
    E: Clone,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let attempts = (max_retries.max(0) as u32).saturating_add(1);
    let mut last_err: Option<E> = None;
    for attempt in 1..=attempts {
        if let Some((flag, cancel_err)) = cancel.as_ref() {
            if flag.load(Ordering::Relaxed) {
                return Err(cancel_err.clone());
            }
        }
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt < attempts {
                    let delay_ms = 200u64.saturating_mul(1u64 << (attempt - 1).min(4));
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

/// Like [`with_retries`], but caps inter-attempt delay at `max_delay_ms` (default-style
/// exponential backoff: 200, 400, 800, …). Intended for background pipelines where
/// long sleeps (multi-second) hurt throughput more than they help recovery.
pub async fn with_retries_background<E, F, Fut, T>(
    max_retries: i64,
    cancel: Option<(&Arc<AtomicBool>, E)>,
    max_delay_ms: u64,
    mut op: F,
) -> Result<T, E>
where
    E: Clone,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let attempts = (max_retries.max(0) as u32).saturating_add(1);
    let mut last_err: Option<E> = None;
    for attempt in 1..=attempts {
        if let Some((flag, cancel_err)) = cancel.as_ref() {
            if flag.load(Ordering::Relaxed) {
                return Err(cancel_err.clone());
            }
        }
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt < attempts {
                    let raw = 200u64.saturating_mul(1u64 << (attempt - 1).min(4));
                    let delay_ms = raw.min(max_delay_ms.max(1));
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

/// [`with_retries`] with capped backoff; returns `(value, attempt_index)` on success
/// where `attempt_index` is 1-based (1 = first try succeeded, 2 = one retry, …).
pub async fn with_retries_counting_background<E, F, Fut, T>(
    max_retries: i64,
    cancel: Option<(&Arc<AtomicBool>, E)>,
    max_delay_ms: u64,
    mut op: F,
) -> Result<(T, u32), E>
where
    E: Clone,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let attempts = (max_retries.max(0) as u32).saturating_add(1);
    let mut last_err: Option<E> = None;
    for attempt in 1..=attempts {
        if let Some((flag, cancel_err)) = cancel.as_ref() {
            if flag.load(Ordering::Relaxed) {
                return Err(cancel_err.clone());
            }
        }
        match op().await {
            Ok(v) => return Ok((v, attempt)),
            Err(e) => {
                last_err = Some(e);
                if attempt < attempts {
                    let raw = 200u64.saturating_mul(1u64 << (attempt - 1).min(4));
                    let delay_ms = raw.min(max_delay_ms.max(1));
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}
