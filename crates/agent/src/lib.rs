//! Enya agent: JSON-RPC session, HTTP server, watch engine.

#[cfg(feature = "serve")]
mod db;
#[cfg(feature = "serve")]
mod engine;
#[cfg(feature = "serve")]
mod router;
mod session;

pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

/// Start the Enya agent server.
#[cfg(feature = "serve")]
pub fn run(workspace: Option<&str>, port: u16, bind: &str, open: bool) -> Result {
    router::run(workspace, port, bind, open)
}

/// Start a JSON-RPC 2.0 session over stdin/stdout.
pub fn run_session() -> Result {
    session::run()
}
