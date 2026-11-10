//! DESIGN (BD-345.S4): 1000 times a printed gold cost numerator divided by attributable ounces sold.
//! Declared identity bases never replace the original operands' printed and registered basis evidence.

use super::*;
use fmine_claim::Claim;
use fmine_numerics::exact_feas::Interval;
use fmine_public::aisc::{IdentityFamily, rate_range};

const RULE: &str = "public-disclosure.cost-rate/1";

#[derive(Debug, Clone)]
pub(super) struct Assessment {
    pub(super) status: &'static str,
    reason: &'static str,
    family: IdentityFamily,
    pub(super) evidence: Vec<SourceEvidence>,
    draft: Option<Interval>,
    derived: Option<Claim<Interval>>,
    refusal: Option<Refusal>,
}

fn column(c: &KeyedCell) -> Option<(&str, u32)> {
    let (sheet, cell) = c.locator.rsplit_once('!')?;
    Some((sheet, fmine_io::xlsx::parse_a1_ref(cell)?.1))
}

fn whole(c: &KeyedCell) -> Result<i64, Refusal> {
    printed(c)
        .filter(|p| p.step == Rat::from_integer(1) && p.value.den() == 1)
        .and_then(|p| i64::try_from(p.value.num()).ok())
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::SourceCellUnusable208,
                "cost-rate operands require whole-number displayed US$M and koz with step one",
            )
        })
}

pub(super) fn assess(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
) -> Option<Assessment> {
    let (prototype, target, numerator, family) = cells.iter().find_map(|sold| {
        if sold.key.line.label != "ounces sold"
            || sold.key.unit != "koz"
            || !matches!(
                sold.key.ownership_basis,
                OwnershipBasis::Attributable { .. }
            )
        {
            return None;
        }
        [
            ("all-in sustaining costs", IdentityFamily::AiscPerOunce),
            ("total cash costs", IdentityFamily::TccPerOunce),
        ]
        .into_iter()
        .find_map(|(numerator, family)| {
            // A grammar prototype changes only metric/unit; it supplies neither a number nor a printed share.
            let mut p = sold.clone();
            p.key.line.label = format!("{numerator} per ounce");
            p.key.unit = "usd_per_oz".into();
            Some((sold, equality_target(draft, f.span, &p)?, numerator, family))
        })
    })?;
    let mut evidence: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.issuer == prototype.key.issuer
                && c.key.subject == prototype.key.subject
                && c.key.valid_period == target.period
                && (c.key.line.label == numerator || c.key.line.label == "ounces sold")
        })
        .map(|c| source_evidence(c, profile, publications, as_of))
        .collect();
    evidence.sort_by_cached_key(|e| (e.source.claim.claim_id, e.to_json()));
    evidence.dedup_by(|a, b| a.source == b.source);
    let mut out = Assessment {
        status: "not_available",
        reason: "unique_dated_same_source_cost_and_attributable_sales_required",
        family,
        evidence,
        draft: None,
        derived: None,
        refusal: None,
    };
    let run = (|| -> Result<(), Refusal> {
        let interval = sums::draft_interval(target)?;
        out.draft = Some(interval);
        let (mut numerators, mut denominators): (Vec<_>, Vec<_>) = out
            .evidence
            .iter_mut()
            .filter(|e| e.admission != "after_cut")
            .partition(|e| e.source.key.line.label == numerator);
        if numerators.len() > 1 || denominators.len() > 1 {
            out.status = "ambiguous";
            out.reason = "competing_cost_or_sales_assertions_are_not_selected_by_amount";
            return Ok(());
        }
        let ([n], [q]) = (numerators.as_mut_slice(), denominators.as_mut_slice()) else {
            return Ok(());
        };
        if n.admission != "published_by_cut" || q.admission != "published_by_cut" {
            return Ok(());
        }
        n.relation = "cost_numerator";
        q.relation = "sold_denominator";
        let (n, q) = (&n.source, &q.source);
        if n.key.unit != "usd_m"
            || q.key.unit != "koz"
            || n.key.reporting_vintage != q.key.reporting_vintage
            || !column(n).is_some_and(|col| Some(col) == column(q))
            || n.key.scope != q.key.scope
            || n.key.line.occurrence != q.key.line.occurrence
            || explicit_basis(q.key.ownership_basis).is_none()
            || !matches!(q.key.ownership_basis, OwnershipBasis::Attributable { .. })
            || (n.key.ownership_basis != OwnershipBasis::Unprinted
                && n.key.ownership_basis != q.key.ownership_basis)
            || [n, q].iter().any(|c| {
                c.claim.additivity != AdditivityClass::ExtensiveAdditive
                    || (c.printed_basis != OwnershipBasis::Unprinted
                        && explicit_basis(c.printed_basis) != explicit_basis(c.key.ownership_basis))
                    || matches!(&c.key.scope, Scope::Region { map_version }
                    if map_version.is_empty() || map_version.starts_with("unregistered:"))
            })
        {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                "cost-rate operands must share source table, column, period, perimeter and declared attributable basis",
            ));
        }
        let denominator = whole(q)?;
        if denominator < 1 {
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                "the sold-ounce rounding enclosure must be strictly positive",
            ));
        }
        let bounds = rate_range(whole(n)?, 1, denominator)
            .map_err(|e| Refusal::new(RefusalCode::NumericalCertificateFailed207, e.0))?;
        out.status = sums::compare(interval, bounds)?;
        out.reason = match out.status {
            "derived" => "same_source_cost_over_sales_supports_the_draft_precision",
            "contradicted" => "cost_rate_enclosure_is_disjoint_from_the_draft",
            _ => "cost_and_sales_rounding_exceeds_the_draft_precision",
        };
        let mut terms = n.claim.uu_terms.clone();
        for term in &q.claim.uu_terms {
            if !terms.contains(term) {
                terms.push(term.clone());
            }
        }
        let mut anchors = n.claim.anchors.clone();
        anchors.extend(q.claim.anchors.iter().cloned());
        anchors.sort();
        anchors.dedup();
        let tag = format!("{}:{}", family.as_str(), sums::interval(bounds));
        out.derived = Some(Claim::new(
            RULE,
            bounds,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            n.claim
                .evidence
                .meet(q.claim.evidence)
                .meet(EvidenceClass::Derived),
            n.claim.epistemic.and(q.claim.epistemic),
            n.claim.state.and(q.claim.state),
            n.claim.confidentiality.join(&q.claim.confidentiality),
            terms,
            vec![n.claim.claim_id, q.claim.claim_id],
            anchors,
        ));
        Ok(())
    })();
    if let Err(refusal) = run {
        out.status = "refused";
        out.reason = "source_interpretation_or_exact_cost_rate_failed";
        out.refusal = Some(refusal);
    }
    Some(out)
}

impl Assessment {
    pub(super) fn to_json(&self) -> String {
        let derived = self.derived.as_ref().map_or_else(
            || "null".into(),
            |c| canonical_json(c, "derived_cost_rate_usd_per_oz", sums::interval(c.value)),
        );
        format!(
            "{{\"rule\":{},\"family\":{},\"identity_basis\":{},\"formula\":\"1000*N/Q\",\"unit\":\"USD/oz\",\"construction\":\"printed numerator and denominator, each plus or minus one half\",\"status\":{},\"reason\":{},\"warning\":{},\"draft_interval\":{},\"derived_claim\":{},\"evidence\":[{}],\"refusal\":{}}}",
            js(RULE),
            js(self.family.as_str()),
            js(self.family.basis()),
            js(self.status),
            js(self.reason),
            js(fmine_metrics::public_derivation::WARNING),
            self.draft.map_or_else(|| "null".into(), sums::interval),
            derived,
            evidence_json(&self.evidence),
            self.refusal.as_ref().map_or_else(
                || "null".into(),
                |r| format!("{{\"code\":{},\"what\":{}}}", js(r.code), js(&r.what))
            )
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "  - Cost rate `{RULE}` ({}), 1000*N/Q in USD/oz: **{}**; {}. {}. {}\n",
            self.family.as_str(),
            self.status,
            self.reason,
            self.family.basis(),
            fmine_metrics::public_derivation::WARNING
        );
        if let Some(c) = &self.derived {
            sums::derived_markdown(&mut out, c.claim_id, c.value);
        }
        out
    }
}
