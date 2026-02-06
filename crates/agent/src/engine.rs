//! Background watch engine for the Enya agent.
//!
//! Loads enabled watches from SQLite, evaluates their PromQL expressions
//! against Prometheus on configured intervals, and records alert/resolve
//! events back into the database.

use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::time::Duration;

use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{error, info, warn};

use enya_headless::watch::{self, ThresholdOp, WatchEvent};

use crate::db::{Db, Watch};

const SYNC_INTERVAL: Duration = Duration::from_secs(10);

/// Run the watch engine as a long-lived background task.
///
/// The engine polls the database for enabled watches every [`SYNC_INTERVAL`] seconds.
/// When the shutdown receiver fires, all watch tasks are aborted and the loop exits.
pub async fn run(db: Arc<Db>, mut shutdown: tokio::sync::broadcast::Receiver<()>) {
    info!("watch engine started");
    let mut tasks: FxHashMap<i64, tokio::task::JoinHandle<()>> = FxHashMap::default();

    loop {
        sync_watches(&db, &mut tasks);
        tokio::select! {
            _ = tokio::time::sleep(SYNC_INTERVAL) => {}
            _ = shutdown.recv() => {
                info!("watch engine shutting down");
                for (id, handle) in tasks.drain() {
                    handle.abort();
                    info!(watch_id = id, "stopped watch");
                }
                break;
            }
        }
    }
}

/// Synchronize running tasks with the current set of enabled watches.
fn sync_watches(db: &Arc<Db>, tasks: &mut FxHashMap<i64, tokio::task::JoinHandle<()>>) {
    let Ok(watches) = db.list_watches() else {
        warn!("failed to load watches from database, skipping sync cycle");
        return;
    };

    let active: FxHashSet<i64> = watches.iter().map(|w| w.id).collect();

    // Cancel tasks for removed/disabled watches
    tasks.retain(|id, handle| {
        let keep = active.contains(id);
        if !keep {
            handle.abort();
            info!(watch_id = id, "stopped watch");
        }
        keep
    });

    // Start tasks for new watches
    for watch in watches {
        if let Entry::Vacant(slot) = tasks.entry(watch.id) {
            info!(watch_id = watch.id, name = %watch.name, "starting watch");
            let db = db.clone();
            slot.insert(tokio::spawn(run_watch(db, watch)));
        }
    }
}

/// Evaluate a single watch on its configured interval.
async fn run_watch(db: Arc<Db>, watch: Watch) {
    let id = watch.id;
    let expression = watch.expression;
    let endpoint = watch.endpoint;
    let is_above = watch.threshold_op == "above";
    let threshold_value = watch.threshold_value;
    let for_duration = watch.sustain_secs.map(|s| s as u64);

    let mut interval = tokio::time::interval(Duration::from_secs(watch.interval_secs as u64));
    let mut first_triggered: Option<std::time::Instant> = None;
    let mut alerting = false;

    interval.tick().await; // skip first immediate tick

    loop {
        interval.tick().await;

        let ep = endpoint.clone();
        let expr = expression.clone();
        let ft = first_triggered;

        let Ok((event, new_ft)) = tokio::task::spawn_blocking(move || {
            let threshold = if is_above {
                ThresholdOp::Above(threshold_value)
            } else {
                ThresholdOp::Below(threshold_value)
            };
            let mut ft = ft;
            let event = watch::tick(&ep, &expr, &threshold, &mut ft, for_duration);
            (event, ft)
        })
        .await
        else {
            error!(watch_id = id, "tick task panicked");
            continue;
        };

        first_triggered = new_ft;
        record_transition(&db, id, &event, &mut alerting);
    }
}

/// Record a database event when the watch transitions between states.
fn record_transition(db: &Db, watch_id: i64, event: &WatchEvent, alerting: &mut bool) {
    let (event_type, value, message) = match event {
        WatchEvent::Alert { value, .. } if !*alerting => {
            *alerting = true;
            let msg = format!("threshold breached: value={value}");
            ("alert", Some(*value), msg)
        }
        WatchEvent::Ok { value, .. } if *alerting => {
            *alerting = false;
            let msg = format!("resolved: value={value}");
            ("resolve", Some(*value), msg)
        }
        WatchEvent::Error { error } => ("error", None, error.clone()),
        _ => return,
    };

    match event_type {
        "alert" => warn!(watch_id, value = ?value, "watch alert: {message}"),
        "resolve" => info!(watch_id, value = ?value, "watch resolved: {message}"),
        "error" => error!(watch_id, "watch error: {message}"),
        _ => {}
    }

    if let Err(e) = db.insert_event(watch_id, event_type, value, Some(&message)) {
        error!(watch_id, event_type, error = %e, "failed to record event");
    }
}
