//! DESIGN (BD-341.S5): bind the paired price sensitivity to verified public inputs.
//!
//! Callers supply the existing byte-checked Mine Stats projection and transcription report. Quantity keys
//! are `aisc_price_assumption_<site>_<period>_<unit>` and `realized_price_<price_subject>_<period>_<unit>`;
//! subjects use lowercase underscore-separated words, periods use lowercase YYYYFY/YYYYQn, and units are
//! `usd_per_oz` or `usd_per_lb`. The Accepted period must independently match. An assumption belongs to the
//! exact guidance document. An explicitly issuer-wide assumption uses `all_sites` in place of the site,
//! retaining its one physical locator; multiple applicable assumptions are ambiguous. A company realized
//! price requires an explicit price subject; it is never substituted.
//! Commodity-price bases remain as transcribed, separate from the source's cost ownership basis.
//!
//! The gate population is 2024Q1–2026Q2, intersected with an explicit --through bound for retrospective
//! reports or with completed quarters at an explicit evidence cut. Every member needs an original-release
//! royalty and an unambiguous agreed price. Retrospective --through bounds do not restrict PDF publication
//! dates; evidence-cut callers use only documents available by their cut. Missing quarters stay required.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_calib::guidance_scores::{GuidanceScore, PrintedOutcome, price};
use fmine_claim::{AdditivityClass, Representation, SpatialSupport};
use fmine_core::hash::Digest32;
use fmine_evidence::{EvidenceClass, KnowledgeState};
use fmine_numerics::exact_feas::{Convention, Rat};
use fmine_public::key::{KeyedCell, OwnershipBasis, PublicValue, Scope, canonical_value_tag};
use fmine_public::sources::{IssuerProfile, SourceKind};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport};
use fmine_registry::sources::{is_date, is_quarter};
use fmine_registry::transcriptions::{Accepted, Decimal, Token, parse_token};

use crate::track_record_data::{OutcomeBinding, TrackRecordRow, cell_basis, limits, specification};

const POPULATION: [&str; 10] = [
    "2024Q1", "2024Q2", "2024Q3", "2024Q4", "2025Q1", "2025Q2", "2025Q3", "2025Q4", "2026Q1",
    "2026Q2",
];
const NOTICE: &str = "Hypothetical AISC price sensitivity beside the unconditional score. Exact proportionality is a compatibility test, not a causal or calibration claim. No inference that the remainder is controllable.";

#[derive(Debug, Clone)]
struct TranscriptEvidence {
    role: String,
    entry: Accepted,
    documents: Vec<CheckedDocument>,
}

#[derive(Debug, Clone)]
pub(crate) struct PriceAssessment {
    pub reason: Option<String>,
    pub missing: Vec<String>,
    /// Required population, including missing members.
    pub quarters: Vec<String>,
    pub report: Option<price::ConditionalReport>,
    scope: String,
    price_subject: String,
    explicit_proxy: bool,
    unit: String,
    cells: Vec<(String, KeyedCell)>,
    transcripts: Vec<TranscriptEvidence>,
}

fn js(value: &str) -> String {
    format!("\"{}\"", crate::escape_json(value))
}
fn strings(values: &[String]) -> String {
    values.iter().map(|v| js(v)).collect::<Vec<_>>().join(",")
}
fn slug(value: &str) -> String {
    crate::track_record_data::site_key(value).replace('-', "_")
}
fn digest(value: &str) -> Result<Digest32, String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid source SHA-256".into());
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[2 * i..2 * i + 2], 16)
            .map_err(|_| "invalid source SHA-256")?;
    }
    Ok(Digest32(bytes))
}
fn exact(d: Decimal) -> Result<Rat, String> {
    let denominator = 10_i128
        .checked_pow(d.scale)
        .ok_or("decimal scale overflow")?;
    Rat::new(d.mantissa, denominator).map_err(|e| format!("{e:?}"))
}
fn printed(token: &str) -> Result<PrintedOutcome, String> {
    let Token::Point(d) = parse_token(token).map_err(|e| e.to_string())? else {
        return Err("a royalty or price must print one value".into());
    };
    let value = exact(d)?;
    let half = exact(Decimal {
        mantissa: 1,
        scale: d.scale,
    })?
    .checked_div(Rat::from_integer(2))
    .map_err(|e| format!("{e:?}"))?;
    Ok(PrintedOutcome {
        value,
        interval: Convention::Closed
            .interval(value, half)
            .map_err(|e| format!("{e:?}"))?,
    })
}
fn ownership(value: OwnershipBasis) -> Result<String, String> {
    match value {
        OwnershipBasis::Consolidated100 => Ok("100%".into()),
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && den > 0 && num <= den => {
            let share = Rat::new(i128::from(num), i128::from(den)).map_err(|e| format!("{e:?}"))?;
            Ok(format!("attributable:{}/{}", share.num(), share.den()))
        }
        _ => Err("royalty/outcome ownership is unprinted or invalid".into()),
    }
}

fn checked_document<'a>(
    a: &Accepted,
    t: &'a TranscriptionReport,
) -> Result<&'a CheckedDocument, String> {
    let docs: Vec<_> = t
        .documents
        .iter()
        .filter(|d| d.file == a.locator.doc)
        .collect();
    let [doc] = docs.as_slice() else {
        return Err("expected one byte-checked document".into());
    };
    if doc.sha256 != a.doc_sha256
        || !is_date(&doc.published_at)
        || doc.published_at_basis.trim().is_empty()
        || a.readers.iter().collect::<BTreeSet<_>>().len() < 2
        || parse_token(&a.token).map_err(|e| e.to_string())? != a.value
    {
        return Err(
            "document hash, publication evidence, reader agreement or parsed token mismatch".into(),
        );
    }
    digest(&a.doc_sha256)?;
    Ok(*doc)
}

impl PriceAssessment {
    fn note(
        &mut self,
        role: &str,
        result: Result<price::CitedValue, String>,
    ) -> Option<price::CitedValue> {
        match result {
            Ok(value) => Some(value),
            Err(reason) => {
                self.missing.push(format!("{role}: {reason}"));
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transcription(
        &mut self,
        t: &TranscriptionReport,
        role: &str,
        quantities: &[String],
        period: &str,
        doc: Option<&str>,
        subject: &str,
        unit: price::RateUnit,
    ) -> Option<price::CitedValue> {
        let mut candidates: Vec<_> = t
            .agreement
            .accepted
            .iter()
            .filter(|a| quantities.contains(&a.quantity) && doc.is_none_or(|d| a.locator.doc == d))
            .collect();
        candidates.sort_by(|a, b| a.locator.cmp(&b.locator));
        for a in &candidates {
            let mut documents: Vec<_> = t
                .documents
                .iter()
                .filter(|d| d.file == a.locator.doc)
                .cloned()
                .collect();
            documents.sort_by(|a, b| {
                (&a.file, &a.sha256, &a.published_at).cmp(&(&b.file, &b.sha256, &b.published_at))
            });
            self.transcripts.push(TranscriptEvidence {
                role: role.into(),
                entry: (*a).clone(),
                documents,
            });
        }
        let result = (|| {
            let [a] = candidates.as_slice() else {
                return Err(format!(
                    "expected one agreed {}, found {}{}",
                    quantities.join(" or "),
                    candidates.len(),
                    doc.map_or(String::new(), |d| format!(" in guidance document {d}"))
                ));
            };
            let checked = checked_document(a, t)?;
            if a.period != period || a.unit != self.unit || a.basis.trim().is_empty() {
                return Err("period, unit or explicit commodity-price basis mismatch".into());
            }
            if doc.is_none() {
                let p = fmine_public::key::parse_period_code(period)
                    .ok_or("unrecognized realized-price period")?;
                let (_, (_, month)) = p.months();
                let end = format!(
                    "{:04}-{month:02}-{}",
                    p.year,
                    if month == 3 || month == 12 { 31 } else { 30 }
                );
                if checked.published_at < end {
                    return Err(format!(
                        "realized-price document dated {} precedes period end {end}",
                        checked.published_at
                    ));
                }
            }
            Ok(price::CitedValue {
                printed: printed(&a.token)?,
                unit,
                subject: subject.into(),
                period: a.period.clone(),
                basis: a.basis.clone(),
                citation: price::Citation {
                    locator: a.locator.to_string(),
                    sha256: digest(&a.doc_sha256)?,
                },
            })
        })();
        self.note(role, result)
    }

    #[allow(clippy::too_many_arguments)]
    fn royalty(
        &mut self,
        p: &IssuerProfile,
        cells: &[KeyedCell],
        subject: &str,
        period: &str,
        release: &str,
        basis: &str,
        unit: price::RateUnit,
    ) -> Option<price::CitedValue> {
        let role = format!("royalty {period} source release {release}");
        let section = match unit {
            price::RateUnit::UsdPerTroyOunce => "ounce",
            price::RateUnit::UsdPerPound => "pound",
        };
        let symbol = match unit {
            price::RateUnit::UsdPerTroyOunce => "oz",
            price::RateUnit::UsdPerPound => "lb",
        };
        let labels = [
            format!("site:costs per {section}:royalties"),
            format!("site:costs per {section} ($/{symbol}):royalties"),
        ];
        let mut candidates: Vec<_> = cells
            .iter()
            .filter(|c| {
                c.key.issuer == p.issuer
                    && c.key.subject == subject
                    && c.key.scope == Scope::Site
                    && c.key.valid_period.code() == period
                    && c.key.reporting_vintage.release == release
                    && labels.contains(&c.key.line.label)
            })
            .collect();
        candidates.sort_by_key(|c| (&c.locator, c.claim.claim_id));
        self.cells
            .extend(candidates.iter().map(|c| (role.clone(), (*c).clone())));
        let result = (|| {
            if !p
                .sources
                .iter()
                .any(|s| s.kind == SourceKind::MineStats && s.quarter == release)
            {
                return Err("required original Mine Stats release is not registered".into());
            }
            let [cell] = candidates.as_slice() else {
                return Err(format!(
                    "expected one royalty cell, found {}; no later comparative substituted",
                    candidates.len()
                ));
            };
            checked_cell(p, cell, &self.unit, basis, unit)
        })();
        self.note(&role, result)
    }

    pub(crate) fn to_json(&self) -> String {
        let cells = self
            .cells
            .iter()
            .map(|(role, c)| format!("{{\"role\":{},\"cell\":{}}}", js(role), c.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        let transcripts = self.transcripts.iter().map(|e| {
            let a = &e.entry;
            let documents = e.documents.iter().map(|d| format!(
                "{{\"file\":{},\"kind\":{},\"published_at\":{},\"published_at_basis\":{},\"sha256\":{}}}",
                js(&d.file),js(&d.kind),js(&d.published_at),js(&d.published_at_basis),js(&d.sha256))).collect::<Vec<_>>().join(",");
            format!("{{\"role\":{},\"locator\":{},\"sha256\":{},\"token\":{},\"quantity\":{},\"period\":{},\"unit\":{},\"basis\":{},\"readers\":[{}],\"documents\":[{}]}}",
                js(&e.role),js(&a.locator.to_string()),js(&a.doc_sha256),js(&a.token),js(&a.quantity),js(&a.period),js(&a.unit),js(&a.basis),strings(&a.readers),documents)
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"rules\":\"source-bound-aisc-price/1\",\"notice\":{},\"scope\":{},\"required_quarters\":[{}],\"price_subject\":{},\"explicit_proxy\":{},\"unit\":{},\"unsplit_reason\":{},\"missing\":[{}],\"paired_report\":{},\"evidence\":{{\"cells\":[{}],\"transcriptions\":[{}]}}}}",
            js(NOTICE),
            js(&self.scope),
            strings(&self.quarters),
            js(&self.price_subject),
            self.explicit_proxy,
            js(&self.unit),
            self.reason.as_deref().map_or("null".into(), js),
            strings(&self.missing),
            self.report
                .as_ref()
                .map_or("null".into(), price::ConditionalReport::to_json),
            cells,
            transcripts
        )
    }

    pub(crate) fn to_markdown(&self) -> String {
        use crate::guidance::{interval, number, text};
        let mut out = format!(
            "{}\n\nPopulation: {}. Price subject: {}{}; units: {}.\n\n",
            NOTICE,
            text(&self.scope),
            text(&self.price_subject),
            if self.explicit_proxy {
                " (explicit proxy)"
            } else {
                " (same site)"
            },
            text(&self.unit)
        );
        if let Some(reason) = &self.reason {
            let _ = writeln!(out, "Unsplit: {}.\n", text(reason));
        }
        if let Some(report) = &self.report {
            match report.gate() {
                price::Gate::Supported { beta, quarters } => {
                    let _ = writeln!(
                        out,
                        "Exact proportionality compatible: β {} over {}.\n",
                        interval(*beta),
                        text(&quarters.join(", "))
                    );
                }
                price::Gate::NotSupported { quarters, bounds } => {
                    let _ = writeln!(
                        out,
                        "Proportionality refuted: {} admits β {}; {} admits β {}.\n",
                        text(&quarters[0]),
                        interval(bounds[0]),
                        text(&quarters[1]),
                        interval(bounds[1])
                    );
                }
                price::Gate::InsufficientHistory => {
                    out.push_str("Insufficient quarterly history.\n\n");
                }
            }
            if let Some(current) = report.current_period() {
                let _ = writeln!(
                    out,
                    "Annual royalty/price β: {}; compatible with historical gate: {}.\n",
                    interval(current.beta),
                    current.compatible
                );
            }
            if let Some(a) = report.adjustment() {
                let z = |v: Option<Rat>| v.map_or("undefined".into(), number);
                let _ = writeln!(
                    out,
                    "| Signed price contribution ({}) | Rounding enclosure | Unconditional z | Conditional z |\n|---|---|---|---|\n| {} | {} | {} | {} |\n",
                    text(&self.unit),
                    number(a.contribution.value),
                    interval(a.contribution.interval),
                    z(report.unconditional().z),
                    z(a.conditional.z)
                );
            }
        }
        for item in &self.missing {
            let _ = writeln!(out, "- {}", text(item));
        }
        if !self.missing.is_empty() {
            out.push('\n');
        }
        if !self.cells.is_empty() || !self.transcripts.is_empty() {
            out.push_str("| Input | Printed value | Source | SHA-256 |\n|---|---|---|---|\n");
            for (role, c) in &self.cells {
                let token = match &c.claim.value {
                    PublicValue::Number {
                        displayed: Ok(v), ..
                    } => v.as_str(),
                    _ => fmine_public::key::raw_token(&c.claim.value),
                };
                let _ = writeln!(
                    out,
                    "| {} | {} | {} (Claim `{}`) | `{}` |",
                    text(role),
                    text(token),
                    text(&c.locator),
                    c.claim.claim_id,
                    c.key.reporting_vintage.file_sha256
                );
            }
            for e in &self.transcripts {
                let _ = writeln!(
                    out,
                    "| {} ({}) | {} {} [{}] | {} | `{}` |",
                    text(&e.role),
                    text(&e.entry.quantity),
                    text(&e.entry.token),
                    text(&e.entry.unit),
                    text(&e.entry.basis),
                    text(&e.entry.locator.to_string()),
                    e.entry.doc_sha256
                );
            }
            out.push('\n');
        }
        out
    }
}

fn checked_cell(
    p: &IssuerProfile,
    c: &KeyedCell,
    unit_name: &str,
    basis: &str,
    unit: price::RateUnit,
) -> Result<price::CitedValue, String> {
    c.claim.check_id().map_err(|e| e.to_string())?;
    if c.key.issuer != p.issuer
        || c.key.scope != Scope::Site
        || c.key.unit != unit_name
        || ownership(c.key.ownership_basis)? != basis
        || !c.claim.confidentiality.is_public()
        || c.claim.stale
        || c.claim.calibration_failed
        || c.claim.support != SpatialSupport::PeriodAggregate
        || c.claim.additivity != AdditivityClass::IntensiveWeighted
        || !matches!(c.claim.representation, Representation::Deterministic)
    {
        return Err("source unit, ownership, support, status or public evidence mismatch".into());
    }
    let synthetic = p.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER
        && c.claim.evidence == EvidenceClass::Assumed
        && c.claim.state == KnowledgeState::Unknown;
    if !matches!(
        c.claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) && !synthetic
    {
        return Err("source is not an observable numeric claim".into());
    }
    let PublicValue::Number {
        displayed: Ok(token),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return Err(
            "source is missing, unreadable, nonnumeric or restricted by subject status".into(),
        );
    };
    // Match the canonical public Number interpretation, not only Claim::check_id (which hashes its tag).
    if c.claim.metric_key != c.key.line.label
        || c.claim.value_tag() != canonical_value_tag(&c.key, &c.claim.value)
    {
        return Err("canonical public value/identity mismatch".into());
    }
    let source_bound = p.sources.iter().any(|s| {
        s.kind == SourceKind::MineStats
            && s.quarter == c.key.reporting_vintage.release
            && s.sha256_hex == c.key.reporting_vintage.file_sha256
            && c.claim.anchors.iter().any(|a| {
                Digest32(a.artifact_sha256).to_hex() == s.sha256_hex
                    && std::path::Path::new(&a.artifact_path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        == Some(s.file_name.as_str())
                    && c.locator == format!("{}:{}", s.file_name, a.locator)
            })
    });
    if !source_bound {
        return Err(
            "cell lacks an anchor to its registered release, file, locator and SHA-256".into(),
        );
    }
    Ok(price::CitedValue {
        printed: printed(token)?,
        unit,
        subject: c.key.subject.clone(),
        period: c.key.valid_period.code(),
        basis: basis.into(),
        citation: price::Citation {
            locator: c.locator.clone(),
            sha256: digest(&c.key.reporting_vintage.file_sha256)?,
        },
    })
}

/// Never replaces or selects an outcome. Missing price evidence only leaves this sensitivity unsplit.
/// Preserve rejected identities without presenting disputed numeric readings as accepted inputs.
fn rejected_identity(
    transcripts: &TranscriptionReport,
    locator: &fmine_registry::transcriptions::Locator,
) -> String {
    let quantities = transcripts
        .agreement
        .rejected_quantities
        .get(locator)
        .map(|values| values.iter().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_else(|| "unknown quantity identity".into());
    let documents = transcripts
        .documents
        .iter()
        .filter(|document| document.file == locator.doc)
        .map(|document| format!("sha256 {}", document.sha256))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{locator} (quantities: {quantities}; {documents})")
}

struct PriceRow<'a> {
    site: &'a str,
    metric: &'a str,
    year: u16,
    guidance: &'a Accepted,
    published_at: &'a str,
    outcome: Option<&'a KeyedCell>,
    score: Option<&'a GuidanceScore>,
}

pub(crate) fn assess(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    row: &TrackRecordRow,
    through: Option<&str>,
    price_subject: Option<&str>,
) -> PriceAssessment {
    let selected = match &row.outcome {
        OutcomeBinding::Resolved { cell, .. } => Some(cell.as_ref()),
        OutcomeBinding::Unresolved { .. } => None,
    };
    assess_row(
        profile,
        transcripts,
        cells,
        PriceRow {
            site: &row.site,
            metric: &row.metric,
            year: row.year,
            guidance: &row.guidance,
            published_at: &row.published_at,
            outcome: selected,
            score: row.score.as_ref(),
        },
        through,
        price_subject,
    )
}

fn assess_row(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    row: PriceRow<'_>,
    through: Option<&str>,
    price_subject: Option<&str>,
) -> PriceAssessment {
    let subject = price_subject.unwrap_or(row.site);
    let mut out = PriceAssessment {
        reason: None,
        missing: vec![],
        quarters: POPULATION
            .iter()
            .filter(|q| through.is_none_or(|t| **q <= t))
            .map(|q| (*q).into())
            .collect(),
        report: None,
        scope: through.map_or(
            "2024Q1–2026Q2 (complete planned population)".into(),
            |t| format!("2024Q1–2026Q2 intersected with explicitly requested --through {t}"),
        ),
        price_subject: subject.into(),
        explicit_proxy: subject != row.site,
        unit: row.guidance.unit.clone(),
        cells: vec![],
        transcripts: vec![],
    };
    let (metric, unit) = match row.metric {
        "aisc_usd_per_oz" => (price::Metric::GoldAisc, price::RateUnit::UsdPerTroyOunce),
        "aisc_usd_per_lb" => (price::Metric::CopperAisc, price::RateUnit::UsdPerPound),
        _ => {
            out.reason = Some("unsupported_metric".into());
            return out;
        }
    };
    if through.is_some_and(|t| !is_quarter(t)) || slug(subject).is_empty() {
        out.reason = Some("invalid_source_context".into());
        return out;
    }
    if !transcripts.agreement.is_consistent() || !transcripts.agreement.single_reader.is_empty() {
        out.reason = Some("incomplete_transcription_agreement".into());
        for (locator, reader) in &transcripts.agreement.single_reader {
            out.missing.push(format!(
                "{}: awaiting an independent second reader; present reader {reader}",
                rejected_identity(transcripts, locator),
            ));
        }
        for disagreement in &transcripts.agreement.disagreements {
            out.missing.push(format!(
                "{}: readers {} disagree on {}",
                rejected_identity(transcripts, &disagreement.locator),
                disagreement.readers.join(", "),
                disagreement.fields.join(", "),
            ));
        }
        for (_, quantity, locators) in &transcripts.agreement.quantity_clashes {
            for locator in locators {
                out.missing.push(format!(
                    "{}: quantity {quantity} is claimed at multiple locators",
                    rejected_identity(transcripts, locator),
                ));
            }
        }
        return out;
    }
    let Some(cell) = row.outcome else {
        out.reason = Some("outcome_unresolved".into());
        return out;
    };
    out.cells
        .push(("selected unconditional outcome".into(), cell.clone()));
    let period = format!("{}FY", row.year);
    let guidance_check = (|| {
        let doc = checked_document(row.guidance, transcripts)?;
        let (guidance_basis, expected_line) =
            specification(row.guidance, row.metric).map_err(|e| e.to_string())?;
        if !transcripts.agreement.accepted.contains(row.guidance)
            || row.guidance.period != period
            || cell.key.valid_period.code() != period
            || cell.key.subject != row.site
            || cell.key.line.label != expected_line
            || doc.published_at != row.published_at
        {
            return Err("guidance/outcome binding changed since unconditional assessment".into());
        }
        let basis = ownership(cell.key.ownership_basis)?;
        if guidance_basis != cell_basis(cell.key.ownership_basis).map_err(|e| e.to_string())? {
            return Err("guidance/outcome ownership or AISC definition mismatch".into());
        }
        out.transcripts.push(TranscriptEvidence {
            role: "scored guidance".into(),
            entry: (*row.guidance).clone(),
            documents: vec![doc.clone()],
        });
        Ok((basis, limits(row.guidance).map_err(|e| e.to_string())?))
    })();
    let (basis, guidance) = match guidance_check {
        Ok(v) => v,
        Err(e) => {
            out.reason = Some("invalid_source_evidence".into());
            out.missing.push(e);
            return out;
        }
    };
    let outcome = out.note(
        "annual outcome",
        checked_cell(profile, cell, &out.unit, &basis, unit),
    );
    if outcome.is_none() {
        out.reason = Some("invalid_source_evidence".into());
        return out;
    }
    let assumed_quantities = [slug(row.site), "all_sites".into()].map(|subject| {
        format!(
            "aisc_price_assumption_{subject}_{}_{}",
            period.to_ascii_lowercase(),
            out.unit
        )
    });
    let assumed = out.transcription(
        transcripts,
        "stated guidance price assumption",
        &assumed_quantities,
        &period,
        Some(&row.guidance.locator.doc),
        subject,
        unit,
    );
    if assumed.is_none() {
        out.reason = Some(
            if out
                .transcripts
                .iter()
                .any(|e| e.role == "stated guidance price assumption")
            {
                "unusable_price_assumption"
            } else {
                "no_stated_price_assumption"
            }
            .into(),
        );
        return out;
    }
    let royalty = out.royalty(
        profile,
        cells,
        row.site,
        &period,
        &cell.key.reporting_vintage.release,
        &basis,
        unit,
    );
    let quantity = |p: &str| {
        format!(
            "realized_price_{}_{}_{}",
            slug(subject),
            p.to_ascii_lowercase(),
            row.guidance.unit
        )
    };
    let realized = out.transcription(
        transcripts,
        "annual realized price",
        &[quantity(&period)],
        &period,
        None,
        subject,
        unit,
    );
    let mut quarters = Vec::new();
    for q in out.quarters.clone() {
        let royalty = out.royalty(profile, cells, row.site, &q, &q, &basis, unit);
        let realized_price = out.transcription(
            transcripts,
            &format!("realized price {q}"),
            &[quantity(&q)],
            &q,
            None,
            subject,
            unit,
        );
        if let (Some(royalty), Some(realized_price)) = (royalty, realized_price) {
            quarters.push(price::RoyaltyQuarter {
                id: q,
                royalty,
                realized_price,
            });
        }
    }
    if !out.missing.is_empty() {
        out.reason = Some("incomplete_price_evidence".into());
        return out;
    }
    let (Some(outcome), Some(royalty), Some(realized_price)) = (outcome, royalty, realized) else {
        out.reason = Some("incomplete_price_evidence".into());
        return out;
    };
    let input = price::PriceInput {
        metric,
        guidance,
        guidance_citation: price::Citation {
            locator: row.guidance.locator.to_string(),
            sha256: match digest(&row.guidance.doc_sha256) {
                Ok(v) => v,
                Err(e) => {
                    out.reason = Some("invalid_source_evidence".into());
                    out.missing.push(e);
                    return out;
                }
            },
        },
        outcome,
        royalty,
        assumed_price: assumed,
        realized_price,
        price_subject: subject.into(),
        quarters,
    };
    match price::conditional(&input) {
        Ok(report) if row.score == Some(report.unconditional()) => {
            out.reason = report.unsplit_reason().map(|r| format!("{r:?}"));
            out.report = Some(report);
        }
        Ok(_) => {
            out.reason = Some("invalid_source_evidence".into());
            out.missing
                .push("unconditional score changed since outcome binding".into());
        }
        Err(e) => {
            out.reason = Some("invalid_source_evidence".into());
            out.missing.push(e.to_string());
        }
    }
    out
}

/// Price the public report's exact first/latest selection, preserving its unresolved denominator.
/// History is bounded by completed planned quarters, not by whichever workbook happens to exist.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assess_as_of(
    profile: &IssuerProfile,
    transcripts: &TranscriptionReport,
    cells: &[KeyedCell],
    row: &fmine_public::guidance::track_record::TrackRecordRow,
    outcome: &fmine_public::guidance::track_record::OutcomeAssessment,
    as_of: &str,
    price_subject: Option<&str>,
) -> PriceAssessment {
    let completed = POPULATION
        .iter()
        .copied()
        .rfind(|quarter| {
            let Some(period) = fmine_public::key::parse_period_code(quarter) else {
                return false;
            };
            let (_, (_, month)) = period.months();
            let end = format!(
                "{:04}-{month:02}-{}",
                period.year,
                if month == 3 || month == 12 { 31 } else { 30 }
            );
            end.as_str() <= as_of
        })
        .unwrap_or("2023Q4");
    let source = match outcome.sources.as_slice() {
        [cell]
            if outcome.score.is_some()
                && outcome.refusal.is_none()
                && crate::track_record_data::site_key(&cell.key.subject) == row.site =>
        {
            Some(cell)
        }
        _ => None,
    };
    let mut assessment = assess_row(
        profile,
        transcripts,
        cells,
        PriceRow {
            site: source.map_or(row.site.as_str(), |c| c.key.subject.as_str()),
            metric: &row.metric,
            year: row.year,
            guidance: &row.guidance,
            published_at: &row.document.published_at,
            outcome: source,
            score: outcome.score.as_ref(),
        },
        Some(completed),
        price_subject,
    );
    assessment.scope = format!(
        "2024Q1–2026Q2 intersected with quarters completed by evidence cut {as_of}; missing original releases remain required"
    );
    if !is_date(as_of) {
        assessment.reason = Some("invalid_source_context".into());
        assessment.report = None;
        assessment
            .missing
            .push("evidence cut must be YYYY-MM-DD".into());
    } else if source.is_none() {
        assessment.cells.extend(
            outcome
                .sources
                .iter()
                .cloned()
                .map(|c| ("unscored evidence-cut outcome".into(), c)),
        );
        if outcome.score.is_some() {
            assessment.reason = Some("invalid_source_evidence".into());
            assessment.missing.push(
                "scored evidence-cut outcome must have one matching canonical site source".into(),
            );
        }
    }
    assessment
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{Claim, Confidentiality};
    use fmine_core::{anchor::SourceAnchor, hash::sha256};
    use fmine_evidence::{EpistemicStatus, KuKind};
    use fmine_public::key::{LineKey, Period, PeriodKind, PublicKey, Vintage};
    use fmine_public::sources::RegisteredSource;

    #[test]
    fn synthetic_assumed_royalties_never_override_negative_knowledge_states() {
        let source_bytes = b"invented Q1 2024 royalty: 50 USD per ounce";
        let source_digest = sha256(source_bytes);
        let file = "Synthetic_Royalty_2024Q1.xlsx";
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.sources.push(RegisteredSource {
            file_name: file.into(),
            kind: SourceKind::MineStats,
            quarter: "2024Q1".into(),
            url: format!("https://example.invalid/{file}"),
            url_basis: "DESIGN: invented in-memory source".into(),
            bytes: source_bytes.len() as u64,
            sha256_hex: source_digest.to_hex(),
        });
        let key = PublicKey {
            issuer: profile.issuer.clone(),
            subject: "Site A".into(),
            valid_period: Period {
                year: 2024,
                kind: PeriodKind::Quarter(1),
            },
            reporting_vintage: Vintage {
                release: "2024Q1".into(),
                file_sha256: source_digest.to_hex(),
            },
            ownership_basis: OwnershipBasis::Consolidated100,
            scope: Scope::Site,
            unit: "usd_per_oz".into(),
            line: LineKey {
                label: "site:costs per ounce:royalties".into(),
                occurrence: 0,
            },
        };
        let cell_for_state = |state| {
            let value = PublicValue::Number {
                stored: "50".into(),
                displayed: Ok("50".into()),
                under_status: None,
            };
            let value_tag = canonical_value_tag(&key, &value);
            let claim = Claim::new(
                &key.line.label,
                value,
                &value_tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::IntensiveWeighted,
                EvidenceClass::Assumed,
                EpistemicStatus::KnownUnknown(KuKind::Parametric),
                state,
                Confidentiality::public(),
                vec![],
                vec![],
                vec![SourceAnchor::new(file, source_digest.0, "QTD!B7")],
            );
            KeyedCell {
                locator: format!("{file}:QTD!B7"),
                key: key.clone(),
                claim,
                printed_basis: key.ownership_basis,
                basis_evidence: None,
                status_evidence: None,
                label_as_printed: None,
                unit_evidence: None,
            }
        };
        for state in [
            KnowledgeState::Known,
            KnowledgeState::Estimated,
            KnowledgeState::Unknown,
        ] {
            let cell = cell_for_state(state);
            cell.claim.check_id().expect("new canonical source Claim");
            let cited = checked_cell(
                &profile,
                &cell,
                "usd_per_oz",
                "100%",
                price::RateUnit::UsdPerTroyOunce,
            )
            .unwrap_or_else(|error| panic!("{state:?}: {error}"));
            assert_eq!(cited.printed.value, Rat::from_integer(50));
            assert_eq!(cited.citation.locator, cell.locator);
            assert_eq!(cited.citation.sha256, source_digest);
        }
        for state in [
            KnowledgeState::Conflicted,
            KnowledgeState::NotObservable,
            KnowledgeState::Redacted,
            KnowledgeState::Stale,
            KnowledgeState::Indeterminate,
            KnowledgeState::NotApplicable,
        ] {
            let cell = cell_for_state(state);
            // Rehash the changed state so failure cannot come from a stale Claim id.
            cell.claim.check_id().expect("new canonical source Claim");
            let error = checked_cell(
                &profile,
                &cell,
                "usd_per_oz",
                "100%",
                price::RateUnit::UsdPerTroyOunce,
            )
            .expect_err("synthetic evidence must preserve an explicit negative state");
            assert_eq!(
                error, "source is not an observable numeric claim",
                "{state:?}"
            );
        }
    }
}
