//! `fmine` CLI entry point (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use fmine_cli::{OutputFormat, execute_command, parse_args};

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let default_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match parse_args(&raw_args, &default_root) {
        Ok((cmd, format, workspace)) => {
            let env = execute_command(&cmd, &workspace);
            match format {
                OutputFormat::Json => println!("{}", env.to_json()),
                OutputFormat::Markdown => {
                    println!("# fmine `{}` ({})\n", env.verb, env.status);
                    println!("```json\n{}\n```", env.to_json());
                }
            }
            ExitCode::from(env.exit_code)
        }
        Err(usage_err) => {
            eprintln!("fmine usage error (exit 2): {usage_err}");
            ExitCode::from(2)
        }
    }
}
