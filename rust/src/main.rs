#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::redundant_clone
    )
)]

pub mod cli;
pub mod config;
pub mod core;
pub mod fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match cli::dispatch(&args) {
        Ok(()) => {
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("snap: {err}");
            std::process::exit(1);
        }
    }
}
