//! Promise-based async helpers for HTTP requests.
//!
//! This module provides thin wrappers around `poll_promise` for use with async
//! HTTP requests.

use poll_promise::Promise;

/// Create a promise channel for use with async HTTP requests.
///
/// Returns a `(Sender<T>, Promise<T>)` pair. Call `sender.send(value)` from
/// within an async task to fulfill the promise.
pub fn promise_channel<T: Send + 'static>() -> (Sender<T>, Promise<T>) {
    let (sender, promise) = Promise::new();
    (Sender(sender), promise)
}

/// A sender for fulfilling a promise.
pub struct Sender<T>(poll_promise::Sender<T>);

impl<T> Sender<T> {
    /// Send a value to the waiting promise.
    pub fn send(self, value: T) {
        self.0.send(value);
    }
}
