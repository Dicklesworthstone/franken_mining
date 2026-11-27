//! `fmined` node daemon binary (`BD-119`, plan §26.6–§26.7, §30.3).

use std::env;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use fmine_runtime::{HostWallClock, WallClock};
use fmine_service::config::{USAGE, parse_cli_args};
use fmine_service::node::FminedNode;

fn now_unix_ns() -> u64 {
    HostWallClock
        .now_ns()
        .ok()
        .and_then(|ns| u64::try_from(ns).ok())
        .unwrap_or(0)
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let parsed = match parse_cli_args(&raw_args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    if parsed.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let node = match FminedNode::boot(parsed.config, now_unix_ns()) {
        Ok(n) => n,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(err.exit_code());
        }
    };

    if parsed.once_status {
        println!("{}", node.status_json());
        return ExitCode::SUCCESS;
    }

    #[cfg(unix)]
    {
        let status = node.status_json();
        let shared = Arc::new(Mutex::new(node));
        let server = match fmine_service::socket::FminedServer::bind(shared) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(err.exit_code());
            }
        };
        println!("{status}");
        match server.serve_until_shutdown() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(err.exit_code())
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (node, Arc::new(Mutex::new(())));
        eprintln!("fmined Unix domain socket serving requires a Unix host; use --once-status");
        ExitCode::from(5)
    }
}
