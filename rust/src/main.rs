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
pub mod http;
pub mod presentation;

fn main() {
    let stream_modes = match presentation::current_stream_modes() {
        Ok(modes) => modes,
        Err(err) => {
            eprintln!("snap: {err}");
            std::process::exit(1);
        }
    };

    let args: Vec<String> = std::env::args().collect();
    match cli::dispatch(&args, stream_modes) {
        Ok(()) => {
            std::process::exit(0);
        }
        Err(err) => {
            let error_msg = format!("snap: {err}");
            let formatted = presentation::format_error(&error_msg, stream_modes.stderr);
            eprint!("{formatted}");
            std::process::exit(1);
        }
    }
}
