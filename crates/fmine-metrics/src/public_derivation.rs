//! Registered additive public-disclosure identities (BD-345.S4).
//! Determinism: Exact
//!
//! DESIGN: closed rational intervals propagate the printed precision of source assertions. They are
//! neither forecasts nor confidence intervals. Only Q1 + Q2 = H1, Q1 + Q2 + Q3 = 9M and Q1 + Q2 + Q3 + Q4 = FY
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
            if !matches!(ordered.len(), 2..=4)
                || ordered.iter().enumerate().any(|(i, c)| {
                    usize::from(c.value.period.first_month) != i * 3 + 1
                        || usize::from(c.value.period.last_month) != i * 3 + 3
                })
            {
                return Err(refusal(
                    RefusalCode::SupportMismatch102,
                    "quarter sum requires exactly Q1/Q2, Q1/Q2/Q3 or Q1/Q2/Q3/Q4",
                ));
            }
            result.period.first_month = 1;
            result.period.last_month = match ordered.len() {
                2 => 6,
                3 => 9,
                _ => 12,
            };
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

// Public API acceptance checks live in crates/xtask/tests/public_lint_derivations.rs.
