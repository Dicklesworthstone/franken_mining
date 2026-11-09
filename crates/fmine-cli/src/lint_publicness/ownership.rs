//! DESIGN (BD-345.S4): ownership is admitted from the same dated source and physical period as the amount.
//! All competing observations stay visible; draft agreement never selects an amount or ownership share.

use super::*;
use fmine_claim::Claim;
use fmine_metrics::public_derivation::{self as kernel, Amount, OwnershipShare};
use fmine_public::key::ShareSource;

#[derive(Debug, Clone)]
pub(super) struct Assessment {
    pub(super) status: &'static str,
    reason: &'static str,
    pub(super) evidence: Vec<SourceEvidence>,
    amount: Option<Claim<Amount>>,
    shares: Vec<Claim<OwnershipShare>>,
    derived: Option<Claim<Amount>>,
    refusal: Option<Refusal>,
}

pub(super) fn assess(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
    retrieved: Option<&str>,
) -> Option<Assessment> {
    let (prototype, target, kind) = cells.iter().find_map(|c| {
        let kind = quantity(c)?;
        if c.key.ownership_basis != OwnershipBasis::Consolidated100
            || !matches!(c.key.scope, Scope::Site | Scope::JointVenture)
        {
            return None;
        }
        // Only the basis category parses the prose. This prototype is never used as source evidence.
        let mut p = c.clone();
        p.key.ownership_basis = OwnershipBasis::Attributable {
            num: 1,
            den: 1,
            source: ShareSource::Printed,
        };
        p.printed_basis = p.key.ownership_basis;
        p.basis_evidence = None;
        Some((c, equality_target(draft, f.span, &p)?, kind))
    })?;
    let mut evidence: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.issuer == prototype.key.issuer
                && c.key.subject == prototype.key.subject
                && c.key.valid_period == target.period
                && (quantity(c) == Some(kind)
                    || (quantity(c).is_some()
                        && matches!(c.key.ownership_basis, OwnershipBasis::Attributable { .. })))
        })
        .map(|c| source_evidence(c, profile, publications, as_of, retrieved))
        .collect();
    evidence.sort_by_cached_key(|e| (e.source.claim.claim_id, e.to_json()));
    let mut out = Assessment {
        status: "not_available",
        reason: "unique_full_mine_amount_and_same_source_printed_share_required",
        evidence,
        amount: None,
        shares: Vec::new(),
        derived: None,
        refusal: None,
    };
    let run = (|| -> Result<(), Refusal> {
        if out.evidence.windows(2).any(|p| {
            p[0].source.claim.claim_id == p[1].source.claim.claim_id && p[0].source != p[1].source
        }) {
            out.status = "ambiguous";
            out.reason = "same_source_identity_has_conflicting_claim_metadata";
            return Ok(());
        }
        out.evidence.dedup_by(|a, b| a.source == b.source);
        let amounts: Vec<_> = out
            .evidence
            .iter()
            .filter(|e| {
                quantity(&e.source) == Some(kind)
                    && e.source.key.ownership_basis == OwnershipBasis::Consolidated100
                    && e.admission != "after_cut"
            })
            .collect();
        if amounts.len() > 1 {
            out.status = "ambiguous";
            out.reason = "competing_full_mine_assertions_are_not_selected_by_amount";
            return Ok(());
        }
        let [source] = amounts.as_slice() else {
            return Ok(());
        };
        if source.admission != "published_by_cut" {
            return Ok(());
        }
        let source = source.source.clone();
        let amount = sums::interpret(&source, kind)?;
        let mut fractions = BTreeSet::new();
        for e in &mut out.evidence {
            if e.source.claim.claim_id == source.claim.claim_id {
                e.relation = "full_mine_operand";
            }
            let OwnershipBasis::Attributable {
                num,
                den,
                source: origin,
            } = e.source.key.ownership_basis
            else {
                continue;
            };
            if e.admission == "after_cut" {
                continue;
            }
            if e.admission != "published_by_cut" {
                out.reason = "ownership_witness_unusable_or_publication_date_unknown";
                return Ok(());
            }
            fractions.insert((num, den));
            if e.source.key.reporting_vintage != source.key.reporting_vintage {
                e.relation = "other_source_ownership_not_transferred";
                continue;
            }
            if origin != ShareSource::Printed
                || e.source.basis_evidence.is_some()
                || e.source.key.scope != source.key.scope
                || e.source.key.line.occurrence != source.key.line.occurrence
            {
                out.reason = "ownership_requires_uncorrected_printed_share_on_the_same_perimeter";
                return Ok(());
            }
            e.relation = "printed_ownership_witness";
            let value = OwnershipShare {
                issuer: source.key.issuer.clone(),
                subject: source.key.subject.clone(),
                scope: amount.value.scope.clone(),
                period: amount.value.period,
                fraction: Rat::new(i128::from(num), i128::from(den)).map_err(|_| {
                    Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        "invalid printed ownership fraction",
                    )
                })?,
            };
            let tag = value.canonical_tag();
            let c = &e.source.claim;
            out.shares.push(Claim::new(
                kernel::SHARE_METRIC,
                value,
                &tag,
                Representation::Deterministic,
                SpatialSupport::PeriodAggregate,
                AdditivityClass::NonAdditive,
                c.evidence.meet(EvidenceClass::Derived),
                c.epistemic,
                c.state,
                c.confidentiality.clone(),
                c.uu_terms.clone(),
                vec![c.claim_id],
                c.anchors.clone(),
            ));
        }
        out.amount = Some(amount.clone());
        if fractions.len() > 1 {
            out.status = "ambiguous";
            out.reason = "conflicting_printed_ownership_shares";
            return Ok(());
        }
        if out.shares.is_empty() {
            return Ok(());
        }
        let derived = kernel::ownership_scale(&amount, &out.shares)?;
        let bounds = derived.value.interval;
        let bad = || {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "ownership comparison overflow",
            )
        };
        let interval = sums::draft_interval(target).map_err(|_| bad())?;
        out.status = sums::compare(interval, bounds).map_err(|_| bad())?;
        out.reason = match out.status {
            "derived" => "dated_full_mine_amount_and_printed_share_support_the_draft_precision",
            "contradicted" => "ownership_scaled_amount_is_disjoint_from_the_draft",
            _ => "ownership_scaled_rounding_exceeds_the_draft_precision",
        };
        out.derived = Some(derived);
        Ok(())
    })();
    if let Err(refusal) = run {
        out.status = "refused";
        out.reason = "source_interpretation_or_exact_ownership_scaling_failed";
        out.refusal = Some(refusal);
    }
    Some(out)
}

impl Assessment {
    pub(super) fn to_json(&self) -> String {
        let shares = self.shares.iter().map(|s| {
            let value = format!("{{\"issuer\":{},\"subject\":{},\"scope\":{},\"year\":{},\"first_month\":{},\"last_month\":{},\"fraction\":{{\"num\":\"{}\",\"den\":\"{}\"}}}}", js(&s.value.issuer), js(&s.value.subject), js(&s.value.scope), s.value.period.year, s.value.period.first_month, s.value.period.last_month, s.value.fraction.num(), s.value.fraction.den());
            canonical_json(s, "printed_ownership_share", value)
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"rule\":{},\"status\":{},\"reason\":{},\"warning\":{},\"amount_claim\":{},\"share_claims\":[{}],\"derived_claim\":{},\"evidence\":[{}],\"refusal\":{}}}",
            js(kernel::OWNERSHIP_SCALE_RULE),
            js(self.status),
            js(self.reason),
            js(kernel::WARNING),
            self.amount
                .as_ref()
                .map_or_else(|| "null".into(), sums::claim),
            shares,
            self.derived
                .as_ref()
                .map_or_else(|| "null".into(), sums::claim),
            evidence_json(&self.evidence),
            self.refusal.as_ref().map_or_else(
                || "null".into(),
                |r| format!("{{\"code\":{},\"what\":{}}}", js(r.code), js(&r.what))
            )
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "  - Ownership scaling `{}`: **{}**; {}. {}\n",
            kernel::OWNERSHIP_SCALE_RULE,
            self.status,
            self.reason,
            kernel::WARNING
        );
        if let Some(c) = &self.derived {
            sums::derived_markdown(&mut out, c.claim_id, c.value.interval);
        }
        for s in &self.shares {
            let _ = writeln!(
                out,
                "    - Ownership Claim `{}`: {}/{}; source Claims {}.",
                s.claim_id,
                s.value.fraction.num(),
                s.value.fraction.den(),
                s.parents
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        out
    }
}
