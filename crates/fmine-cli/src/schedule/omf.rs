//! DESIGN: bounded source reads and CLI envelopes for OMF schedules, shells and designs (BD-194/252).
//! Determinism: DeterministicPerIsa

use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_io::omf::{design_report, scheduling, shells_report};
use scheduling::external_lp::{self, Request, ValueFiles};

use crate::{Envelope, NextAction, worksheet_input};

pub const USAGE: &str = "plan schedule --omf <model.omf> --spec <schedule.json> [--list-schedule] [--export-lp | --lp-sha256 <hex> [--lp-primal <values.txt>] [--lp-dual <values.txt>]]";
pub const SHELLS_USAGE: &str = "plan shells --omf <model.omf> --spec <shells.json> [--list-blocks]";
pub const DESIGN_USAGE: &str = "plan design --omf <model.omf> --spec <design.json> [--list-blocks]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Schedule,
    Shells,
    Design,
}

impl Kind {
    fn labels(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Schedule => ("plan schedule", "--list-schedule", scheduling::NO_CLAIM),
            Self::Shells => ("plan shells", "--list-blocks", shells_report::NO_CLAIM),
            Self::Design => ("plan design", "--list-blocks", design_report::NO_CLAIM),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub kind: Kind,
    pub omf: PathBuf,
    pub spec: PathBuf,
    pub list_items: bool,
    pub export_lp: bool,
    pub lp_sha256: Option<String>,
    pub lp_values: [Option<PathBuf>; 2],
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut fields, words) =
        crate::journal_arguments(args, &["--list-schedule", "--list-blocks", "--export-lp"])?;
    let kind = match words.as_slice() {
        ["plan", "schedule"] => Kind::Schedule,
        ["plan", "shells"] => Kind::Shells,
        ["plan", "design"] => Kind::Design,
        _ => return Err(format!("{USAGE} | {SHELLS_USAGE} | {DESIGN_USAGE}")),
    };
    let (verb, list_flag, _) = kind.labels();
    if fields
        .iter()
        .any(|(k, v)| ![list_flag, "--export-lp"].contains(&k.as_str()) && v.trim().is_empty())
    {
        return Err(format!("{verb}: option values must be nonempty"));
    }
    let command = Command {
        kind,
        omf: fields
            .remove("--omf")
            .map(PathBuf::from)
            .ok_or_else(|| format!("{verb} requires --omf"))?,
        spec: fields
            .remove("--spec")
            .map(PathBuf::from)
            .ok_or_else(|| format!("{verb} requires --spec"))?,
        list_items: fields.remove(list_flag).is_some(),
        export_lp: fields.remove("--export-lp").is_some(),
        lp_sha256: fields.remove("--lp-sha256"),
        lp_values: ["--lp-primal", "--lp-dual"].map(|key| fields.remove(key).map(PathBuf::from)),
    };
    let imported = command.lp_values.iter().any(Option::is_some);
    if !fields.is_empty()
        || imported != command.lp_sha256.is_some()
        || (command.export_lp && imported)
        || (kind != Kind::Schedule && (command.export_lp || imported))
    {
        return Err(format!("{verb}: incompatible or unknown OMF options"));
    }
    Ok(command)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let (verb, _, no_claim) = command.kind.labels();
    let run = || -> Result<Envelope, Refusal> {
        let (bytes, source) =
            worksheet_input::read_bytes(&root.join(&command.omf), "OMF", 1024 * 1024 * 1024)?;
        let (spec, spec_source) = worksheet_input::read(
            &root.join(&command.spec),
            &format!("{verb} declaration"),
            1024 * 1024,
        )?;
        let values = ValueFiles::read(root, &command.lp_values)?;
        let request = command
            .lp_sha256
            .as_deref()
            .map(|digest| values.request(digest))
            .or(command.export_lp.then_some(Request::Export));
        let report = if request.is_some() {
            scheduling::evaluate_with_lp(
                &bytes,
                &source,
                &spec,
                &spec_source,
                command.list_items,
                request,
            )?
        } else {
            let evaluate = match command.kind {
                Kind::Schedule => scheduling::evaluate,
                Kind::Shells => shells_report::evaluate,
                Kind::Design => design_report::evaluate,
            };
            evaluate(&bytes, &source, &spec, &spec_source, command.list_items)?
        };
        let mut envelope = Envelope::ok(verb, report.json, Vec::new());
        envelope.rendered_markdown = Some(report.markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            verb,
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect OMF declarations",
            )],
        )
    });
    envelope.no_claim = Some(if command.export_lp {
        external_lp::NO_CLAIM
    } else {
        no_claim
    });
    envelope
}
