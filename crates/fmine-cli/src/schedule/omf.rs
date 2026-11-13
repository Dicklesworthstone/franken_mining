//! DESIGN: bounded source reads and CLI envelopes for OMF schedules, shells and designs (BD-194/252).
//! Determinism: DeterministicPerIsa

use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_io::omf::{design_report, scheduling, shells_report};

use crate::{Envelope, NextAction, worksheet_input};

pub const USAGE: &str = "plan schedule --omf <model.omf> --spec <schedule.json> [--list-schedule]";
pub const SHELLS_USAGE: &str = "plan shells --omf <model.omf> --spec <shells.json> [--list-blocks]";
pub const DESIGN_USAGE: &str = "plan design --omf <model.omf> --spec <design.json> [--list-blocks]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Schedule,
    Shells,
    Design,
}

impl Kind {
    fn labels(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::Schedule => (
                "plan schedule",
                USAGE,
                "--list-schedule",
                scheduling::NO_CLAIM,
            ),
            Self::Shells => (
                "plan shells",
                SHELLS_USAGE,
                "--list-blocks",
                shells_report::NO_CLAIM,
            ),
            Self::Design => (
                "plan design",
                DESIGN_USAGE,
                "--list-blocks",
                design_report::NO_CLAIM,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub kind: Kind,
    pub omf: PathBuf,
    pub spec: PathBuf,
    pub list_items: bool,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let kind = match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("plan"), Some("schedule")) => Kind::Schedule,
        (Some("plan"), Some("shells")) => Kind::Shells,
        (Some("plan"), Some("design")) => Kind::Design,
        _ => return Err(format!("{USAGE} | {SHELLS_USAGE} | {DESIGN_USAGE}")),
    };
    let (verb, usage, list_flag, _) = kind.labels();
    let (mut omf, mut spec, mut list_items) = (None, None, false);
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if flag == list_flag {
            if std::mem::replace(&mut list_items, true) {
                return Err(format!("{verb}: duplicate {list_flag}"));
            }
        } else {
            let slot = match flag {
                "--omf" => &mut omf,
                "--spec" => &mut spec,
                _ => return Err(format!("unsupported OMF option {flag}; {usage}")),
            };
            i += 1;
            let value = args
                .get(i)
                .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
                .ok_or_else(|| format!("{verb}: missing value for {flag}; {usage}"))?;
            if slot.replace(PathBuf::from(value)).is_some() {
                return Err(format!("{verb}: duplicate {flag}"));
            }
        }
        i += 1;
    }
    Ok(Command {
        kind,
        omf: omf.ok_or_else(|| format!("{verb} requires --omf"))?,
        spec: spec.ok_or_else(|| format!("{verb} --omf requires --spec"))?,
        list_items,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let (verb, _, _, no_claim) = command.kind.labels();
    let run = || -> Result<Envelope, Refusal> {
        let (bytes, source) =
            worksheet_input::read_bytes(&root.join(&command.omf), "OMF", 1024 * 1024 * 1024)?;
        let (spec, spec_source) = worksheet_input::read(
            &root.join(&command.spec),
            &format!("{verb} declaration"),
            1024 * 1024,
        )?;
        let (json, markdown) = match command.kind {
            Kind::Schedule => {
                let report =
                    scheduling::evaluate(&bytes, &source, &spec, &spec_source, command.list_items)?;
                (report.json, report.markdown)
            }
            Kind::Shells => {
                let report = shells_report::evaluate(
                    &bytes,
                    &source,
                    &spec,
                    &spec_source,
                    command.list_items,
                )?;
                (report.json, report.markdown)
            }
            Kind::Design => {
                let report = design_report::evaluate(
                    &bytes,
                    &source,
                    &spec,
                    &spec_source,
                    command.list_items,
                )?;
                (report.json, report.markdown)
            }
        };
        let mut envelope = Envelope::ok(verb, json, Vec::new());
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            verb,
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the required OMF declarations, selectors, units and scaling",
            )],
        )
    });
    envelope.no_claim = Some(no_claim);
    envelope
}
