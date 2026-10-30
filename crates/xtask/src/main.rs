//! CLI entrypoint for `cargo run -p xtask -- <subcommand>` (`GATE-LAYERS-00`, BD-003..BD-015).

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("check-all");
    let root = workspace_root();

    match cmd {
        "loc" | "check-budget" => match xtask::check_loc_budget(&root) {
            Ok(report) => {
                println!(
                    "LOC Budget Table ({} crates): actual={} / planned={} / ceiling={}",
                    report.rows.len(),
                    report.total_actual,
                    report.total_planned,
                    report.total_ceiling
                );
                for r in &report.rows {
                    if r.actual_loc > 0 {
                        println!(
                            "  {:20} {:5} {:6} / {:6} (ceil {:6})",
                            r.name, r.layer, r.actual_loc, r.planned_loc, r.ceiling_loc
                        );
                    }
                }
                ExitCode::SUCCESS
            }
            Err(errs) => {
                for e in errs {
                    eprintln!("ERROR [loc]: {e}");
                }
                ExitCode::from(2)
            }
        },
        "check-layers" => run_one("check-layers", xtask::check_layers(&root)),
        "check-deps" => run_one("check-deps", xtask::check_deps(&root)),
        "check-oracle" => run_one("check-oracle", xtask::check_oracle(&root)),
        "check-caps" => run_one("check-caps", xtask::check_caps(&root)),
        "check-lifts" => run_one("check-lifts", xtask::check_lifts(&root)),
        "check-scope" => run_one("check-scope", xtask::check_scope(&root)),
        "check-claims" => run_one("check-claims", xtask::check_claims(&root)),
        "check-det" => run_one("check-det", xtask::check_det(&root)),
        "registry-check" => run_one(
            "registry-check",
            xtask::check_sources_registry(&root).map(|lines| {
                for line in lines {
                    println!("{line}");
                }
            }),
        ),
        "check-all" => match xtask::check_all(&root) {
            Ok(report) => {
                println!(
                    "GATE-LAYERS-00 PASS: {} crates registered, {} actual Rust LOC (planned {}, ceiling {})",
                    report.rows.len(),
                    report.total_actual,
                    report.total_planned,
                    report.total_ceiling
                );
                ExitCode::SUCCESS
            }
            Err(errs) => {
                for e in errs {
                    eprintln!("GATE-LAYERS-00 FAIL: {e}");
                }
                ExitCode::from(2)
            }
        },
        other => {
            eprintln!("Unknown xtask command: {other}");
            ExitCode::from(2)
        }
    }
}

fn run_one(name: &str, res: Result<(), Vec<String>>) -> ExitCode {
    match res {
        Ok(()) => {
            println!("{name}: PASS");
            ExitCode::SUCCESS
        }
        Err(errs) => {
            for e in errs {
                eprintln!("{name} FAIL: {e}");
            }
            ExitCode::from(2)
        }
    }
}
