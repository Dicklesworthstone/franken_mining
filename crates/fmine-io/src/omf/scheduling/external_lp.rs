//! DESIGN (plan §15.4, bd-06kg): source-bound external LP exchange, with advisory primal shares.
//! Only the stored binary64 relaxation receives the imported dual certificate. Integer schedule
//! feasibility, objective and Lagrangian gap remain independently checked by the scheduling adapter.
//! Determinism: DeterministicPerIsa

use std::path::{Path, PathBuf};

use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_lp::{mps, pdlp::certified_bound};
use fmine_optimize::toposort::{check, search};

use super::{Report, checked, escape_json, invalid, numerical, ratio, source_json};
use crate::omf::pit::{PitModel, ScheduleSpec};

/// Bound each imported text and the canonical MPS export before it is retained in a report.
pub const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_COLUMNS: u128 = 1_000_000;
const MAX_ROWS: u128 = 2_000_000;
const MAX_ENTRIES: u128 = 8_000_000;
pub const NO_CLAIM: &str = "An exported binary64 LP and, when supplied, a locally recomputed bound for that exact stored relaxation. Source attributes and discount factors may round when converted to binary64. The raw bound is in scaled value units, not original attribute units or the integer schedule objective. It is never used to tighten the integer schedule certificate or gap. Imported primal shares are advisory; only independently checked schedules may become incumbents. No solver execution, empirical calibration, approved slope, reserve, operational mine plan or execution authorization is supplied.";

/// Named value files use the complete `C0...` or `R0...` population emitted by `mps::write`.
/// The pinned MPS includes comments binding both complete original source-byte digests.
#[derive(Debug, Clone, Copy)]
pub enum Request<'a> {
    Export,
    Import {
        mps_sha256: &'a str,
        primal: Option<(&'a str, &'a SourceAnchor)>,
        dual: Option<(&'a str, &'a SourceAnchor)>,
    },
}

/// Retained, bounded file snapshots for the optional primal and dual inputs.
pub struct ValueFiles {
    values: [Option<(String, SourceAnchor)>; 2],
}

impl ValueFiles {
    /// Read each supplied file once through the common bounded source reader.
    ///
    /// # Errors
    /// Refuses unreadable, non-regular, non-UTF-8 or oversized files.
    pub fn read(root: &Path, paths: &[Option<PathBuf>; 2]) -> Result<Self, Refusal> {
        let [primal, dual] = paths.each_ref().map(|path| {
            path.as_ref()
                .map(|path| {
                    crate::source::read(&root.join(path), "external LP values", MAX_TEXT_BYTES)
                })
                .transpose()
        });
        Ok(Self {
            values: [primal?, dual?],
        })
    }

    /// Borrow the retained bytes and their original anchors; evaluation rechecks both digests.
    pub fn request<'a>(&'a self, mps_sha256: &'a str) -> Request<'a> {
        let [primal, dual] = self
            .values
            .each_ref()
            .map(|value| value.as_ref().map(|(text, source)| (text.as_str(), source)));
        Request::Import {
            mps_sha256,
            primal,
            dual,
        }
    }
}

pub(super) enum Outcome {
    Export(Report),
    Import(Imported),
}

pub(super) struct Imported {
    pub(super) guidance: Option<(Vec<Option<u32>>, i128)>,
    json: String,
    markdown: String,
}

/// LP rows and coefficients grow differently from the time-expanded closure. Check them before
/// `schedule_lp` allocates COO/CSR matrices. The text estimate covers every coefficient, row and
/// bound line at 128 bytes each, with three possible lines per row/column and fixed header slack.
fn dimensions(model: &PitModel<'_>, spec: &ScheduleSpec<'_>) -> Result<(), Refusal> {
    let n = model.grid().block_count() as u128;
    let t = spec.periods as u128;
    let a = model.arc_count() as u128;
    let rows = a * t + n * (t - 1) + spec.capacities.len() as u128 * t;
    let columns = n * t;
    if columns > MAX_COLUMNS || rows > MAX_ROWS {
        return Err(numerical(
            "external LP exceeds 1000000 columns or 2000000 rows",
        ));
    }
    let mut nonzero_uses = 0_u128;
    for capacity in spec.capacities {
        nonzero_uses += model
            .uses(capacity.attribute, capacity.per_unit)?
            .iter()
            .filter(|&&value| value != 0)
            .count() as u128;
    }
    let entries = 2 * a * t + 2 * n * (t - 1) + (2 * t - 1) * nonzero_uses;
    let text_bound = 128 * (entries + 3 * rows + 3 * columns + 16);
    if entries > MAX_ENTRIES || text_bound > u128::from(MAX_TEXT_BYTES) {
        return Err(numerical(
            "external LP exceeds 8000000 sparse entries or the conservative 64 MiB MPS text budget",
        ));
    }
    Ok(())
}

fn source(text: &str, anchor: &SourceAnchor) -> Result<String, Refusal> {
    if text.len() as u64 > MAX_TEXT_BYTES || anchor.artifact_path.trim().is_empty() {
        return Err(invalid(
            "external LP values require a source path and at most 64 MiB",
        ));
    }
    let actual = sha256(text.as_bytes());
    if actual.0 != anchor.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "external LP values {} do not bind the supplied bytes; actual sha256={actual}",
                anchor.artifact_path
            ),
        ));
    }
    Ok(source_json(anchor, text.len()))
}

pub(super) fn evaluate(
    model: &PitModel<'_>,
    spec: &ScheduleSpec<'_>,
    sources: [(&SourceAnchor, usize); 2],
    request: Request<'_>,
) -> Result<Outcome, Refusal> {
    let (primal_source, dual_source) = match request {
        Request::Export => (None, None),
        Request::Import {
            mps_sha256,
            primal,
            dual,
        } => {
            if (primal.is_none() && dual.is_none())
                || mps_sha256.len() != 64
                || !mps_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid(
                    "external LP import requires a lowercase MPS SHA-256 and primal and/or dual values",
                ));
            }
            (
                primal
                    .map(|(text, anchor)| source(text, anchor))
                    .transpose()?,
                dual.map(|(text, anchor)| source(text, anchor))
                    .transpose()?,
            )
        }
    };
    dimensions(model, spec)?;
    let problem = model.schedule_lp(spec)?;
    let mps = format!(
        "* fmine.omf-schedule-lp.v1\n* omf_sha256={}\n* spec_sha256={}\n{}",
        fmine_core::Digest32(sources[0].0.artifact_sha256),
        fmine_core::Digest32(sources[1].0.artifact_sha256),
        mps::write("fmine_schedule", &problem)?,
    );
    if mps.len() as u64 > MAX_TEXT_BYTES {
        return Err(numerical("canonical external LP exceeds 64 MiB"));
    }
    let digest = sha256(mps.as_bytes());
    let common = format!(
        "\"schema\":\"fmine.omf-schedule-lp.v1\",\"mps_sha256\":\"{digest}\",\"sources\":{{\"omf\":{},\"spec\":{}}},\"columns\":{},\"rows\":{},\"nonzeros\":{},\"column_order\":{{\"formula\":\"C(t*blocks+b): share mined by period t\",\"block_order\":\"pitflow_top_down_reversed_omf_layers\",\"blocks\":{},\"periods\":{}}},\"objective\":{{\"sense\":\"minimize_negated_discounted_scaled_value\",\"coefficient_model\":\"stored_binary64\",\"discount\":\"(1+rate/scale)^(-t) in binary64\",\"value_per_unit\":{},\"bound_unit\":\"scaled_value_units\",\"used_for_integer_schedule_gap\":false}},\"no_claim\":\"{}\"",
        source_json(sources[0].0, sources[0].1),
        source_json(sources[1].0, sources[1].1),
        problem.cols(),
        problem.rows(),
        problem.nnz(),
        model.grid().block_count(),
        spec.periods,
        spec.per_unit,
        escape_json(NO_CLAIM),
    );
    let Request::Import {
        mps_sha256,
        primal,
        dual,
    } = request
    else {
        return Ok(Outcome::Export(Report {
            json: format!("{{{common},\"mps\":\"{}\"}}", escape_json(&mps)),
            markdown: format!(
                "# OMF schedule LP export\n\nMPS SHA-256: `{digest}`. {} columns, {} rows, {} nonzeros.\n\nThe raw LP objective uses {} scaled value units per attribute unit. The comments bind the exact OMF and declaration bytes.\n\n{NO_CLAIM}\n\n```mps\n{mps}```\n",
                problem.cols(),
                problem.rows(),
                problem.nnz(),
                spec.per_unit,
            ),
        }));
    };
    if mps_sha256 != digest.to_hex() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "external LP MPS digest differs: supplied {mps_sha256}, reconstructed {digest}"
            ),
        ));
    }
    let denominator = i128::from(spec.per_unit) * i128::from(search::DISCOUNT_SCALE);
    let (guidance, primal_json) = if let Some((text, _)) = primal {
        let values = mps::read_values(text, mps::Names::Columns, &problem)?;
        let guided = model.guided_schedule(spec, values)?;
        let schedule = guided.incumbent().schedule();
        let assessment = check(guided.problem(), schedule)?;
        let value = search::discounted_value(guided.problem(), schedule, guided.factors())?;
        if !assessment.feasible() {
            return Err(numerical(
                "external LP guidance failed independent schedule feasibility",
            ));
        }
        let json = format!(
            "{{\"source\":{},\"admission\":\"advisory_shares_only\",\"lp_feasibility_claimed\":false,\"searched_starts\":{},\"checked\":{},\"integer_discounted_value\":{}}}",
            primal_source.as_deref().unwrap_or("null"),
            guided.incumbent().searched.len(),
            checked(&assessment, model)?,
            ratio(value, denominator),
        );
        (Some((schedule.to_vec(), value)), json)
    } else {
        (None, "null".into())
    };
    let (dual_json, bound_note) = if let Some((text, _)) = dual {
        let values = mps::read_values(text, mps::Names::Rows, &problem)?;
        let bound = certified_bound(&problem, &values)?;
        let value = bound.map_or_else(|| "null".into(), |b| (-b.value()).to_string());
        let status = if bound.is_some() {
            "certified"
        } else {
            "no_finite_bound"
        };
        let json = format!(
            "{{\"source\":{},\"status\":\"{status}\",\"upper_bound_scaled\":{value},\"repaired_rows\":{},\"implied_columns\":{}}}",
            dual_source.as_deref().unwrap_or("null"),
            bound.map_or_else(|| "null".into(), |b| b.repaired_rows().to_string()),
            bound.map_or_else(|| "null".into(), |b| b.implied_columns().to_string()),
        );
        (
            json,
            format!("Imported dual: `{status}`; raw LP upper bound {value} scaled value units."),
        )
    } else {
        (
            "null".into(),
            "No row multipliers supplied; no external LP bound is claimed.".into(),
        )
    };
    Ok(Outcome::Import(Imported {
        guidance,
        json: format!("{{{common},\"primal\":{primal_json},\"dual\":{dual_json}}}"),
        markdown: format!(
            "\n## External LP\n\nPinned MPS SHA-256: `{digest}`. {bound_note}\n\n{NO_CLAIM}\n"
        ),
    }))
}

impl Imported {
    pub(super) fn append(self, report: &mut Report, selected: bool) {
        let _ = report.json.pop();
        report.json.push_str(&format!(
            ",\"external_lp\":{},\"external_lp_selected\":{selected}}}",
            self.json
        ));
        report.markdown.push_str(&self.markdown);
        report.markdown.push_str(if selected {
            "\nImported shares improved the exact incumbent; its feasibility and integer Lagrangian certificate are checked above.\n"
        } else {
            "\nThe existing exact incumbent is retained. Imported guidance can replace it only on a strict improvement.\n"
        });
    }
}
