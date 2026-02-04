#[cfg(feature = "ui")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli;
#[cfg(feature = "serve")]
mod serve;
mod session;

use std::process::ExitCode;

const ART: &[u8] = include_bytes!("../art.txt");

fn main() -> ExitCode {
    print_art();
    cli::run()
}

pub fn print_art() {
    let art_str = std::str::from_utf8(ART).expect("Invalid UTF-8 in art.txt");
    print!("{art_str}");
}
