//! `fmine` CLI entry point (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// Exit code when the envelope cannot be written to stdout: an I/O error (plan §30.1, `BD-112`).
const EXIT_IO: u8 = 5;

fn main() -> ExitCode {
    let raw_args: Result<Vec<_>, _> = env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect();
    let default_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // Every invocation, a usage error included, prints exactly one envelope (BD-329.S3).
    let (text, exit_code) = match raw_args {
        Ok(args) => fmine_cli::run(&args, &default_root),
        Err(_) => {
            let env =
                fmine_cli::Envelope::usage_error("unknown", "arguments must be valid Unicode");
            (env.to_json(), env.exit_code)
        }
    };
    // A closed or full stdout is exit 5 with a line on stderr. `println!` would panic and exit 101, which is not
    // in the exit-code dictionary, and the envelope's own code would claim an outcome its reader never got.
    let mut out = io::stdout().lock();
    if let Err(e) = writeln!(out, "{text}").and_then(|()| out.flush()) {
        let _ = writeln!(
            io::stderr(),
            "fmine: the envelope could not be written to stdout: {e}"
        );
        return ExitCode::from(EXIT_IO);
    }
    ExitCode::from(exit_code)
}
