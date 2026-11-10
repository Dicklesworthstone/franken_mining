//! DESIGN (BD-345.S4): dated evidence of a supported equality in the registered public record.
//! Publication and numeric verification are independent. A matching later publication cannot supply
//! evidence at an earlier cut, and a retrieval date is never relabelled as a publication date.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::{AdditivityClass, Representation, SpatialSupport};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_io::numtok::{Dec, Token, tokenize};
use fmine_metrics::roll_forward::quarter::{self, Observation, Printed, Quantity, Status};
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{
    KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope, canonical_value_tag,
};
use fmine_public::lint::{LintFinding, LintMode, LintReport, Verdict, equality_target, lint_cited};
use fmine_public::sources::{IssuerProfile, RegisteredSource};
use fmine_registry::sources::{PUBLISHED_AT_BASES, is_date};

use crate::json_string as js;

mod ownership;
mod rates;
mod sums;

pub(super) const NO_CLAIM: &str = "Checks supported equalities, additive cumulative-to-quarter differences, complete quarter sums, full-mine to attributable amounts and gold AISC/by-product TCC cost-over-sales rates against dated, registered public sources at the requested cut. Ownership scaling requires the same source's explicit printed share for the same physical period. Cost rates require same-table whole-number US$M and attributable koz operands under their declared identity basis. Earliest means earliest matching registered publication, not a complete search of the public record. Exact derived enclosures retain displayed rounding uncertainty and source ancestry. A non-obvious derived figure can itself be new information and requires disclosure review. This does not establish the truth of an issuer assertion, authorize disclosure or assess legal compliance. Other derived identities are not assessed; explicitly marked forward statements have their own guidance assessment.";

#[derive(Debug, Clone)]
pub(super) struct Publication {
    pub file: String,
    pub sha256: String,
    pub published_at: String,
    pub published_at_basis: String,
}

#[derive(Debug, Clone)]
struct SourceEvidence {
    source: KeyedCell,
    file: Option<String>,
    url: Option<String>,
    published_at: Option<String>,
    publication_basis: Option<String>,
    known_available_by: Option<String>,
    relation: &'static str,
    admission: &'static str,
    reason: &'static str,
}

#[derive(Debug, Clone)]
struct Finding {
    span: (usize, usize),
    status: &'static str,
    reason: &'static str,
    first_registered_disclosure: Option<String>,
    evidence: Vec<SourceEvidence>,
    derivation: Option<Derivation>,
    quarter_sum: Option<sums::Assessment>,
    ownership: Option<ownership::Assessment>,
    cost_rate: Option<rates::Assessment>,
}

#[derive(Debug, Clone)]
struct Derivation {
    report: quarter::Report,
    evidence: Vec<SourceEvidence>,
}

#[derive(Debug, Clone)]
pub(super) struct PublicnessReport {
    as_of: String,
    draft_digest: Digest32,
    claims_digest: Digest32,
    findings: Vec<Finding>,
}

type Family = (String, String, Period, Scope, String, String, u32);

fn optional(s: Option<&str>) -> String {
    s.map_or_else(|| "null".to_string(), js)
}

// This is a conservative search partition, not a second equality grammar. Every candidate still goes
// through the existing checker with the original draft amount, unit, period and basis.
fn family(c: &KeyedCell) -> Family {
    let label = c.key.line.label.as_str();
    let metric = if label.contains("aisc") || label.contains("all-in sustaining") {
        "aisc"
    } else if label.contains("c1") {
        "c1"
    } else if label.contains("tcc") || label.contains("total cash costs") {
        "tcc"
    } else if label.contains("cos_") || label.contains("cost of sales") {
        "cost_of_sales"
    } else if label.starts_with("summary:au_prod_") || label.starts_with("summary:cu_prod_") {
        "production"
    } else if label.contains("sold") {
        "sales"
    } else if label.contains("grade") && label.ends_with("processed") {
        "processed_grade"
    } else if label.contains("recovery") {
        "recovery"
    } else {
        label
    };
    (
        c.key.issuer.clone(),
        c.key.subject.clone(),
        c.key.valid_period,
        c.key.scope.clone(),
        c.key.unit.clone(),
        metric.to_string(),
        c.key.line.occurrence,
    )
}

fn publication_map<'a>(
    profile: &IssuerProfile,
    publications: &'a [Publication],
) -> Result<BTreeMap<(String, String), &'a Publication>, Refusal> {
    let mut out = BTreeMap::new();
    for p in publications {
        let Some(source) = profile.sources.iter().find(|s| s.file_name == p.file) else {
            // A --through snapshot may deliberately omit a registered later source.
            continue;
        };
        if source.sha256_hex != p.sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "publication metadata does not identify the registered bytes of {}",
                    p.file
                ),
            ));
        }
        if !is_date(&p.published_at)
            || !PUBLISHED_AT_BASES.iter().any(|prefix| {
                p.published_at_basis
                    .strip_prefix(prefix)
                    .is_some_and(|rest| !rest.trim().is_empty())
            })
        {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "{} lacks a valid publication date and its explicit basis",
                    p.file
                ),
            ));
        }
        if out.insert((p.file.clone(), p.sha256.clone()), p).is_some() {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!("{} has more than one publication metadata row", p.file),
            ));
        }
    }
    Ok(out)
}

fn source_for<'a>(c: &KeyedCell, profile: &'a IssuerProfile) -> Option<&'a RegisteredSource> {
    let mut matches = profile.sources.iter().filter(|s| {
        s.quarter == c.key.reporting_vintage.release
            && s.sha256_hex == c.key.reporting_vintage.file_sha256
            && c.claim.anchors.iter().any(|a| {
                Path::new(&a.artifact_path)
                    .file_name()
                    .and_then(|p| p.to_str())
                    == Some(s.file_name.as_str())
                    && Digest32(a.artifact_sha256).to_hex() == s.sha256_hex
                    && c.locator == format!("{}:{}", s.file_name, a.locator)
            })
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn usable(c: &KeyedCell) -> bool {
    c.claim.check_id().is_ok()
        && c.claim.metric_key == c.key.line.label
        && c.claim.value_tag() == canonical_value_tag(&c.key, &c.claim.value)
        && c.claim.evidence == EvidenceClass::ISSUER
        && matches!(
            c.claim.state,
            KnowledgeState::Known | KnowledgeState::Estimated
        )
        && c.claim.support == SpatialSupport::PeriodAggregate
        && matches!(c.claim.representation, Representation::Deterministic)
        && !c.claim.stale
        && !c.claim.calibration_failed
        && matches!(
            c.claim.value,
            PublicValue::Number {
                displayed: Ok(_),
                under_status: None,
                ..
            }
        )
}

fn source_evidence(
    c: &KeyedCell,
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
) -> SourceEvidence {
    let source = source_for(c, profile);
    let publication = source
        .and_then(|s| publications.get(&(s.file_name.clone(), s.sha256_hex.clone())))
        .copied();
    let (admission, reason) = if !usable(c) || c.key.issuer != profile.issuer {
        (
            "unusable",
            "source_claim_identity_or_evidence_not_admissible",
        )
    } else if source.is_none() {
        (
            "unusable",
            "source_filename_hash_release_or_locator_not_registered",
        )
    } else if let Some(p) = publication {
        if p.published_at.as_str() <= as_of {
            (
                "published_by_cut",
                "registered_publication_date_at_or_before_cut",
            )
        } else {
            ("after_cut", "registered_publication_date_after_cut")
        }
    } else {
        (
            "date_unknown",
            "retrieval_and_reporting_period_do_not_establish_publication",
        )
    };
    SourceEvidence {
        source: c.clone(),
        file: source.map(|s| s.file_name.clone()),
        url: source.map(|s| s.url.clone()),
        published_at: publication.map(|p| p.published_at.clone()),
        publication_basis: publication.map(|p| p.published_at_basis.clone()),
        known_available_by: source.and_then(|s| {
            fmine_registry::sources::available_date(None, None, s.retrieval(&profile.retrieved_utc))
                .ok()
        }),
        relation: "unchecked",
        admission,
        reason,
    }
}

fn sort_evidence(evidence: &mut Vec<SourceEvidence>) {
    evidence.sort_by(|a, b| {
        (&a.published_at, &a.source.locator, a.source.claim.claim_id).cmp(&(
            &b.published_at,
            &b.source.locator,
            b.source.claim.claim_id,
        ))
    });
    evidence.dedup_by(|a, b| a.source.claim.claim_id == b.source.claim.claim_id);
}

fn quantity(c: &KeyedCell) -> Option<Quantity> {
    match (c.key.line.label.as_str(), c.key.unit.as_str()) {
        ("summary:au_prod_100_koz" | "summary:au_prod_attr_koz", "koz") => {
            Some(Quantity::GoldProductionKoz)
        }
        ("summary:au_sold_attr_koz" | "ounces sold", "koz") => Some(Quantity::GoldSalesKoz),
        (
            "summary:cu_prod_100_kt"
            | "summary:cu_prod_attr_kt"
            | "site::production (thousands of tonnes)",
            "kt",
        ) => Some(Quantity::CopperProductionKt),
        _ => None,
    }
}

fn cumulative_quarter(kind: PeriodKind) -> Option<u8> {
    match kind {
        PeriodKind::Quarter(1) => Some(1),
        PeriodKind::H1 => Some(2),
        PeriodKind::NineMonths => Some(3),
        PeriodKind::FullYear => Some(4),
        _ => None,
    }
}

fn explicit_basis(basis: OwnershipBasis) -> Option<String> {
    match basis {
        OwnershipBasis::Consolidated100 => Some("100%".into()),
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && den > 0 && num <= den => {
            let share = Rat::new(i128::from(num), i128::from(den)).ok()?;
            Some(format!("attributable:{}/{}", share.num(), share.den()))
        }
        _ => None,
    }
}

fn decimal(d: Dec) -> Option<Rat> {
    Rat::new(d.mantissa, 10_i128.checked_pow(d.scale)?).ok()
}

fn printed(c: &KeyedCell) -> Option<Printed> {
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &c.claim.value
    else {
        return None;
    };
    let tokens = tokenize(text);
    let [Token::Number(n)] = tokens.as_slice() else {
        return None;
    };
    if text.trim() != n.raw.trim()
        || n.range_with.is_some()
        || n.unit.is_some()
        || n.currency.is_some()
        || n.scale_word.is_some()
        || n.hedge.is_some()
    {
        return None;
    }
    Some(Printed {
        value: decimal(n.value)?,
        step: decimal(n.step)?,
    })
}

fn derivation(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
) -> Option<Derivation> {
    // Prototypes bind prose only. No target-period Claim is manufactured, and a prototype's numeric
    // value never selects a vintage. Gather every sibling, including corrected/conflicting bases.
    let prototypes: Vec<_> = cells
        .iter()
        .filter_map(|c| {
            let kind = quantity(c)?;
            let target = equality_target(draft, f.span, c)?;
            let PeriodKind::Quarter(q @ 2..=4) = target.period.kind else {
                return None;
            };
            (c.key.valid_period.year == target.period.year
                && c.key.ownership_basis != OwnershipBasis::Unprinted)
                .then_some((c, target, q, kind))
        })
        .collect();
    let (_, target, q, kind) = *prototypes.first()?;
    let mut evidence: Vec<_> = cells
        .iter()
        .filter(|c| {
            quantity(c) == Some(kind)
                && c.key.valid_period.year == target.period.year
                && cumulative_quarter(c.key.valid_period.kind).is_some_and(|n| n == q || n == q - 1)
                && prototypes.iter().any(|(p, _, _, _)| {
                    p.key.issuer == c.key.issuer
                        && p.key.subject == c.key.subject
                        && std::mem::discriminant(&p.key.ownership_basis)
                            == std::mem::discriminant(&c.key.ownership_basis)
                })
        })
        .map(|c| source_evidence(c, profile, publications, as_of))
        .collect();
    sort_evidence(&mut evidence);
    let inputs = evidence
        .iter_mut()
        .filter_map(|e| {
            if !e.published_at.as_deref().is_some_and(|date| date <= as_of) {
                return None;
            }
            let c = &e.source;
            let basis = explicit_basis(c.key.ownership_basis);
            let compatible = basis.is_some()
                && c.claim.additivity == AdditivityClass::ExtensiveAdditive
                && !matches!(&c.key.scope, Scope::Region { map_version } if map_version.is_empty() || map_version.starts_with("unregistered:"))
                && !(c.basis_evidence.is_some()
                    && c.printed_basis != OwnershipBasis::Unprinted
                    && explicit_basis(c.printed_basis) != basis);
            let value = (e.admission == "published_by_cut" && compatible)
                .then(|| printed(c))
                .flatten();
            e.relation = if value.is_some() {
                "cumulative_input"
            } else {
                "unusable_cumulative_input"
            };
            let series = format!(
                "{:?}",
                (
                    &c.key.issuer,
                    &c.key.subject,
                    target.period.year,
                    &c.key.scope,
                    kind,
                    &c.key.unit,
                    c.key.line.occurrence,
                    basis
                )
            );
            Some(Observation {
                source_id: c.claim.claim_id.to_string(),
                series,
                through_quarter: cumulative_quarter(c.key.valid_period.kind)?,
                printed: value,
            })
        })
        .collect::<Vec<_>>();
    Some(Derivation {
        report: quarter::assess(
            kind,
            q,
            Printed {
                value: decimal(target.value)?,
                step: decimal(target.step)?,
            },
            &inputs,
        ),
        evidence,
    })
}

// Probe the original line without changing its prose or amount. Discovery stores the selected Claim id
// as metadata; its temporary comparison must insert a link. Cited findings already have a link to replace.
fn probe(draft: &str, f: &LintFinding, c: &KeyedCell) -> Option<LintFinding> {
    let before = draft.get(..f.span.0)?;
    let after = draft.get(f.span.1..)?;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    let end = after.find('\n').map_or(draft.len(), |i| f.span.1 + i);
    let line = draft.get(start..end)?;
    let span = (f.span.0 - start, f.span.1 - start);
    let id = c.claim.claim_id.to_string();
    let (text, expected) = match (f.mode, &f.claim_id) {
        (LintMode::Cited, Some(original)) if original.len() == id.len() => (
            line.replace(&format!("fmine:{original}"), &format!("fmine:{id}")),
            span,
        ),
        (LintMode::Cited, Some(_)) => return None,
        _ => (
            format!(
                "{}[{}](fmine:{id}){}",
                &line[..span.0],
                &line[span.0..span.1],
                &line[span.1..]
            ),
            (span.0 + 1, span.1 + 1),
        ),
    };
    lint_cited(&text, std::slice::from_ref(c))
        .findings
        .into_iter()
        .find(|candidate| candidate.span == expected)
}

fn relation(f: Option<&LintFinding>) -> &'static str {
    let Some(f) = f else {
        return "unchecked";
    };
    if !f.also.is_empty() {
        return "unchecked";
    }
    match f.verdict {
        Verdict::Verified => "matching_value_and_basis",
        Verdict::ValueMismatch => "different_value_same_basis",
        Verdict::RoundingBeyondTolerance => "precision_unresolved",
        Verdict::BasisMismatch
            if matches!(
                f.reason,
                "ownership" | "sold_produced_denominator" | "metric_definition"
            ) =>
        {
            "matching_value_other_basis"
        }
        _ => "unchecked",
    }
}

// Singleton probes must not erase an ambiguous ownership share from the original semantic population.
fn ambiguous_attributable_shares(f: &LintFinding, candidates: &[&KeyedCell]) -> bool {
    let population: Vec<_> = if f.candidates.is_empty() {
        let Some(source) = &f.source else {
            return false;
        };
        if !matches!(
            source.key.ownership_basis,
            OwnershipBasis::Attributable { .. }
        ) {
            return false;
        }
        let co_product = fmine_public::aisc::is_coproduct_rate(&source.key.line.label);
        candidates
            .iter()
            .copied()
            .filter(|c| fmine_public::aisc::is_coproduct_rate(&c.key.line.label) == co_product)
            .collect()
    } else {
        f.candidates.iter().collect()
    };
    let mut shares = BTreeSet::new();
    for c in population {
        if matches!(c.key.ownership_basis, OwnershipBasis::Attributable { .. }) {
            let Some(share) = explicit_basis(c.key.ownership_basis) else {
                return true;
            };
            shares.insert(share);
        }
    }
    shares.len() > 1
}

/// The caller supplies the same verified source projection used by lint. Publications identify exact
/// workbook bytes; absent dates stay absent even when a reporting quarter or retrieval time is known.
pub(super) fn assess(
    draft: &str,
    lint: &LintReport,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &[Publication],
    as_of: &str,
) -> Result<PublicnessReport, Refusal> {
    if !is_date(as_of) {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            "public-record cut must be YYYY-MM-DD",
        ));
    }
    if sha256(draft.as_bytes()) != lint.draft_digest {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "public-record assessment received a different draft from the equality report",
        ));
    }
    if fmine_public::key::claims_digest(cells) != lint.claims_digest {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "public-record assessment received a different Claim snapshot from the equality report",
        ));
    }
    if cells.iter().any(|c| !c.claim.confidentiality.is_public()) {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            "non-public evidence in a public-record assessment",
        ));
    }
    let publications = publication_map(profile, publications)?;
    let mut index: BTreeMap<Family, Vec<&KeyedCell>> = BTreeMap::new();
    for c in cells {
        index.entry(family(c)).or_default().push(c);
    }
    let mut findings = Vec::with_capacity(lint.findings.len());
    for f in &lint.findings {
        let keys: BTreeSet<_> = f
            .source
            .iter()
            .chain(f.candidates.iter())
            .map(family)
            .collect();
        let mut result = Finding {
            span: f.span,
            status: "not_assessed",
            reason: "no_unique_supported_semantic_target",
            first_registered_disclosure: None,
            evidence: Vec::new(),
            derivation: None,
            quarter_sum: None,
            ownership: None,
            cost_rate: None,
        };
        if keys.len() == 1 {
            let candidates = keys.iter().next().and_then(|k| index.get(k));
            let ambiguous_share = candidates.is_some_and(|c| ambiguous_attributable_shares(f, c));
            for c in candidates.into_iter().flatten() {
                let mut evidence = source_evidence(c, profile, &publications, as_of);
                if evidence.admission != "unusable" {
                    evidence.relation = relation(probe(draft, f, c).as_ref());
                }
                result.evidence.push(evidence);
            }
            sort_evidence(&mut result.evidence);
            if ambiguous_share {
                result.reason = "attributable_share_unspecified";
            } else if let Some(first) = result.evidence.iter().find(|e| {
                e.admission == "published_by_cut" && e.relation == "matching_value_and_basis"
            }) {
                result.status = "disclosed";
                result.reason = "matching_assertion_in_the_dated_registered_record";
                result.first_registered_disclosure = Some(first.source.claim.claim_id.to_string());
            } else if result.evidence.iter().any(|e| {
                e.admission == "published_by_cut" && e.relation == "matching_value_other_basis"
            }) {
                result.status = "disclosed_other_basis";
                result.reason = "a_matching_amount_is_published_with_an_incompatible_basis";
            } else if result.evidence.iter().any(|e| {
                e.admission == "published_by_cut" && e.relation == "different_value_same_basis"
            }) {
                result.status = "contradicts_disclosed";
                result.reason =
                    "dated_same_basis_assertions_have_nonoverlapping_displayed_intervals";
            } else if result
                .evidence
                .iter()
                .any(|e| e.admission == "date_unknown" && e.relation != "unchecked")
            {
                result.status = "publication_date_unknown";
                result.reason = "matching_series_has_no_registered_publication_date";
            } else if result
                .evidence
                .iter()
                .any(|e| e.admission == "published_by_cut" && e.relation == "precision_unresolved")
            {
                result.status = "precision_not_established";
                result.reason = "publication_does_not_support_the_draft_precision";
            } else if result
                .evidence
                .iter()
                .any(|e| e.admission == "after_cut" && e.relation != "unchecked")
            {
                result.status = "not_in_public_record";
                result.reason = "relevant_registered_assertions_are_published_after_the_cut";
            } else if result.evidence.iter().any(|e| e.admission == "unusable") {
                result.status = "evidence_unusable";
                result.reason = "source_identity_or_evidence_admission_failed";
            }
        }
        result.derivation = derivation(draft, f, cells, profile, &publications, as_of);
        result.quarter_sum = sums::assess(draft, f, cells, profile, &publications, as_of);
        result.ownership = ownership::assess(draft, f, cells, profile, &publications, as_of);
        result.cost_rate = rates::assess(draft, f, cells, profile, &publications, as_of);
        let derived_reason = if result
            .derivation
            .as_ref()
            .is_some_and(|d| d.report.status == Status::Derived)
        {
            Some("every_admitted_cumulative_difference_supports_the_draft_precision")
        } else if result
            .quarter_sum
            .as_ref()
            .is_some_and(|s| s.status == "derived")
        {
            Some("complete_dated_quarter_sum_supports_the_draft_precision")
        } else if result
            .ownership
            .as_ref()
            .is_some_and(|s| s.status == "derived")
        {
            Some("same_source_printed_ownership_supports_the_draft_precision")
        } else if result
            .cost_rate
            .as_ref()
            .is_some_and(|s| s.status == "derived")
        {
            Some("same_source_cost_over_sales_supports_the_draft_precision")
        } else {
            None
        };
        if let Some(reason) = derived_reason
            && !matches!(
                result.status,
                "disclosed" | "disclosed_other_basis" | "contradicts_disclosed"
            )
            && !result.evidence.iter().any(|e| {
                e.admission == "published_by_cut" && e.relation == "different_value_same_basis"
            })
        {
            result.status = "derivable";
            result.reason = reason;
        }
        findings.push(result);
    }
    Ok(PublicnessReport {
        as_of: as_of.to_string(),
        draft_digest: lint.draft_digest,
        claims_digest: lint.claims_digest,
        findings,
    })
}

impl SourceEvidence {
    fn to_json(&self) -> String {
        format!(
            "{{\"file\":{},\"url\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"known_available_by_retrieval\":{},\"relation\":{},\"admission\":{},\"reason\":{},\"source_claim\":{},\"canonical_claim\":{}}}",
            optional(self.file.as_deref()),
            optional(self.url.as_deref()),
            js(&self.source.key.reporting_vintage.file_sha256),
            optional(self.published_at.as_deref()),
            optional(self.publication_basis.as_deref()),
            optional(self.known_available_by.as_deref()),
            js(self.relation),
            js(self.admission),
            js(self.reason),
            self.source.to_json(),
            canonical_json(&self.source.claim, "public_source", self.source.to_json()),
        )
    }
}

fn canonical_json<T>(claim: &fmine_claim::Claim<T>, category: &str, value: String) -> String {
    crate::claim_inspection::canonical_claim_json(claim, category, value).unwrap_or_else(|r| {
        format!(
            "{{\"refusal\":{{\"code\":{},\"what\":{}}}}}",
            js(r.code),
            js(&r.what)
        )
    })
}

fn evidence_json(evidence: &[SourceEvidence]) -> String {
    evidence
        .iter()
        .map(SourceEvidence::to_json)
        .collect::<Vec<_>>()
        .join(",")
}

impl Derivation {
    fn to_json(&self) -> String {
        format!(
            "{{{},\"evidence\":[{}]}}",
            self.report.body_json(),
            evidence_json(&self.evidence)
        )
    }
}

impl PublicnessReport {
    fn body_json(&self) -> String {
        let findings = self.findings.iter().map(|f| {
            format!(
                "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"status\":{},\"reason\":{},\"first_registered_disclosure\":{},\"evidence\":[{}],\"derivation\":{},\"quarter_sum\":{},\"ownership_scaling\":{},\"cost_rate\":{}}}",
                f.span.0, f.span.1, js(f.status), js(f.reason),
                optional(f.first_registered_disclosure.as_deref()),
                evidence_json(&f.evidence),
                f.derivation.as_ref().map_or_else(|| "null".into(), Derivation::to_json),
                f.quarter_sum.as_ref().map_or_else(|| "null".into(), sums::Assessment::to_json),
                f.ownership.as_ref().map_or_else(|| "null".into(), ownership::Assessment::to_json),
                f.cost_rate.as_ref().map_or_else(|| "null".into(), rates::Assessment::to_json),
            )
        }).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.lint.public-record.v1\",\"as_of\":{},\"draft_sha256\":{},\"claims_digest\":{},\"publication_selection\":\"earliest matching dated registered assertion; later and conflicting assertions retained\",\"unsupported\":[\"cost_of_sales_co_product_and_copper_rates\",\"noninteger_cost_rate_operands\",\"reverse_ownership_scaling\",\"unrestricted_prose\"],\"no_claim\":{},\"findings\":[{}]",
            js(&self.as_of),
            js(&self.draft_digest.to_hex()),
            js(&self.claims_digest.to_hex()),
            js(NO_CLAIM),
            findings,
        )
    }

    pub(super) fn digest(&self) -> Digest32 {
        blake3_hash(self.body_json().as_bytes())
    }

    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"report_digest\":{},{} }}",
            js(&self.digest().to_hex()),
            self.body_json()
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let escape = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('|', "&#124;")
                .replace(char::from(96), "&#96;")
                .replace(['\n', '\r'], " ")
        };
        let mut out = format!(
            "## Public record at {}\n\n{}\n\nReport: {}.\n\n| Bytes | Assessment | Earliest matching registered Claim |\n| --- | --- | --- |\n",
            self.as_of,
            NO_CLAIM,
            self.digest().to_hex(),
        );
        for f in &self.findings {
            let _ = writeln!(
                out,
                "| {}..{} | {} | {} |",
                f.span.0,
                f.span.1,
                f.status,
                f.first_registered_disclosure.as_deref().unwrap_or("none")
            );
        }
        out.push_str("\n### Publication evidence\n\n");
        for f in &self.findings {
            let _ = writeln!(out, "- {}..{}: {}.", f.span.0, f.span.1, f.reason);
            if let Some(sum) = &f.quarter_sum {
                out.push_str(&sum.to_markdown());
            }
            if let Some(ownership) = &f.ownership {
                out.push_str(&ownership.to_markdown());
            }
            if let Some(rate) = &f.cost_rate {
                out.push_str(&rate.to_markdown());
            }
            if let Some(d) = &f.derivation {
                let _ = writeln!(
                    out,
                    "  - Cumulative derivation {}: {}. {}.",
                    d.report.rule(),
                    d.report.status.as_str(),
                    escape(&d.report.reason)
                );
                for pair in &d.report.pairs {
                    let _ = writeln!(
                        out,
                        "    - {} minus {}: [{}/{}, {}/{}] {}; {}.",
                        pair.cumulative_claim,
                        pair.prior_claim,
                        pair.interval.lo.num(),
                        pair.interval.lo.den(),
                        pair.interval.hi.num(),
                        pair.interval.hi.den(),
                        d.report.quantity.unit(),
                        pair.relation.as_str()
                    );
                }
            }
            for e in f
                .evidence
                .iter()
                .chain(f.derivation.iter().flat_map(|d| &d.evidence))
                .chain(f.quarter_sum.iter().flat_map(|s| &s.evidence))
                .chain(f.ownership.iter().flat_map(|s| &s.evidence))
                .chain(f.cost_rate.iter().flat_map(|s| &s.evidence))
            {
                let _ = writeln!(
                    out,
                    "  - {}: {}; {}; publication {} ({}); SHA-256 {}; Claim {}.",
                    escape(&e.source.locator),
                    e.relation,
                    e.admission,
                    e.published_at.as_deref().unwrap_or("unknown"),
                    escape(e.publication_basis.as_deref().unwrap_or("not registered")),
                    e.source.key.reporting_vintage.file_sha256,
                    e.source.claim.claim_id,
                );
                if e.published_at.is_none()
                    && let Some(retrieved) = &e.known_available_by
                {
                    let _ = writeln!(
                        out,
                        "    Verified available by retrieval on {retrieved}; this is not a publication date."
                    );
                }
            }
        }
        out
    }
}

include!("../../xtask/testdata/cli_lint_publicness_unit_tests.rs");
