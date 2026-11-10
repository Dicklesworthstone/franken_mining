//! DESIGN: bounded source reads and CLI envelopes for OMF period schedules (BD-194/252).
//! Determinism: DeterministicPerIsa

use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_io::omf::scheduling;

use crate::{Envelope, NextAction, worksheet_input};

pub const USAGE: &str = "plan schedule --omf <model.omf> --spec <schedule.json> [--list-schedule]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub omf: PathBuf,
    pub spec: PathBuf,
    pub list_schedule: bool,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut omf, mut spec, mut list_schedule) = (None, None, false);
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if flag == "--list-schedule" {
            if std::mem::replace(&mut list_schedule, true) {
                return Err("plan schedule: duplicate --list-schedule".into());
            }
        } else {
            let slot = match flag {
                "--omf" => &mut omf,
                "--spec" => &mut spec,
                _ => return Err(format!("unsupported OMF option {flag}; {USAGE}")),
            };
            i += 1;
            let value = args
                .get(i)
                .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
                .ok_or_else(|| format!("plan schedule: missing value for {flag}; {USAGE}"))?;
            if slot.replace(PathBuf::from(value)).is_some() {
                return Err(format!("plan schedule: duplicate {flag}"));
            }
        }
        i += 1;
    }
    Ok(Command {
        omf: omf.ok_or("plan schedule requires --omf")?,
        spec: spec.ok_or("plan schedule --omf requires --spec")?,
        list_schedule,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (bytes, source) =
            worksheet_input::read_bytes(&root.join(&command.omf), "OMF", 1024 * 1024 * 1024)?;
        let (spec, spec_source) = worksheet_input::read(
            &root.join(&command.spec),
            "OMF schedule declaration",
            1024 * 1024,
        )?;
        let report =
            scheduling::evaluate(&bytes, &source, &spec, &spec_source, command.list_schedule)?;
        let mut envelope = Envelope::ok("plan schedule", report.json, Vec::new());
        envelope.rendered_markdown = Some(report.markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan schedule",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect OMF schedule declarations, capacity scales and candidate periods",
            )],
        )
    });
    envelope.no_claim = Some(scheduling::NO_CLAIM);
    envelope
}
