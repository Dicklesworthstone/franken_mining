//! `fmine` CLI entry point (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let default_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // Every invocation, a usage error included, prints exactly one envelope (BD-329.S3).
    let (text, exit_code) = fmine_cli::run(&raw_args, &default_root);
    println!("{text}");
    ExitCode::from(exit_code)
}
