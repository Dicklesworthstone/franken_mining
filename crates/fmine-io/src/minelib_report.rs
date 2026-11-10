//! DESIGN: source-bound MineLib ultimate-pit composition and untrusted candidate comparison (BD-194).
//! Determinism: Exact. This report certifies only the supplied integer-scaled values and precedence arcs.

use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::pitflow::{self, candidate, minelib};

use crate::omf::pit_report::{MAX_BYTES, escape_json, source_json};

pub const NO_CLAIM: &str = "The maximum-value closed set of the supplied block values and precedence arcs. Values and \
arcs are taken as given: no price, cost, recovery or slope is derived or checked, and the pit is not a reserve, \
a schedule or a mine plan.";

pub struct Options {
    pub decimals: u32,
    pub round: bool,
    pub list_blocks: bool,
}

pub struct Candidate<'a> {
    pub text: &'a str,
    pub source: &'a SourceAnchor,
    pub display_path: &'a str,
}

pub struct Report {
    pub json: String,
    pub markdown: String,
    pub warnings: Vec<String>,
}

/// `units` at 10^`places` as an exact decimal; the parser bounds places before rendering.
fn decimal(units: i128, places: u32) -> String {
    let scale = 10_u128.pow(places);
    let sign = if units < 0 { "-" } else { "" };
    let abs = units.unsigned_abs();
    let (int, frac) = (abs / scale, abs % scale);
    if places == 0 {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac:0width$}", width = places as usize)
    }
}

fn bind(text: &str, source: &SourceAnchor) -> Result<(), Refusal> {
    if text.len() as u64 > MAX_BYTES || source.artifact_path.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            "MineLib source must have a nonempty path and contain at most 1 GiB",
        ));
    }
    let actual = sha256(text.as_bytes());
    if actual.0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "MineLib source {} does not bind the supplied bytes; actual sha256={actual}",
                source.artifact_path
            ),
        ));
    }
    Ok(())
}

/// Solve the admitted MineLib pair and independently check an optional external candidate.
///
/// # Errors
/// Refuses incorrectly bound or oversized sources, invalid MineLib inputs, and failed certificates.
pub fn evaluate(
    upit: &str,
    upit_source: &SourceAnchor,
    prec: &str,
    prec_source: &SourceAnchor,
    options: &Options,
    candidate: Option<Candidate<'_>>,
) -> Result<Report, Refusal> {
    bind(upit, upit_source)?;
    bind(prec, prec_source)?;
    let rounding = if options.round {
        minelib::Rounding::HalfAwayFromZero
    } else {
        minelib::Rounding::Refuse
    };
    let model = minelib::parse(upit, prec, options.decimals, rounding)?;
    let pit = pitflow::solve(&model.problem)?;
    // Recheck the returned witness against the exact parsed problem before reporting any optimum.
    if pitflow::verify(&model.problem, pit.in_pit(), pit.arc_flow())? != pit.value() {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plan pit: the re-verified value differs from the solver's",
        ));
    }
    let d = options.decimals;
    let stats = pit.stats();
    // Each rounding moves a pit's value by at most half a unit at `d` places.
    let bound = i128::try_from(model.rounded)
        .ok()
        .and_then(|r| r.checked_mul(5))
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "plan pit: the rounding bound overflows",
            )
        })?;
    let mut payload = format!(
        "{{\"schema\":\"fmine.plan-pit.v1\",\"model\":{{\"format\":\"minelib_upit\",\"name\":\"{}\",\"upit\":{},\"prec\":{}}},\"blocks\":{},\"arcs\":{},\"decimals\":{d},\"rounding\":\"{}\",\"rounded_values\":{},\"value_bound\":\"{}\",\"value\":\"{}\",\"pit_blocks\":{},\"certificate\":{{\"verified\":true,\"method\":\"pseudoflow final arc flows; pit closed, no flow enters it, excess non-negative inside and non-positive outside\"}},\"solver\":{{\"mergers\":{},\"splits\":{},\"relabels\":{},\"max_label\":{}}}",
        escape_json(&model.name),
        source_json(upit_source, upit.len()),
        source_json(prec_source, prec.len()),
        model.problem.node_count(),
        model.problem.arc_count(),
        if options.round {
            "half_away_from_zero"
        } else {
            "refuse"
        },
        model.rounded,
        decimal(bound, d + 1),
        decimal(i128::from(pit.value()), d),
        pit.pit_size(),
        stats.mergers,
        stats.splits,
        stats.relabels,
        stats.max_label,
    );
    if options.list_blocks {
        let ids: Vec<String> = pit
            .in_pit()
            .iter()
            .enumerate()
            .filter(|(_, inside)| **inside)
            .map(|(b, _)| b.to_string())
            .collect();
        payload.push_str(&format!(",\"in_pit\":[{}]", ids.join(",")));
    }
    let mut candidate_line = String::new();
    if let Some(input) = candidate {
        bind(input.text, input.source)?;
        let in_set = candidate::parse_ids(input.text, model.problem.node_count())?;
        let c = candidate::check(&model.problem, &in_set)?;
        let gap = i128::from(pit.value()) - i128::from(c.value);
        if c.closed() && gap < 0 {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "plan pit: a closed candidate exceeds the certified optimum",
            ));
        }
        let verdict = match (c.closed(), gap) {
            (false, _) => "not_closed",
            (true, 0) => "optimal",
            (true, _) => "suboptimal",
        };
        let gap_text = if c.closed() {
            format!("\"{}\"", decimal(gap, d))
        } else {
            "null".to_string()
        };
        let witnesses: Vec<String> = c
            .first_unmet
            .iter()
            .map(|(b, p)| format!("[{b},{p}]"))
            .collect();
        payload.push_str(&format!(
            ",\"candidate\":{{\"source\":{},\"blocks\":{},\"value\":\"{}\",\"unmet_requirements\":{},\"first_unmet\":[{}],\"gap_to_optimum\":{gap_text},\"verdict\":\"{verdict}\"}}",
            source_json(input.source, input.text.len()), c.blocks, decimal(i128::from(c.value), d),
            c.unmet, witnesses.join(","),
        ));
        candidate_line = format!(
            "\n**Candidate** `{}`: {verdict}. {} blocks, value {}, {} unmet requirements, gap to the certified optimum {}.\n",
            input.display_path,
            c.blocks,
            decimal(i128::from(c.value), d),
            c.unmet,
            gap_text.trim_matches('"'),
        );
    }
    payload.push('}');
    let mut warnings = Vec::new();
    if model.rounded > 0 {
        warnings.push(format!(
            "{} values were rounded to {d} places; the value is exact for the rounded problem and within the stated bound of the file's optimum.",
            model.rounded
        ));
    }
    let markdown = format!(
        "# Ultimate pit: {}\n\n| blocks | arcs | pit blocks | value | rounded values | certificate |\n|---:|---:|---:|---:|---:|---|\n| {} | {} | {} | {} | {} | verified |\n{candidate_line}\n{NO_CLAIM}\n",
        model.name,
        model.problem.node_count(),
        model.problem.arc_count(),
        pit.pit_size(),
        decimal(i128::from(pit.value()), d),
        model.rounded
    );
    Ok(Report {
        json: payload,
        markdown,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_and_candidate_sources_cannot_reuse_other_byte_identities() {
        let upit = "NAME: synthetic\nTYPE: UPIT\nNBLOCKS: 1\nOBJECTIVE_FUNCTION:\n0 2.5\nEOF\n";
        let prec = "0 0\n";
        let text = "0\n";
        let source = |path, bytes: &str| {
            SourceAnchor::new(path, sha256(bytes.as_bytes()).0, "whole source")
        };
        let upit_source = source("pit.upit", upit);
        let prec_source = source("pit.prec", prec);
        let candidate_source = source("candidate.sol", text);
        let wrong = source("other.txt", "other bytes");
        let options = Options {
            decimals: 1,
            round: false,
            list_blocks: true,
        };
        for changed in 0..3 {
            let result = evaluate(
                upit,
                if changed == 0 { &wrong } else { &upit_source },
                prec,
                if changed == 1 { &wrong } else { &prec_source },
                &options,
                Some(Candidate {
                    text,
                    source: if changed == 2 {
                        &wrong
                    } else {
                        &candidate_source
                    },
                    display_path: "candidate.sol",
                }),
            );
            assert_eq!(result.err().unwrap().code, "FMINE-REF-505");
        }
        let report = evaluate(
            upit,
            &upit_source,
            prec,
            &prec_source,
            &options,
            Some(Candidate {
                text,
                source: &candidate_source,
                display_path: "candidate.sol",
            }),
        )
        .unwrap();
        assert!(report.json.contains("\"value\":\"2.5\""));
        let optimum = "\"gap_to_optimum\":\"0.0\",\"verdict\":\"optimal\"";
        assert!(report.json.contains(optimum));
    }
}
