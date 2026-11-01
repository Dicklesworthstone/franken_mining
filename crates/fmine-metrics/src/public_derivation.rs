//! Registered additive public-disclosure identities (BD-345.S4).
//! Determinism: Exact
//!
//! DESIGN: closed rational intervals propagate the printed precision of source assertions. They are
//! neither forecasts nor confidence intervals. Only Q1 + Q2 = H1 and Q1 + Q2 + Q3 + Q4 = FY
//! are admitted here. Publication admission belongs to the source adapter; cumulative differences
//! use the existing `roll_forward::quarter` identity.

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};

use crate::guidance::{Ownership, Period};

type Result<T> = std::result::Result<T, Refusal>;
pub const INPUT_METRIC: &str = "public_disclosure.period_amount";
pub const WARNING: &str = "Arithmetic over dated public assertions does not make the derived figure an issuer disclosure. A non-obvious derived figure can itself be new information; obtain appropriate disclosure review.";

/// Extensive quantities only: rates, grades, recovery and per-unit costs cannot enter these identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountKind {
    GoldProductionKoz,
    CopperProductionKt,
    GoldSalesKoz,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount {
    pub issuer: String,
    pub subject: String,
    /// Canonical operation/region scope, including its version when membership changes.
    pub scope: String,
    pub kind: AmountKind,
    pub ownership: Ownership,
    pub period: Period,
    pub interval: Interval,
}

impl Amount {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PDAM", 1);
        enc.put_str(&self.issuer);
        enc.put_str(&self.subject);
        enc.put_str(&self.scope);
        enc.put_u8(match self.kind {
            AmountKind::GoldProductionKoz => 0,
            AmountKind::CopperProductionKt => 1,
            AmountKind::GoldSalesKoz => 2,
        });
        match self.ownership {
            Ownership::Full => enc.put_u8(0),
            Ownership::Attributable { share } => {
                enc.put_u8(1);
                enc.put_i128(share.num());
                enc.put_i128(share.den());
            }
        }
        enc.put_u16(self.period.year);
        enc.put_u8(self.period.first_month);
        enc.put_u8(self.period.last_month);
        for bound in [self.interval.lo, self.interval.hi] {
            enc.put_i128(bound.num());
            enc.put_i128(bound.den());
        }
        enc.put_u8(u8::from(self.interval.lo_strict));
        enc.put_u8(u8::from(self.interval.hi_strict));
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    QuarterSum,
}

impl Rule {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::QuarterSum => "public-disclosure.quarter-sum/1",
        }
    }
}

fn refusal(code: RefusalCode, detail: &str) -> Refusal {
    Refusal::new(code, format!("public disclosure derivation: {detail}"))
}

fn checked<T>(result: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    result.map_err(|_| {
        refusal(
            RefusalCode::NumericalCertificateFailed207,
            "exact interval arithmetic overflow",
        )
    })
}

fn validate(claim: &Claim<Amount>) -> Result<()> {
    claim.check_id()?;
    if claim.metric_key != INPUT_METRIC || claim.value_tag() != claim.value.canonical_tag() {
        return Err(refusal(
            RefusalCode::FrozenRecordMutation507,
            "source interpretation differs from its canonical identity",
        ));
    }
    if !claim.confidentiality.is_public() {
        return Err(refusal(RefusalCode::MnpiWall301, "non-public operand"));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(refusal(RefusalCode::StaleAnchor502, "stale operand"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || claim.calibration_failed
        || claim.representation != Representation::Deterministic
        || claim.support != SpatialSupport::PeriodAggregate
        || claim.anchors.is_empty()
        || claim.anchors.iter().any(|a| {
            a.artifact_path.is_empty() || a.locator.is_empty() || a.artifact_sha256 == [0; 32]
        })
    {
        return Err(refusal(
            RefusalCode::SourceCellUnusable208,
            "operand lacks an admissible deterministic source interpretation",
        ));
    }
    if claim.additivity != AdditivityClass::ExtensiveAdditive {
        return Err(refusal(
            RefusalCode::NonAdditiveAverage103,
            "period addition requires an extensive quantity",
        ));
    }
    let a = &claim.value;
    if a.issuer.trim().is_empty()
        || a.subject.trim().is_empty()
        || a.scope.trim().is_empty()
        || a.period.year == 0
        || a.period.first_month == 0
        || a.period.first_month > a.period.last_month
        || a.period.last_month > 12
        || a.interval.lo_strict
        || a.interval.hi_strict
        || checked(a.interval.lo.checked_cmp(a.interval.hi))?.is_gt()
    {
        return Err(refusal(
            RefusalCode::SupportMismatch102,
            "invalid series, calendar support or closed rounding interval",
        ));
    }
    if let Ownership::Attributable { share } = a.ownership
        && (share.signum() <= 0 || checked(share.checked_cmp(Rat::from_integer(1)))?.is_gt())
    {
        return Err(refusal(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "ownership share must lie in (0, 1]",
        ));
    }
    Ok(())
}

/// Applies a named identity without choosing inputs by their numerical agreement with a draft.
/// Source order does not affect the result or Claim identity. Signed rounding bounds are retained.
pub fn derive(rule: Rule, inputs: &[Claim<Amount>]) -> Result<Claim<Amount>> {
    let Some(first) = inputs.first() else {
        return Err(refusal(RefusalCode::NoClaimInDomain201, "missing operands"));
    };
    let a = &first.value;
    for input in inputs {
        validate(input)?;
        let b = &input.value;
        if (
            a.issuer.as_str(),
            a.subject.as_str(),
            a.scope.as_str(),
            a.kind,
            a.ownership,
        ) != (
            b.issuer.as_str(),
            b.subject.as_str(),
            b.scope.as_str(),
            b.kind,
            b.ownership,
        ) {
            return Err(refusal(
                RefusalCode::BasisMismatch105,
                "operands differ in issuer, subject, scope, metric/unit or ownership",
            ));
        }
        if a.period.year != b.period.year {
            return Err(refusal(
                RefusalCode::SupportMismatch102,
                "mixed calendar years",
            ));
        }
    }
    let mut ordered: Vec<_> = inputs.iter().collect();
    ordered.sort_by_key(|c| {
        (
            c.value.period.first_month,
            c.value.period.last_month,
            c.claim_id,
        )
    });
    let mut result = a.clone();
    match rule {
        Rule::QuarterSum => {
            if !matches!(ordered.len(), 2 | 4)
                || ordered.iter().enumerate().any(|(i, c)| {
                    usize::from(c.value.period.first_month) != i * 3 + 1
                        || usize::from(c.value.period.last_month) != i * 3 + 3
                })
            {
                return Err(refusal(
                    RefusalCode::SupportMismatch102,
                    "quarter sum requires exactly Q1/Q2 or Q1/Q2/Q3/Q4",
                ));
            }
            result.period.first_month = 1;
            result.period.last_month = if ordered.len() == 2 { 6 } else { 12 };
            result.interval.lo = Rat::from_integer(0);
            result.interval.hi = Rat::from_integer(0);
            for c in &ordered {
                result.interval.lo = checked(result.interval.lo.checked_add(c.value.interval.lo))?;
                result.interval.hi = checked(result.interval.hi.checked_add(c.value.interval.hi))?;
            }
        }
    }
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut terms = Vec::new();
    let mut parents = Vec::new();
    let mut anchors = Vec::new();
    for c in ordered {
        evidence = evidence.meet(c.evidence);
        epistemic = epistemic.and(c.epistemic);
        state = state.and(c.state);
        confidentiality = confidentiality.join(&c.confidentiality);
        for term in &c.uu_terms {
            if !terms.contains(term) {
                terms.push(term.clone());
            }
        }
        parents.push(c.claim_id);
        anchors.extend(c.anchors.iter().cloned());
    }
    parents.sort();
    parents.dedup();
    anchors.sort();
    anchors.dedup();
    let tag = result.canonical_tag();
    Ok(Claim::new(
        rule.id(),
        result,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        evidence,
        epistemic,
        state,
        confidentiality,
        terms,
        parents,
        anchors,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;

    fn amount(first: u8, last: u8, printed: i128) -> Claim<Amount> {
        let value = Amount {
            issuer: "invented".into(),
            subject: "Site A".into(),
            scope: "site".into(),
            kind: AmountKind::GoldProductionKoz,
            ownership: Ownership::Full,
            period: Period {
                year: 2030,
                first_month: first,
                last_month: last,
            },
            interval: Interval {
                lo: Rat::new(2 * printed - 1, 2).unwrap(),
                hi: Rat::new(2 * printed + 1, 2).unwrap(),
                lo_strict: false,
                hi_strict: false,
            },
        };
        let tag = value.canonical_tag();
        Claim::new(
            INPUT_METRIC,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "invented.xlsx",
                [first + last; 32],
                format!("Q{last}!B1"),
            )],
        )
    }

    fn reseal(source: Claim<Amount>) -> Claim<Amount> {
        let mut result = Claim::new(
            &source.metric_key,
            source.value.clone(),
            &source.value.canonical_tag(),
            source.representation,
            source.support,
            source.additivity,
            source.evidence,
            source.epistemic,
            source.state,
            source.confidentiality,
            source.uu_terms,
            source.parents,
            source.anchors,
        );
        result.stale = source.stale;
        result.calibration_failed = source.calibration_failed;
        result
    }

    #[test]
    fn exact_sum_overflow_refuses_in_both_directions_without_partial_output() {
        for (limit, increment) in [(i128::MAX, 1), (i128::MIN, -1)] {
            let mut q1 = amount(1, 3, 0);
            q1.value.interval.lo = Rat::from_integer(limit);
            q1.value.interval.hi = Rat::from_integer(limit);
            let mut q2 = amount(4, 6, 0);
            q2.value.interval.lo = Rat::from_integer(increment);
            q2.value.interval.hi = Rat::from_integer(increment);
            let sources = [reseal(q1), reseal(q2)];
            assert!(sources.iter().all(|source| source.check_id().is_ok()));
            assert_eq!(
                derive(Rule::QuarterSum, &sources).unwrap_err().code,
                "FMINE-REF-207"
            );
        }
    }

    #[test]
    fn canonical_but_unusable_and_placeholder_inputs_never_become_summed_amounts() {
        for state in [
            KnowledgeState::Unknown,
            KnowledgeState::NotApplicable,
            KnowledgeState::NotObservable,
            KnowledgeState::Conflicted,
            KnowledgeState::Indeterminate,
            KnowledgeState::Redacted,
            KnowledgeState::Stale,
        ] {
            let mut bad = amount(1, 3, 100);
            bad.state = state;
            let bad = reseal(bad);
            assert!(bad.check_id().is_ok());
            assert!(derive(Rule::QuarterSum, &[bad, amount(4, 6, 100)]).is_err());
        }
        for change in 0..9 {
            let mut bad = amount(1, 3, 100);
            match change {
                0 => {
                    bad.representation = Representation::Placeholder {
                        token: "—".into()
                    }
                }
                1 => bad.anchors.clear(),
                2 => bad.anchors[0].artifact_sha256 = [0; 32],
                3 => bad.anchors[0].locator.clear(),
                4 => bad.anchors[0].artifact_path.clear(),
                5 => bad.confidentiality = Confidentiality::internal(),
                6 => bad.calibration_failed = true,
                7 => bad.support = SpatialSupport::PointCore,
                _ => bad.value.interval.lo_strict = true,
            }
            let bad = reseal(bad);
            assert!(bad.check_id().is_ok());
            assert!(
                derive(Rule::QuarterSum, &[bad, amount(4, 6, 100)]).is_err(),
                "admission control {change}"
            );
        }
    }

    #[test]
    fn summation_preserves_source_identity_weaker_metadata_and_unknown_terms() {
        let mut q1 = amount(1, 3, 100);
        q1.evidence = EvidenceClass::Measured;
        let q1 = reseal(q1);
        let mut q2 = amount(4, 6, 100);
        q2.state = KnowledgeState::Estimated;
        q2.epistemic = EpistemicStatus::KnownUnknown(fmine_evidence::KuKind::Parametric);
        q2.uu_terms.push(fmine_claim::UuTerm {
            scenario_id: "invented-period-reclassification".into(),
            description: "Synthetic unknown accounting allocation".into(),
            downside_exposure_usd_m: 1.0,
            trigger_indicator: "later issuer correction".into(),
        });
        let q2 = reseal(q2);
        let output = derive(Rule::QuarterSum, &[q2.clone(), q1.clone()]).unwrap();
        assert!(output.check_id().is_ok());
        assert_eq!(output.evidence, EvidenceClass::ISSUER);
        assert!(output.evidence.no_stronger_than(q1.evidence));
        assert!(output.evidence.no_stronger_than(q2.evidence));
        assert_eq!(output.state, q2.state);
        assert_eq!(output.epistemic, q2.epistemic);
        assert_eq!(output.uu_terms, q2.uu_terms);
        let mut parents = vec![q1.claim_id, q2.claim_id];
        parents.sort();
        assert_eq!(output.parents, parents);
        let mut anchors = [q1.anchors.clone(), q2.anchors.clone()].concat();
        anchors.sort();
        assert_eq!(output.anchors, anchors);
        assert_eq!(output.value_tag(), output.value.canonical_tag());
        assert_eq!(
            output.claim_id,
            derive(Rule::QuarterSum, &[q1, q2]).unwrap().claim_id
        );
    }

    #[test]
    fn addition_propagates_rounding_and_keeps_zero_as_a_real_observation() {
        let q1 = amount(1, 3, 100);
        let q2 = amount(4, 6, 150);
        let result = derive(Rule::QuarterSum, &[q2.clone(), q1.clone()]).unwrap();
        assert_eq!(result.value.interval.lo, Rat::from_integer(249));
        assert_eq!(result.value.interval.hi, Rat::from_integer(251));
        assert_eq!(
            result.value.period,
            Period {
                year: 2030,
                first_month: 1,
                last_month: 6
            }
        );
        assert_eq!(result.parents.len(), 2);
        assert_ne!(result.evidence, EvidenceClass::Measured);
        assert_eq!(
            result.claim_id,
            derive(Rule::QuarterSum, &[q1, q2]).unwrap().claim_id
        );
        let zero = derive(Rule::QuarterSum, &[amount(1, 3, 0), amount(4, 6, 0)]).unwrap();
        assert_eq!(zero.value.interval.lo, Rat::from_integer(-1));
        assert_eq!(zero.value.interval.hi, Rat::from_integer(1));
    }

    #[test]
    fn quarter_sums_require_complete_disjoint_half_or_full_year() {
        let qs: Vec<_> = (0..4).map(|q| amount(q * 3 + 1, q * 3 + 3, 100)).collect();
        assert_eq!(
            derive(Rule::QuarterSum, &qs[..2])
                .unwrap()
                .value
                .interval
                .lo,
            Rat::from_integer(199)
        );
        let fy = derive(Rule::QuarterSum, &qs).unwrap();
        assert_eq!(
            fy.value.interval,
            Interval {
                lo: Rat::from_integer(398),
                hi: Rat::from_integer(402),
                lo_strict: false,
                hi_strict: false
            }
        );
        let mut reverse = qs.clone();
        reverse.reverse();
        assert_eq!(
            fy.claim_id,
            derive(Rule::QuarterSum, &reverse).unwrap().claim_id
        );
        for invalid in [&qs[..1], &qs[..3], &qs[1..3]] {
            assert!(derive(Rule::QuarterSum, invalid).is_err());
        }
        assert!(derive(Rule::QuarterSum, &[qs[0].clone(), qs[0].clone()]).is_err());
    }

    #[test]
    fn altered_source_and_nonadditive_or_unknown_evidence_refuse() {
        let q1 = amount(1, 3, 100);
        let q2 = amount(4, 6, 250);
        let mut altered = q1.clone();
        altered.value.subject = "Another site".into();
        assert_eq!(
            derive(Rule::QuarterSum, &[altered, q2.clone()])
                .unwrap_err()
                .code,
            "FMINE-REF-507"
        );
        for change in [0, 1, 2] {
            let mut bad = q1.clone();
            match change {
                0 => bad.additivity = AdditivityClass::IntensiveWeighted,
                1 => bad.state = KnowledgeState::Unknown,
                _ => bad.stale = true,
            }
            // Rebuild identity: refusal must check admission, not merely detect an old Claim id.
            bad = Claim::new(
                INPUT_METRIC,
                bad.value.clone(),
                &bad.value.canonical_tag(),
                bad.representation,
                bad.support,
                bad.additivity,
                bad.evidence,
                bad.epistemic,
                bad.state,
                bad.confidentiality,
                bad.uu_terms,
                bad.parents,
                bad.anchors,
            );
            if change == 2 {
                bad.stale = true;
            }
            assert!(derive(Rule::QuarterSum, &[bad, q2.clone()]).is_err());
        }
    }

    #[test]
    fn different_basis_unit_scope_and_year_do_not_mix() {
        let q1 = amount(1, 3, 100);
        for change in 0..5 {
            let mut h1 = amount(4, 6, 250);
            match change {
                0 => {
                    h1.value.ownership = Ownership::Attributable {
                        share: Rat::new(4, 5).unwrap(),
                    }
                }
                1 => h1.value.kind = AmountKind::CopperProductionKt,
                2 => h1.value.scope = "region-version-2".into(),
                3 => h1.value.period.year = 2031,
                _ => h1.value.subject = "Site B".into(),
            }
            h1 = Claim::new(
                INPUT_METRIC,
                h1.value.clone(),
                &h1.value.canonical_tag(),
                h1.representation,
                h1.support,
                h1.additivity,
                h1.evidence,
                h1.epistemic,
                h1.state,
                h1.confidentiality,
                h1.uu_terms,
                h1.parents,
                h1.anchors,
            );
            assert!(derive(Rule::QuarterSum, &[q1.clone(), h1]).is_err());
        }
    }
}
