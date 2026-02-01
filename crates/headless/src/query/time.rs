use std::time::SystemTime;

use crate::Result;

/// Get the current time as Unix seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Parse a time specification into Unix seconds.
///
/// Supported formats:
/// - `"now"` → current time
/// - Relative duration: `"1h"`, `"30m"`, `"-2d"` → subtracted from now
/// - Unix timestamp: `"1704067200"` → used as-is
/// - ISO 8601: `"2024-01-01T00:00:00Z"` → parsed via chrono
pub fn parse_time(spec: &str, now: u64) -> Result<u64> {
    let spec = spec.trim();

    if spec.eq_ignore_ascii_case("now") {
        return Ok(now);
    }

    // Try as a pure Unix timestamp (all digits)
    if spec.chars().all(|c| c.is_ascii_digit()) && !spec.is_empty() {
        return spec.parse::<u64>().map_err(|e| e.into());
    }

    // Try as ISO 8601 (contains 'T' or starts with 4 digits + '-')
    if spec.contains('T') || (spec.len() >= 10 && spec[4..5] == *"-") {
        let dt = chrono::DateTime::parse_from_rfc3339(spec)
            .or_else(|_| {
                // Try without timezone (assume UTC)
                chrono::NaiveDateTime::parse_from_str(spec, "%Y-%m-%dT%H:%M:%S")
                    .map(|naive| naive.and_utc().fixed_offset())
            })
            .map_err(|e| format!("invalid timestamp '{spec}': {e}"))?;
        return Ok(dt.timestamp() as u64);
    }

    // Try as relative duration (strip leading '-' if present)
    let dur_str = spec.strip_prefix('-').unwrap_or(spec);
    let secs = parse_duration_secs(dur_str)?;
    Ok(now.saturating_sub(secs))
}

/// Parse a duration string into seconds.
///
/// Supported: `"15s"`, `"1m"`, `"5m"`, `"1h"`, `"6h"`, `"1d"`, `"7d"`
pub fn parse_duration_secs(spec: &str) -> Result<u64> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty duration".into());
    }

    let (num_str, suffix) = spec.split_at(spec.len() - 1);
    let num: u64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration '{spec}' (expected e.g. 15s, 5m, 1h, 1d)"))?;

    let multiplier = match suffix {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        "w" => 604800,
        _ => return Err(format!("unknown duration suffix '{suffix}' (use s, m, h, d, w)").into()),
    };

    Ok(num * multiplier)
}

/// Format a Unix timestamp as a human-readable UTC string.
pub fn format_timestamp(ts: f64) -> String {
    use chrono::{DateTime, Utc};
    let nanos = (ts.fract().abs() * 1e9) as u32;
    DateTime::<Utc>::from_timestamp(ts as i64, nanos)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| format!("{ts}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration_secs("15s").unwrap(), 15);
        assert_eq!(parse_duration_secs("5m").unwrap(), 300);
        assert_eq!(parse_duration_secs("1h").unwrap(), 3600);
        assert_eq!(parse_duration_secs("2d").unwrap(), 172800);
        assert_eq!(parse_duration_secs("1w").unwrap(), 604800);
    }

    #[test]
    fn test_parse_time_now() {
        let now = 1700000000;
        assert_eq!(parse_time("now", now).unwrap(), now);
    }

    #[test]
    fn test_parse_time_relative() {
        let now = 1700000000;
        assert_eq!(parse_time("1h", now).unwrap(), now - 3600);
        assert_eq!(parse_time("-30m", now).unwrap(), now - 1800);
    }

    #[test]
    fn test_parse_time_unix() {
        let now = 1700000000;
        assert_eq!(parse_time("1704067200", now).unwrap(), 1704067200);
    }

    #[test]
    fn test_parse_time_iso8601() {
        let now = 1700000000;
        assert_eq!(parse_time("2024-01-01T00:00:00Z", now).unwrap(), 1704067200);
    }
}
