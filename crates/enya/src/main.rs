#[cfg(feature = "ui")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli;
#[cfg(feature = "serve")]
mod serve;
mod session;

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run()
}
