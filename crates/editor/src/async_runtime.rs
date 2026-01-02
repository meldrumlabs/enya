//! Async runtime abstraction for cross-platform async execution.
//!
//! Provides a unified interface for spawning async tasks on both native (tokio)
//! and WASM (wasm-bindgen-futures) platforms.
//!
//! # Usage
//!
//! Create the runtime at application startup:
//!
//! ```ignore
//! // In main.rs (native only)
//! let tokio_runtime = tokio::runtime::Runtime::new().unwrap();
//! let async_runtime = AsyncRuntime::new(tokio_runtime.handle().clone());
//!
//! // Pass to your app
//! let app = EnyaApp::new(cc, async_runtime);
//! ```
//!
//! On WASM, the runtime is created automatically with no configuration needed.

use std::future::Future;

/// Handle to an async runtime for spawning tasks.
///
/// On native platforms, this wraps a tokio runtime handle.
/// On WASM, it uses wasm-bindgen-futures for task spawning.
#[derive(Clone)]
pub struct AsyncRuntime {
    #[cfg(not(target_arch = "wasm32"))]
    inner: tokio::runtime::Handle,
}

impl AsyncRuntime {
    /// Create a new async runtime from a tokio handle (native only).
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self { inner: handle }
    }

    /// Create a new async runtime for WASM.
    ///
    /// On WASM, no external runtime is needed - tasks are spawned using
    /// `wasm-bindgen-futures::spawn_local`.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// Spawn a future on the async runtime.
    ///
    /// The future will run to completion. Any result is discarded.
    /// Use channels to communicate results back if needed.
    ///
    /// On native, the future must be `Send` since it runs on a thread pool.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.inner.spawn(future);
    }

    /// Spawn a future on the async runtime.
    ///
    /// The future will run to completion. Any result is discarded.
    /// Use channels to communicate results back if needed.
    ///
    /// On WASM, futures don't need to be `Send` since everything runs on a single thread.
    #[cfg(target_arch = "wasm32")]
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }

    /// Spawn a future that returns a value, returning a handle to await it.
    ///
    /// On native, this returns a `JoinHandle` that can be awaited.
    /// On WASM, the future is spawned but the result cannot be awaited
    /// (returns a unit type instead).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_with_handle<F, T>(&self, future: F) -> tokio::task::JoinHandle<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.inner.spawn(future)
    }

    /// Spawn a blocking task on a dedicated thread pool.
    ///
    /// Use this for CPU-intensive or blocking I/O operations that would
    /// otherwise block the async runtime.
    ///
    /// On WASM, this just runs the closure synchronously (no true blocking
    /// thread pool available).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_blocking<F, T>(&self, f: F) -> tokio::task::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        // Enter the runtime context so spawn_blocking knows which runtime to use
        let _guard = self.inner.enter();
        tokio::task::spawn_blocking(f)
    }

    /// Get the underlying tokio handle (native only).
    ///
    /// Use this when you need direct access to tokio APIs.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn handle(&self) -> &tokio::runtime::Handle {
        &self.inner
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for AsyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}
