use crate::Result;
use crate::query::promql::{self, PromData};
use crate::query::time;

/// Threshold comparison operator.
pub enum ThresholdOp {
    Above(f64),
    Below(f64),
}

/// Configuration for a watch run.
pub struct WatchConfig<'a> {
    pub expression: &'a str,
    pub endpoint: Option<&'a str>,
    pub workspace: Option<&'a str>,
    pub threshold: ThresholdOp,
    /// Poll interval in seconds.
    pub every: u64,
    /// Condition must sustain for this many seconds before triggering (None = immediate).
    pub for_duration: Option<u64>,
    pub json: bool,
}

/// Run the watch loop.
///
/// Returns `Ok(true)` if the threshold was triggered (caller should exit 1),
/// or loops forever until SIGINT kills the process.
pub fn run(config: &WatchConfig) -> Result<bool> {
    let base_url = promql::resolve_endpoint(config.endpoint, config.workspace)?;

    if !config.json {
        eprintln!(
            "Watching: {}  every {}s  threshold {}",
            config.expression,
            config.every,
            format_threshold(&config.threshold),
        );
        if let Some(dur) = config.for_duration {
            eprintln!("Must sustain for {dur}s before alerting");
        }
    }

    let mut first_triggered: Option<std::time::Instant> = None;

    loop {
        match promql::query_instant(&base_url, config.expression) {
            Ok(data) => {
                let (triggered, extreme_val, series_count) =
                    check_threshold(&data, &config.threshold);

                if triggered {
                    let trigger_start = first_triggered.get_or_insert_with(std::time::Instant::now);
                    let triggered_secs = trigger_start.elapsed().as_secs();

                    if let Some(for_dur) = config.for_duration {
                        if triggered_secs >= for_dur {
                            print_status(
                                config,
                                "ALERT",
                                extreme_val,
                                series_count,
                                Some(triggered_secs),
                            );
                            return Ok(true);
                        }
                        print_status(
                            config,
                            "WARN",
                            extreme_val,
                            series_count,
                            Some(triggered_secs),
                        );
                    } else {
                        print_status(config, "ALERT", extreme_val, series_count, None);
                        return Ok(true);
                    }
                } else {
                    first_triggered = None;
                    print_status(config, "OK", extreme_val, series_count, None);
                }
            }
            Err(e) => {
                if config.json {
                    println!(
                        "{}",
                        serde_json::json!({"timestamp": now_formatted(), "status": "error", "error": e.to_string()})
                    );
                } else {
                    eprintln!("[{}] ERROR  {e}", now_formatted());
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(config.every));
    }
}

/// Check if any value in the PromQL result violates the threshold.
///
/// Returns `(triggered, extreme_value, series_count)`.
pub fn check_threshold(data: &PromData, threshold: &ThresholdOp) -> (bool, f64, usize) {
    let mut triggered = false;
    let mut extreme_val = match threshold {
        ThresholdOp::Above(_) => f64::NEG_INFINITY,
        ThresholdOp::Below(_) => f64::INFINITY,
    };

    for series in &data.result {
        // Instant queries use `value`, range queries use `values`.
        let val_str = series
            .value
            .as_ref()
            .map(|(_, v)| v.as_str())
            .or_else(|| series.values.last().map(|(_, v)| v.as_str()));

        if let Some(val_str) = val_str {
            if let Ok(val) = val_str.parse::<f64>() {
                match threshold {
                    ThresholdOp::Above(t) => {
                        if val > extreme_val {
                            extreme_val = val;
                        }
                        if val > *t {
                            triggered = true;
                        }
                    }
                    ThresholdOp::Below(t) => {
                        if val < extreme_val {
                            extreme_val = val;
                        }
                        if val < *t {
                            triggered = true;
                        }
                    }
                }
            }
        }
    }

    (triggered, extreme_val, data.result.len())
}

pub fn format_threshold(threshold: &ThresholdOp) -> String {
    match threshold {
        ThresholdOp::Above(v) => format!("> {v}"),
        ThresholdOp::Below(v) => format!("< {v}"),
    }
}

fn now_formatted() -> String {
    time::format_timestamp(time::now_secs() as f64)
}

fn print_status(
    config: &WatchConfig,
    status: &str,
    value: f64,
    series_count: usize,
    triggered_secs: Option<u64>,
) {
    if config.json {
        let mut obj = serde_json::json!({
            "timestamp": now_formatted(),
            "status": status.to_lowercase(),
            "value": value,
            "threshold": format_threshold(&config.threshold),
            "series_count": series_count,
        });
        if let Some(secs) = triggered_secs {
            obj["triggered_for_secs"] = serde_json::json!(secs);
        }
        println!("{obj}");
    } else {
        let triggered_str = match triggered_secs {
            Some(s) => format!("  [triggered {s}s]"),
            None => String::new(),
        };
        let pad = match status {
            "OK" => "     ",
            "WARN" => "   ",
            _ => "  ",
        };
        println!(
            "[{}] {status}{pad}value={value:.6}  ({})  {series_count} series{triggered_str}",
            now_formatted(),
            format_threshold(&config.threshold),
        );
    }
}
