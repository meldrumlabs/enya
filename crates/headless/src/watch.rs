use console::style;

use crate::Result;
use crate::query::promql::{self, PromData};
use crate::query::time;

/// Threshold comparison operator.
pub enum ThresholdOp {
    Above(f64),
    Below(f64),
}

/// Event emitted by a single watch tick.
pub enum WatchEvent {
    Ok {
        value: f64,
        series_count: usize,
    },
    Warn {
        value: f64,
        series_count: usize,
        triggered_for_secs: u64,
    },
    Alert {
        value: f64,
        series_count: usize,
    },
    Error {
        error: String,
    },
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

/// Perform a single watch tick: query the endpoint and check threshold.
///
/// `first_triggered` tracks when the threshold was first breached (for `--for` sustain logic).
/// The caller owns this state across ticks.
#[allow(clippy::disallowed_types)] // headless is native-only
pub fn tick(
    base_url: &str,
    expression: &str,
    threshold: &ThresholdOp,
    first_triggered: &mut Option<std::time::Instant>,
    for_duration: Option<u64>,
) -> WatchEvent {
    match promql::query_instant(base_url, expression) {
        Ok(data) => {
            let (triggered, extreme_val, series_count) = check_threshold(&data, threshold);

            if triggered {
                let trigger_start = first_triggered.get_or_insert_with(std::time::Instant::now);
                let triggered_secs = trigger_start.elapsed().as_secs();

                if let Some(for_dur) = for_duration {
                    if triggered_secs >= for_dur {
                        WatchEvent::Alert {
                            value: extreme_val,
                            series_count,
                        }
                    } else {
                        WatchEvent::Warn {
                            value: extreme_val,
                            series_count,
                            triggered_for_secs: triggered_secs,
                        }
                    }
                } else {
                    WatchEvent::Alert {
                        value: extreme_val,
                        series_count,
                    }
                }
            } else {
                *first_triggered = None;
                WatchEvent::Ok {
                    value: extreme_val,
                    series_count,
                }
            }
        }
        Err(e) => WatchEvent::Error {
            error: e.to_string(),
        },
    }
}

/// Run the watch loop.
///
/// Returns `Ok(true)` if the threshold was triggered (caller should exit 1),
/// or loops forever until SIGINT kills the process.
pub fn run(config: &WatchConfig) -> Result<bool> {
    let base_url = promql::resolve_endpoint(config.endpoint, config.workspace)?;

    if !config.json {
        eprintln!(
            "{}  {}  every {}s  threshold {}",
            style("Watching:").bold(),
            config.expression,
            config.every,
            format_threshold(&config.threshold),
        );
        if let Some(dur) = config.for_duration {
            eprintln!("Must sustain for {dur}s before alerting");
        }
    }

    #[allow(clippy::disallowed_types)] // headless is native-only
    let mut first_triggered: Option<std::time::Instant> = None;

    loop {
        let event = tick(
            &base_url,
            config.expression,
            &config.threshold,
            &mut first_triggered,
            config.for_duration,
        );

        let is_alert = matches!(event, WatchEvent::Alert { .. });
        print_event(config, &event);

        if is_alert {
            return Ok(true);
        }

        std::thread::sleep(std::time::Duration::from_secs(config.every));
    }
}

/// Raw CLI parameters for `run_cli`, before threshold/duration parsing.
pub struct WatchCliParams<'a> {
    pub expression: &'a str,
    pub endpoint: Option<&'a str>,
    pub workspace: Option<&'a str>,
    pub above: Option<f64>,
    pub below: Option<f64>,
    pub every: &'a str,
    pub for_duration: Option<&'a str>,
    pub json: bool,
}

/// High-level CLI entry point: parse raw CLI args and run the watch loop.
///
/// Handles threshold construction from `above`/`below` and duration parsing from
/// human-readable strings (e.g. "30s", "5m"). Returns `Ok(true)` if threshold triggered.
pub fn run_cli(params: &WatchCliParams) -> Result<bool> {
    let threshold = match (params.above, params.below) {
        (Some(v), None) => ThresholdOp::Above(v),
        (None, Some(v)) => ThresholdOp::Below(v),
        _ => return Err("exactly one of --above or --below must be specified".into()),
    };
    let every_secs = time::parse_duration_secs(params.every)?;
    let for_secs = params
        .for_duration
        .map(time::parse_duration_secs)
        .transpose()?;
    let config = WatchConfig {
        expression: params.expression,
        endpoint: params.endpoint,
        workspace: params.workspace,
        threshold,
        every: every_secs,
        for_duration: for_secs,
        json: params.json,
    };
    run(&config)
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

fn print_event(config: &WatchConfig, event: &WatchEvent) {
    match event {
        WatchEvent::Ok {
            value,
            series_count,
        } => print_status(config, "OK", *value, *series_count, None),
        WatchEvent::Warn {
            value,
            series_count,
            triggered_for_secs,
        } => print_status(
            config,
            "WARN",
            *value,
            *series_count,
            Some(*triggered_for_secs),
        ),
        WatchEvent::Alert {
            value,
            series_count,
        } => print_status(config, "ALERT", *value, *series_count, None),
        WatchEvent::Error { error } => {
            if config.json {
                println!(
                    "{}",
                    serde_json::json!({"timestamp": now_formatted(), "status": "error", "error": error})
                );
            } else {
                eprintln!("[{}] {}  {error}", now_formatted(), style("ERROR").red());
            }
        }
    }
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
        let styled_status = match status {
            "OK" => style(format!("{status:<7}")).green(),
            "WARN" => style(format!("{status:<7}")).yellow(),
            "ALERT" => style(format!("{status:<7}")).red().bold(),
            _ => style(format!("{status:<7}")),
        };
        println!(
            "[{}] {styled_status}value={value:.6}  ({})  {series_count} series{triggered_str}",
            now_formatted(),
            format_threshold(&config.threshold),
        );
    }
}
