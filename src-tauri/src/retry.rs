
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[tokio::test]
    async fn with_retries_ok_first_attempt() {
        let r: Result<i32, &str> = with_retries(3, None, || async { Ok(42) }).await;
        assert_eq!(r, Ok(42));
    }

    #[tokio::test]
    async fn with_retries_zero_means_single_failed_attempt() {
        let r: Result<(), &str> = with_retries(0, None, || async { Err("nope") }).await;
        assert_eq!(r, Err("nope"));
    }

    #[tokio::test]
    async fn with_retries_cancel_before_attempt() {
        let flag = Arc::new(AtomicBool::new(true));
        let cancel: (&Arc<AtomicBool>, &str) = (&flag, "stopped");
        let r: Result<(), &str> = with_retries(5, Some(cancel), || async { Ok(()) }).await;
        assert_eq!(r, Err("stopped"));
    }

    #[tokio::test]
    async fn with_retries_succeeds_on_second_attempt() {
        let n = Arc::new(AtomicU32::new(0));
        let n2 = n.clone();
        let r: Result<&str, &str> = with_retries(
            2,
            None,
            move || {
                let n2 = n2.clone();
                async move {
                    if n2.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err("retry")
                    } else {
                        Ok("done")
                    }
                }
            },
        )
        .await;
        assert_eq!(r, Ok("done"));
    }

    #[tokio::test]
    async fn with_retries_background_caps_delay() {
        let n = Arc::new(AtomicU32::new(0));
        let n2 = n.clone();
        let r: Result<(), &str> = with_retries_background(
            1,
            None,
            50,
            move || {
                let n2 = n2.clone();
                async move {
                    if n2.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err("once")
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await;
        assert_eq!(r, Ok(()));
    }

    #[tokio::test]
    async fn with_retries_counting_background_returns_attempt_index() {
        let n = Arc::new(AtomicU32::new(0));
        let n2 = n.clone();
        let r = with_retries_counting_background(
            2,
            None,
            50,
            move || {
                let n2 = n2.clone();
                async move {
                    if n2.fetch_add(1, Ordering::SeqCst) < 2 {
                        Err("x")
                    } else {
                        Ok(99_u8)
                    }
                }
            },
        )
        .await;
        assert_eq!(r, Ok((99, 3)));
    }
}
