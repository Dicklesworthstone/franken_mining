//! Determinism: BitwisePortable
//! L3 empirical geomaterial correlation cards (`BD-241`, plan §13.10).
//!
//! Covers empirical laws such as `UCS(Is50)`, `density(S, Fe)`, `Bond's law`, `PRI(TOC)`,
//! `recovery(CuAS/CuT)`, and `Archard wear`. Every card declares an exact `ValidityBox`
//! `[min_x, max_x]`, refuses out-of-box evaluation with `FMINE-REF-203`, and caps its output
//! evidence at `ModelledCalibrated` (with calibration id) or `ModelledUncalibrated` (never
//! promoted to `Measured` or `Derived` by verification; plan §13.10).

use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::EvidenceClass;
use fmine_units::ExactQty;

use crate::MaterialsError;

/// Named L3 empirical correlation families from plan §13.10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrelationKind {
    UcsFromIs50,
    DensityFromSulphur,
    BondWorkIndex,
    PriFromToc,
    RecoveryFromCuAsRatio,
    ArchardWear,
}

/// Exact rational closed validity interval `[min_x, max_x]` over a named input unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidityBox {
    pub input_unit: String,
    pub min_x: ExactQty,
    pub max_x: ExactQty,
}

impl ValidityBox {
    /// Construct a validated closed interval `min_x <= max_x`.
    pub fn new(
        input_unit: impl Into<String>,
        min_x: ExactQty,
        max_x: ExactQty,
    ) -> Result<Self, MaterialsError> {
        let input_unit = input_unit.into();
        if input_unit.trim().is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "validity_box.input_unit",
                detail: "input_unit must be non-empty".to_string(),
            });
        }
        if min_x.checked_cmp(max_x)? == core::cmp::Ordering::Greater {
            return Err(MaterialsError::InvalidSpec {
                field: "validity_box",
                detail: "min_x must be <= max_x".to_string(),
            });
        }
        Ok(Self {
            input_unit,
            min_x,
            max_x,
        })
    }

    /// Check whether `x` lies inside `[min_x, max_x]`.
    pub fn contains(&self, x: ExactQty) -> Result<bool, MaterialsError> {
        Ok(x.checked_cmp(self.min_x)? != core::cmp::Ordering::Less
            && x.checked_cmp(self.max_x)? != core::cmp::Ordering::Greater)
    }
}

/// Exact-rational affine L3 correlation card `y(x) = intercept + slope * x ± residual_half_width`
/// (`BD-241`, plan §13.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearCorrelationCard {
    pub card_id: String,
    pub kind: CorrelationKind,
    pub validity_box: ValidityBox,
    pub output_unit: String,
    pub intercept: ExactQty,
    pub slope: ExactQty,
    pub residual_half_width: ExactQty,
    /// Optional scored calibration record (`cal_...`); when `Some`, output is `ModelledCalibrated`,
    /// otherwise strictly `ModelledUncalibrated` (never promoted).
    pub calibration_id: Option<ObjectId>,
}

/// Evaluated L3 correlation output carrying exact interval `[lo, nominal, hi]` and capped evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationEval {
    pub card_id: String,
    pub output_unit: String,
    pub lo: ExactQty,
    pub nominal: ExactQty,
    pub hi: ExactQty,
    pub evidence: EvidenceClass,
}

impl LinearCorrelationCard {
    /// Construct a validated L3 correlation card.
    #[allow(
        clippy::too_many_arguments,
        reason = "typed exact-rational L3 correlation card constructor"
    )]
    pub fn build(
        card_id: impl Into<String>,
        kind: CorrelationKind,
        validity_box: ValidityBox,
        output_unit: impl Into<String>,
        intercept: ExactQty,
        slope: ExactQty,
        residual_half_width: ExactQty,
        calibration_id: Option<ObjectId>,
    ) -> Result<Self, MaterialsError> {
        let card_id = card_id.into();
        let output_unit = output_unit.into();
        if card_id.trim().is_empty() || output_unit.trim().is_empty() {
            return Err(MaterialsError::InvalidSpec {
                field: "correlation_card",
                detail: "card_id and output_unit must be non-empty".to_string(),
            });
        }
        if residual_half_width.signum() < 0 {
            return Err(MaterialsError::InvalidSpec {
                field: "residual_half_width",
                detail: "residual_half_width must be >= 0".to_string(),
            });
        }
        if let Some(cal) = calibration_id
            && cal.kind != PrefixKind::Calibration
        {
            return Err(MaterialsError::InvalidSpec {
                field: "calibration_id",
                detail: "calibration_id must have PrefixKind::Calibration".to_string(),
            });
        }
        Ok(Self {
            card_id,
            kind,
            validity_box,
            output_unit,
            intercept,
            slope,
            residual_half_width,
            calibration_id,
        })
    }

    /// Evaluate the correlation card at `(x, input_unit)` inside its exact `ValidityBox`.
    pub fn evaluate(
        &self,
        x: ExactQty,
        input_unit: &str,
    ) -> Result<CorrelationEval, MaterialsError> {
        if self
            .validity_box
            .min_x
            .checked_cmp(self.validity_box.max_x)?
            == core::cmp::Ordering::Greater
            || self.residual_half_width.signum() < 0
        {
            return Err(MaterialsError::InvalidSpec {
                field: "correlation_card",
                detail: "validity_box requires min_x <= max_x and residual_half_width >= 0"
                    .to_string(),
            });
        }
        if input_unit != self.validity_box.input_unit {
            return Err(MaterialsError::UnitMismatch {
                expected: self.validity_box.input_unit.clone(),
                actual: input_unit.to_string(),
            });
        }
        if !self.validity_box.contains(x)? {
            return Err(MaterialsError::OutsideValidityBox {
                card_id: self.card_id.clone(),
                detail: format!(
                    "input {}/{} lies outside [{}/{}, {}/{}] {}",
                    x.num(),
                    x.den(),
                    self.validity_box.min_x.num(),
                    self.validity_box.min_x.den(),
                    self.validity_box.max_x.num(),
                    self.validity_box.max_x.den(),
                    self.validity_box.input_unit
                ),
            });
        }
        let nominal = self.intercept.checked_add(self.slope.checked_mul(x)?)?;
        let lo = nominal.checked_sub(self.residual_half_width)?;
        let hi = nominal.checked_add(self.residual_half_width)?;
        // Plan §13.10: L3 correlation cards emit Validated (ModelledCalibrated) only when calibrated,
        // else Estimated (ModelledUncalibrated); never promoted by verification.
        let evidence = if self.calibration_id.is_some() {
            EvidenceClass::ModelledCalibrated
        } else {
            EvidenceClass::ModelledUncalibrated
        };
        Ok(CorrelationEval {
            card_id: self.card_id.clone(),
            output_unit: self.output_unit.clone(),
            lo,
            nominal,
            hi,
            evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l3_correlation_cards_validity_box_and_no_promotion() {
        // UCS(Is50) = 22 * Is50 ± 12 MPa for Is50 in [1, 10] MPa (uncalibrated).
        let vbox =
            ValidityBox::new("MPa", ExactQty::from_integer(1), ExactQty::from_integer(10)).unwrap();
        let uncal = LinearCorrelationCard::build(
            "ucs_from_is50_granodiorite",
            CorrelationKind::UcsFromIs50,
            vbox.clone(),
            "MPa",
            ExactQty::from_integer(0),
            ExactQty::from_integer(22),
            ExactQty::from_integer(12),
            None,
        )
        .expect("build uncalibrated card");

        let ev = uncal
            .evaluate(ExactQty::from_integer(5), "MPa")
            .expect("in-box eval");
        assert_eq!(ev.lo, ExactQty::from_integer(98));
        assert_eq!(ev.nominal, ExactQty::from_integer(110));
        assert_eq!(ev.hi, ExactQty::from_integer(122));
        assert_eq!(ev.evidence, EvidenceClass::ModelledUncalibrated);

        // Out-of-validity-box input refuses with FMINE-REF-203 (never extrapolated silently).
        let oob = uncal
            .evaluate(ExactQty::from_integer(12), "MPa")
            .unwrap_err();
        assert_eq!(oob.to_refusal().code, "FMINE-REF-203");

        // Wrong unit refuses with FMINE-REF-105.
        let unit_err = uncal
            .evaluate(ExactQty::from_integer(5), "psi")
            .unwrap_err();
        assert_eq!(unit_err.to_refusal().code, "FMINE-REF-105");

        // Calibrated card carries ModelledCalibrated; wrong ObjectId prefix refuses at build time.
        let cal_id = ObjectId::from_canonical_bytes(PrefixKind::Calibration, b"cal-ucs-2026");
        let cal_card = LinearCorrelationCard::build(
            "ucs_from_is50_calibrated",
            CorrelationKind::UcsFromIs50,
            vbox.clone(),
            "MPa",
            ExactQty::from_integer(0),
            ExactQty::from_integer(22),
            ExactQty::from_integer(12),
            Some(cal_id),
        )
        .expect("build calibrated card");
        let cal_ev = cal_card
            .evaluate(ExactQty::from_integer(5), "MPa")
            .expect("calibrated eval");
        assert_eq!(cal_ev.evidence, EvidenceClass::ModelledCalibrated);

        let wrong_id = ObjectId::from_canonical_bytes(PrefixKind::Claim, b"not-a-cal");
        assert!(
            LinearCorrelationCard::build(
                "bad_cal_prefix",
                CorrelationKind::UcsFromIs50,
                vbox,
                "MPa",
                ExactQty::from_integer(0),
                ExactQty::from_integer(22),
                ExactQty::from_integer(12),
                Some(wrong_id),
            )
            .is_err()
        );
    }
}
