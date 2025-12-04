use crate::core::Core;
use crate::options::TaskMetricsOptions;
use enya_metrics_store::MetricsStore;
use enya_metrics_store::{MetricName, Value};
use metrics::{
    Counter, CounterFn, Gauge, GaugeFn, Histogram, HistogramFn, Key, KeyName, Metadata, Recorder,
    SharedString, Unit,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    select,
    sync::{mpsc, watch},
    task::JoinHandle,
};
#[cfg(tokio_unstable)]
use tracing::error;
use tracing::{debug, info, warn};

type MetricSender = mpsc::UnboundedSender<MetricUpdate>;
type MetricReceiver = mpsc::UnboundedReceiver<MetricUpdate>;

/// Coordinates background tasks that persist metrics into the [`MetricsStore`].
pub struct Ingestor {
    shutdown_tx: watch::Sender<bool>,
    join_handles: Vec<JoinHandle<()>>,
}

impl Ingestor {
    pub fn spawn(core: Core, task_metrics: &TaskMetricsOptions) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (tx, rx) = mpsc::unbounded_channel();

        install_metrics_recorder(tx.clone());

        let ingest_handle =
            tokio::spawn(metrics_ingest_loop(core.clone(), rx, shutdown_rx.clone()));

        let mut join_handles = vec![ingest_handle];

        if let Some(tokio_handle) = spawn_tokio_metrics(core.clone(), shutdown_rx.clone()) {
            join_handles.push(tokio_handle);
        }

        #[cfg(feature = "macros")]
        if task_metrics.enabled {
            let task_handle = spawn_task_monitor_metrics(core, shutdown_rx, task_metrics.interval);
            join_handles.push(task_handle);
        }

        Self {
            shutdown_tx,
            join_handles,
        }
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);

        for handle in self.join_handles {
            if let Err(err) = handle.await {
                warn!(error = ?err, "ingestor task terminated unexpectedly");
            }
        }
    }
}

fn install_metrics_recorder(sender: MetricSender) {
    static RECORDER_INSTALLED: std::sync::Once = std::sync::Once::new();

    RECORDER_INSTALLED.call_once(|| {
        if let Err(err) = metrics::set_global_recorder(StoreRecorder::new(sender)) {
            warn!(error = ?err, "failed to install metrics recorder");
        } else {
            info!("installed MetricsStore recorder for metrics-rs events");
        }
    });
}

async fn metrics_ingest_loop(
    core: Core,
    mut rx: MetricReceiver,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        select! {
            biased;
            msg = rx.recv() => {
                match msg {
                    Some(update) => persist_metric_event(core.metrics(), update).await,
                    None => break,
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }

    debug!("metrics ingestion loop exited");
}

async fn persist_metric_event(store: &MetricsStore, update: MetricUpdate) {
    match update {
        MetricUpdate::Counter { key, update } => {
            let value = match update {
                CounterUpdate::Increment(delta) | CounterUpdate::Absolute(delta) => delta as f64,
            };
            persist_value(store, &key, value).await;
        }
        MetricUpdate::Gauge { key, update } => {
            let value = match update {
                GaugeUpdate::Increment(delta) => delta,
                GaugeUpdate::Decrement(delta) => -delta,
                GaugeUpdate::Set(value) => value,
            };
            persist_value(store, &key, value).await;
        }
        MetricUpdate::Histogram { key, value } => persist_value(store, &key, value).await,
    }
}

async fn persist_value(store: &MetricsStore, key: &Key, value: f64) {
    let metric = match MetricName::try_from(key.name()) {
        Ok(metric) => metric,
        Err(_) => {
            warn!("ignoring metric with invalid name: {}", key.name());
            return;
        }
    };

    let mut owned_tags = Vec::new();
    for label in key.labels() {
        owned_tags.push((label.key().to_string(), label.value().to_string()));
    }

    let borrowed: Vec<(&str, &str)> = owned_tags
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    if let Err(err) = store.ingest(metric, value as Value, &borrowed).await {
        warn!(error = ?err, metric = key.name(), "failed to write metric to store");
    }
}

fn spawn_tokio_metrics(core: Core, shutdown_rx: watch::Receiver<bool>) -> Option<JoinHandle<()>> {
    #[cfg(tokio_unstable)]
    {
        Some(tokio::spawn(tokio_metrics_loop(core, shutdown_rx)))
    }

    #[cfg(not(tokio_unstable))]
    {
        let _ = core;
        let _ = shutdown_rx;
        None
    }
}

#[cfg(tokio_unstable)]
async fn tokio_metrics_loop(core: Core, mut shutdown_rx: watch::Receiver<bool>) {
    use tokio_metrics::RuntimeMonitor;

    let handle = tokio::runtime::Handle::current();
    let monitor = RuntimeMonitor::new(&handle);
    let mut intervals = monitor.intervals();
    let mut ticker = tokio::time::interval(Duration::from_secs(30));

    loop {
        select! {
            _ = ticker.tick() => {
                if let Some(metrics) = intervals.next() {
                    persist_runtime_metrics(core.metrics(), &metrics).await;
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }

    debug!("tokio metrics loop exited");
}

#[cfg(tokio_unstable)]
async fn persist_runtime_metrics(store: &MetricsStore, metrics: &tokio_metrics::RuntimeMetrics) {
    record_runtime_metric(
        store,
        "tokio.runtime.total_park_count",
        metrics.total_park_count as f64,
    )
    .await;
    record_runtime_metric(
        store,
        "tokio.runtime.injection_queue_depth",
        metrics.injection_queue_depth as f64,
    )
    .await;
    record_runtime_metric(
        store,
        "tokio.runtime.num_remote_schedules",
        metrics.num_remote_schedules as f64,
    )
    .await;
    record_runtime_metric(
        store,
        "tokio.runtime.budget_forced_yield_count",
        metrics.budget_forced_yield_count as f64,
    )
    .await;
    record_runtime_metric(
        store,
        "tokio.runtime.io_driver_ready_count",
        metrics.io_driver_ready_count as f64,
    )
    .await;
    record_runtime_metric(
        store,
        "tokio.runtime.mean_poll_duration_ns",
        metrics.mean_poll_duration.as_nanos() as f64,
    )
    .await;
}

#[cfg(tokio_unstable)]
async fn record_runtime_metric(store: &MetricsStore, name: &str, value: f64) {
    match MetricName::try_from(name) {
        Ok(metric) => {
            if let Err(err) = store.ingest(metric, value as Value, &[]).await {
                warn!(error = ?err, metric = name, "failed to persist tokio runtime metric");
            }
        }
        Err(_) => error!(metric = name, "invalid runtime metric name"),
    }
}

/// Spawns a background task that collects metrics from all registered TaskMonitors.
#[cfg(feature = "macros")]
fn spawn_task_monitor_metrics(
    core: Core,
    shutdown_rx: watch::Receiver<bool>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(task_monitor_metrics_loop(core, shutdown_rx, interval))
}

/// Periodically collects metrics from all registered TaskMonitors.
#[cfg(feature = "macros")]
async fn task_monitor_metrics_loop(
    core: Core,
    mut shutdown_rx: watch::Receiver<bool>,
    interval: Duration,
) {
    use crate::task_registry::registered_monitors;

    let mut ticker = tokio::time::interval(interval);

    loop {
        select! {
            _ = ticker.tick() => {
                let monitors = registered_monitors();
                for (task_name, monitor) in monitors {
                    persist_task_metrics(core.metrics(), task_name, monitor).await;
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }

    debug!("task monitor metrics loop exited");
}

/// Persists metrics from a single TaskMonitor.
#[cfg(feature = "macros")]
async fn persist_task_metrics(
    store: &MetricsStore,
    task_name: &str,
    monitor: &tokio_metrics::TaskMonitor,
) {
    let metrics = monitor.cumulative();
    let tags: &[(&str, &str)] = &[("task", task_name)];

    // Poll metrics
    record_task_metric(
        store,
        "task.poll.count",
        metrics.total_poll_count as f64,
        tags,
    )
    .await;
    record_task_metric(
        store,
        "task.poll.duration_ns",
        metrics.total_poll_duration.as_nanos() as f64,
        tags,
    )
    .await;
    record_task_metric(
        store,
        "task.poll.slow_count",
        metrics.total_slow_poll_count as f64,
        tags,
    )
    .await;

    // Idle metrics
    record_task_metric(
        store,
        "task.idle.duration_ns",
        metrics.total_idle_duration.as_nanos() as f64,
        tags,
    )
    .await;

    // Scheduling metrics
    record_task_metric(
        store,
        "task.scheduled.duration_ns",
        metrics.total_scheduled_duration.as_nanos() as f64,
        tags,
    )
    .await;
    record_task_metric(
        store,
        "task.scheduled.long_count",
        metrics.total_long_delay_count as f64,
        tags,
    )
    .await;

    // First poll delay
    record_task_metric(
        store,
        "task.first_poll.delay_ns",
        metrics.total_first_poll_delay.as_nanos() as f64,
        tags,
    )
    .await;

    // Task counts
    record_task_metric(
        store,
        "task.instrumented.count",
        metrics.instrumented_count as f64,
        tags,
    )
    .await;
    record_task_metric(
        store,
        "task.dropped.count",
        metrics.dropped_count as f64,
        tags,
    )
    .await;
}

/// Records a single task metric with tags.
#[cfg(feature = "macros")]
async fn record_task_metric(store: &MetricsStore, name: &str, value: f64, tags: &[(&str, &str)]) {
    match MetricName::try_from(name) {
        Ok(metric) => {
            if let Err(err) = store.ingest(metric, value as Value, tags).await {
                warn!(error = ?err, metric = name, "failed to persist task monitor metric");
            }
        }
        Err(_) => warn!(metric = name, "invalid task metric name"),
    }
}

#[derive(Clone)]
struct StoreRecorder {
    sender: MetricSender,
}

impl StoreRecorder {
    fn new(sender: MetricSender) -> Self {
        Self { sender }
    }
}

impl Recorder for StoreRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
        Counter::from_arc(Arc::new(ChannelCounter::new(
            self.sender.clone(),
            key.clone(),
        )))
    }

    fn register_gauge(&self, key: &Key, _metadata: &Metadata<'_>) -> Gauge {
        Gauge::from_arc(Arc::new(ChannelGauge::new(
            self.sender.clone(),
            key.clone(),
        )))
    }

    fn register_histogram(&self, key: &Key, _metadata: &Metadata<'_>) -> Histogram {
        Histogram::from_arc(Arc::new(ChannelHistogram::new(
            self.sender.clone(),
            key.clone(),
        )))
    }
}

struct ChannelCounter {
    sender: MetricSender,
    key: Key,
}

impl ChannelCounter {
    fn new(sender: MetricSender, key: Key) -> Self {
        Self { sender, key }
    }

    fn send(&self, update: CounterUpdate) {
        let _ = self.sender.send(MetricUpdate::Counter {
            key: self.key.clone(),
            update,
        });
    }
}

impl CounterFn for ChannelCounter {
    fn increment(&self, value: u64) {
        self.send(CounterUpdate::Increment(value));
    }

    fn absolute(&self, value: u64) {
        self.send(CounterUpdate::Absolute(value));
    }
}

struct ChannelGauge {
    sender: MetricSender,
    key: Key,
}

impl ChannelGauge {
    fn new(sender: MetricSender, key: Key) -> Self {
        Self { sender, key }
    }

    fn send(&self, update: GaugeUpdate) {
        let _ = self.sender.send(MetricUpdate::Gauge {
            key: self.key.clone(),
            update,
        });
    }
}

impl GaugeFn for ChannelGauge {
    fn increment(&self, value: f64) {
        self.send(GaugeUpdate::Increment(value));
    }

    fn decrement(&self, value: f64) {
        self.send(GaugeUpdate::Decrement(value));
    }

    fn set(&self, value: f64) {
        self.send(GaugeUpdate::Set(value));
    }
}

struct ChannelHistogram {
    sender: MetricSender,
    key: Key,
}

impl ChannelHistogram {
    fn new(sender: MetricSender, key: Key) -> Self {
        Self { sender, key }
    }
}

impl HistogramFn for ChannelHistogram {
    fn record(&self, value: f64) {
        let _ = self.sender.send(MetricUpdate::Histogram {
            key: self.key.clone(),
            value,
        });
    }
}

enum MetricUpdate {
    Counter { key: Key, update: CounterUpdate },
    Gauge { key: Key, update: GaugeUpdate },
    Histogram { key: Key, value: f64 },
}

enum CounterUpdate {
    Increment(u64),
    Absolute(u64),
}

enum GaugeUpdate {
    Increment(f64),
    Decrement(f64),
    Set(f64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::value_as_f64;
    use enya_metrics_store::{Database, MetricName, object_store};
    use object_store::local::LocalFileSystem;
    use std::sync::Arc;
    use tempfile::TempDir;

    const METRIC_NAME: &str = "enya.test.metric";
    const FILTER: &str = "env:test";
    const GROUP: &str = "test";

    async fn temp_store() -> (TempDir, MetricsStore) {
        let dir = TempDir::new().expect("tempdir");
        let object_store =
            Arc::new(LocalFileSystem::new_with_prefix(dir.path()).expect("object store"));
        let db = Database::builder()
            .open(object_store, "/")
            .await
            .expect("open database");
        let store = MetricsStore::new(db, None, None);
        (dir, store)
    }

    async fn read_sum(store: &MetricsStore) -> f64 {
        let metric = MetricName::try_from(METRIC_NAME).unwrap();
        let result = store
            .database()
            .sum(metric, "env")
            .filter(FILTER)
            .build()
            .await
            .expect("build sum")
            .collect()
            .await
            .expect("collect sum");
        value_as_f64(result[GROUP][0].value)
    }

    #[tokio::test]
    async fn persist_value_writes_metric_sample() {
        let (_tmp, store) = temp_store().await;
        let key = Key::from_parts(METRIC_NAME, &[("env", GROUP)]);

        persist_value(&store, &key, 42.5).await;

        assert!((read_sum(&store).await - 42.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn persist_metric_event_handles_counter() {
        let (_tmp, store) = temp_store().await;
        let key = Key::from_parts(METRIC_NAME, &[("env", GROUP)]);

        persist_metric_event(
            &store,
            MetricUpdate::Counter {
                key,
                update: CounterUpdate::Increment(7),
            },
        )
        .await;
        assert!((read_sum(&store).await - 7.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn persist_metric_event_handles_gauge() {
        let (_tmp, store) = temp_store().await;
        let key = Key::from_parts(METRIC_NAME, &[("env", GROUP)]);

        persist_metric_event(
            &store,
            MetricUpdate::Gauge {
                key,
                update: GaugeUpdate::Set(-3.0),
            },
        )
        .await;
        assert!((read_sum(&store).await - (-3.0)).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn persist_metric_event_handles_histogram() {
        let (_tmp, store) = temp_store().await;
        let key = Key::from_parts(METRIC_NAME, &[("env", GROUP)]);

        persist_metric_event(&store, MetricUpdate::Histogram { key, value: 5.5 }).await;
        assert!((read_sum(&store).await - 5.5).abs() < f64::EPSILON);
    }
}
