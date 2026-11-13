//! DESIGN (BD-345.S4): printed gold and copper cost numerators divided by explicitly based metal sales.
//! Declared identity bases never replace the original operands' printed and registered basis evidence.

use super::*;
use fmine_claim::Claim;
use fmine_metrics::public_derivation::{CostRateUnit, derive_cost_rate};
use fmine_numerics::exact_feas::Interval;
use fmine_public::aisc::IdentityFamily;

#[derive(Debug, Clone, Copy)]
struct RateKind {
    unit: CostRateUnit,
    family: &'static str,
    numerator: &'static str,
    rate_label: &'static str,
    basis: &'static str,
}

impl RateKind {
    const ALL: [Self; 5] = [
        Self {
            unit: CostRateUnit::GoldKoz,
            family: "aisc_per_oz",
            numerator: "all-in sustaining costs",
            rate_label: "all-in sustaining costs per ounce",
            basis: IdentityFamily::AiscPerOunce.basis(),
        },
        Self {
            unit: CostRateUnit::GoldKoz,
            family: "tcc_per_oz",
            numerator: "total cash costs",
            rate_label: "total cash costs per ounce",
            basis: IdentityFamily::TccPerOunce.basis(),
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "aisc_per_lb",
            numerator: "site:costs ($ millions):all-in sustaining costs",
            rate_label: "site:costs per pound:all-in sustaining costs",
            basis: "AISC US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "c1_per_lb",
            numerator: "site:costs ($ millions):c1 cash costs",
            rate_label: "site:costs per pound:c1 cash costs",
            basis: "C1 cash costs US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
        Self {
            unit: CostRateUnit::CopperMillionPounds,
            family: "cos_per_lb",
            numerator: "site:costs ($ millions):cost of sales",
            rate_label: "site:costs per pound:cost of sales",
            basis: "Cost of sales US$M over copper sales in millions of pounds, both on the same explicitly printed ownership basis",
        },
    ];

    const fn denominator(self) -> &'static str {
        match self.unit {
            CostRateUnit::GoldKoz => "ounces sold",
            CostRateUnit::CopperMillionPounds => "site::sales (millions of pounds)",
        }
    }

    fn accepts_basis(self, cell: &KeyedCell) -> bool {
        explicit_basis(cell.key.ownership_basis).is_some()
            && (self.unit == CostRateUnit::CopperMillionPounds
                || matches!(
                    cell.key.ownership_basis,
                    OwnershipBasis::Attributable { .. }
                ))
    }
}

#[derive(Debug, Clone)]
pub(super) struct Assessment {
    pub(super) status: &'static str,
    reason: &'static str,
    kind: RateKind,
    pub(super) evidence: Vec<SourceEvidence>,
    draft: Option<Interval>,
    derived: Option<Claim<Interval>>,
    refusal: Option<Refusal>,
}

fn column(c: &KeyedCell) -> Option<(&str, u32)> {
    let (sheet, cell) = c.locator.rsplit_once('!')?;
    Some((sheet, fmine_io::xlsx::parse_a1_ref(cell)?.1))
}

pub(super) fn assess(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
) -> Option<Assessment> {
    let (prototype, target, kind) = cells.iter().find_map(|sold| {
        RateKind::ALL.into_iter().find_map(|kind| {
            if sold.key.line.label != kind.denominator()
                || sold.key.unit != kind.unit.denominator_unit()
                || !kind.accepts_basis(sold)
            {
                return None;
            }
            // A grammar prototype changes only metric/unit; it supplies neither a number nor a printed share.
            let mut p = sold.clone();
            p.key.line.label = kind.rate_label.into();
            p.key.unit = kind.unit.rate_unit().into();
            Some((sold, equality_target(draft, f.span, &p)?, kind))
        })
    })?;
    let mut evidence: Vec<_> = cells
        .iter()
        .filter(|c| {
            c.key.issuer == prototype.key.issuer
                && c.key.subject == prototype.key.subject
                && c.key.valid_period == target.period
                && (c.key.line.label == kind.numerator || c.key.line.label == kind.denominator())
        })
        .map(|c| source_evidence(c, profile, publications, as_of))
        .collect();
    evidence.sort_by_cached_key(|e| (e.source.claim.claim_id, e.to_json()));
    evidence.dedup_by(|a, b| a.source == b.source);
    let mut out = Assessment {
        status: "not_available",
        reason: "unique_dated_same_source_cost_and_explicit_sales_basis_required",
        kind,
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
            .partition(|e| e.source.key.line.label == kind.numerator);
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
            || q.key.unit != kind.unit.denominator_unit()
            || n.key.reporting_vintage != q.key.reporting_vintage
            || !column(n).is_some_and(|col| Some(col) == column(q))
            || n.key.scope != q.key.scope
            || n.key.line.occurrence != q.key.line.occurrence
            || !kind.accepts_basis(q)
            || (n.key.ownership_basis != OwnershipBasis::Unprinted
                && explicit_basis(n.key.ownership_basis) != explicit_basis(q.key.ownership_basis))
            || (kind.unit == CostRateUnit::CopperMillionPounds
                && ([n, q]
                    .iter()
                    .any(|c| c.printed_basis == OwnershipBasis::Unprinted)
                    || explicit_basis(n.key.ownership_basis)
                        != explicit_basis(q.key.ownership_basis)))
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
                "cost-rate operands must share source table, column, period, perimeter and an admissible explicit ownership basis",
            ));
        }
        let derived = derive_cost_rate(
            kind.unit,
            kind.family,
            (&n.claim, printed(n)),
            (&q.claim, printed(q)),
        )?;
        out.status = sums::compare(interval, derived.value)?;
        out.reason = match out.status {
            "derived" => "same_source_cost_over_sales_supports_the_draft_precision",
            "contradicted" => "cost_rate_enclosure_is_disjoint_from_the_draft",
            _ => "cost_and_sales_rounding_exceeds_the_draft_precision",
        };
        out.derived = Some(derived);
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
            |c| {
                canonical_json(
                    c,
                    &format!("derived_cost_rate_{}", self.kind.unit.rate_unit()),
                    sums::interval(c.value),
                )
            },
        );
        format!(
            "{{\"rule\":{},\"family\":{},\"identity_basis\":{},\"formula\":{},\"unit\":{},\"construction\":\"each printed operand plus or minus half its own displayed step\",\"status\":{},\"reason\":{},\"warning\":{},\"draft_interval\":{},\"derived_claim\":{},\"evidence\":[{}],\"refusal\":{}}}",
            js(self.kind.unit.rule()),
            js(self.kind.family),
            js(self.kind.basis),
            js(self.kind.unit.formula()),
            js(self.kind.unit.display_unit()),
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
            "  - Cost rate `{}` ({}), {} in {}: **{}**; {}. {}. {}\n",
            self.kind.unit.rule(),
            self.kind.family,
            self.kind.unit.formula(),
            self.kind.unit.display_unit(),
            self.status,
            self.reason,
            self.kind.basis,
            fmine_metrics::public_derivation::WARNING
        );
        if let Some(c) = &self.derived {
            sums::derived_markdown(&mut out, c.claim_id, c.value);
        }
        out
    }
}
