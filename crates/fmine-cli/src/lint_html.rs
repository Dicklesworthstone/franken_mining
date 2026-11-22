//! Disclosure equality checks over an explicitly selected registered HTML population.
//! Publication evidence describes source files under today's registry; it never dates the mapping.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::numtok::Dec;
use fmine_metrics::guidance::{Ownership, Period as CalendarPeriod};
use fmine_metrics::public_derivation::{self as arithmetic, Amount, AmountKind, Rule};
use fmine_metrics::roll_forward::quarter::{self, Observation, Printed, Quantity};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::html::{HtmlClaim, HtmlClaimsReport};
use fmine_public::key::{
    KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope, canonical_value_tag,
};
use fmine_public::lint::{LintFinding, LintReport, equality_target};
use fmine_public::sources::is_issuer_name;
use fmine_registry::sources::is_date;

use crate::claim_inspection::canonical_claim_json;
use crate::{Envelope, NextAction, json_string as js};

pub(super) const NO_CLAIM: &str = "Checks supported equalities and separate exact production/sales period arithmetic against the selected registered HTML Claims. Source publication dates come from the current registry; historical availability of the mapping and registry is not established. Arithmetic over public assertions does not make a derived figure an issuer disclosure. Credit treatment is not inferred from AISC/TCC labels. This does not establish the truth of issuer assertions, verify other prose, authenticate a historical information set or authorize disclosure.";

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), js)
}

fn source_date(cell: &KeyedCell, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let projections: Vec<_> = snapshot
        .claims
        .iter()
        .filter(|claim| claim.cell.claim.claim_id == cell.claim.claim_id)
        .collect();
    let [projection] = projections.as_slice() else {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint finding does not identify one Claim in its verified projection",
        ));
    };
    let value_anchor = &projection.value_source.anchor;
    // Registry digests are unique; an in-workspace symlink may give the value anchor a different basename.
    let matches: Vec<_> = snapshot
        .sources
        .iter()
        .filter(|source| {
            source.sha256 == cell.key.reporting_vintage.file_sha256
                && source.release == cell.key.reporting_vintage.release
                && cell.claim.anchors.iter().any(|anchor| {
                    anchor == value_anchor
                        && Digest32(anchor.artifact_sha256).to_hex() == source.sha256
                })
        })
        .collect();
    let [source] = matches.as_slice() else {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint finding does not identify one source in its verified projection",
        ));
    };
    if snapshot
        .as_of
        .as_deref()
        .is_some_and(|cut| source.published_at.as_str() > cut)
    {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint projection contains a source after its publication cut",
        ));
    }
    Ok(format!(
        "{{\"claim_id\":{},\"file\":{},\"sha256\":{},\"release\":{},\"url\":{},\"published_at\":{},\"published_at_basis\":{},\"availability\":{}}}",
        js(&cell.claim.claim_id.to_string()),
        js(&source.file),
        js(&source.sha256),
        js(&source.release),
        js(&source.url),
        js(&source.published_at),
        js(&source.published_at_basis),
        js(if snapshot.as_of.is_some() {
            "published_by_cut_under_current_registry"
        } else {
            "registered_publication_date"
        }),
    ))
}

fn finding_dates(finding: &LintFinding, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let mut ids = BTreeSet::new();
    let sources = finding
        .source
        .iter()
        .chain(&finding.candidates)
        .chain(&finding.later)
        .filter(|cell| ids.insert(cell.claim.claim_id))
        .map(|cell| source_date(cell, snapshot))
        .collect::<Result<Vec<_>, _>>()?;
    let status = if sources.is_empty() {
        "no_source_resolved_in_selected_population"
    } else if snapshot.as_of.is_some() {
        "resolved_sources_published_by_cut"
    } else {
        "resolved_source_dates_recorded"
    };
    Ok(format!(
        "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"equality_verdict\":{},\"status\":{},\"sources\":[{}]}}",
        finding.span.0,
        finding.span.1,
        js(finding.verdict.as_str()),
        js(status),
        sources.join(","),
    ))
}

fn publication_json(lint: &LintReport, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let findings = lint
        .findings
        .iter()
        .map(|finding| finding_dates(finding, snapshot))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.html_source_publication.v1\",\"as_of\":{},\"scope\":\"registered_source_availability_separate_from_equality\",\"registry_basis\":\"current_captured_registry\",\"mapping_historical_availability\":\"not_established\",\"registry_historical_availability\":\"not_established\",\"excluded_sources\":{},\"excluded_source_bytes_read\":false,\"unresolved_citations\":\"An absent Claim is unresolved in this selection; its absence does not establish that the assertion was unpublished\",\"findings\":[{findings}]}}",
        optional(snapshot.as_of.as_deref()),
        snapshot.excluded_sources.len(),
    ))
}

fn quantity(cell: &KeyedCell) -> Option<(AmountKind, Quantity)> {
    match (cell.key.line.label.as_str(), cell.key.unit.as_str()) {
        ("html:gold_production_koz", "koz") => {
            Some((AmountKind::GoldProductionKoz, Quantity::GoldProductionKoz))
        }
        ("html:copper_production_kt", "kt") => {
            Some((AmountKind::CopperProductionKt, Quantity::CopperProductionKt))
        }
        ("html:gold_sales_koz", "koz") => Some((AmountKind::GoldSalesKoz, Quantity::GoldSalesKoz)),
        _ => None,
    }
}

fn decimal(value: Dec) -> Result<Rat, Refusal> {
    10_i128
        .checked_pow(value.scale)
        .and_then(|den| Rat::new(value.mantissa, den).ok())
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "HTML period arithmetic: exact decimal overflow",
            )
        })
}

fn calendar(period: Period) -> CalendarPeriod {
    let ((year, first_month), (_, last_month)) = period.months();
    CalendarPeriod {
        year,
        first_month,
        last_month,
    }
}

/// A source assertion is interpreted only after its identity and printed quantity are checked.
/// The exact HTML token supplies the half-step enclosure; a placeholder or status flag is never zero.
fn amount(source: &HtmlClaim, kind: AmountKind) -> Result<Claim<Amount>, Refusal> {
    let cell = &source.cell;
    let invalid = || {
        Refusal::new(
            RefusalCode::SourceCellUnusable208,
            format!(
                "HTML period arithmetic requires an unflagged, exactly printed extensive quantity at {}",
                cell.locator
            ),
        )
    };
    cell.claim.check_id()?;
    let PublicValue::Number {
        stored,
        displayed: Ok(displayed),
        under_status: None,
    } = &cell.claim.value
    else {
        return Err(invalid());
    };
    let number = source.exact_number.as_ref().ok_or_else(invalid)?;
    if stored != &number.raw
        || displayed != &number.raw
        || number.percent
        || cell.claim.metric_key != cell.key.line.label
        || cell.claim.value_tag() != canonical_value_tag(&cell.key, &cell.claim.value)
        || cell.key.scope != Scope::Site
        || cell.printed_basis != cell.key.ownership_basis
        || cell.claim.additivity != AdditivityClass::ExtensiveAdditive
        || cell.claim.support != SpatialSupport::PeriodAggregate
        || cell.claim.representation != Representation::Deterministic
        || cell.claim.stale
        || cell.claim.calibration_failed
        || !matches!(
            cell.claim.state,
            KnowledgeState::Known | KnowledgeState::Estimated
        )
        || !cell.claim.confidentiality.is_public()
    {
        return Err(invalid());
    }
    let ownership = match cell.key.ownership_basis {
        OwnershipBasis::Consolidated100 => Ownership::Full,
        OwnershipBasis::Attributable { num, den, .. } => Ownership::Attributable {
            share: Rat::new(i128::from(num), i128::from(den)).map_err(|_| invalid())?,
        },
        OwnershipBasis::Unprinted => return Err(invalid()),
    };
    let value = Amount {
        issuer: cell.key.issuer.clone(),
        subject: cell.key.subject.clone(),
        scope: format!("site;occurrence={}", cell.key.line.occurrence),
        kind,
        ownership,
        period: calendar(cell.key.valid_period),
        interval: Printed {
            value: decimal(number.value)?,
            step: decimal(number.step)?,
        }
        .interval()
        .map_err(|_| invalid())?,
    };
    let tag = value.canonical_tag();
    Ok(Claim::new(
        arithmetic::INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        cell.claim.evidence.meet(EvidenceClass::Derived),
        cell.claim.epistemic,
        cell.claim.state,
        cell.claim.confidentiality.clone(),
        cell.claim.uu_terms.clone(),
        vec![cell.claim.claim_id],
        cell.claim.anchors.clone(),
    ))
}

fn interval_json(value: Interval) -> String {
    let rational = |q: Rat| {
        format!(
            "{{\"num\":{},\"den\":{}}}",
            js(&q.num().to_string()),
            js(&q.den().to_string())
        )
    };
    format!(
        "{{\"lo\":{},\"hi\":{},\"lo_strict\":false,\"hi_strict\":false}}",
        rational(value.lo),
        rational(value.hi)
    )
}

fn amount_json(claim: &Claim<Amount>) -> Result<String, Refusal> {
    let value = &claim.value;
    let (metric, unit) = match value.kind {
        AmountKind::GoldProductionKoz => ("gold_production", "koz"),
        AmountKind::CopperProductionKt => ("copper_production", "kt"),
        AmountKind::GoldSalesKoz => ("gold_sales", "koz"),
    };
    let ownership = match value.ownership {
        Ownership::Full => "100%".into(),
        Ownership::Attributable { share } => {
            format!("attributable:{}/{}", share.num(), share.den())
        }
    };
    canonical_claim_json(
        claim,
        "html_period_arithmetic",
        format!(
            "{{\"issuer\":{},\"subject\":{},\"scope\":{},\"metric\":{},\"unit\":{},\"ownership\":{},\"period\":{{\"year\":{},\"first_month\":{},\"last_month\":{}}},\"interval\":{}}}",
            js(&value.issuer),
            js(&value.subject),
            js(&value.scope),
            js(metric),
            js(unit),
            js(&ownership),
            value.period.year,
            value.period.first_month,
            value.period.last_month,
            interval_json(value.interval),
        ),
    )
}

fn compare(draft: Interval, source: Interval) -> Result<&'static str, Refusal> {
    let cmp = |a: Rat, b: Rat| {
        a.checked_cmp(b).map_err(|_| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "HTML period arithmetic comparison overflow",
            )
        })
    };
    Ok(
        if cmp(draft.lo, source.lo)?.is_le() && cmp(draft.hi, source.hi)?.is_ge() {
            "derived"
        } else if cmp(draft.hi, source.lo)?.is_lt() || cmp(draft.lo, source.hi)?.is_gt() {
            "contradicted"
        } else {
            "precision_not_established"
        },
    )
}

/// The difference kernel owns the arithmetic; this adapter binds its one admitted pair to Claims.
fn difference(
    kind: Quantity,
    period: Period,
    draft: Printed,
    inputs: &[Claim<Amount>],
) -> Result<Claim<Amount>, Refusal> {
    let [prior, current] = inputs else {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "HTML difference needs both cumulative periods",
        ));
    };
    let same = |value: &Amount| {
        (
            value.issuer.clone(),
            value.subject.clone(),
            value.scope.clone(),
            value.kind,
            value.ownership,
            value.period.year,
        )
    };
    if same(&prior.value) != same(&current.value) {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            "HTML cumulative operands differ in subject, physical support, unit or ownership",
        ));
    }
    let PeriodKind::Quarter(q) = period.kind else {
        return Err(Refusal::new(
            RefusalCode::SupportMismatch102,
            "HTML cumulative difference must target one quarter",
        ));
    };
    let printed = |claim: &Claim<Amount>| -> Result<Printed, Refusal> {
        let i = claim.value.interval;
        let failed = |_| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "HTML cumulative enclosure overflow",
            )
        };
        Ok(Printed {
            value: i
                .lo
                .checked_add(i.hi)
                .and_then(|v| v.checked_div(Rat::from_integer(2)))
                .map_err(failed)?,
            step: i.hi.checked_sub(i.lo).map_err(failed)?,
        })
    };
    let observations = [prior, current]
        .into_iter()
        .map(|claim| {
            Ok(Observation {
                source_id: claim.claim_id.to_string(),
                // Compatibility was checked above; the same bound series is used for both operands.
                series: prior.value.canonical_tag(),
                through_quarter: claim.value.period.last_month / 3,
                printed: Some(printed(claim)?),
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let report = quarter::assess(kind, q, draft, &observations);
    let [pair] = report.pairs.as_slice() else {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            report.reason,
        ));
    };
    let mut value = current.value.clone();
    value.period = calendar(period);
    value.interval = pair.interval;
    let tag = value.canonical_tag();
    let mut parents = vec![prior.claim_id, current.claim_id];
    parents.sort();
    let mut anchors = prior.anchors.clone();
    anchors.extend(current.anchors.iter().cloned());
    anchors.sort();
    anchors.dedup();
    let mut terms = prior.uu_terms.clone();
    for term in &current.uu_terms {
        if !terms.contains(term) {
            terms.push(term.clone());
        }
    }
    Ok(Claim::new(
        format!("public-disclosure.{}/1", report.rule()),
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        prior
            .evidence
            .meet(current.evidence)
            .meet(EvidenceClass::Derived),
        prior.epistemic.and(current.epistemic),
        prior.state.and(current.state),
        prior.confidentiality.join(&current.confidentiality),
        terms,
        parents,
        anchors,
    ))
}

fn arithmetic_finding(
    draft: &str,
    finding: &LintFinding,
    snapshot: &HtmlClaimsReport,
) -> Result<Option<(String, String)>, Refusal> {
    // An explicit citation or a direct discovery result keeps its equality verdict. Arithmetic never
    // repairs a bad citation or chooses a different source to make an already published number fit.
    if finding.claim_id.is_some() || finding.source.is_some() || !finding.candidates.is_empty() {
        return Ok(None);
    }
    let matching: Vec<_> = snapshot
        .claims
        .iter()
        .filter_map(|source| {
            let kind = quantity(&source.cell)?;
            Some((
                source,
                equality_target(draft, finding.span, &source.cell)?,
                kind,
            ))
        })
        .collect();
    let Some((prototype, target, (amount_kind, quantity))) = matching.first().copied() else {
        return Ok(None);
    };
    let (rule, required) = match target.period.kind {
        PeriodKind::Quarter(2) => ("H1-Q1", vec![PeriodKind::Quarter(1), PeriodKind::H1]),
        PeriodKind::Quarter(3) => ("9M-H1", vec![PeriodKind::H1, PeriodKind::NineMonths]),
        PeriodKind::Quarter(4) => ("FY-9M", vec![PeriodKind::NineMonths, PeriodKind::FullYear]),
        PeriodKind::H1 => ("quarter_sum", (1..=2).map(PeriodKind::Quarter).collect()),
        PeriodKind::NineMonths => ("quarter_sum", (1..=3).map(PeriodKind::Quarter).collect()),
        PeriodKind::FullYear => ("quarter_sum", (1..=4).map(PeriodKind::Quarter).collect()),
        _ => return Ok(None),
    };
    let mut sources: Vec<_> = matching
        .into_iter()
        .filter_map(|(source, t, kind)| {
            (t == target
                && kind.0 == amount_kind
                && source.cell.key.issuer == prototype.cell.key.issuer
                && source.cell.key.subject == prototype.cell.key.subject
                && source.cell.key.valid_period.year == target.period.year
                && (required.contains(&source.cell.key.valid_period.kind)
                    || source.cell.key.valid_period == target.period))
                .then_some(source)
        })
        .collect();
    sources.sort_by_key(|s| (s.cell.key.valid_period, s.cell.claim.claim_id));
    let mut status = "not_available";
    let mut reason = "one source for each required physical period is needed".to_string();
    let mut inputs = Vec::new();
    let mut derived = None;
    let mut refusal = None;
    let mut draft_interval = None;
    let evaluate = (|| -> Result<(), Refusal> {
        let printed = Printed {
            value: decimal(target.value)?,
            step: decimal(target.step)?,
        };
        let bounds = printed
            .interval()
            .map_err(|why| Refusal::new(RefusalCode::NumericalCertificateFailed207, why))?;
        draft_interval = Some(bounds);
        if sources
            .iter()
            .any(|s| s.cell.key.valid_period == target.period)
        {
            status = "direct_assertion_available";
            reason =
                "inspect or cite the directly reported assertion; arithmetic does not replace it"
                    .into();
            return Ok(());
        }
        let mut counts = BTreeMap::new();
        for source in &sources {
            *counts
                .entry(source.cell.key.valid_period.kind)
                .or_insert(0_usize) += 1;
        }
        if counts.values().any(|count| *count > 1) {
            status = "ambiguous";
            reason = "competing admitted source assertions remain visible; no vintage is selected by amount".into();
            return Ok(());
        }
        let missing: Vec<_> = required
            .iter()
            .filter(|p| counts.get(*p) != Some(&1))
            .map(|kind| {
                Period {
                    year: target.period.year,
                    kind: *kind,
                }
                .code()
            })
            .collect();
        if !missing.is_empty() {
            reason = format!("missing required physical periods: {}", missing.join(", "));
            return Ok(());
        }
        inputs = sources
            .iter()
            .map(|s| amount(s, amount_kind))
            .collect::<Result<Vec<_>, _>>()?;
        let output = if matches!(target.period.kind, PeriodKind::Quarter(_)) {
            difference(quantity, target.period, printed, &inputs)?
        } else {
            arithmetic::derive(Rule::QuarterSum, &inputs)?
        };
        status = compare(bounds, output.value.interval)?;
        reason = match status {
            "derived" => "the complete exact source enclosure fits the draft precision",
            "contradicted" => "the exact source enclosure is disjoint from the draft",
            _ => "source rounding uncertainty exceeds the draft precision",
        }
        .into();
        derived = Some(output);
        Ok(())
    })();
    if let Err(error) = evaluate {
        status = "refused";
        reason = error.what.clone();
        refusal = Some(error);
    }
    let evidence = sources
        .iter()
        .map(|source| {
            Ok(format!(
                "{{\"claim\":{},\"publication\":{}}}",
                canonical_claim_json(&source.cell.claim, "html_source", source.cell.to_json())?,
                source_date(&source.cell, snapshot)?,
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let input_json = inputs
        .iter()
        .map(amount_json)
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let derived_json = derived
        .as_ref()
        .map(amount_json)
        .transpose()?
        .unwrap_or_else(|| "null".into());
    let refusal_json = refusal.as_ref().map_or_else(
        || "null".into(),
        |r| {
            format!(
                "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                js(r.code),
                js(&r.what),
                js(r.fix),
            )
        },
    );
    let json = format!(
        "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"status\":{},\"reason\":{},\"rule\":{},\"target_period\":{},\"unit\":{},\"draft_interval\":{},\"source_evidence\":[{evidence}],\"inputs\":[{input_json}],\"derived_claim\":{derived_json},\"refusal\":{refusal_json}}}",
        finding.span.0,
        finding.span.1,
        js(status),
        js(&reason),
        js(rule),
        js(&target.period.code()),
        js(quantity.unit()),
        draft_interval.map_or_else(|| "null".into(), interval_json),
    );
    let mut markdown = format!(
        "- Bytes {}–{}: **{status}**, {rule}, {}. {}\n",
        finding.span.0,
        finding.span.1,
        target.period.code(),
        crate::guidance::text(&reason)
    );
    if let Some(claim) = derived {
        let i = claim.value.interval;
        let _ = writeln!(
            markdown,
            "  - Derived Claim `{}`: [{}/{}, {}/{}] {}.",
            claim.claim_id,
            i.lo.num(),
            i.lo.den(),
            i.hi.num(),
            i.hi.den(),
            quantity.unit()
        );
    }
    for source in sources {
        let _ = writeln!(
            markdown,
            "  - Source Claim `{}`: {}; SHA-256 `{}`.",
            source.cell.claim.claim_id,
            crate::guidance::text(&source.cell.locator),
            source.cell.key.reporting_vintage.file_sha256
        );
    }
    Ok(Some((json, markdown)))
}

fn arithmetic_report(
    draft: &str,
    lint: &LintReport,
    snapshot: &HtmlClaimsReport,
) -> Result<(String, String), Refusal> {
    let mut findings = Vec::new();
    let mut markdown = String::new();
    for finding in &lint.findings {
        if let Some((json, text)) = arithmetic_finding(draft, finding, snapshot)? {
            findings.push(json);
            markdown.push_str(&text);
        }
    }
    let body = format!(
        "\"schema\":\"fmine.html_period_arithmetic.v1\",\"as_of\":{},\"draft_sha256\":{},\"claims_digest\":{},\"definitions_sha256\":{},\"registry_sha256\":{},\"selection\":\"complete unique admitted periods; no vintage selected by amount\",\"publication_basis\":\"current registered source dates; historical mapping availability unproved\",\"warning\":{},\"findings\":[{}]",
        optional(snapshot.as_of.as_deref()),
        js(&lint.draft_digest.to_hex()),
        js(&snapshot.claims_digest.to_hex()),
        js(&snapshot.definitions_sha256),
        js(&snapshot.registry_sha256),
        js(arithmetic::WARNING),
        findings.join(","),
    );
    let mut frame = Encoder::new_frame(*b"HLDA", 1);
    frame.put_str(&body);
    let json = format!(
        "{{\"digest\":{},{}}}",
        js(&blake3_hash(&frame.finish()).to_hex()),
        body
    );
    if !markdown.is_empty() {
        markdown = format!(
            "\n## HTML period arithmetic\n\n{}\n\n{}",
            arithmetic::WARNING,
            markdown
        );
    }
    Ok((json, markdown))
}

pub(super) fn execute(
    path: &Path,
    issuer: &str,
    definitions: &Path,
    discover: bool,
    as_of: Option<&str>,
    root: &Path,
) -> Envelope {
    let actions = || vec![NextAction::new("fmine robot-docs", crate::ROBOT_DOCS_WHY)];
    if !is_issuer_name(issuer)
        || issuer == "synthetic"
        || definitions.as_os_str().is_empty()
        || as_of.is_some_and(|cut| !is_date(cut))
    {
        return Envelope::usage_error(
            "lint",
            "HTML lint requires a registered issuer, an explicit HTML mapping and an optional valid YYYY-MM-DD publication cut",
        );
    }
    let run = || -> Result<Envelope, Refusal> {
        let draft = crate::lint::read_draft(&root.join(path))?;
        let snapshot = fmine_public::html::run(root, issuer, definitions, as_of)?;
        let cells: Vec<_> = snapshot
            .claims
            .iter()
            .map(|claim| claim.cell.clone())
            .collect();
        let lint = if discover {
            fmine_public::lint::lint_discover(&draft, &cells)
        } else {
            fmine_public::lint::lint_cited(&draft, &cells)
        };
        if lint.claims_digest != snapshot.claims_digest {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                "HTML lint changed the canonical source population",
            ));
        }
        let (arithmetic, arithmetic_markdown) = arithmetic_report(&draft, &lint, &snapshot)?;
        let payload = format!(
            "{{\"schema\":\"fmine.lint_html_snapshot.v1\",\"issuer\":{},\"lane\":\"public\",\"read_mode\":\"verified_html_source_projection\",\"source_state_root\":null,\"as_of\":{},\"lint\":{},\"source_publication\":{},\"source_arithmetic\":{arithmetic},\"html_projection\":{}}}",
            js(issuer),
            optional(as_of),
            lint.to_json(),
            publication_json(&lint, &snapshot)?,
            crate::html::report_json(&snapshot)?,
        );
        let mut markdown = lint.to_markdown();
        markdown.push_str(&arithmetic_markdown);
        let _ = writeln!(markdown,
            "\n## HTML source publication evidence\n\n{}\n\nPublication evidence is separate from the equality verdict. Historical availability of the mapping and registry is not established. Unresolved citations remain unresolved; no future source bytes are read.\n",
            as_of.map_or_else(|| "Registered source dates are reported retrospectively.".into(),
                |cut| format!("The selected HTML sources have registered publication dates on or before {cut}.")),
        );
        markdown.push_str(&crate::html::markdown(&snapshot));
        let mut envelope = Envelope::ok("lint", payload, actions());
        envelope.no_claim = Some(NO_CLAIM);
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| Envelope::refused("lint", refusal, actions()))
}
