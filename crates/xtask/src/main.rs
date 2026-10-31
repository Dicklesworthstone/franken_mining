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
        "check-amend" => {
            let (base, require_git) = match (args.get(1).map(String::as_str), args.get(2)) {
                (None, _) => ("HEAD", false),
                (Some("--base"), Some(base)) => (base.as_str(), true),
                _ => {
                    eprintln!("usage: xtask check-amend [--base <rev>]");
                    return ExitCode::from(2);
                }
            };
            run_one(
                "check-amend",
                xtask::loc::check_amend(&root, base, require_git).map(|r| {
                    println!("{}", r.log_line());
                    for w in &r.warnings {
                        println!("WARN [check-amend]: {w}");
                    }
                }),
            )
        }
        "check-layers" => run_one("check-layers", xtask::check_layers(&root)),
        "check-deps" => run_one("check-deps", xtask::check_deps(&root)),
        "check-oracle" => run_one("check-oracle", xtask::check_oracle(&root)),
        "check-caps" => run_one("check-caps", xtask::check_caps(&root)),
        "check-lifts" => match args.get(1).map(String::as_str) {
            None => run_one("check-lifts", xtask::lifts::check_lifts(&root)),
            Some("--upstream") => {
                let Some(upstream) = args.get(2).map(PathBuf::from) else {
                    eprintln!("usage: xtask check-lifts [--upstream <frankensim checkout>]");
                    return ExitCode::from(2);
                };
                run_one(
                    "check-lifts --upstream",
                    xtask::lifts::check_upstream(&root, &upstream).map(|log| {
                        for line in log {
                            println!("{line}");
                        }
                    }),
                )
            }
            Some(other) => {
                eprintln!("unknown check-lifts flag {other}");
                ExitCode::from(2)
            }
        },
        "check-stack-lock" => run_one(
            "check-stack-lock",
            xtask::check_stack_lock(&root).map(|r| println!("{}", r.log_line())),
        ),
        "check-scope" => run_one("check-scope", xtask::check_scope(&root)),
        "check-beads" => run_one(
            "check-beads",
            xtask::beads::check_beads(&root).map(|r| {
                println!("{}", r.log_line());
                for w in &r.warnings {
                    println!("WARN [check-beads]: {w}");
                }
            }),
        ),
        "check-claims" => run_one("check-claims", xtask::check_claims(&root)),
        "check-det" => run_one(
            "check-det",
            xtask::check_det(&root).map(|r| println!("{}", r.log_line())),
        ),
        "check-refusals" => run_one(
            "check-refusals",
            xtask::refusals::check_refusals(&root).map(|r| {
                println!("{}", r.log_line());
                for w in &r.warnings {
                    println!("WARN [check-refusals]: {w}");
                }
            }),
        ),
        "gen-refusals" => run_one(
            "gen-refusals",
            xtask::refusals::gen_refusals(&root).map(|wrote| {
                let what = if wrote { "wrote" } else { "unchanged:" };
                println!("{what} {}", xtask::refusals::CATALOG_PATH);
            }),
        ),
        "registry-check" => run_one(
            "registry-check",
            xtask::check_sources_registry(&root).map(|lines| {
                for line in lines {
                    println!("{line}");
                }
            }),
        ),
        "forecast-rules-hash" | "forecast-record-hash" | "forecast-record-verify" => {
            let Some(path) = args.get(1).map(PathBuf::from) else {
                eprintln!("{cmd} needs a path");
                return ExitCode::from(2);
            };
            let res = match cmd {
                "forecast-rules-hash" => xtask::forecast::rules_hash(&path).map(|d| vec![d]),
                "forecast-record-hash" => {
                    xtask::forecast::record_hash(&root, &path).map(|d| vec![d])
                }
                _ => xtask::forecast::record_verify(&root, &path),
            };
            run_one(
                cmd,
                res.map(|lines| {
                    for line in lines {
                        println!("{line}");
                    }
                }),
            )
        }
        "forecast-rules-order" => run_one(
            cmd,
            xtask::forecast::rules_order(&root).map(|line| println!("{line}")),
        ),
        "forecast-baseline" => run_one(
            cmd,
            xtask::forecast::baseline_lines(&root).map(|lines| {
                for line in lines {
                    println!("{line}");
                }
            }),
        ),
        "check-int-as-001a" => match xtask::int_as::check_int_as_001a(&root) {
            Ok((report, lock)) => {
                if args.get(1).map(String::as_str) == Some("--json") {
                    for row in &report.rows {
                        println!("{}", row.log_line());
                    }
                }
                println!("{}", report.status_line(&lock));
                ExitCode::SUCCESS
            }
            Err(errs) => {
                for e in errs {
                    eprintln!("ERROR [check-int-as-001a]: {e}");
                }
                ExitCode::from(2)
            }
        },
        "check-all" => match xtask::check_all(&root) {
            Ok(report) => {
                println!(
                    "GATE-LAYERS-00 PASS: {} crates registered, {} actual Rust LOC (planned {}, ceiling {})",
                    report.rows.len(),
                    report.total_actual,
                    report.total_planned,
                    report.total_ceiling
                );
                // check_all ran these checks and they passed; read again for their counts.
                if let Ok(det) = xtask::check_det(&root) {
                    println!("{}", det.log_line());
                }
                if let Ok(lock) = xtask::check_stack_lock(&root) {
                    println!("{}", lock.log_line());
                }
                if let Ok(report) = xtask::refusals::check_refusals(&root) {
                    println!("{}", report.log_line());
                    for w in &report.warnings {
                        println!("WARN [check-refusals]: {w}");
                    }
                }
                if let Ok((report, lock)) = xtask::int_as::check_int_as_001a(&root) {
                    println!("{}", report.status_line(&lock));
                }
                if let Ok(report) = xtask::loc::check_amend(&root, "HEAD", false) {
                    println!("{}", report.log_line());
                    for w in &report.warnings {
                        println!("WARN [check-amend]: {w}");
                    }
                }
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
