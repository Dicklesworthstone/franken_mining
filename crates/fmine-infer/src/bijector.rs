//! Smooth unconstrained-to-constrained parameter bijectors with exact log-abs-det-Jacobian
//! (`BD-232`, `fm-mcmc`).
//! Determinism: BitwisePortable

use fmine_core::det;
use fmine_evidence::{Refusal, RefusalCode};

use crate::ad::{Real, Tape, Var, sigmoid_f64, softplus_f64};

/// Smooth bijection mapping unconstrained $z \in \mathbb{R}$ to a constrained parameter domain $x$
/// with exact volume correction $\log |\mathrm{d}x / \mathrm{d}z|$.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bijector {
    /// $x = z \in (-\infty, +\infty)$, $\log|J| = 0$.
    Identity,
    /// $x = \exp(z) \in (0, +\infty)$, $\log|J| = z$.
    Positive,
    /// $x = \sigma(z) \in (0, 1)$, $\log|J| = -\operatorname{softplus}(-z) - \operatorname{softplus}(z)$.
    UnitInterval,
    /// $x = a + \exp(z) \in (a, +\infty)$, $\log|J| = z$.
    LowerBounded { lower: Real },
    /// $x = a + (b - a)\sigma(z) \in (a, b)$ with $a < b$,
    /// $\log|J| = \ln(b - a) - \operatorname{softplus}(-z) - \operatorname{softplus}(z)$.
    Bounded { lower: Real, upper: Real },
}

impl Bijector {
    /// Validate bijector parameters (e.g. finite bounds with `lower < upper`).
    pub fn validate(&self) -> Result<(), Refusal> {
        match *self {
            Self::Identity | Self::Positive | Self::UnitInterval => Ok(()),
            Self::LowerBounded { lower } => {
                let _ = lower.validate()?;
                Ok(())
            }
            Self::Bounded { lower, upper } => {
                let lo = lower.validate()?.0;
                let hi = upper.validate()?.0;
                if !(hi > lo && (hi - lo).is_finite()) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("Bounded bijector requires finite lower < upper, got [{lo}, {hi}]"),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Map unconstrained $z \in \mathbb{R}$ to constrained $x$.
    pub fn forward(&self, z: Real) -> Result<Real, Refusal> {
        self.validate()?;
        let zv = z.validate()?.0;
        let x = match *self {
            Self::Identity => zv,
            Self::Positive => det::exp(zv),
            Self::UnitInterval => sigmoid_f64(zv),
            Self::LowerBounded { lower } => lower.0 + det::exp(zv),
            Self::Bounded { lower, upper } => lower.0 + (upper.0 - lower.0) * sigmoid_f64(zv),
        };
        if !x.is_finite() {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!("bijector forward overflow at z={zv}"),
            ));
        }
        Ok(Real(if x == 0.0 { 0.0 } else { x }))
    }

    /// Map constrained $x$ back to unconstrained $z \in \mathbb{R}$, refusing boundary/outside values.
    pub fn inverse(&self, x: Real) -> Result<Real, Refusal> {
        self.validate()?;
        let xv = x.validate()?.0;
        let z = match *self {
            Self::Identity => xv,
            Self::Positive => {
                if xv <= 0.0 {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("Positive bijector inverse requires x > 0, got {xv}"),
                    ));
                }
                det::ln(xv)
            }
            Self::UnitInterval => {
                if !(xv > 0.0 && xv < 1.0) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("UnitInterval bijector inverse requires x in (0, 1), got {xv}"),
                    ));
                }
                det::ln(xv) - det::ln(1.0 - xv)
            }
            Self::LowerBounded { lower } => {
                let d = xv - lower.0;
                if d <= 0.0 {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("LowerBounded inverse requires x > {}, got {xv}", lower.0),
                    ));
                }
                det::ln(d)
            }
            Self::Bounded { lower, upper } => {
                if !(xv > lower.0 && xv < upper.0) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "Bounded inverse requires x in ({}, {}), got {xv}",
                            lower.0, upper.0
                        ),
                    ));
                }
                det::ln(xv - lower.0) - det::ln(upper.0 - xv)
            }
        };
        if !z.is_finite() {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!("bijector inverse non-finite at x={xv}"),
            ));
        }
        Ok(Real(if z == 0.0 { 0.0 } else { z }))
    }

    /// Exact log absolute determinant of the Jacobian $\log |\mathrm{d}x / \mathrm{d}z|$ at $z$.
    pub fn log_det_jacobian(&self, z: Real) -> Result<Real, Refusal> {
        self.validate()?;
        let zv = z.validate()?.0;
        let ladj = match *self {
            Self::Identity => 0.0,
            Self::Positive | Self::LowerBounded { .. } => zv,
            Self::UnitInterval => -softplus_f64(-zv) - softplus_f64(zv),
            Self::Bounded { lower, upper } => {
                det::ln(upper.0 - lower.0) - softplus_f64(-zv) - softplus_f64(zv)
            }
        };
        if !ladj.is_finite() {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!("bijector log_det_jacobian non-finite at z={zv}"),
            ));
        }
        Ok(Real(if ladj == 0.0 { 0.0 } else { ladj }))
    }

    /// Tape-differentiated forward transform returning `(constrained_x, log_abs_det_jacobian)`.
    pub fn forward_var(&self, tape: &mut Tape, z: Var) -> Result<(Var, Var), Refusal> {
        self.validate()?;
        match *self {
            Self::Identity => {
                let zero = tape.constant(Real(0.0))?;
                Ok((z, zero))
            }
            Self::Positive => {
                let x = tape.exp_op(z)?;
                Ok((x, z))
            }
            Self::UnitInterval => {
                let x = tape.sigmoid(z)?;
                let ls1 = tape.log_sigmoid(z)?;
                let ls2 = tape.log_one_minus_sigmoid(z)?;
                let ladj = tape.add(ls1, ls2)?;
                Ok((x, ladj))
            }
            Self::LowerBounded { lower } => {
                let e = tape.exp_op(z)?;
                let lo = tape.constant(lower)?;
                let x = tape.add(lo, e)?;
                Ok((x, z))
            }
            Self::Bounded { lower, upper } => {
                let span = Real(upper.0 - lower.0).validate()?;
                let sig = tape.sigmoid(z)?;
                let span_v = tape.constant(span)?;
                let scaled = tape.mul(span_v, sig)?;
                let lo_v = tape.constant(lower)?;
                let x = tape.add(lo_v, scaled)?;

                let ln_span = tape.constant(Real(det::ln(span.0)).validate()?)?;
                let ls1 = tape.log_sigmoid(z)?;
                let ls2 = tape.log_one_minus_sigmoid(z)?;
                let unit_ladj = tape.add(ls1, ls2)?;
                let ladj = tape.add(ln_span, unit_ladj)?;
                Ok((x, ladj))
            }
        }
    }
}
