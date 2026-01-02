//! Promise-based async helpers for HTTP requests.
//!
//! This module provides thin wrappers around `poll_promise` for use with async
//! HTTP requests. Use [`promise_channel`] to create a sender/receiver pair,
//! then send the result from an async task.

use poll_promise::Promise;

/// Create a promise channel for use with async HTTP requests.
///
/// Returns a `(Sender<T>, Promise<T>)` pair. Call `sender.send(value)` from
/// within an async task to fulfill the promise.
///
/// # Example
///
/// ```ignore
/// let (sender, promise) = promise_channel();
/// spawn(async move {
///     let result = client.get(&url).send().await;
///     sender.send(process(result));
/// });
/// // Later, poll the promise:
/// if let Some(result) = promise.ready() {
///     // Handle result
/// }
/// ```
pub fn promise_channel<T: Send + 'static>() -> (Sender<T>, Promise<T>) {
    let (sender, promise) = Promise::new();
    (Sender(sender), promise)
}

/// A sender for fulfilling a promise.
///
/// This is a thin wrapper around `poll_promise::Sender` to provide a consistent API.
pub struct Sender<T>(poll_promise::Sender<T>);

impl<T> Sender<T> {
    /// Send a value to the waiting promise.
    ///
    /// This consumes the sender, ensuring the promise can only be fulfilled once.
    pub fn send(self, value: T) {
        self.0.send(value);
    }
}
