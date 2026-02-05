use std::io::{BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;
type HandlerResult = std::result::Result<serde_json::Value, (i32, String)>;

// -- JSON-RPC 2.0 types -------------------------------------------------------

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Serialize)]
struct RpcNotification {
    jsonrpc: &'static str,
    method: String,
    params: serde_json::Value,
}

impl RpcResponse {
    fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError { code, message }),
        }
    }
}

// -- Session state -------------------------------------------------------------

struct WatchHandle {
    #[allow(dead_code)]
    thread: std::thread::JoinHandle<()>,
    stop_flag: Arc<AtomicBool>,
    expression: String,
}

struct Session {
    should_shutdown: bool,
    next_watch_id: u64,
    watches: FxHashMap<u64, WatchHandle>,
    notification_tx: mpsc::Sender<RpcNotification>,
    notification_rx: mpsc::Receiver<RpcNotification>,
}

impl Session {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            should_shutdown: false,
            next_watch_id: 1,
            watches: FxHashMap::default(),
            notification_tx: tx,
            notification_rx: rx,
        }
    }

    fn shutdown(&mut self) {
        for (_, handle) in self.watches.drain() {
            handle.stop_flag.store(true, Ordering::SeqCst);
        }
    }
}

// -- Entry point ---------------------------------------------------------------

pub fn run() -> Result {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let reader = std::io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut session = Session::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Drain pending watch notifications before processing the request.
        drain_notifications(&session.notification_rx, &mut writer)?;

        let response = handle_line(&mut session, &line);
        if let Some(resp) = response {
            serde_json::to_writer(&mut writer, &resp)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }

        if session.should_shutdown {
            break;
        }
    }

    session.shutdown();
    Ok(())
}

fn drain_notifications(
    rx: &mpsc::Receiver<RpcNotification>,
    writer: &mut impl Write,
) -> std::io::Result<()> {
    while let Ok(notification) = rx.try_recv() {
        serde_json::to_writer(&mut *writer, &notification).ok();
        writer.write_all(b"\n")?;
    }
    writer.flush()
}

// -- Dispatch ------------------------------------------------------------------

fn handle_line(session: &mut Session, line: &str) -> Option<RpcResponse> {
    let req: RpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Some(RpcResponse::err(
                serde_json::Value::Null,
                -32700,
                format!("parse error: {e}"),
            ));
        }
    };

    let id = req.id.clone().unwrap_or(serde_json::Value::Null);

    let result = match req.method.as_str() {
        // Workspace
        "workspace.list" => workspace_list(),
        "workspace.show" => workspace_show(&req.params),
        "workspace.init" => workspace_init(&req.params),
        "workspace.rm" => workspace_rm(&req.params),
        "workspace.get" => workspace_get(&req.params),
        "workspace.set" => workspace_set(&req.params),
        "workspace.add_section" => workspace_add_section(&req.params),
        "workspace.add_pane" => workspace_add_pane(&req.params),
        "workspace.remove_section" => workspace_remove_section(&req.params),
        "workspace.remove_pane" => workspace_remove_pane(&req.params),
        "workspace.snapshot" => workspace_snapshot(&req.params),
        // Query
        "query.instant" => query_instant(&req.params),
        "query.range" => query_range(&req.params),
        // Metrics discovery
        "metrics.list" => metrics_list(&req.params),
        "metrics.labels" => metrics_labels(&req.params),
        "metrics.label_values" => metrics_label_values(&req.params),
        "metrics.info" => metrics_info(&req.params),
        "metrics.series" => metrics_series(&req.params),
        // Watch
        "watch.start" => watch_start(session, &req.params),
        "watch.stop" => watch_stop(session, &req.params),
        "watch.list" => watch_list(session),
        // Plugins
        "plugins.list" => plugins_list(),
        "plugins.commands" => plugins_commands(),
        "plugins.install" => plugins_install(&req.params),
        "plugins.remove" => plugins_remove(&req.params),
        // Exec
        "exec.run" => exec_run(&req.params),
        // Session
        "session.info" => session_info(),
        "session.shutdown" => session_shutdown(session),
        _ => Err((-32601, format!("method not found: {}", req.method))),
    };

    Some(match result {
        Ok(value) => RpcResponse::ok(id, value),
        Err((code, msg)) => RpcResponse::err(id, code, msg),
    })
}

// -- Param helpers -------------------------------------------------------------

fn param_str(params: &serde_json::Value, key: &str) -> Option<String> {
    params.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn require_str(
    params: &serde_json::Value,
    key: &str,
) -> std::result::Result<String, (i32, String)> {
    param_str(params, key).ok_or((-32602, format!("missing required param: {key}")))
}

fn param_f64(params: &serde_json::Value, key: &str) -> Option<f64> {
    params.get(key).and_then(|v| v.as_f64())
}

fn param_bool(params: &serde_json::Value, key: &str) -> Option<bool> {
    params.get(key).and_then(|v| v.as_bool())
}

fn param_u64(params: &serde_json::Value, key: &str) -> Option<u64> {
    params.get(key).and_then(|v| v.as_u64())
}

fn map_err(e: impl std::fmt::Display) -> (i32, String) {
    (-32603, e.to_string())
}

// -- Workspace handlers --------------------------------------------------------

fn workspace_list() -> HandlerResult {
    let result = enya_headless::workspace::list_core();
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_show(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let ws = enya_headless::workspace::show_core(&name).map_err(map_err)?;
    serde_json::to_value(&ws).map_err(map_err)
}

fn workspace_init(params: &serde_json::Value) -> HandlerResult {
    let name = param_str(params, "name");
    let endpoint = param_str(params, "endpoint");
    let template = param_str(params, "template");
    let result =
        enya_headless::workspace::init_core(name, endpoint.as_deref(), template.as_deref(), None)
            .map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_rm(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let result = enya_headless::workspace::rm_core(&name).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_get(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let key = require_str(params, "key")?;
    let result = enya_headless::workspace::get_core(&name, &key).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_set(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let key = require_str(params, "key")?;
    let value = require_str(params, "value")?;
    let result = enya_headless::workspace::set_core(&name, &key, &value).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_add_section(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let section_name = require_str(params, "section_name")?;
    let layout = param_str(params, "layout").unwrap_or_else(|| "horizontal".to_string());
    let columns = param_u64(params, "columns").map(|c| c as usize);
    let collapsed = param_bool(params, "collapsed").unwrap_or(false);
    let result = enya_headless::workspace::add_section_core(
        &name,
        &section_name,
        &layout,
        columns,
        collapsed,
    )
    .map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_add_pane(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let query = require_str(params, "query")?;
    let pane_name = param_str(params, "pane_name");
    let section = param_str(params, "section");
    let tag = param_str(params, "tag");
    let unit = param_str(params, "unit");
    let granularity = param_str(params, "granularity");
    let visualization = param_str(params, "visualization");
    let description = param_str(params, "description");
    let result =
        enya_headless::workspace::add_pane_core(&enya_headless::workspace::AddPaneParams {
            name: &name,
            query: &query,
            pane_name: pane_name.as_deref(),
            section: section.as_deref(),
            tag: tag.as_deref(),
            unit: unit.as_deref(),
            granularity: granularity.as_deref(),
            visualization: visualization.as_deref(),
            description: description.as_deref(),
        })
        .map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_remove_section(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let section_name = require_str(params, "section_name")?;
    let result =
        enya_headless::workspace::remove_section_core(&name, &section_name).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_remove_pane(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let pane = require_str(params, "pane")?;
    let section = param_str(params, "section");
    let result = enya_headless::workspace::remove_pane_core(&name, &pane, section.as_deref())
        .map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn workspace_snapshot(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let endpoint = param_str(params, "endpoint");
    let ws = enya_headless::workspace::show_core(&name).map_err(map_err)?;
    let base_url = enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), Some(&name))
        .map_err(map_err)?;
    enya_headless::workspace::snapshot(&base_url, &ws).map_err(map_err)
}

// -- Query handlers ------------------------------------------------------------

fn query_instant(params: &serde_json::Value) -> HandlerResult {
    let expression = require_str(params, "expression")?;
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let data =
        enya_headless::query::promql::query_instant(&base_url, &expression).map_err(map_err)?;
    Ok(data.to_json())
}

fn query_range(params: &serde_json::Value) -> HandlerResult {
    let expression = require_str(params, "expression")?;
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");
    let start = param_str(params, "start").unwrap_or_else(|| "1h".to_string());
    let end = param_str(params, "end").unwrap_or_else(|| "now".to_string());
    let step = param_str(params, "step").unwrap_or_else(|| "60s".to_string());

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let now = enya_headless::query::time::now_secs();
    let start_secs = enya_headless::query::time::parse_time(&start, now).map_err(map_err)?;
    let end_secs = enya_headless::query::time::parse_time(&end, now).map_err(map_err)?;
    let step_secs = enya_headless::query::time::parse_duration_secs(&step).map_err(map_err)?;

    let data = enya_headless::query::promql::query_range(
        &base_url,
        &expression,
        start_secs,
        end_secs,
        step_secs,
    )
    .map_err(map_err)?;

    Ok(data.to_json())
}

// -- Metrics discovery handlers -------------------------------------------------

fn metrics_list(params: &serde_json::Value) -> HandlerResult {
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");
    let match_selector = param_str(params, "match");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let names = enya_headless::query::discovery::list_metrics(&base_url, match_selector.as_deref())
        .map_err(map_err)?;

    Ok(serde_json::json!({"metrics": names, "count": names.len()}))
}

fn metrics_labels(params: &serde_json::Value) -> HandlerResult {
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");
    let match_selector = param_str(params, "match");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let labels = enya_headless::query::discovery::list_labels(&base_url, match_selector.as_deref())
        .map_err(map_err)?;

    Ok(serde_json::json!({"labels": labels, "count": labels.len()}))
}

fn metrics_label_values(params: &serde_json::Value) -> HandlerResult {
    let label = require_str(params, "label")?;
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let values =
        enya_headless::query::discovery::label_values(&base_url, &label).map_err(map_err)?;

    Ok(serde_json::json!({"label": label, "values": values, "count": values.len()}))
}

fn metrics_info(params: &serde_json::Value) -> HandlerResult {
    let metric = param_str(params, "metric");
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let infos = enya_headless::query::discovery::metric_info(&base_url, metric.as_deref())
        .map_err(map_err)?;

    let items: Vec<serde_json::Value> = infos
        .iter()
        .map(|i| {
            serde_json::json!({
                "metric": i.metric,
                "type": i.metric_type,
                "help": i.help,
                "unit": i.unit,
            })
        })
        .collect();

    Ok(serde_json::json!({"metrics": items, "count": items.len()}))
}

fn metrics_series(params: &serde_json::Value) -> HandlerResult {
    let selector = require_str(params, "selector")?;
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let entries =
        enya_headless::query::discovery::query_series(&base_url, &selector).map_err(map_err)?;

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| serde_json::json!(e.labels))
        .collect();

    Ok(serde_json::json!({"series": items, "count": items.len()}))
}

// -- Watch handlers ------------------------------------------------------------

fn watch_start(session: &mut Session, params: &serde_json::Value) -> HandlerResult {
    let expression = require_str(params, "expression")?;
    let endpoint = param_str(params, "endpoint");
    let workspace = param_str(params, "workspace");
    let above = param_f64(params, "above");
    let below = param_f64(params, "below");
    let every_str = param_str(params, "every").unwrap_or_else(|| "30s".to_string());
    let for_str = param_str(params, "for");

    let threshold = match (above, below) {
        (Some(v), None) => enya_headless::watch::ThresholdOp::Above(v),
        (None, Some(v)) => enya_headless::watch::ThresholdOp::Below(v),
        _ => {
            return Err((
                -32602,
                "exactly one of \"above\" or \"below\" is required".to_string(),
            ));
        }
    };

    let every_secs =
        enya_headless::query::time::parse_duration_secs(&every_str).map_err(map_err)?;
    let for_secs = for_str
        .as_deref()
        .map(enya_headless::query::time::parse_duration_secs)
        .transpose()
        .map_err(map_err)?;

    let base_url =
        enya_headless::query::promql::resolve_endpoint(endpoint.as_deref(), workspace.as_deref())
            .map_err(map_err)?;

    let watch_id = session.next_watch_id;
    session.next_watch_id += 1;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop = stop_flag.clone();
    let tx = session.notification_tx.clone();
    let expr = expression.clone();

    let thread = std::thread::spawn(move || {
        watch_thread(
            watch_id, &base_url, &expr, threshold, every_secs, for_secs, stop, tx,
        );
    });

    session.watches.insert(
        watch_id,
        WatchHandle {
            thread,
            stop_flag,
            expression,
        },
    );

    Ok(serde_json::json!({"watch_id": watch_id}))
}

#[allow(clippy::too_many_arguments)]
fn watch_thread(
    watch_id: u64,
    base_url: &str,
    expression: &str,
    threshold: enya_headless::watch::ThresholdOp,
    every_secs: u64,
    for_secs: Option<u64>,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<RpcNotification>,
) {
    use enya_headless::watch::WatchEvent;

    let mut first_triggered: Option<std::time::Instant> = None;
    let threshold_str = enya_headless::watch::format_threshold(&threshold);

    while !stop.load(Ordering::SeqCst) {
        let event = enya_headless::watch::tick(
            base_url,
            expression,
            &threshold,
            &mut first_triggered,
            for_secs,
        );

        match &event {
            WatchEvent::Ok {
                value,
                series_count,
            } => {
                let _ = tx.send(RpcNotification {
                    jsonrpc: "2.0",
                    method: "watch.status".to_string(),
                    params: serde_json::json!({
                        "watch_id": watch_id,
                        "status": "ok",
                        "value": value,
                        "threshold": threshold_str,
                        "series_count": series_count,
                    }),
                });
            }
            WatchEvent::Warn {
                value,
                series_count,
                triggered_for_secs,
            } => {
                let _ = tx.send(RpcNotification {
                    jsonrpc: "2.0",
                    method: "watch.status".to_string(),
                    params: serde_json::json!({
                        "watch_id": watch_id,
                        "status": "warn",
                        "value": value,
                        "threshold": threshold_str,
                        "series_count": series_count,
                        "triggered_for_secs": triggered_for_secs,
                    }),
                });
            }
            WatchEvent::Alert {
                value,
                series_count,
            } => {
                let _ = tx.send(RpcNotification {
                    jsonrpc: "2.0",
                    method: "watch.triggered".to_string(),
                    params: serde_json::json!({
                        "watch_id": watch_id,
                        "value": value,
                        "threshold": threshold_str,
                        "series_count": series_count,
                    }),
                });
                return;
            }
            WatchEvent::Error { error } => {
                let _ = tx.send(RpcNotification {
                    jsonrpc: "2.0",
                    method: "watch.status".to_string(),
                    params: serde_json::json!({
                        "watch_id": watch_id,
                        "status": "error",
                        "error": error,
                    }),
                });
            }
        }

        // Sleep in small increments so stop_flag is checked promptly.
        let mut slept = 0u64;
        while slept < every_secs * 1000 && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(250));
            slept += 250;
        }
    }
}

fn watch_stop(session: &mut Session, params: &serde_json::Value) -> HandlerResult {
    let watch_id = param_u64(params, "watch_id")
        .ok_or((-32602, "missing required param: watch_id".to_string()))?;

    if let Some(handle) = session.watches.remove(&watch_id) {
        handle.stop_flag.store(true, Ordering::SeqCst);
        Ok(serde_json::json!({"watch_id": watch_id, "stopped": true}))
    } else {
        Err((-32603, format!("watch not found: {watch_id}")))
    }
}

fn watch_list(session: &Session) -> HandlerResult {
    let watches: Vec<serde_json::Value> = session
        .watches
        .iter()
        .map(|(id, h)| serde_json::json!({"watch_id": id, "expression": h.expression}))
        .collect();
    Ok(serde_json::json!({"watches": watches}))
}

// -- Plugin handlers -----------------------------------------------------------

fn plugins_list() -> HandlerResult {
    let result = enya_headless::plugins::plugins_list_core();
    serde_json::to_value(&result).map_err(map_err)
}

fn plugins_commands() -> HandlerResult {
    let result = enya_headless::plugins::plugins_commands_core();
    serde_json::to_value(&result).map_err(map_err)
}

fn plugins_install(params: &serde_json::Value) -> HandlerResult {
    let source = require_str(params, "source")?;
    let result = enya_headless::plugins::plugins_install_core(&source).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

fn plugins_remove(params: &serde_json::Value) -> HandlerResult {
    let name = require_str(params, "name")?;
    let result = enya_headless::plugins::plugins_remove_core(&name).map_err(map_err)?;
    serde_json::to_value(&result).map_err(map_err)
}

// -- Exec handler --------------------------------------------------------------

fn exec_run(params: &serde_json::Value) -> HandlerResult {
    let command = require_str(params, "command")?;
    let args = param_str(params, "args").unwrap_or_default();
    enya_headless::plugins::exec_core(&command, &args).map_err(map_err)
}

// -- Session handlers ----------------------------------------------------------

fn session_info() -> HandlerResult {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": ["workspace", "query", "metrics", "watch", "plugins"],
    }))
}

fn session_shutdown(session: &mut Session) -> HandlerResult {
    session.should_shutdown = true;
    Ok(serde_json::json!({"status": "shutting_down"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use enya_config::WorkspaceConfig;

    fn pretty(val: &serde_json::Value) -> String {
        serde_json::to_string_pretty(val).unwrap()
    }

    fn fmt_err(err: (i32, String)) -> String {
        format!("({}) {}", err.0, err.1)
    }

    fn fmt_resp(resp: &RpcResponse) -> String {
        serde_json::to_string_pretty(&serde_json::to_value(resp).unwrap()).unwrap()
    }

    // -- session_info / session_shutdown ----------------------------------------

    #[test]
    fn test_session_info() {
        let result = session_info().unwrap();
        let s = pretty(&result).replace(env!("CARGO_PKG_VERSION"), "[version]");
        insta::assert_snapshot!(s, @r#"
        {
          "capabilities": [
            "workspace",
            "query",
            "metrics",
            "watch",
            "plugins"
          ],
          "version": "[version]"
        }
        "#);
    }

    #[test]
    fn test_session_shutdown() {
        let mut session = Session::new();
        assert!(!session.should_shutdown);
        let result = session_shutdown(&mut session).unwrap();
        assert!(session.should_shutdown);
        insta::assert_snapshot!(pretty(&result), @r#"
        {
          "status": "shutting_down"
        }
        "#);
    }

    // -- param helpers ---------------------------------------------------------

    #[test]
    fn test_param_str() {
        let params = serde_json::json!({"name": "hello", "count": 42});
        assert_eq!(param_str(&params, "name"), Some("hello".to_string()));
        assert_eq!(param_str(&params, "missing"), None);
        assert_eq!(param_str(&params, "count"), None);
    }

    #[test]
    fn test_require_str_ok() {
        let params = serde_json::json!({"name": "hello"});
        assert_eq!(require_str(&params, "name").unwrap(), "hello");
    }

    #[test]
    fn test_require_str_missing() {
        let params = serde_json::json!({});
        insta::assert_snapshot!(
            fmt_err(require_str(&params, "name").unwrap_err()),
            @"(-32602) missing required param: name"
        );
    }

    #[test]
    fn test_param_f64() {
        let params = serde_json::json!({"val": 2.75, "int_val": 42, "str": "nope"});
        assert!((param_f64(&params, "val").unwrap() - 2.75).abs() < f64::EPSILON);
        assert!((param_f64(&params, "int_val").unwrap() - 42.0).abs() < f64::EPSILON);
        assert_eq!(param_f64(&params, "str"), None);
        assert_eq!(param_f64(&params, "missing"), None);
    }

    #[test]
    fn test_param_bool() {
        let params = serde_json::json!({"flag": true, "off": false, "str": "true"});
        assert_eq!(param_bool(&params, "flag"), Some(true));
        assert_eq!(param_bool(&params, "off"), Some(false));
        assert_eq!(param_bool(&params, "str"), None);
    }

    #[test]
    fn test_param_u64() {
        let params = serde_json::json!({"id": 7, "neg": -1, "float": 3.5});
        assert_eq!(param_u64(&params, "id"), Some(7));
        assert_eq!(param_u64(&params, "neg"), None);
        assert_eq!(param_u64(&params, "float"), None);
    }

    // -- handle_line dispatch --------------------------------------------------

    #[test]
    fn test_handle_line_parse_error() {
        let mut session = Session::new();
        let resp = handle_line(&mut session, "not json at all").unwrap();
        assert_eq!(resp.error.as_ref().unwrap().code, -32700);
        assert!(resp.id.is_null());
    }

    #[test]
    fn test_handle_line_method_not_found() {
        let mut session = Session::new();
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","id":1,"method":"no.such.method","params":{}}"#,
        )
        .unwrap();
        insta::assert_snapshot!(fmt_resp(&resp), @r#"
        {
          "error": {
            "code": -32601,
            "message": "method not found: no.such.method"
          },
          "id": 1,
          "jsonrpc": "2.0"
        }
        "#);
    }

    #[test]
    fn test_handle_line_session_info() {
        let mut session = Session::new();
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","id":42,"method":"session.info","params":{}}"#,
        )
        .unwrap();
        let s = fmt_resp(&resp).replace(env!("CARGO_PKG_VERSION"), "[version]");
        insta::assert_snapshot!(s, @r#"
        {
          "id": 42,
          "jsonrpc": "2.0",
          "result": {
            "capabilities": [
              "workspace",
              "query",
              "metrics",
              "watch",
              "plugins"
            ],
            "version": "[version]"
          }
        }
        "#);
    }

    #[test]
    fn test_handle_line_preserves_string_id() {
        let mut session = Session::new();
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","id":"abc-123","method":"session.info","params":{}}"#,
        )
        .unwrap();
        assert_eq!(resp.id, serde_json::json!("abc-123"));
    }

    #[test]
    fn test_handle_line_null_id_on_missing() {
        let mut session = Session::new();
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","method":"session.info","params":{}}"#,
        )
        .unwrap();
        assert!(resp.id.is_null());
    }

    #[test]
    fn test_handle_line_shutdown() {
        let mut session = Session::new();
        assert!(!session.should_shutdown);
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","id":1,"method":"session.shutdown","params":{}}"#,
        )
        .unwrap();
        assert!(session.should_shutdown);
        insta::assert_snapshot!(fmt_resp(&resp), @r#"
        {
          "id": 1,
          "jsonrpc": "2.0",
          "result": {
            "status": "shutting_down"
          }
        }
        "#);
    }

    #[test]
    fn test_handle_line_missing_required_param() {
        let mut session = Session::new();
        let resp = handle_line(
            &mut session,
            r#"{"jsonrpc":"2.0","id":1,"method":"workspace.show","params":{}}"#,
        )
        .unwrap();
        insta::assert_snapshot!(fmt_resp(&resp), @r#"
        {
          "error": {
            "code": -32602,
            "message": "missing required param: name"
          },
          "id": 1,
          "jsonrpc": "2.0"
        }
        "#);
    }

    // -- watch handlers (stateless checks) -------------------------------------

    #[test]
    fn test_watch_list_empty() {
        let session = Session::new();
        insta::assert_snapshot!(pretty(&watch_list(&session).unwrap()), @r#"
        {
          "watches": []
        }
        "#);
    }

    #[test]
    fn test_watch_stop_not_found() {
        let mut session = Session::new();
        let params = serde_json::json!({"watch_id": 999});
        insta::assert_snapshot!(
            fmt_err(watch_stop(&mut session, &params).unwrap_err()),
            @"(-32603) watch not found: 999"
        );
    }

    #[test]
    fn test_watch_start_requires_threshold() {
        let mut session = Session::new();
        let params = serde_json::json!({"expression": "up"});
        insta::assert_snapshot!(
            fmt_err(watch_start(&mut session, &params).unwrap_err()),
            @r#"(-32602) exactly one of "above" or "below" is required"#
        );
    }

    // -- workspace lifecycle (temp dir) ----------------------------------------

    fn temp_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-ws.toml");
        let ws = WorkspaceConfig::new("test-ws");
        ws.save(&path).unwrap();
        (dir, path)
    }

    /// Mask the workspace name from a handler result (it echoes the input name
    /// which is a temp-dir path).
    fn mask_ws(val: &serde_json::Value) -> String {
        let mut v = val.clone();
        if let Some(obj) = v.as_object_mut() {
            if obj.contains_key("workspace") {
                obj.insert("workspace".into(), serde_json::json!("[ws]"));
            }
            if obj.contains_key("removed") {
                obj.insert("removed".into(), serde_json::json!("[path]"));
            }
        }
        serde_json::to_string_pretty(&v).unwrap()
    }

    #[test]
    fn test_workspace_show() {
        let (_dir, path) = temp_workspace();
        let result =
            workspace_show(&serde_json::json!({"name": path.display().to_string()})).unwrap();
        assert_eq!(result["workspace"]["name"], "test-ws");
    }

    #[test]
    fn test_workspace_get_set() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        let set_result =
            workspace_set(&serde_json::json!({"name": p, "key": "time.preset", "value": "1h"}))
                .unwrap();
        insta::assert_snapshot!(mask_ws(&set_result), @r#"
        {
          "key": "time.preset",
          "value": "1h",
          "workspace": "[ws]"
        }
        "#);

        let get_result =
            workspace_get(&serde_json::json!({"name": p, "key": "time.preset"})).unwrap();
        insta::assert_snapshot!(mask_ws(&get_result), @r#"
        {
          "key": "time.preset",
          "value": "1h",
          "workspace": "[ws]"
        }
        "#);
    }

    #[test]
    fn test_workspace_add_section_and_remove() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        let result = workspace_add_section(&serde_json::json!({
            "name": p,
            "section_name": "API Metrics",
            "layout": "grid",
            "columns": 3,
        }))
        .unwrap();
        insta::assert_snapshot!(mask_ws(&result), @r#"
        {
          "layout": "grid",
          "section": "API Metrics",
          "workspace": "[ws]"
        }
        "#);

        let ws = WorkspaceConfig::load(&path).unwrap();
        assert!(ws.find_section("API Metrics").is_some());

        let result = workspace_remove_section(
            &serde_json::json!({"name": p, "section_name": "API Metrics"}),
        )
        .unwrap();
        insta::assert_snapshot!(mask_ws(&result), @r#"
        {
          "panes_removed": 0,
          "removed_section": "API Metrics",
          "workspace": "[ws]"
        }
        "#);

        let ws = WorkspaceConfig::load(&path).unwrap();
        assert!(ws.find_section("API Metrics").is_none());
    }

    #[test]
    fn test_workspace_add_section_duplicate() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        workspace_add_section(&serde_json::json!({"name": p, "section_name": "Dup"})).unwrap();
        insta::assert_snapshot!(
            fmt_err(workspace_add_section(&serde_json::json!({"name": p, "section_name": "Dup"})).unwrap_err()),
            @"(-32603) section already exists: Dup"
        );
    }

    #[test]
    fn test_workspace_add_pane_and_remove() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        workspace_add_section(&serde_json::json!({"name": p, "section_name": "Infra"})).unwrap();

        let result = workspace_add_pane(&serde_json::json!({
            "name": p,
            "query": "rate(http_requests_total[5m])",
            "pane_name": "Request Rate",
            "section": "Infra",
            "unit": "req/s",
            "tag": "Critical",
        }))
        .unwrap();
        insta::assert_snapshot!(mask_ws(&result), @r#"
        {
          "pane": "Request Rate",
          "query": "rate(http_requests_total[5m])",
          "section": "Infra",
          "workspace": "[ws]"
        }
        "#);

        let result = workspace_remove_pane(&serde_json::json!({
            "name": p,
            "pane": "Request Rate",
            "section": "Infra",
        }))
        .unwrap();
        insta::assert_snapshot!(mask_ws(&result), @r#"
        {
          "removed_pane": "Request Rate",
          "section": "Infra",
          "workspace": "[ws]"
        }
        "#);
    }

    #[test]
    fn test_workspace_rm() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        assert!(path.exists());
        let result = workspace_rm(&serde_json::json!({"name": p})).unwrap();
        insta::assert_snapshot!(mask_ws(&result), @r#"
        {
          "removed": "[path]"
        }
        "#);
        assert!(!path.exists());
    }

    #[test]
    fn test_workspace_rm_not_found() {
        let err = workspace_rm(&serde_json::json!({"name": "nonexistent_ws_xyz"})).unwrap_err();
        assert_eq!(err.0, -32603);
    }

    #[test]
    fn test_workspace_add_section_invalid_layout() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        insta::assert_snapshot!(
            fmt_err(workspace_add_section(&serde_json::json!({
                "name": p,
                "section_name": "Bad",
                "layout": "diagonal",
            })).unwrap_err()),
            @"(-32603) invalid layout: diagonal (expected: horizontal, vertical, grid, tabs)"
        );
    }

    #[test]
    fn test_workspace_remove_pane_not_found() {
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        insta::assert_snapshot!(
            fmt_err(workspace_remove_pane(&serde_json::json!({"name": p, "pane": "Ghost Pane"})).unwrap_err()),
            @"(-32603) pane not found: Ghost Pane"
        );
    }

    // -- plugins (stateless shape checks) --------------------------------------

    #[test]
    fn test_plugins_list_shape() {
        let result = plugins_list().unwrap();
        assert!(result["dir"].is_string());
        assert!(result["plugins"].is_array());
    }

    #[test]
    fn test_plugins_commands_shape() {
        let result = plugins_commands().unwrap();
        assert!(result["commands"].is_array());
    }

    // -- exec (error paths) ----------------------------------------------------

    #[test]
    fn test_exec_run_missing_command_param() {
        insta::assert_snapshot!(
            fmt_err(exec_run(&serde_json::json!({})).unwrap_err()),
            @"(-32602) missing required param: command"
        );
    }

    #[test]
    fn test_exec_run_unknown_command() {
        insta::assert_snapshot!(
            fmt_err(exec_run(&serde_json::json!({"command": "nonexistent_cmd_xyz"})).unwrap_err()),
            @"(-32603) unknown command: nonexistent_cmd_xyz"
        );
    }

    // -- metrics discovery (error paths) -----------------------------------------

    #[test]
    fn test_metrics_list_missing_endpoint() {
        let err = metrics_list(&serde_json::json!({})).unwrap_err();
        assert_eq!(err.0, -32603);
        assert!(err.1.contains("no endpoint specified"));
    }

    #[test]
    fn test_metrics_label_values_missing_label() {
        insta::assert_snapshot!(
            fmt_err(metrics_label_values(&serde_json::json!({})).unwrap_err()),
            @"(-32602) missing required param: label"
        );
    }

    #[test]
    fn test_metrics_series_missing_selector() {
        insta::assert_snapshot!(
            fmt_err(metrics_series(&serde_json::json!({})).unwrap_err()),
            @"(-32602) missing required param: selector"
        );
    }

    #[test]
    fn test_workspace_snapshot_missing_name() {
        insta::assert_snapshot!(
            fmt_err(workspace_snapshot(&serde_json::json!({})).unwrap_err()),
            @"(-32602) missing required param: name"
        );
    }

    // -- full handle_line workspace lifecycle -----------------------------------

    #[test]
    fn test_handle_line_workspace_lifecycle() {
        let mut session = Session::new();
        let (_dir, path) = temp_workspace();
        let p = path.display().to_string();

        // Show
        let resp = handle_line(
            &mut session,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "workspace.show",
                "params": {"name": p},
            })
            .to_string(),
        )
        .unwrap();
        assert!(resp.error.is_none());
        assert_eq!(
            resp.result.as_ref().unwrap()["workspace"]["name"],
            "test-ws"
        );

        // Set
        let resp = handle_line(
            &mut session,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 2,
                "method": "workspace.set",
                "params": {"name": p, "key": "view.theme", "value": "light"},
            })
            .to_string(),
        )
        .unwrap();
        assert!(resp.error.is_none());

        // Get — verify round-trip
        let resp = handle_line(
            &mut session,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 3,
                "method": "workspace.get",
                "params": {"name": p, "key": "view.theme"},
            })
            .to_string(),
        )
        .unwrap();
        insta::assert_snapshot!(mask_ws(&resp.result.unwrap()), @r#"
        {
          "key": "view.theme",
          "value": "light",
          "workspace": "[ws]"
        }
        "#);

        // Delete
        let resp = handle_line(
            &mut session,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 4,
                "method": "workspace.rm",
                "params": {"name": p},
            })
            .to_string(),
        )
        .unwrap();
        assert!(resp.error.is_none());
        assert!(!path.exists());
    }
}
