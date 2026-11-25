//! `plan pit`: certified ultimate pits from MineLib pairs or regular OMF block models (BD-194/250).
//! MineLib candidates remain untrusted (bd-nq06); OMF slopes and value scaling are explicit declarations.
//! Determinism: DeterministicPerIsa (OMF value scaling); MineLib arithmetic is Exact.

use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_io::{
    minelib_report,
    omf::{
        self,
        pit_report::{MAX_BYTES, PitWitness},
    },
};
use fmine_optimize::{
    pitflow::minelib,
    plan_cert::{PlanCertMetadata, certify_ultimate_pit},
};

use crate::{
    Envelope, NextAction,
    plan_certificate::{self, CertificateBasis},
    worksheet_input,
};

pub const USAGE: &str = "plan pit --upit <file.upit> --prec <file.prec> [--decimals N] [--round] [--list-blocks] [--candidate <file>] | plan pit --omf <file.omf> --block-model <name> --value-attribute <name> --per-unit N --slope-rise N --slope-run N --slope-levels N --round [--list-blocks]\nBoth forms accept [--certificate-currency CODE --certificate-horizon strategic|tactical|short_term]";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{",
    r#""input":"MineLib UPIT pair: <name>.upit (NAME, TYPE: UPIT, NBLOCKS, OBJECTIVE_FUNCTION, one '<block> <value>' line per block) and <name>.prec (one '<block> <count> <required>...' line per block); each file at most 1 GiB, SHA-256 bound","#,
    r#""values":"Plain decimals scaled to integers at --decimals places (default 6, at most 9); a value with more non-zero places refuses unless --round, which rounds half away from zero and reports the count and the bound on the value","#,
    r#""output":"Exact value at the declared places, pit and model sizes, the pseudoflow certificate re-verified (closed pit, no flow into it, excess signs), solver counters; --list-blocks adds the in-pit block ids","#,
    r#""candidate":"MineLib only: --candidate <file>, whitespace-separated block ids ('%' comments), SHA-256 bound; reports its exact value, unmet requirements and witnesses, certified gap when closed, and verdict not_closed, optimal or suboptimal; candidate claims are ignored","#,
    r#""omf":{"command":"plan pit --omf <file.omf> --block-model <name> --value-attribute <name> --per-unit N --slope-rise N --slope-run N --slope-levels N --round [--list-blocks]","input":"OMF 2.x archive, regular file at most 1 GiB, exact bytes SHA-256 bound; one uniquely named regular parent-block model with W straight up and a unique Number attribute on Primitives","slopes":"Uniform declared slope tangent rise/run with positive integer parts; positive slope-levels sets the finite cone depth. No geotechnical slope approval is inferred","values":"Positive integer per-unit scale is mandatory. --round explicitly accepts the existing OMF adapter's nearest-integer scaling, halves away from zero. Certificate covers this scaled integer model only; raw-attribute optimum is not certified","block_order":"Listed block ids are OMF order: U fastest, then V, then W from bottom upward","output":"fmine.plan-pit.omf.v1: source, selectors, coordinate context, slope declaration, scale, exact scaled value and ratio, pit/arc counts and verified pseudoflow certificate","limits":"No sub-block values, tensor grid, sector/domain-specific slope table, candidate, schedule, price/cost derivation or reserve claim; MineLib-only flags refuse"},"#,
    r#""plan_certificate":"Both forms accept --certificate-currency CODE with --certificate-horizon strategic|tactical|short_term. JSON includes payload.plan_certificate with canonical FMT-PCERT-1 bytes and digest. The closure proof covers the scaled model. Use fmine-verify plancert with --inputs-dir to also rehash the original files.","#,
    r#""no_claim":"Not a reserve, schedule or mine plan"}"#,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    MineLib { upit: PathBuf, prec: PathBuf },
    Omf(OmfInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmfInput {
    pub path: PathBuf,
    pub spec: omf::pit_report::PitSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: Input,
    pub decimals: u32,
    pub round: bool,
    pub list_blocks: bool,
    pub candidate: Option<PathBuf>,
    certificate: Option<CertificateBasis>,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("plan")
        || args.get(1).map(String::as_str) != Some("pit")
    {
        return Err(USAGE.into());
    }
    let (mut upit, mut prec, mut decimals, mut candidate) = (None, None, None, None);
    let (mut omf, mut block_model, mut value_attribute) = (None, None, None);
    let (mut per_unit, mut slope_rise, mut slope_run, mut slope_levels) = (None, None, None, None);
    let (mut certificate_currency, mut certificate_horizon) = (None, None);
    let (mut round, mut list_blocks) = (false, false);
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if let Some(seen) = match flag {
            "--round" => Some(std::mem::replace(&mut round, true)),
            "--list-blocks" => Some(std::mem::replace(&mut list_blocks, true)),
            _ => None,
        } {
            if seen {
                return Err(format!("plan pit: duplicate {flag}"));
            }
            i += 1;
            continue;
        }
        let value = args
            .get(i + 1)
            .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("plan pit: missing value for {flag}; {USAGE}"))?;
        let fresh = match flag {
            "--upit" => upit.replace(PathBuf::from(value)).is_none(),
            "--prec" => prec.replace(PathBuf::from(value)).is_none(),
            "--candidate" => candidate.replace(PathBuf::from(value)).is_none(),
            "--omf" => omf.replace(PathBuf::from(value)).is_none(),
            "--block-model" => block_model.replace(value.clone()).is_none(),
            "--value-attribute" => value_attribute.replace(value.clone()).is_none(),
            "--certificate-currency" => certificate_currency.replace(value.as_str()).is_none(),
            "--certificate-horizon" => certificate_horizon.replace(value.as_str()).is_none(),
            "--per-unit" | "--slope-rise" | "--slope-run" | "--slope-levels" => {
                let number = value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| format!("plan pit: {flag} must be a positive u32 integer"))?;
                match flag {
                    "--per-unit" => per_unit.replace(number).is_none(),
                    "--slope-rise" => slope_rise.replace(number).is_none(),
                    "--slope-run" => slope_run.replace(number).is_none(),
                    _ => slope_levels.replace(number).is_none(),
                }
            }
            "--decimals" => {
                let d = value
                    .parse::<u32>()
                    .ok()
                    .filter(|d| *d <= minelib::MAX_DECIMALS)
                    .ok_or_else(|| {
                        format!(
                            "plan pit: --decimals must be 0 to {}",
                            minelib::MAX_DECIMALS
                        )
                    })?;
                decimals.replace(d).is_none()
            }
            _ => return Err(format!("plan pit: unsupported option {flag}; {USAGE}")),
        };
        if !fresh {
            return Err(format!("plan pit: duplicate {flag}"));
        }
        i += 2;
    }
    let input = if let Some(path) = omf {
        if upit.is_some() || prec.is_some() || decimals.is_some() || candidate.is_some() {
            return Err(
                "plan pit: --omf cannot combine with --upit, --prec, --decimals or --candidate"
                    .into(),
            );
        }
        if !round {
            return Err(
                "plan pit: --omf requires --round to accept scaled integer block values".into(),
            );
        }
        Input::Omf(OmfInput {
            path,
            spec: omf::pit_report::PitSpec {
                block_model: block_model.ok_or("plan pit --omf requires --block-model")?,
                value_attribute: value_attribute
                    .ok_or("plan pit --omf requires --value-attribute")?,
                per_unit: per_unit.ok_or("plan pit --omf requires --per-unit")?,
                slope_rise: slope_rise.ok_or("plan pit --omf requires --slope-rise")?,
                slope_run: slope_run.ok_or("plan pit --omf requires --slope-run")?,
                slope_levels: slope_levels.ok_or("plan pit --omf requires --slope-levels")?,
            },
        })
    } else {
        if block_model.is_some()
            || value_attribute.is_some()
            || per_unit.is_some()
            || slope_rise.is_some()
            || slope_run.is_some()
            || slope_levels.is_some()
        {
            return Err("plan pit: OMF model, value and slope options require --omf".into());
        }
        Input::MineLib {
            upit: upit.ok_or("plan pit requires --upit or --omf")?,
            prec: prec.ok_or("plan pit requires --prec")?,
        }
    };
    Ok(Command {
        input,
        decimals: decimals.unwrap_or(6),
        round,
        list_blocks,
        candidate,
        certificate: CertificateBasis::parse(
            "plan pit",
            certificate_currency,
            certificate_horizon,
        )?,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    match &command.input {
        Input::MineLib { upit, prec } => execute_minelib(command, root, upit, prec),
        Input::Omf(input) => execute_omf(command, input, root),
    }
}

fn append_certificate(
    meta: Option<PlanCertMetadata>,
    witness: PitWitness,
    json: &mut String,
    markdown: &mut String,
) -> Result<(), Refusal> {
    if let Some(meta) = meta {
        let certificate = certify_ultimate_pit(&meta, &witness.problem, &witness.solution)?;
        plan_certificate::attach(&certificate, json, markdown)?;
    }
    Ok(())
}

fn execute_minelib(command: &Command, root: &Path, upit_path: &Path, prec_path: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (upit, upit_source) = worksheet_input::read(&root.join(upit_path), "UPIT", MAX_BYTES)?;
        let (prec, prec_source) = worksheet_input::read(&root.join(prec_path), "PREC", MAX_BYTES)?;
        let candidate = command
            .candidate
            .as_ref()
            .map(|path| {
                worksheet_input::read(&root.join(path), "CANDIDATE", MAX_BYTES)
                    .map(|(text, source)| (text, source, path.display().to_string()))
            })
            .transpose()?;
        let (mut report, witness) = minelib_report::evaluate_with_witness(
            &upit,
            &upit_source,
            &prec,
            &prec_source,
            &minelib_report::Options {
                decimals: command.decimals,
                round: command.round,
                list_blocks: command.list_blocks,
            },
            candidate
                .as_ref()
                .map(|(text, source, display_path)| minelib_report::Candidate {
                    text,
                    source,
                    display_path,
                }),
        )?;
        let meta = command
            .certificate
            .as_ref()
            .map(|basis| {
                basis.metadata(
                    &witness.model_name,
                    "declared-upit",
                    10_u128.pow(command.decimals),
                    root,
                    [("upit", &upit_source), ("precedence", &prec_source)]
                        .into_iter()
                        .chain(
                            candidate
                                .as_ref()
                                .map(|(_, source, _)| ("candidate", source)),
                        ),
                )
            })
            .transpose()?;
        append_certificate(meta, witness, &mut report.json, &mut report.markdown)?;
        let mut envelope = Envelope::ok("plan pit", report.json, Vec::new());
        envelope.no_claim = Some(minelib_report::NO_CLAIM);
        envelope.warnings = report.warnings;
        envelope.rendered_markdown = Some(report.markdown);
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan pit",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the MineLib UPIT input format, decimal places and rounding rule",
            )],
        )
    })
}

fn execute_omf(command: &Command, input: &OmfInput, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (bytes, source) =
            worksheet_input::read_bytes(&root.join(&input.path), "OMF", MAX_BYTES)?;
        let (mut report, witness) = omf::pit_report::evaluate_with_witness(
            &bytes,
            &source,
            &input.spec,
            command.list_blocks,
        )?;
        let meta = command
            .certificate
            .as_ref()
            .map(|basis| {
                if let Some(units) = &witness.value_units {
                    basis.check_omf_currency(units)?;
                }
                basis.metadata(
                    &witness.model_name,
                    "declared-omf-pit",
                    u128::from(input.spec.per_unit),
                    root,
                    [("omf", &source)],
                )
            })
            .transpose()?;
        append_certificate(meta, witness, &mut report.json, &mut report.markdown)?;
        let mut envelope = Envelope::ok("plan pit", report.json, Vec::new());
        envelope.rendered_markdown = Some(report.markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "plan pit",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the supported OMF parent grid, value attribute and explicit slope/scale declarations",
            )],
        )
    });
    envelope.no_claim = Some(omf::pit_report::NO_CLAIM);
    envelope.warnings.push(omf::pit_report::WARNING.into());
    envelope
}
