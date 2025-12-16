//! Promise-based async helpers for ehttp.
//!
//! Bridges ehttp's callback-based API to poll-promise for cleaner async handling.

use poll_promise::Promise;

/// Create a promise that will be fulfilled when the sender is called.
///
/// Returns a (sender, promise) pair. Call `sender.send(value)` from a callback
/// to fulfill the promise.
///
/// This is a thin wrapper around `poll_promise::Promise::new()` that re-exports
/// the sender type for convenience.
///
/// # Example
///
/// ```ignore
/// let (sender, promise) = promise_channel();
///
/// ehttp::fetch(request, move |result| {
///     sender.send(result);
/// });
///
/// // Later, each frame:
/// if let Some(result) = promise.ready() {
///     // Handle result
/// }
/// ```
pub fn promise_channel<T: Send + 'static>() -> (Sender<T>, Promise<T>) {
    let (sender, promise) = Promise::new();
    (Sender(sender), promise)
}

/// A one-shot sender for promise results.
///
/// Wrapper around `poll_promise::Sender` for API consistency.
pub struct Sender<T>(poll_promise::Sender<T>);

impl<T> Sender<T> {
    /// Send a value to the waiting promise.
    pub fn send(self, value: T) {
        self.0.send(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promise_channel() {
        let (sender, promise) = promise_channel();
        sender.send(42);
        // The value should be ready immediately after send
        assert_eq!(promise.ready(), Some(&42));
    }
}
