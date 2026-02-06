//! Shared HTTP utilities for LLM provider clients.
//!
//! Centralises the ureq POST + HTTP error classification logic so that
//! retry, header handling, and error mapping live in one place.

use std::sync::mpsc::SyncSender;
use std::time::Duration;

use crate::types::{AgentError, AgentEvent};

/// Maximum number of automatic retries for transient errors.
const MAX_RETRIES: u32 = 3;

/// Initial backoff delay between retries (milliseconds).
const BASE_DELAY_MS: u64 = 500;

/// Maximum backoff delay cap (milliseconds).
const MAX_DELAY_MS: u64 = 30_000;

/// Perform a streaming POST request to an LLM API with automatic retry.
///
/// Retries on transient errors (429 rate-limit, 5xx server errors, network
/// errors) with exponential backoff.  Non-retryable errors (401 auth, 400
/// bad request, parse errors) are returned immediately.
///
/// The `parse` callback receives the HTTP response body and should read SSE
/// events, emitting [`AgentEvent`]s through `tx`.
pub fn streaming_post(
    url: &str,
    headers: &[(&str, &str)],
    body: &str,
    parse: fn(ureq::Body, &SyncSender<AgentEvent>) -> Result<(), AgentError>,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let mut last_error = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = calculate_delay(attempt, last_error.as_ref());
            tracing::warn!(
                attempt,
                delay_ms = delay.as_millis(),
                "retrying LLM request"
            );
            std::thread::sleep(delay);
        }

        match do_post(url, headers, body, parse, tx) {
            Ok(()) => return Ok(()),
            Err(e) if is_retryable(&e) => {
                tracing::warn!(attempt, error = %e, "transient error");
                last_error = Some(e);
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_error.expect("at least one attempt"))
}

/// Single-attempt POST request.
fn do_post(
    url: &str,
    headers: &[(&str, &str)],
    body: &str,
    parse: fn(ureq::Body, &SyncSender<AgentEvent>) -> Result<(), AgentError>,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let mut req = ureq::post(url).header("Content-Type", "application/json");
    for &(key, value) in headers {
        req = req.header(key, value);
    }

    let response = match req.send(body) {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(classify_http_error(status));
        }
        Err(e) => return Err(classify_network_error(&e)),
    };

    parse(response.into_body(), tx)
}

/// Map an HTTP status code to the appropriate [`AgentError`] variant.
fn classify_http_error(status: u16) -> AgentError {
    match status {
        401 => AgentError::Auth("Invalid API key".into()),
        429 => AgentError::RateLimited {
            retry_after_secs: None,
        },
        _ => AgentError::Http(format!("HTTP {status}")),
    }
}

/// Map a ureq network/transport error to [`AgentError`].
fn classify_network_error(e: &ureq::Error) -> AgentError {
    AgentError::Http(e.to_string())
}

/// Whether an error is worth retrying.
fn is_retryable(e: &AgentError) -> bool {
    match e {
        AgentError::RateLimited { .. } => true,
        AgentError::Http(msg) => {
            // 5xx server errors
            msg.starts_with("HTTP 5")
                // network/connection errors from ureq
                || msg.contains("connection")
                || msg.contains("timed out")
                || msg.contains("reset")
        }
        _ => false,
    }
}

/// Calculate the retry delay with exponential backoff.
fn calculate_delay(attempt: u32, last_error: Option<&AgentError>) -> Duration {
    // Honour Retry-After if the server provided one
    if let Some(AgentError::RateLimited {
        retry_after_secs: Some(secs),
    }) = last_error
    {
        return Duration::from_secs(*secs);
    }
    // Exponential backoff: 500ms, 1s, 2s, ...
    let millis = BASE_DELAY_MS.saturating_mul(2u64.pow(attempt - 1));
    Duration::from_millis(millis.min(MAX_DELAY_MS))
}
