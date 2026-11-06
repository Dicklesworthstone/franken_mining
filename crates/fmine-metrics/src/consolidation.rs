//! DESIGN (BD-273): exact differences between a declared AISC subtotal and its complete member columns.
//! Determinism: Exact
//! Signed source lines retain their printed interpretation; no ownership rescaling,
//! corporate allocation or JV allocation is inferred from a difference.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_units::ExactQty as Rat;

pub const INPUT_METRIC: &str = "aisc.component_comparison.input/1";
pub const OUTPUT_METRIC: &str = "aisc.component_comparison.result/1";
pub const NO_CLAIM: &str = "Exact comparison of signed, printed AISC table components for explicitly selected members. Differences are observations, not identified corporate or JV allocations. No ownership conversion is applied. Printed arithmetic closure does not establish an accounting reconciliation, the truth of an issuer assertion, or comparability outside this table. Bounds are closed display-rounding boxes, not confidence intervals.";
const ZERO: Rat = Rat::from_integer(0);

type Result<T> = std::result::Result<T, Refusal>;

/// A printed amount and its declared display step, in USD millions or thousand ounces according to its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Printed {
    pub point: Rat,
    pub step: Rat,
}

/// A complete column: every component appears in the same source-row order as `Table::component_labels`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub subject: String,
    pub components: Vec<Printed>,
    pub numerator_usd_m: Printed,
    pub sold_koz: Printed,
}

/// One source table and explicit member selection. Numeric observations cross the API inside a Claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub issuer: String,
    pub release: String,
    pub period: String,
    /// The same-table, as-printed interpretation and cited unit/denominator labels, never an inferred share.
    pub basis: String,
    /// One explicit policy per member subject. Extra and missing members refuse.
    pub membership: BTreeMap<String, String>,
    pub component_labels: Vec<String>,
    pub aggregate: Column,
    pub members: Vec<Column>,
}

fn qty(enc: &mut Encoder, q: Rat) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

impl Table {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"ACIN", 1);
        for s in [&self.issuer, &self.release, &self.period, &self.basis] {
            enc.put_str(s);
        }
        enc.put_u64(self.membership.len() as u64);
        for (subject, rule) in &self.membership {
            enc.put_str(subject);
            enc.put_str(rule);
        }
        enc.put_u64(self.component_labels.len() as u64);
        for s in &self.component_labels {
            enc.put_str(s);
        }
        let mut members: Vec<_> = self.members.iter().collect();
        members.sort_by_key(|c| &c.subject);
        enc.put_u64(members.len() as u64);
        for column in std::iter::once(&self.aggregate).chain(members) {
            enc.put_str(&column.subject);
            enc.put_u64(column.components.len() as u64);
            for p in column
                .components
                .iter()
                .chain([&column.numerator_usd_m, &column.sold_koz])
            {
                qty(&mut enc, p.point);
                qty(&mut enc, p.step);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Exact marginal enclosure over the independent, closed display-rounding boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difference {
    pub point: Rat,
    pub lo: Rat,
    pub hi: Rat,
}

fn exact<T>(r: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    r.map_err(|e| Refusal::new(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}

impl Difference {
    fn zero() -> Self {
        Self {
            point: ZERO,
            lo: ZERO,
            hi: ZERO,
        }
    }

    fn printed(p: Printed) -> Result<Self> {
        if p.step.signum() < 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "negative display step",
            ));
        }
        let half = exact(p.step.checked_div(Rat::from_integer(2)))?;
        Ok(Self {
            point: p.point,
            lo: exact(p.point.checked_sub(half))?,
            hi: exact(p.point.checked_add(half))?,
        })
    }

    fn add(self, other: Self) -> Result<Self> {
        Ok(Self {
            point: exact(self.point.checked_add(other.point))?,
            lo: exact(self.lo.checked_add(other.lo))?,
            hi: exact(self.hi.checked_add(other.hi))?,
        })
    }

    fn sub(self, other: Self) -> Result<Self> {
        Ok(Self {
            point: exact(self.point.checked_sub(other.point))?,
            lo: exact(self.lo.checked_sub(other.hi))?,
            hi: exact(self.hi.checked_sub(other.lo))?,
        })
    }

    #[must_use]
    pub fn contains_zero(self) -> bool {
        self.lo.signum() <= 0 && self.hi.signum() >= 0
    }

    fn encode(self, enc: &mut Encoder) {
        for v in [self.point, self.lo, self.hi] {
            qty(enc, v);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub components_usd_m: Vec<Difference>,
    pub summed_components_usd_m: Difference,
    pub numerator_usd_m: Difference,
    pub sold_koz: Difference,
    /// Numerator gap less all named component gaps; never assigned a balancing cost label.
    pub unexplained_usd_m: Difference,
    /// Aggregate first, then members sorted by subject; each printed numerator less its own component sum.
    pub column_footings_usd_m: Vec<(String, Difference)>,
    /// Exact equality of every printed component footing, the sales sum and the unexplained remainder.
    pub printed_arithmetic_closed: bool,
}

impl Comparison {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"ACOU", 1);
        enc.put_u64(self.components_usd_m.len() as u64);
        for v in self.components_usd_m.iter().chain([
            &self.summed_components_usd_m,
            &self.numerator_usd_m,
            &self.sold_koz,
            &self.unexplained_usd_m,
        ]) {
            v.encode(&mut enc);
        }
        enc.put_u64(self.column_footings_usd_m.len() as u64);
        for (subject, v) in &self.column_footings_usd_m {
            enc.put_str(subject);
            v.encode(&mut enc);
        }
        enc.put_u8(u8::from(self.printed_arithmetic_closed));
        blake3_hash(&enc.finish()).to_hex()
    }
}

fn sum(values: impl IntoIterator<Item = Printed>) -> Result<Difference> {
    values
        .into_iter()
        .try_fold(Difference::zero(), |a, b| a.add(Difference::printed(b)?))
}

/// Compare one complete source table without converting bases or substituting missing amounts.
pub fn compare(input: &Claim<Table>) -> Result<Claim<Comparison>> {
    input.check_id()?;
    let refuse = |code, what| Refusal::new(code, what);
    if input.metric_key != INPUT_METRIC || input.value_tag() != input.value.canonical_tag() {
        return Err(refuse(
            RefusalCode::FrozenRecordMutation507,
            "component table differs from its bound input",
        ));
    }
    let admitted_state = matches!(
        input.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || (input.state == KnowledgeState::Unknown
        && input.evidence == EvidenceClass::Assumed
        && input.value.issuer == "synthetic");
    if input.stale || !admitted_state || input.calibration_failed {
        return Err(refuse(
            RefusalCode::NotObservable202,
            "stale, inadmissible knowledge state or failed-calibration table",
        ));
    }
    if input.support != SpatialSupport::PeriodAggregate
        || input.additivity != AdditivityClass::NonAdditive
        || input.representation != Representation::Deterministic
    {
        return Err(refuse(
            RefusalCode::SupportMismatch102,
            "a deterministic period table is required",
        ));
    }
    if !input.confidentiality.is_public() {
        return Err(refuse(
            RefusalCode::MnpiWall301,
            "public component comparisons require public source evidence",
        ));
    }
    if input.anchors.is_empty()
        || input.anchors.iter().any(|a| {
            a.artifact_path.is_empty() || a.locator.is_empty() || a.artifact_sha256 == [0; 32]
        })
    {
        return Err(refuse(
            RefusalCode::NoClaimInDomain201,
            "every source anchor requires a path, locator and digest",
        ));
    }
    let t = &input.value;
    if [&t.issuer, &t.release, &t.period, &t.basis]
        .iter()
        .any(|s| s.trim().is_empty())
        || t.members.is_empty()
        || t.membership.is_empty()
        || t.component_labels.is_empty()
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "source scope, components and explicit membership are required",
        ));
    }
    let mut members: Vec<_> = t.members.iter().collect();
    members.sort_by_key(|c| &c.subject);
    let columns: Vec<_> = std::iter::once(&t.aggregate)
        .chain(members.iter().copied())
        .collect();
    let names: BTreeSet<_> = columns.iter().map(|c| &c.subject).collect();
    if names.len() != columns.len()
        || names.iter().any(|s| s.trim().is_empty())
        || columns
            .iter()
            .any(|c| c.components.len() != t.component_labels.len())
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "duplicate or missing subject, or incomplete component population",
        ));
    }
    let labels: BTreeSet<_> = t.component_labels.iter().collect();
    if labels.len() != t.component_labels.len()
        || labels.iter().any(|s| s.trim().is_empty())
        || !t.membership.keys().eq(members.iter().map(|c| &c.subject))
        || t.membership.values().any(|rule| rule.trim().is_empty())
    {
        return Err(refuse(
            RefusalCode::BasisMismatch105,
            "component labels must be distinct and every selected member must have exactly one declared policy",
        ));
    }
    if columns.iter().any(|c| c.sold_koz.point.signum() < 0) {
        return Err(refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "negative sold ounces",
        ));
    }
    let gap =
        |parent: Printed, children: Vec<Printed>| Difference::printed(parent)?.sub(sum(children)?);
    let components_usd_m = (0..t.component_labels.len())
        .map(|i| {
            gap(
                t.aggregate.components[i],
                members.iter().map(|m| m.components[i]).collect(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let summed_components_usd_m = components_usd_m
        .iter()
        .try_fold(Difference::zero(), |a, b| a.add(*b))?;
    let numerator_usd_m = gap(
        t.aggregate.numerator_usd_m,
        members.iter().map(|c| c.numerator_usd_m).collect(),
    )?;
    let sold_koz = gap(
        t.aggregate.sold_koz,
        members.iter().map(|c| c.sold_koz).collect(),
    )?;
    let unexplained_usd_m = numerator_usd_m.sub(summed_components_usd_m)?;
    let column_footings_usd_m = columns
        .iter()
        .map(|c| {
            Ok((
                c.subject.clone(),
                Difference::printed(c.numerator_usd_m)?.sub(sum(c.components.iter().copied())?)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let printed_arithmetic_closed = sold_koz.point == ZERO
        && unexplained_usd_m.point == ZERO
        && column_footings_usd_m.iter().all(|(_, v)| v.point == ZERO);
    let result = Comparison {
        components_usd_m,
        summed_components_usd_m,
        numerator_usd_m,
        sold_koz,
        unexplained_usd_m,
        column_footings_usd_m,
        printed_arithmetic_closed,
    };
    let tag = result.canonical_tag();
    Ok(Claim::new(
        OUTPUT_METRIC,
        result,
        &tag,
        Representation::Deterministic,
        input.support,
        AdditivityClass::NonAdditive,
        input.evidence.meet(EvidenceClass::Derived),
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        vec![input.claim_id],
        input.anchors.clone(),
    ))
}
