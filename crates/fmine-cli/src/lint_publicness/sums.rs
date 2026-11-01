//! DESIGN (BD-345.S4): complete quarter sums with explicit source publication admission.
//! Duplicate admissible quarters stay ambiguous; neither a value nor a preferred vintage selects one.

use super::*;
use fmine_claim::Claim;
use fmine_metrics::guidance::{Ownership, Period as CalendarPeriod};
use fmine_metrics::public_derivation::{self as kernel, Amount, AmountKind, Rule};
use fmine_numerics::exact_feas::Interval;

#[derive(Debug, Clone)]
pub(super) struct Assessment {
    pub(super) status: &'static str,
    reason: &'static str,
    draft: Option<Interval>,
    pub(super) evidence: Vec<SourceEvidence>,
    inputs: Vec<Claim<Amount>>,
    derived: Option<Claim<Amount>>,
    refusal: Option<Refusal>,
}

fn interpret(c: &KeyedCell, kind: Quantity) -> Result<Claim<Amount>, Refusal> {
    let bad = |why| Refusal::new(RefusalCode::SourceCellUnusable208, why);
    let basis = explicit_basis(c.key.ownership_basis);
    if basis.is_none()
        || c.claim.additivity != AdditivityClass::ExtensiveAdditive
        || matches!(&c.key.scope, Scope::Region { map_version } if map_version.is_empty() || map_version.starts_with("unregistered:"))
        || (c.basis_evidence.is_some()
            && c.printed_basis != OwnershipBasis::Unprinted
            && explicit_basis(c.printed_basis) != basis)
    {
        return Err(bad(
            "quarter sum requires explicit compatible extensive basis and scope",
        ));
    }
    let ownership = match c.key.ownership_basis {
        OwnershipBasis::Consolidated100 => Ownership::Full,
        OwnershipBasis::Attributable { num, den, .. } => Ownership::Attributable {
            share: Rat::new(i128::from(num), i128::from(den))
                .map_err(|_| bad("invalid quarter ownership fraction"))?,
        },
        OwnershipBasis::Unprinted => return Err(bad("quarter ownership is unprinted")),
    };
    let ((year, first_month), (_, last_month)) = c.key.valid_period.months();
    let value = Amount {
        issuer: c.key.issuer.clone(),
        subject: c.key.subject.clone(),
        scope: format!("{:?};occurrence={}", c.key.scope, c.key.line.occurrence),
        kind: match kind {
            Quantity::GoldProductionKoz => AmountKind::GoldProductionKoz,
            Quantity::CopperProductionKt => AmountKind::CopperProductionKt,
            Quantity::GoldSalesKoz => AmountKind::GoldSalesKoz,
        },
        ownership,
        period: CalendarPeriod {
            year,
            first_month,
            last_month,
        },
        interval: printed(c)
            .ok_or_else(|| bad("quarter has no usable printed precision"))?
            .interval()
            .map_err(|_| bad("quarter rounding interval overflow"))?,
    };
    let tag = value.canonical_tag();
    Ok(Claim::new(
        kernel::INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        c.claim.evidence.meet(EvidenceClass::Derived),
        c.claim.epistemic,
        c.claim.state,
        c.claim.confidentiality.clone(),
        c.claim.uu_terms.clone(),
        vec![c.claim.claim_id],
        c.claim.anchors.clone(),
    ))
}

/// Grammar prototypes identify semantic series only; their amounts never select an operand.
pub(super) fn assess(
    draft: &str,
    f: &LintFinding,
    cells: &[KeyedCell],
    profile: &IssuerProfile,
    publications: &BTreeMap<(String, String), &Publication>,
    as_of: &str,
    retrieved: Option<&str>,
) -> Option<Assessment> {
    let prototypes: Vec<_> = cells
        .iter()
        .filter_map(|c| {
            let kind = quantity(c)?;
            let target = equality_target(draft, f.span, c)?;
            (matches!(target.period.kind, PeriodKind::H1 | PeriodKind::FullYear)
                && c.key.valid_period.year == target.period.year
                && c.key.ownership_basis != OwnershipBasis::Unprinted)
                .then_some((c, target, kind))
        })
        .collect();
    let (_, target, kind) = *prototypes.first()?;
    let quarters = if target.period.kind == PeriodKind::H1 {
        2
    } else {
        4
    };
    let mut evidence: Vec<_> = cells.iter().filter(|c| {
        quantity(c) == Some(kind) && c.key.valid_period.year == target.period.year
            && matches!(c.key.valid_period.kind, PeriodKind::Quarter(q) if (1..=quarters).contains(&q))
            && prototypes.iter().any(|(p, _, _)| p.key.issuer == c.key.issuer
                && p.key.subject == c.key.subject
                && std::mem::discriminant(&p.key.ownership_basis) == std::mem::discriminant(&c.key.ownership_basis))
    }).map(|c| source_evidence(c, profile, publications, as_of, retrieved)).collect();
    sort_evidence(&mut evidence);
    let mut out = Assessment {
        status: "not_available",
        reason: "complete_dated_quarter_population_required",
        draft: None,
        evidence,
        inputs: Vec::new(),
        derived: None,
        refusal: None,
    };
    let run = (|| -> Result<(), Refusal> {
        let draft = Printed {
            value: decimal(target.value).ok_or_else(|| arithmetic("draft value overflow"))?,
            step: decimal(target.step).ok_or_else(|| arithmetic("draft precision overflow"))?,
        }
        .interval()
        .map_err(|_| arithmetic("draft interval overflow"))?;
        out.draft = Some(draft);
        let mut counts = BTreeMap::new();
        let mut unavailable = false;
        for e in &mut out.evidence {
            if e.published_at.as_deref().is_some_and(|date| date > as_of) {
                continue;
            }
            if e.admission == "unusable" {
                e.relation = "unusable_quarter_operand";
                unavailable = true;
                continue;
            }
            if e.admission != "published_by_cut" {
                continue;
            }
            let input = interpret(&e.source, kind)?;
            e.relation = "quarter_operand";
            *counts
                .entry(e.source.key.valid_period.kind)
                .or_insert(0_usize) += 1;
            out.inputs.push(input);
        }
        if counts.values().any(|count| *count > 1) {
            out.status = "ambiguous";
            out.reason = "competing_quarter_assertions_are_not_selected_by_amount";
            return Ok(());
        }
        if unavailable || (1..=quarters).any(|q| counts.get(&PeriodKind::Quarter(q)) != Some(&1)) {
            return Ok(());
        }
        let derived = kernel::derive(Rule::QuarterSum, &out.inputs)?;
        let source = derived.value.interval;
        let cmp = |a: Rat, b: Rat| {
            a.checked_cmp(b)
                .map_err(|_| arithmetic("interval comparison overflow"))
        };
        (out.status, out.reason) =
            if cmp(draft.lo, source.lo)?.is_le() && cmp(draft.hi, source.hi)?.is_ge() {
                (
                    "derived",
                    "complete_quarter_sum_supports_the_draft_precision",
                )
            } else if cmp(draft.hi, source.lo)?.is_lt() || cmp(draft.lo, source.hi)?.is_gt() {
                (
                    "contradicted",
                    "complete_quarter_sum_is_disjoint_from_the_draft",
                )
            } else {
                (
                    "precision_not_established",
                    "quarter_rounding_uncertainty_exceeds_the_draft_precision",
                )
            };
        out.derived = Some(derived);
        Ok(())
    })();
    if let Err(refusal) = run {
        out.status = "refused";
        out.reason = "source_interpretation_or_exact_quarter_sum_failed";
        out.refusal = Some(refusal);
    }
    Some(out)
}

fn arithmetic(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

fn interval(i: Interval) -> String {
    format!(
        "{{\"lo\":{{\"num\":\"{}\",\"den\":\"{}\"}},\"hi\":{{\"num\":\"{}\",\"den\":\"{}\"}},\"lo_strict\":false,\"hi_strict\":false}}",
        i.lo.num(),
        i.lo.den(),
        i.hi.num(),
        i.hi.den()
    )
}

fn claim(c: &Claim<Amount>) -> String {
    let mut body = String::new();
    for byte in c.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let a = &c.value;
    let basis = match a.ownership {
        Ownership::Full => "100%".into(),
        Ownership::Attributable { share } => {
            format!("attributable:{}/{}", share.num(), share.den())
        }
    };
    format!(
        "{{\"claim_id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"evidence\":{},\"knowledge\":{},\"public\":{},\"issuer\":{},\"subject\":{},\"scope\":{},\"kind\":{},\"basis\":{},\"year\":{},\"first_month\":{},\"last_month\":{},\"interval\":{},\"parents\":[{}]}}",
        js(&c.claim_id.to_string()),
        js(&c.metric_key),
        js(c.value_tag()),
        js(&body),
        js(&format!("{:?}", c.evidence)),
        js(&format!("{:?}", c.state)),
        c.confidentiality.is_public(),
        js(&a.issuer),
        js(&a.subject),
        js(&a.scope),
        js(&format!("{:?}", a.kind)),
        js(&basis),
        a.period.year,
        a.period.first_month,
        a.period.last_month,
        interval(a.interval),
        c.parents
            .iter()
            .map(|id| js(&id.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

impl Assessment {
    pub(super) fn to_json(&self) -> String {
        let refusal = self.refusal.as_ref().map_or_else(
            || "null".into(),
            |r| {
                format!(
                    "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                    js(r.code),
                    js(&r.what),
                    js(r.fix)
                )
            },
        );
        format!(
            "{{\"rule\":{},\"status\":{},\"reason\":{},\"warning\":{},\"draft_interval\":{},\"inputs\":[{}],\"derived_claim\":{},\"evidence\":[{}],\"refusal\":{}}}",
            js(Rule::QuarterSum.id()),
            js(self.status),
            js(self.reason),
            js(kernel::WARNING),
            self.draft.map_or_else(|| "null".into(), interval),
            self.inputs.iter().map(claim).collect::<Vec<_>>().join(","),
            self.derived.as_ref().map_or_else(|| "null".into(), claim),
            self.evidence
                .iter()
                .map(SourceEvidence::to_json)
                .collect::<Vec<_>>()
                .join(","),
            refusal
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "  - Quarter sum `{}`: **{}**; {}. {}\n",
            Rule::QuarterSum.id(),
            self.status,
            self.reason,
            kernel::WARNING
        );
        if let Some(c) = &self.derived {
            let i = c.value.interval;
            let _ = writeln!(
                out,
                "    - Derived Claim `{}`: [{}/{}, {}/{}].",
                c.claim_id,
                i.lo.num(),
                i.lo.den(),
                i.hi.num(),
                i.hi.den()
            );
        }
        for e in &self.evidence {
            let at = e.source.locator.replace(['\n', '\r', '`'], " ");
            let _ = writeln!(
                out,
                "    - `{at}`, sha256 `{}`, published {}, admission {}; {}.",
                e.source.key.reporting_vintage.file_sha256,
                e.published_at.as_deref().unwrap_or("unknown"),
                e.admission,
                e.relation
            );
        }
        out
    }
}
