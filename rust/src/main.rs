#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::redundant_clone
    )
)]

pub mod core;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--version" {
        println!("snap 0.1.0");
        std::process::exit(0);
    }

    eprintln!("snap: not implemented");
    std::process::exit(1);
}
