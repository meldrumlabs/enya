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

#[cfg(test)]
mod tests {
    use super::*;

    // -- is_retryable --

    #[test]
    fn rate_limited_is_retryable() {
        let err = AgentError::RateLimited {
            retry_after_secs: Some(30),
        };
        assert!(is_retryable(&err));
    }

    #[test]
    fn rate_limited_without_retry_after_is_retryable() {
        let err = AgentError::RateLimited {
            retry_after_secs: None,
        };
        assert!(is_retryable(&err));
    }

    #[test]
    fn server_error_is_retryable() {
        let err = AgentError::Http("HTTP 500".into());
        assert!(is_retryable(&err));

        let err = AgentError::Http("HTTP 502".into());
        assert!(is_retryable(&err));

        let err = AgentError::Http("HTTP 503".into());
        assert!(is_retryable(&err));
    }

    #[test]
    fn connection_error_is_retryable() {
        let err = AgentError::Http("connection refused".into());
        assert!(is_retryable(&err));

        let err = AgentError::Http("connection timed out".into());
        assert!(is_retryable(&err));

        let err = AgentError::Http("connection reset".into());
        assert!(is_retryable(&err));
    }

    #[test]
    fn auth_error_not_retryable() {
        let err = AgentError::Auth("bad key".into());
        assert!(!is_retryable(&err));
    }

    #[test]
    fn parse_error_not_retryable() {
        let err = AgentError::Parse("invalid json".into());
        assert!(!is_retryable(&err));
    }

    #[test]
    fn client_error_not_retryable() {
        let err = AgentError::Http("HTTP 400".into());
        assert!(!is_retryable(&err));
    }

    #[test]
    fn process_error_not_retryable() {
        let err = AgentError::Process("spawn failed".into());
        assert!(!is_retryable(&err));
    }

    // -- classify_http_error --

    #[test]
    fn classify_401_as_auth() {
        match classify_http_error(401) {
            AgentError::Auth(_) => {}
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[test]
    fn classify_429_as_rate_limited() {
        match classify_http_error(429) {
            AgentError::RateLimited { .. } => {}
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn classify_500_as_http() {
        match classify_http_error(500) {
            AgentError::Http(msg) => assert!(msg.contains("500")),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn classify_400_as_http() {
        match classify_http_error(400) {
            AgentError::Http(msg) => assert!(msg.contains("400")),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    // -- calculate_delay --

    #[test]
    fn delay_exponential_backoff() {
        let d1 = calculate_delay(1, None);
        let d2 = calculate_delay(2, None);
        let d3 = calculate_delay(3, None);

        assert_eq!(d1, Duration::from_millis(500));
        assert_eq!(d2, Duration::from_millis(1000));
        assert_eq!(d3, Duration::from_millis(2000));
    }

    #[test]
    fn delay_capped_at_max() {
        let d = calculate_delay(20, None);
        assert_eq!(d, Duration::from_millis(MAX_DELAY_MS));
    }

    #[test]
    fn delay_honours_retry_after() {
        let err = AgentError::RateLimited {
            retry_after_secs: Some(45),
        };
        let d = calculate_delay(1, Some(&err));
        assert_eq!(d, Duration::from_secs(45));
    }

    #[test]
    fn delay_ignores_non_rate_limit_errors() {
        let err = AgentError::Http("HTTP 500".into());
        let d = calculate_delay(1, Some(&err));
        assert_eq!(d, Duration::from_millis(500));
    }
}
