//! DESIGN (BD-345.S4): dated evidence of a supported equality in the registered public record.
//! Publication and numeric verification are independent. A matching later publication cannot supply
//! evidence at an earlier cut, and a retrieval date is never relabelled as a publication date.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use fmine_claim::{Representation, SpatialSupport};
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_public::key::{KeyedCell, Period, PublicValue, Scope, canonical_value_tag};
use fmine_public::lint::{LintFinding, LintReport, Verdict, lint_cited};
use fmine_public::sources::{IssuerProfile, RegisteredSource};
use fmine_registry::sources::{PUBLISHED_AT_BASES, is_date};

pub(super) const NO_CLAIM: &str = "Checks supported equalities against dated, registered public sources at the requested cut. Earliest means earliest matching registered publication, not a complete search of the public record. It does not establish the truth of an issuer assertion, authorize disclosure or assess legal compliance. Derived identities and forward guidance are not assessed.";

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
}

#[derive(Debug, Clone)]
pub(super) struct PublicnessReport {
    as_of: String,
    draft_digest: Digest32,
    claims_digest: Digest32,
    findings: Vec<Finding>,
}

type Family = (String, String, Period, Scope, String, String, u32);

fn js(s: &str) -> String {
    format!("\"{}\"", crate::escape_json(s))
}

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

// Probe only the original line, retaining all its prose and amount. Replacing a source link is an
// internal comparison, never a modification to the user's draft or a newly created source Claim.
fn probe(draft: &str, f: &LintFinding, c: &KeyedCell) -> Option<LintFinding> {
    let before = draft.get(..f.span.0)?;
    let after = draft.get(f.span.1..)?;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    let end = after.find('\n').map_or(draft.len(), |i| f.span.1 + i);
    let line = draft.get(start..end)?;
    let span = (f.span.0 - start, f.span.1 - start);
    let id = c.claim.claim_id.to_string();
    let (text, expected) = match &f.claim_id {
        Some(original) if original.len() == id.len() => (
            line.replace(&format!("fmine:{original}"), &format!("fmine:{id}")),
            span,
        ),
        Some(_) => return None,
        None => (
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
    use fmine_public::key::OwnershipBasis;
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
        let co_product =
            source.key.line.label == "total cash costs per ounce (on a co-product basis)";
        candidates
            .iter()
            .copied()
            .filter(|c| {
                (c.key.line.label == "total cash costs per ounce (on a co-product basis)")
                    == co_product
            })
            .collect()
    } else {
        f.candidates.iter().collect()
    };
    let mut shares = BTreeSet::new();
    for c in population {
        if let OwnershipBasis::Attributable { num, den, .. } = c.key.ownership_basis {
            if num <= 0 || den <= 0 || num > den {
                return true;
            }
            let Ok(share) = fmine_numerics::exact_feas::Rat::new(i128::from(num), i128::from(den))
            else {
                return true;
            };
            shares.insert((share.num(), share.den()));
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
    let retrieved = fmine_calib::record::parse_utc(&profile.retrieved_utc)
        .and_then(|_| profile.retrieved_utc.get(..10));
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
        };
        if keys.len() == 1 {
            let candidates = keys.iter().next().and_then(|k| index.get(k));
            let ambiguous_share = candidates.is_some_and(|c| ambiguous_attributable_shares(f, c));
            for c in candidates.into_iter().flatten() {
                let matched_source = source_for(c, profile);
                let publication = matched_source
                    .and_then(|s| publications.get(&(s.file_name.clone(), s.sha256_hex.clone())))
                    .copied();
                let (admission, reason) = if !usable(c) || c.key.issuer != profile.issuer {
                    (
                        "unusable",
                        "source_claim_identity_or_evidence_not_admissible",
                    )
                } else if matched_source.is_none() {
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
                let comparison = if admission == "unusable" {
                    "unchecked"
                } else {
                    relation(probe(draft, f, c).as_ref())
                };
                result.evidence.push(SourceEvidence {
                    source: (*c).clone(),
                    file: matched_source.map(|s| s.file_name.clone()),
                    url: matched_source.map(|s| s.url.clone()),
                    published_at: publication.map(|p| p.published_at.clone()),
                    publication_basis: publication.map(|p| p.published_at_basis.clone()),
                    known_available_by: matched_source.and(retrieved).map(str::to_string),
                    relation: comparison,
                    admission,
                    reason,
                });
            }
            result.evidence.sort_by(|a, b| {
                (&a.published_at, &a.source.locator, a.source.claim.claim_id).cmp(&(
                    &b.published_at,
                    &b.source.locator,
                    b.source.claim.claim_id,
                ))
            });
            result
                .evidence
                .dedup_by(|a, b| a.source.claim.claim_id == b.source.claim.claim_id);
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
            } else if result
                .evidence
                .iter()
                .any(|e| e.admission == "date_unknown" && e.relation != "unchecked")
            {
                result.status = "publication_date_unknown";
                result.reason = "matching_series_has_no_registered_publication_date";
            } else if result.evidence.iter().any(|e| {
                e.admission == "published_by_cut" && e.relation == "different_value_same_basis"
            }) {
                result.status = "contradicts_disclosed";
                result.reason =
                    "dated_same_basis_assertions_have_nonoverlapping_displayed_intervals";
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
            "{{\"file\":{},\"url\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{},\"known_available_by_retrieval\":{},\"relation\":{},\"admission\":{},\"reason\":{},\"source_claim\":{}}}",
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
        )
    }
}

impl PublicnessReport {
    fn body_json(&self) -> String {
        let findings = self.findings.iter().map(|f| {
            format!(
                "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"status\":{},\"reason\":{},\"first_registered_disclosure\":{},\"evidence\":[{}]}}",
                f.span.0, f.span.1, js(f.status), js(f.reason),
                optional(f.first_registered_disclosure.as_deref()),
                f.evidence.iter().map(SourceEvidence::to_json).collect::<Vec<_>>().join(","),
            )
        }).collect::<Vec<_>>().join(",");
        format!(
            "\"schema\":\"fmine.lint.public-record.v1\",\"as_of\":{},\"draft_sha256\":{},\"claims_digest\":{},\"publication_selection\":\"earliest matching dated registered assertion; later and conflicting assertions retained\",\"unsupported\":[\"derived_identities\",\"forward_guidance\"],\"no_claim\":{},\"findings\":[{}]",
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
            for e in &f.evidence {
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

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{AdditivityClass, Claim, Confidentiality};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::EpistemicStatus;
    use fmine_public::key::{LineKey, OwnershipBasis, PeriodKind, PublicKey, Vintage};
    use fmine_public::sources::SourceKind;

    fn fixture(values: &[&str]) -> (IssuerProfile, Vec<KeyedCell>, Vec<Publication>) {
        let mut profile = fmine_public::synthetic::synthetic_profile();
        profile.issuer = "example".to_string();
        profile.retrieved_utc = "2030-09-01T00:00:00Z".to_string();
        profile.sources.clear();
        let mut cells = Vec::new();
        let mut dates = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let file = format!("Quarter{}.xlsx", i + 1);
            let hash = Digest32([u8::try_from(i + 1).expect("small fixture"); 32]);
            let release = format!("2030Q{}", i + 1);
            profile.sources.push(RegisteredSource {
                file_name: file.clone(),
                kind: SourceKind::MineStats,
                quarter: release.clone(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "W1 naming rule".to_string(),
                bytes: 1,
                sha256_hex: hash.to_hex(),
            });
            dates.push(Publication {
                file: file.clone(),
                sha256: hash.to_hex(),
                published_at: if i == 0 { "2030-05-01" } else { "2030-08-01" }.to_string(),
                published_at_basis: "FACT: synthetic publication calendar".to_string(),
            });
            let key = PublicKey {
                issuer: profile.issuer.clone(),
                subject: "Site A".to_string(),
                valid_period: Period {
                    year: 2030,
                    kind: PeriodKind::Quarter(1),
                },
                reporting_vintage: Vintage {
                    release,
                    file_sha256: hash.to_hex(),
                },
                ownership_basis: OwnershipBasis::Unprinted,
                scope: Scope::Site,
                unit: "usd_per_oz".to_string(),
                line: LineKey {
                    label: "summary:aisc_usd_per_oz".to_string(),
                    occurrence: 0,
                },
            };
            let value = PublicValue::Number {
                stored: (*value).to_string(),
                displayed: Ok((*value).to_string()),
                under_status: None,
            };
            let tag = canonical_value_tag(&key, &value);
            let claim = Claim::new(
                &key.line.label,
                value,
                &tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::IntensiveWeighted,
                EvidenceClass::ISSUER,
                EpistemicStatus::KnownKnown,
                KnowledgeState::Known,
                Confidentiality::public(),
                vec![],
                vec![],
                vec![SourceAnchor::new(&file, hash.0, "QTD!D5")],
            );
            cells.push(KeyedCell {
                locator: format!("{file}:QTD!D5"),
                key,
                claim,
                printed_basis: OwnershipBasis::Unprinted,
                basis_evidence: None,
                status_evidence: None,
                label_as_printed: None,
                unit_evidence: None,
            });
        }
        (profile, cells, dates)
    }

    fn draft(c: &KeyedCell, amount: &str, basis: &str) -> String {
        format!(
            "Site A Q1 2030 {basis} AISC per ounce sold is [{amount}](fmine:{}).",
            c.claim.claim_id
        )
    }

    fn evaluate(
        text: &str,
        cells: &[KeyedCell],
        profile: &IssuerProfile,
        dates: &[Publication],
        cut: &str,
    ) -> PublicnessReport {
        assess(text, &lint_cited(text, cells), cells, profile, dates, cut).expect("assessment")
    }

    #[test]
    fn release_cut_excludes_future_bytes_then_admits_exact_cited_publication() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let before = evaluate(&text, &cells, &profile, &dates, "2030-04-30");
        assert_eq!(before.findings[0].status, "not_in_public_record");
        assert_eq!(before.findings[0].evidence[0].admission, "after_cut");
        let after = evaluate(&text, &cells, &profile, &dates, "2030-05-01");
        assert_eq!(after.findings[0].status, "disclosed");
        assert!(after.to_json().contains(&dates[0].sha256));
        assert!(
            after
                .to_json()
                .contains("FACT: synthetic publication calendar")
        );
        assert!(after.to_markdown().contains("Quarter1.xlsx:QTD!D5"));
        assert_ne!(before.digest(), after.digest());
    }

    #[test]
    fn later_citation_can_resolve_to_earlier_matching_publication_without_overwriting_vintages() {
        let (profile, cells, dates) = fixture(&["1250", "1250"]);
        let text = draft(&cells[1], "$1,250/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert!(
            report.findings[0]
                .evidence
                .iter()
                .any(|e| e.admission == "after_cut")
        );
        let mut reversed = cells.clone();
        reversed.reverse();
        assert_eq!(
            report.to_json(),
            evaluate(&text, &reversed, &profile, &dates, "2030-06-01").to_json()
        );
    }

    #[test]
    fn contradictory_and_other_basis_assertions_never_become_disclosed_equalities() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,500/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "contradicts_disclosed");
        let other = draft(&cells[0], "$1,250/oz", "attributable");
        let report = evaluate(&other, &cells, &profile, &dates, "2030-06-01");
        assert_eq!(report.findings[0].status, "disclosed_other_basis");
        let unsupported = other.replace("Site A", "Another Mine");
        assert_eq!(
            evaluate(&unsupported, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "not_assessed"
        );
    }

    #[test]
    fn retrieval_dates_and_invented_precision_are_not_publication_evidence() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let report = evaluate(&text, &cells, &profile, &[], "2030-10-01");
        assert_eq!(report.findings[0].status, "publication_date_unknown");
        assert_eq!(
            report.findings[0].evidence[0].known_available_by.as_deref(),
            Some("2030-09-01")
        );
        assert!(report.findings[0].first_registered_disclosure.is_none());
        let precise = draft(&cells[0], "$1,250.00/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&precise, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "precision_not_established"
        );
    }

    #[test]
    fn discovery_preserves_ambiguous_equalities_but_identifies_dated_public_appearances() {
        let (profile, cells, dates) = fixture(&["1250", "1250"]);
        let text = "Site A Q1 2030 ownership basis unprinted AISC per ounce sold is $1,250/oz.";
        let lint = fmine_public::lint::lint_discover(text, &cells);
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
        let report =
            assess(text, &lint, &cells, &profile, &dates, "2030-06-01").expect("dated appearances");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
    }

    fn with_share(cell: &mut KeyedCell, num: i64, den: i64) {
        cell.key.ownership_basis = OwnershipBasis::Attributable {
            num,
            den,
            source: fmine_public::key::ShareSource::Printed,
        };
        cell.printed_basis = cell.key.ownership_basis;
        let old = &cell.claim;
        cell.claim = Claim::new(
            &old.metric_key,
            old.value.clone(),
            &canonical_value_tag(&cell.key, &old.value),
            old.representation.clone(),
            old.support,
            old.additivity,
            old.evidence,
            old.epistemic,
            old.state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
    }

    #[test]
    fn undisclosed_share_ambiguity_survives_singleton_value_probes() {
        for other_amount in ["1250", "1500"] {
            let (profile, mut cells, dates) = fixture(&["1250", other_amount]);
            with_share(&mut cells[0], 1, 2);
            with_share(&mut cells[1], 4, 5);
            let text = "Site A Q1 2030 attributable AISC per ounce sold is $1,250/oz.";
            let lint = fmine_public::lint::lint_discover(text, &cells);
            assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
            assert_eq!(
                lint.findings[0].reason,
                "multiple_claims_matching_stated_keys"
            );
            let report = assess(text, &lint, &cells, &profile, &dates, "2030-09-01")
                .expect("ambiguous shares");
            assert_eq!(report.findings[0].status, "not_assessed");
            assert_eq!(report.findings[0].reason, "attributable_share_unspecified");
            assert_eq!(report.findings[0].evidence.len(), 2);
            assert!(report.findings[0].first_registered_disclosure.is_none());
            let cited = draft(&cells[0], "$1,250/oz", "attributable");
            assert_eq!(
                evaluate(&cited, &cells, &profile, &dates, "2030-09-01").findings[0].reason,
                "attributable_share_unspecified"
            );
        }
    }

    #[test]
    fn equivalent_shares_keep_the_earliest_matching_publication() {
        let (profile, mut cells, dates) = fixture(&["1250", "1250"]);
        with_share(&mut cells[0], 1, 2);
        with_share(&mut cells[1], 2, 4);
        let text = "Site A Q1 2030 attributable AISC per ounce sold is $1,250/oz.";
        let lint = fmine_public::lint::lint_discover(text, &cells);
        assert_eq!(lint.findings[0].verdict, Verdict::Ambiguous);
        let report =
            assess(text, &lint, &cells, &profile, &dates, "2030-09-01").expect("equivalent shares");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
        let cited = draft(&cells[1], "$1,250/oz", "attributable");
        let report = evaluate(&cited, &cells, &profile, &dates, "2030-09-01");
        assert_eq!(report.findings[0].status, "disclosed");
        assert_eq!(
            report.findings[0].first_registered_disclosure,
            Some(cells[0].claim.claim_id.to_string())
        );
    }

    #[test]
    fn a_different_claim_snapshot_cannot_reuse_an_equality_report() {
        let (profile, cells, dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        let lint = lint_cited(&text, &cells);
        assert_eq!(
            assess(&text, &lint, &[], &profile, &dates, "2030-06-01")
                .expect_err("snapshot mismatch")
                .code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn mismatched_publication_digest_and_changed_draft_refuse() {
        let (profile, cells, mut dates) = fixture(&["1250"]);
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        dates[0].sha256 = Digest32([99; 32]).to_hex();
        assert!(
            assess(
                &text,
                &lint_cited(&text, &cells),
                &cells,
                &profile,
                &dates,
                "2030-06-01"
            )
            .is_err()
        );
        assert!(
            assess(
                "changed",
                &lint_cited(&text, &cells),
                &cells,
                &profile,
                &[],
                "2030-06-01"
            )
            .is_err()
        );
        dates[0].sha256 = profile.sources[0].sha256_hex.clone();
        let original = evaluate(&text, &cells, &profile, &dates, "2030-06-01");
        dates[0].published_at_basis = "INFERENCE: explicitly corrected calendar basis".to_string();
        assert_ne!(
            original.digest(),
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").digest()
        );
    }

    #[test]
    fn altered_or_unknown_source_claim_cannot_gain_public_evidence_status() {
        let (profile, mut cells, dates) = fixture(&["1250"]);
        cells[0].claim.state = KnowledgeState::Unknown;
        // Preserve a valid Unknown Claim identity, rather than relying only on a stale digest to reject it.
        let old = &cells[0].claim;
        cells[0].claim = Claim::new(
            &old.metric_key,
            old.value.clone(),
            old.value_tag(),
            old.representation.clone(),
            old.support,
            old.additivity,
            EvidenceClass::Assumed,
            old.epistemic,
            old.state,
            old.confidentiality.clone(),
            old.uu_terms.clone(),
            old.parents.clone(),
            old.anchors.clone(),
        );
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "evidence_unusable"
        );
        let (profile, mut cells, dates) = fixture(&["1250"]);
        cells[0].locator = "Different.xlsx:QTD!D5".to_string();
        let text = draft(&cells[0], "$1,250/oz", "ownership basis unprinted");
        assert_eq!(
            evaluate(&text, &cells, &profile, &dates, "2030-06-01").findings[0].status,
            "evidence_unusable"
        );
    }
}
