//! DESIGN (bd-srqc): signed quarterly copper-value sensitivity at explicitly declared USD/lb prices.
//! Determinism: Exact (checked rational arithmetic and canonical Claim values).
//!
//! This prices the descriptive model difference, not recoverable production, revenue or profit.
//! The price is a constant nominal scenario on the original quarter, with no annualization or FX.

use super::*;
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_units::{AVDP_LB_TO_GRAMS_DEN, AVDP_LB_TO_GRAMS_NUM};

pub const RULE: &str = "recovery.copper-price-sensitivity/1";
pub const NOTICE: &str = "Signed model-implied copper difference multiplied by a caller-declared constant nominal USD/lb price on the same quarter and ownership basis. This is a hypothetical gross metal-value sensitivity, not recoverable copper, sales revenue, profit, monetary downside, a forecast or an annualized estimate. Prices are assumptions, not market observations. Input-rounding enclosures have no probability or confidence interpretation.";

/// An exact hypothetical price, distinct from a rounded or observed commodity price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopperPrice {
    pub scenario: String,
    pub usd_per_lb: Rat,
}

impl CopperPrice {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCPS", 1);
        enc.put_str(RULE);
        enc.put_str(&self.scenario);
        put_rat(&mut enc, self.usd_per_lb);
        blake3_hash(&enc.finish()).to_hex()
    }

    fn validate(&self) -> Result<()> {
        if self.scenario.is_empty()
            || self.scenario.len() > 64
            || !self
                .scenario
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        {
            return fail(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "price scenario needs a 1–64 character ASCII identifier",
            );
        }
        if self.usd_per_lb.signum() < 0 {
            return fail(
                RefusalCode::OutsidePhysicalDomain109,
                "the nominal copper-price scenario must be nonnegative",
            );
        }
        Ok(())
    }

    /// Record the declaration itself as the source. Its digest authenticates no external market price.
    pub fn declared_claim(self) -> Result<Claim<Self>> {
        self.validate()?;
        let tag = self.canonical_tag();
        let anchor = SourceAnchor::new(
            "declared:recovery-copper-price",
            sha256(tag.as_bytes()).0,
            self.scenario.clone(),
        );
        Ok(Claim::new(
            "recovery.declared_copper_price/1",
            self,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor],
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sensitivity {
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub model: Model,
    pub price: CopperPrice,
    pub copper_difference_kt: Estimate,
    /// The exact point and source-rounding enclosure, in millions of nominal USD.
    pub value_difference_usd_m: Estimate,
}

impl Sensitivity {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCSV", 1);
        enc.put_str(RULE);
        support(&mut enc, &self.subject, self.period, self.ownership);
        enc.put_u8(self.model as u8);
        enc.put_str(&self.price.canonical_tag());
        put_estimate(&mut enc, self.copper_difference_kt);
        put_estimate(&mut enc, self.value_difference_usd_m);
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Price an admitted model projection without changing its metal, time or ownership basis.
pub fn assess(
    projection: &Claim<ModelProjection>,
    price: &Claim<CopperPrice>,
) -> Result<Claim<Sensitivity>> {
    envelope(projection)?;
    envelope(price)?;
    projection.check_id()?;
    price.check_id()?;
    if projection.value_tag() != projection.value.canonical_tag()
        || price.value_tag() != price.value.canonical_tag()
    {
        return fail(
            RefusalCode::FrozenRecordMutation507,
            "price or recovery value differs from its canonical Claim",
        );
    }
    let expected_metric = match projection.value.model {
        Model::ConstantTails => "recovery.constant_tails/1",
        Model::GradeDependentTails => "recovery.grade_dependent_tails/1",
    };
    if projection.metric_key != expected_metric
        || price.metric_key != "recovery.declared_copper_price/1"
        || price.evidence != EvidenceClass::Assumed
        || projection.additivity != AdditivityClass::NonAdditive
        || price.additivity != AdditivityClass::NonAdditive
        || !matches!(projection.representation, Representation::Deterministic)
        || !matches!(price.representation, Representation::Deterministic)
    {
        return fail(
            RefusalCode::BasisMismatch105,
            "requires a descriptive recovery projection and explicitly assumed exact USD/lb price",
        );
    }
    price.value.validate()?;
    let p = &projection.value;
    if p.subject.trim().is_empty()
        || p.period.year == 0
        || p.period.year > 9999
        || !matches!(p.period.first_month, 1 | 4 | 7 | 10)
        || p.period.last_month != p.period.first_month + 2
        || p.ownership != Ownership::Full
    {
        return fail(
            RefusalCode::SupportMismatch102,
            "recovery sensitivity retains the full-site complete calendar quarter",
        );
    }
    check_estimate(p.copper_difference_kt)?;
    // 1 kt = 10^9 grams; dividing USD by 10^6 gives million USD. Statutory lb/gram
    // constants are shared with fmine-units, so no approximate tonne/pound factor enters.
    let factor = exact(Rat::new(1_000 * AVDP_LB_TO_GRAMS_DEN, AVDP_LB_TO_GRAMS_NUM))?;
    let value = Sensitivity {
        subject: p.subject.clone(),
        period: p.period,
        ownership: p.ownership,
        model: p.model,
        price: price.value.clone(),
        copper_difference_kt: p.copper_difference_kt,
        value_difference_usd_m: p.copper_difference_kt.mul(Estimate::exact(exact(
            factor.checked_mul(price.value.usd_per_lb),
        )?))?,
    };
    let mut provenance = Provenance::new();
    provenance.include(projection);
    provenance.include(price);
    let tag = value.canonical_tag();
    Ok(provenance.claim(RULE, value, &tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection(point: Rat, lo: Rat, hi: Rat) -> Claim<ModelProjection> {
        let value = ModelProjection {
            subject: "Synthetic copper site".into(),
            period: Period {
                year: 2031,
                first_month: 1,
                last_month: 3,
            },
            ownership: Ownership::Full,
            model: Model::ConstantTails,
            expected_recovery_percent: Estimate::exact(Rat::from_integer(90)),
            gap_pp: Estimate::exact(ONE),
            copper_difference_kt: Estimate {
                point,
                interval: bounds(lo, hi),
            },
            fraction_of_reported_output: Estimate::exact(ONE),
        };
        let tag = value.canonical_tag();
        Claim::new(
            "recovery.constant_tails/1",
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::ModelledUncalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "synthetic/quarter",
                [9; 32],
                "copper difference",
            )],
        )
    }

    fn price() -> Claim<CopperPrice> {
        CopperPrice {
            scenario: "declared-four".into(),
            usd_per_lb: Rat::from_integer(4),
        }
        .declared_claim()
        .unwrap()
    }

    #[test]
    fn statutory_conversion_preserves_signed_and_cross_zero_bounds() {
        // 0.45359237 kt = one million lb. At 4 USD/lb this is exactly 4 million USD.
        let kt = Rat::new(45_359_237, 100_000_000).unwrap();
        let p = projection(
            kt,
            kt.checked_neg().unwrap(),
            kt.checked_mul(Rat::from_integer(2)).unwrap(),
        );
        let out = assess(&p, &price()).unwrap();
        assert_eq!(
            out.value.value_difference_usd_m,
            Estimate {
                point: Rat::from_integer(4),
                interval: bounds(Rat::from_integer(-4), Rat::from_integer(8))
            }
        );
        assert_eq!(out.value.period, p.value.period);
        assert_eq!(out.evidence, EvidenceClass::Assumed);
        assert_eq!(out.parents.len(), 2);
        let negative = projection(kt.checked_neg().unwrap(), kt.checked_neg().unwrap(), ZERO);
        assert_eq!(
            assess(&negative, &price())
                .unwrap()
                .value
                .value_difference_usd_m
                .point,
            Rat::from_integer(-4)
        );
        let zero = CopperPrice {
            scenario: "zero".into(),
            usd_per_lb: ZERO,
        }
        .declared_claim()
        .unwrap();
        assert_eq!(
            assess(&p, &zero).unwrap().value.value_difference_usd_m,
            Estimate::exact(ZERO)
        );
    }

    #[test]
    fn changed_values_stale_inputs_and_invalid_scenarios_refuse() {
        let p = projection(ONE, ZERO, ONE);
        let mut changed = p.clone();
        changed.value.copper_difference_kt.point = ZERO;
        assert_eq!(
            assess(&changed, &price()).unwrap_err().code,
            "FMINE-REF-507"
        );
        changed = p.clone();
        changed.stale = true;
        assert_eq!(
            assess(&changed, &price()).unwrap_err().code,
            "FMINE-REF-502"
        );
        let mut changed_price = price();
        changed_price.value.usd_per_lb = ONE;
        assert_eq!(
            assess(&p, &changed_price).unwrap_err().code,
            "FMINE-REF-507"
        );
        for (id, amount) in [
            ("", ONE),
            ("bad id", ONE),
            ("negative", Rat::from_integer(-1)),
        ] {
            assert!(
                CopperPrice {
                    scenario: id.into(),
                    usd_per_lb: amount
                }
                .declared_claim()
                .is_err()
            );
        }
    }
}
