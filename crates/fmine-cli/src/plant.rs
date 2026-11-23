//! DESIGN: source-bound declared plant worksheets over the accounting owners (BD-177).
//! Exact nominal accounting and conditional Gaussian reconciliation remain separate modes.
//! Determinism: Exact for balance; DeterministicPerIsa for reconciliation.

mod grinding;
mod network;
pub(crate) mod production;
mod project;
mod raw_balance;
mod recovery;
mod rtd;
mod trajectory;
mod transfer;
mod two_product;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_core::Digest32;
use fmine_evidence::Refusal;
use fmine_plant::inventory::worksheet;
pub use fmine_plant::inventory::worksheet::Mode;
use fmine_plant::inventory::worksheet::claim_json;
use fmine_units::ExactQty;

use crate::{Envelope, NextAction, json_string as quoted, worksheet_input};

pub const BALANCE_USAGE: &str = "plant balance --input <worksheet.toml>";
pub const TWO_PRODUCT_USAGE: &str =
    "plant balance --feed <material.toml> --concentrate <material.toml> --tailings <material.toml>";
pub const RECONCILE_USAGE: &str = "plant reconcile --input <worksheet.toml>";
pub const RAW_BALANCE_USAGE: &str = "plant reconcile-raw --input <worksheet.toml>";
pub const RAW_BALANCE_DOCS_JSON: &str = fmine_plant::raw_measurements::balance::ROBOT_DOCS_JSON;
pub const PROJECT_USAGE: &str = "plant project --input <worksheet.toml>";
pub const PROJECT_DOCS_JSON: &str = project::ROBOT_DOCS_JSON;
pub const TRANSFER_USAGE: &str = "plant transfer --input <worksheet.toml>";
pub const TRANSFER_DOCS_JSON: &str = transfer::ROBOT_DOCS_JSON;
pub const NETWORK_USAGE: &str = "plant flowsheet --input <worksheet.toml>";
pub const NETWORK_DOCS_JSON: &str = network::ROBOT_DOCS_JSON;
pub const INFER_USAGE: &str = "plant infer --input <worksheet.toml>";
pub const INFERENCE_DOCS_JSON: &str = network::INFERENCE_ROBOT_DOCS_JSON;
pub const RECOVERY_USAGE: &str = "plant recovery --input <worksheet.toml>";
pub const RECOVERY_DOCS_JSON: &str = recovery::ROBOT_DOCS_JSON;
pub const RTD_USAGE: &str = "plant rtd --input <worksheet.toml>";
pub const RTD_DOCS_JSON: &str = fmine_plant::rtd::worksheet::ROBOT_DOCS_JSON;
pub const RTD_INVERSE_USAGE: &str = "plant rtd-infer --input <worksheet.toml>";
pub const RTD_INVERSE_DOCS_JSON: &str = fmine_plant::rtd::inverse::worksheet::ROBOT_DOCS_JSON;
pub const GRINDING_USAGE: &str = "plant grinding --input <worksheet.toml>";
pub const GRINDING_DOCS_JSON: &str = fmine_plant::grinding::worksheet::ROBOT_DOCS_JSON;
pub const PRODUCTION_USAGE: &str = "plant production --input <coupling.toml> --grinding <grinding.toml> --recovery <recovery.toml>";
pub const PRODUCTION_DOCS_JSON: &str = fmine_plant::production::worksheet::ROBOT_DOCS_JSON;
pub const TRAJECTORY_USAGE: &str = "plant trajectory --input <worksheet.toml>";
pub const TRAJECTORY_DOCS_JSON: &str = fmine_plant::trajectory::worksheet::ROBOT_DOCS_JSON;
const MAX_BYTES: u64 = worksheet::MAX_BYTES;
pub const ROBOT_DOCS_JSON: &str = worksheet::ROBOT_DOCS_JSON;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    TwoProduct {
        feed: PathBuf,
        concentrate: PathBuf,
        tailings: PathBuf,
    },
    Inventory {
        mode: Mode,
        input: PathBuf,
    },
    RawBalance {
        input: PathBuf,
    },
    Project {
        input: PathBuf,
    },
    Transfer {
        input: PathBuf,
    },
    Network {
        input: PathBuf,
    },
    Infer {
        input: PathBuf,
    },
    Recovery {
        input: PathBuf,
    },
    Rtd {
        input: PathBuf,
    },
    RtdInverse {
        input: PathBuf,
    },
    Grinding {
        input: PathBuf,
    },
    Production {
        input: PathBuf,
        grinding: PathBuf,
        recovery: PathBuf,
    },
    Trajectory {
        input: PathBuf,
    },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let three_streams = args.get(1).is_some_and(|w| w == "balance")
        && args
            .iter()
            .skip(2)
            .any(|w| matches!(w.as_str(), "--feed" | "--concentrate" | "--tailings"));
    if args.first().is_some_and(|word| word == "plant")
        && (three_streams || args.get(1).is_some_and(|w| w == "production"))
    {
        let (options, usage) = if three_streams {
            (["--feed", "--concentrate", "--tailings"], TWO_PRODUCT_USAGE)
        } else {
            (["--input", "--grinding", "--recovery"], PRODUCTION_USAGE)
        };
        if args.len() != 8 {
            return Err(usage.into());
        }
        let mut paths = BTreeMap::new();
        for pair in args[2..].as_chunks::<2>().0 {
            if !options.contains(&pair[0].as_str())
                || pair[1].trim().is_empty()
                || pair[1].starts_with("--")
                || paths
                    .insert(pair[0].as_str(), PathBuf::from(&pair[1]))
                    .is_some()
            {
                return Err(usage.into());
            }
        }
        return if three_streams {
            Ok(Command::TwoProduct {
                feed: paths.remove("--feed").ok_or(usage)?,
                concentrate: paths.remove("--concentrate").ok_or(usage)?,
                tailings: paths.remove("--tailings").ok_or(usage)?,
            })
        } else {
            Ok(Command::Production {
                input: paths.remove("--input").ok_or(usage)?,
                grinding: paths.remove("--grinding").ok_or(usage)?,
                recovery: paths.remove("--recovery").ok_or(usage)?,
            })
        };
    }
    if args.len() != 4
        || args[0] != "plant"
        || args[2] != "--input"
        || args[3].trim().is_empty()
        || args[3].starts_with("--")
    {
        return Err(format!(
            "{BALANCE_USAGE} | {RECONCILE_USAGE} | {RAW_BALANCE_USAGE} | {PROJECT_USAGE} | {TRANSFER_USAGE} | {NETWORK_USAGE} | {INFER_USAGE} | {RECOVERY_USAGE} | {RTD_USAGE} | {RTD_INVERSE_USAGE} | {GRINDING_USAGE} | {TRAJECTORY_USAGE}"
        ));
    }
    let input = PathBuf::from(&args[3]);
    match args[1].as_str() {
        "balance" => Ok(Command::Inventory {
            mode: Mode::Balance,
            input,
        }),
        "reconcile" => Ok(Command::Inventory {
            mode: Mode::Reconcile,
            input,
        }),
        "reconcile-raw" => Ok(Command::RawBalance { input }),
        "project" => Ok(Command::Project { input }),
        "transfer" => Ok(Command::Transfer { input }),
        "flowsheet" => Ok(Command::Network { input }),
        "infer" => Ok(Command::Infer { input }),
        "recovery" => Ok(Command::Recovery { input }),
        "rtd" => Ok(Command::Rtd { input }),
        "rtd-infer" => Ok(Command::RtdInverse { input }),
        "grinding" => Ok(Command::Grinding { input }),
        "trajectory" => Ok(Command::Trajectory { input }),
        _ => Err(format!(
            "{BALANCE_USAGE} | {RECONCILE_USAGE} | {RAW_BALANCE_USAGE} | {PROJECT_USAGE} | {TRANSFER_USAGE} | {NETWORK_USAGE} | {INFER_USAGE} | {RECOVERY_USAGE} | {RTD_USAGE} | {RTD_INVERSE_USAGE} | {GRINDING_USAGE} | {TRAJECTORY_USAGE}"
        )),
    }
}

fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn numbers(values: impl IntoIterator<Item = f64>) -> String {
    format!(
        "[{}]",
        values.into_iter().map(number).collect::<Vec<_>>().join(",")
    )
}
fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect plant worksheet units, times, complete roles and covariance requirements",
    )]
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let (mode, input) = match command {
        Command::TwoProduct {
            feed,
            concentrate,
            tailings,
        } => return two_product::execute(root, [feed, concentrate, tailings]),
        Command::Inventory { mode, input } => (*mode, input),
        Command::RawBalance { input } => return raw_balance::execute(input, root),
        Command::Project { input } => return project::execute(input, root),
        Command::Transfer { input } => return transfer::execute(input, root),
        Command::Network { input } => return network::execute(input, root),
        Command::Infer { input } => return network::execute_unmeasured(input, root),
        Command::Recovery { input } => return recovery::execute(input, root),
        Command::Rtd { input } => return rtd::execute(input, root),
        Command::RtdInverse { input } => return rtd::execute_inverse(input, root),
        Command::Grinding { input } => return grinding::execute(input, root),
        Command::Production {
            input,
            grinding,
            recovery,
        } => {
            return production::execute(input, grinding, recovery, root);
        }
        Command::Trajectory { input } => return trajectory::execute(input, root),
    };
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(input), "plant worksheet", MAX_BYTES)?;
        let result = worksheet::evaluate(&text, &source, mode)?;
        let mut envelope = match result.refusal() {
            Some(refusal) => Envelope::refused(mode.verb(), refusal.clone(), actions()),
            None => Envelope::ok(mode.verb(), String::new(), vec![]),
        };
        envelope.payload_json = result.to_json().into();
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused(mode.verb(), refusal, actions()));
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope.warnings.push("Worksheet observations, completeness and any error policy are Assumed, Internal and uncalibrated; exact byte binding is not independent source verification.".into());
    envelope
}
