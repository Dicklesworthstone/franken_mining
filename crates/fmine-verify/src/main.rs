//! Standalone offline verifier binary (`fmine-verify`, `BD-140`, `GOAL-024`).

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (json, code) = fmine_verify::run_cli(&args);
    println!("{json}");
    ExitCode::from(code)
}
