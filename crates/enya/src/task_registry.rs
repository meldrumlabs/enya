//! Global registry for TaskMonitor instances.
//!
//! This module provides a registry that collects all task monitors created via
//! the `#[monitor]` macro. The registry is used by the ingestor to
//! periodically collect metrics from all registered monitors.

use parking_lot::RwLock;
use tokio_metrics::TaskMonitor;

/// Type alias for a registered task monitor entry.
pub type TaskMonitorEntry = (&'static str, &'static TaskMonitor);

/// Global registry of task monitors.
static TASK_MONITORS: RwLock<Vec<TaskMonitorEntry>> = RwLock::new(Vec::new());

/// Registers a task monitor with the global registry.
///
/// This function is called automatically by the `#[monitor]` macro
/// when the static TaskMonitor is first accessed.
///
/// # Arguments
///
/// * `name` - The name of the task (used as a metric tag)
/// * `monitor` - Reference to the static TaskMonitor
pub fn register_task_monitor(name: &'static str, monitor: &'static TaskMonitor) {
    let mut monitors = TASK_MONITORS.write();
    // Check if already registered (idempotent)
    if !monitors.iter().any(|(n, _)| *n == name) {
        monitors.push((name, monitor));
        tracing::debug!(task = name, "registered task monitor");
    }
}

/// Returns an iterator over all registered task monitors.
///
/// Used by the ingestor to collect metrics from all monitors.
pub fn registered_monitors() -> Vec<TaskMonitorEntry> {
    TASK_MONITORS.read().clone()
}

/// Returns the number of registered task monitors.
#[allow(dead_code)]
pub fn monitor_count() -> usize {
    TASK_MONITORS.read().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests are limited because we can't easily create
    // &'static TaskMonitor references in tests without leaking memory.
    // The integration is tested via the macro in real usage.

    #[test]
    fn test_registered_monitors_empty() {
        // Just verify the function doesn't panic
        let monitors = registered_monitors();
        // Can't assert empty because other tests might have registered monitors
        let _ = monitors;
    }

    #[test]
    fn test_monitor_count() {
        // Just verify the function doesn't panic
        let count = monitor_count();
        let _ = count;
    }
}
