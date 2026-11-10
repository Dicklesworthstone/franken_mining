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
pub const RATE_RULES_VERSION: &str = "aisc.component-rate-bridge/1";
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

    fn point(value: Rat) -> Self {
        Self {
            point: value,
            lo: value,
            hi: value,
        }
    }

    fn mul(self, other: Self) -> Result<Self> {
        let corners = [
            exact(self.lo.checked_mul(other.lo))?,
            exact(self.lo.checked_mul(other.hi))?,
            exact(self.hi.checked_mul(other.lo))?,
            exact(self.hi.checked_mul(other.hi))?,
        ];
        let (mut lo, mut hi) = (corners[0], corners[0]);
        for corner in &corners[1..] {
            if exact(corner.checked_cmp(lo))?.is_lt() {
                lo = *corner;
            }
            if exact(corner.checked_cmp(hi))?.is_gt() {
                hi = *corner;
            }
        }
        Ok(Self {
            point: exact(self.point.checked_mul(other.point))?,
            lo,
            hi,
        })
    }

    fn positive_reciprocal(self) -> Result<Self> {
        if self.lo.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                "per-ounce comparison requires strictly positive aggregate and combined-member sold-quantity rounding bounds",
            ));
        }
        let one = Rat::from_integer(1);
        Ok(Self {
            point: exact(one.checked_div(self.point))?,
            lo: exact(one.checked_div(self.hi))?,
            hi: exact(one.checked_div(self.lo))?,
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

/// Recomputed rates and an additive bridge over the same source table, in USD per ounce sold.
/// Each interval is a marginal rounding enclosure. Terms share operands and must not be treated as
/// independent uncertainty distributions. A zero arithmetic remainder is not an accounting explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateComparison {
    pub aggregate_usd_per_oz: Difference,
    /// Sum of member numerators divided by sum of member sold quantities. This pooled rate needs no
    /// individual rate for a member with zero sales, and never averages the printed rate rows.
    pub members_usd_per_oz: Difference,
    pub gap_usd_per_oz: Difference,
    /// Named component differences divided by the aggregate's sold quantity, in source-row order.
    pub components_usd_per_oz: Vec<Difference>,
    /// Unexplained dollar remainder divided by aggregate sold ounces, never assigned to a cost line.
    pub unexplained_usd_per_oz: Difference,
    /// 1000 * sum(member numerators) * (1 / aggregate sold koz - 1 / member sold koz).
    pub denominator_usd_per_oz: Difference,
    /// Checked exact point identity; does not require the unexplained or denominator terms to vanish.
    pub point_closure_verified: bool,
}

impl RateComparison {
    fn encode(&self, enc: &mut Encoder) {
        enc.put_str(RATE_RULES_VERSION);
        for d in [
            self.aggregate_usd_per_oz,
            self.members_usd_per_oz,
            self.gap_usd_per_oz,
            self.unexplained_usd_per_oz,
            self.denominator_usd_per_oz,
        ] {
            d.encode(enc);
        }
        enc.put_u64(self.components_usd_per_oz.len() as u64);
        for d in &self.components_usd_per_oz {
            d.encode(enc);
        }
        enc.put_u8(u8::from(self.point_closure_verified));
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
    /// A separate rate refusal preserves the useful dollar comparison when sales cannot define a rate.
    pub rates: Result<RateComparison>,
}

impl Comparison {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"ACOU", 2);
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
        match &self.rates {
            Ok(rates) => {
                enc.put_u8(0);
                rates.encode(&mut enc);
            }
            Err(refusal) => {
                enc.put_u8(1);
                enc.put_str(refusal.code);
                enc.put_str(&refusal.what);
                enc.put_str(refusal.fix);
                enc.put_u8(refusal.exit_code);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

fn sum(values: impl IntoIterator<Item = Printed>) -> Result<Difference> {
    values
        .into_iter()
        .try_fold(Difference::zero(), |a, b| a.add(Difference::printed(b)?))
}

fn compare_rates(
    table: &Table,
    members: &[&Column],
    components_usd_m: &[Difference],
    unexplained_usd_m: Difference,
) -> Result<RateComparison> {
    let scale = Difference::point(Rat::from_integer(1000));
    let aggregate_inverse = Difference::printed(table.aggregate.sold_koz)?.positive_reciprocal()?;
    let members_inverse = sum(members.iter().map(|c| c.sold_koz))?.positive_reciprocal()?;
    let aggregate_numerator = Difference::printed(table.aggregate.numerator_usd_m)?;
    let members_numerator = sum(members.iter().map(|c| c.numerator_usd_m))?;
    let aggregate_usd_per_oz = aggregate_numerator.mul(aggregate_inverse)?.mul(scale)?;
    let members_usd_per_oz = members_numerator.mul(members_inverse)?.mul(scale)?;
    let gap_usd_per_oz = aggregate_usd_per_oz.sub(members_usd_per_oz)?;
    let components_usd_per_oz = components_usd_m
        .iter()
        .map(|d| d.mul(aggregate_inverse)?.mul(scale))
        .collect::<Result<Vec<_>>>()?;
    let unexplained_usd_per_oz = unexplained_usd_m.mul(aggregate_inverse)?.mul(scale)?;
    let denominator_usd_per_oz = members_numerator
        .mul(aggregate_inverse.sub(members_inverse)?)?
        .mul(scale)?;
    // The denominator term is necessary even when the numerator's named components foot exactly.
    // Compare exact points, not rounded displays or the overlapping marginal enclosures.
    let total = components_usd_per_oz.iter().try_fold(
        exact(
            unexplained_usd_per_oz
                .point
                .checked_add(denominator_usd_per_oz.point),
        )?,
        |a, b| exact(a.checked_add(b.point)),
    )?;
    if total != gap_usd_per_oz.point {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "per-ounce named components, unexplained remainder and denominator term do not close exactly",
        ));
    }
    Ok(RateComparison {
        aggregate_usd_per_oz,
        members_usd_per_oz,
        gap_usd_per_oz,
        components_usd_per_oz,
        unexplained_usd_per_oz,
        denominator_usd_per_oz,
        point_closure_verified: true,
    })
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
    let rates = compare_rates(t, &members, &components_usd_m, unexplained_usd_m);
    let result = Comparison {
        components_usd_m,
        summed_components_usd_m,
        numerator_usd_m,
        sold_koz,
        unexplained_usd_m,
        column_footings_usd_m,
        printed_arithmetic_closed,
        rates,
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

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::Confidentiality;
    use fmine_core::{anchor::SourceAnchor, sha256};
    use fmine_evidence::EpistemicStatus;

    fn n(value: i128) -> Rat {
        Rat::from_integer(value)
    }

    fn q(num: i128, den: i128) -> Rat {
        Rat::new(num, den).unwrap()
    }

    fn printed(value: i128) -> Printed {
        Printed {
            point: n(value),
            step: n(1),
        }
    }

    fn table() -> Table {
        Table {
            issuer: "invented".into(),
            release: "2030Q1".into(),
            period: "2030Q1".into(),
            basis: "signed USD million amounts; attributable thousand ounces sold".into(),
            membership: [("member".into(), "declared complete member".into())].into(),
            component_labels: vec!["cost of sales".into(), "depreciation".into()],
            aggregate: Column {
                subject: "aggregate".into(),
                components: vec![printed(100), printed(-20)],
                numerator_usd_m: printed(80),
                sold_koz: printed(10),
            },
            members: vec![Column {
                subject: "member".into(),
                components: vec![printed(60), printed(-10)],
                numerator_usd_m: printed(50),
                sold_koz: printed(8),
            }],
        }
    }

    fn run(table: Table) -> Claim<Comparison> {
        let tag = table.canonical_tag();
        compare(&Claim::new(
            INPUT_METRIC,
            table,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "invented.xlsx",
                sha256(b"invented source table").0,
                "Q1 2030!B5:C8",
            )],
        ))
        .unwrap()
    }

    #[test]
    fn signed_rate_bridge_has_exact_known_answers_and_rounding_bounds() {
        let result = run(table());
        let r = result.value.rates.as_ref().unwrap();
        assert_eq!(r.aggregate_usd_per_oz.point, n(8000));
        assert_eq!(r.members_usd_per_oz.point, n(6250));
        assert_eq!(r.gap_usd_per_oz.point, n(1750));
        assert_eq!(r.components_usd_per_oz[0].point, n(4000));
        assert_eq!(r.components_usd_per_oz[1].point, n(-1000));
        assert_eq!(r.unexplained_usd_per_oz.point, n(0));
        assert_eq!(r.denominator_usd_per_oz.point, n(-1250));
        assert!(r.point_closure_verified);
        // Independent Fraction arithmetic: [79.5/10.5, 80.5/9.5] * 1000 and
        // [49.5/8.5, 50.5/7.5] * 1000, with the subtraction endpoints crossed.
        assert_eq!(
            (r.aggregate_usd_per_oz.lo, r.aggregate_usd_per_oz.hi),
            (q(53000, 7), q(161000, 19))
        );
        assert_eq!(
            (r.members_usd_per_oz.lo, r.members_usd_per_oz.hi),
            (q(99000, 17), q(20200, 3))
        );
        assert_eq!(
            (r.gap_usd_per_oz.lo, r.gap_usd_per_oz.hi),
            (q(17600, 21), q(856000, 323))
        );
        assert_eq!(
            (r.components_usd_per_oz[1].lo, r.components_usd_per_oz[1].hi),
            (q(-22000, 19), q(-6000, 7))
        );
        // Closure does not remove the already-detected sales discrepancy.
        assert!(!result.value.printed_arithmetic_closed);
        assert_eq!(result.evidence, EvidenceClass::Assumed);
    }

    fn encloses(interval: Difference, value: Rat) {
        assert!(!interval.lo.checked_cmp(value).unwrap().is_gt());
        assert!(!interval.hi.checked_cmp(value).unwrap().is_lt());
    }

    #[test]
    fn marginal_rate_bounds_enclose_every_signed_source_box_corner() {
        for sign in [1, -1] {
            let mut t = table();
            for c in std::iter::once(&mut t.aggregate).chain(&mut t.members) {
                for p in c.components.iter_mut().chain([&mut c.numerator_usd_m]) {
                    p.point = p.point.checked_mul(n(sign)).unwrap();
                }
                c.sold_koz.step = q(1, 10);
            }
            let source: Vec<_> = std::iter::once(&t.aggregate)
                .chain(&t.members)
                .flat_map(|c| c.components.iter().chain([&c.numerator_usd_m, &c.sold_koz]))
                .copied()
                .collect();
            let r = run(t).value.rates.unwrap();
            // Eight independent printed inputs, including signed costs and decimal sales precision.
            for mask in 0..256 {
                let v: Vec<_> = source
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let half = p.step.checked_div(n(2)).unwrap();
                        p.point
                            .checked_add(
                                half.checked_mul(n(if mask & (1 << i) == 0 { -1 } else { 1 }))
                                    .unwrap(),
                            )
                            .unwrap()
                    })
                    .collect();
                let rate = |amount: Rat, sold: Rat| {
                    amount
                        .checked_div(sold)
                        .unwrap()
                        .checked_mul(n(1000))
                        .unwrap()
                };
                let aggregate = rate(v[2], v[3]);
                let member = rate(v[6], v[7]);
                encloses(r.aggregate_usd_per_oz, aggregate);
                encloses(r.members_usd_per_oz, member);
                encloses(r.gap_usd_per_oz, aggregate.checked_sub(member).unwrap());
                for i in 0..2 {
                    encloses(
                        r.components_usd_per_oz[i],
                        rate(v[i].checked_sub(v[i + 4]).unwrap(), v[3]),
                    );
                }
                let dollar_remainder = v[2]
                    .checked_sub(v[6])
                    .unwrap()
                    .checked_sub(v[0].checked_sub(v[4]).unwrap())
                    .unwrap()
                    .checked_sub(v[1].checked_sub(v[5]).unwrap())
                    .unwrap();
                encloses(r.unexplained_usd_per_oz, rate(dollar_remainder, v[3]));
                encloses(
                    r.denominator_usd_per_oz,
                    rate(v[6], v[3]).checked_sub(member).unwrap(),
                );
            }
        }
    }

    #[test]
    fn unavailable_rate_domains_keep_the_dollar_comparison_and_bind_the_refusal() {
        for aggregate in [true, false] {
            let mut t = table();
            let column = if aggregate {
                &mut t.aggregate
            } else {
                &mut t.members[0]
            };
            // The printed midpoint is positive, but its rounding box touches zero.
            column.sold_koz = Printed {
                point: q(1, 2),
                step: n(1),
            };
            let result = run(t);
            assert_eq!(
                result.value.rates.as_ref().unwrap_err().code,
                "FMINE-REF-104"
            );
            assert_eq!(result.value.components_usd_m[0].point, n(40));
            let mut altered = result.value.clone();
            altered.rates = Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                "changed refusal",
            ));
            assert_ne!(altered.canonical_tag(), result.value.canonical_tag());
        }
        let mut t = table();
        for column in std::iter::once(&mut t.aggregate).chain(&mut t.members) {
            column.components = vec![
                Printed {
                    point: n(i128::MAX / 8),
                    step: ZERO,
                },
                Printed {
                    point: ZERO,
                    step: ZERO,
                },
            ];
            column.numerator_usd_m = column.components[0];
            column.sold_koz = Printed {
                point: n(1),
                step: ZERO,
            };
        }
        let result = run(t);
        assert!(result.value.printed_arithmetic_closed);
        assert_eq!(
            result.value.rates.as_ref().unwrap_err().code,
            "FMINE-REF-207"
        );
    }
}
