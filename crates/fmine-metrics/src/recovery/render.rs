//! Retained typed recovery values for public Claim inspection (bd-srqc, BD-115).
//! Determinism: Exact (serializes supplied values without recalculating numerical estimates).
//!
//! These codecs preserve the established inspection JSON bytes. Rendering a value does not validate
//! its Claim, authenticate its source, or turn descriptive recovery comparisons into forecasts.

use std::fmt::Write as _;

use super::{Estimate, Fit, ModelProjection, Ownership, Period, Quantity, SquareRoot};
use fmine_numerics::exact_feas::Rat;

fn js(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn finite(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".into()
    }
}

/// An exact rational as the inspection format's quoted numerator/denominator.
#[must_use]
pub fn rational(value: Rat) -> String {
    js(&format!("{}/{}", value.num(), value.den()))
}

/// Preserve the point, exact endpoints and strictness of a descriptive estimate.
#[must_use]
pub fn estimate(value: Estimate) -> String {
    format!(
        "{{\"point\":{},\"lo\":{},\"hi\":{},\"lo_strict\":{},\"hi_strict\":{}}}",
        rational(value.point),
        rational(value.interval.lo),
        rational(value.interval.hi),
        value.interval.lo_strict,
        value.interval.hi_strict
    )
}

/// Retain a numerical square-root summary; nonfinite diagnostic values render as JSON null.
#[must_use]
pub fn sqrt(value: SquareRoot) -> String {
    format!(
        "{{\"approx\":{},\"lo\":{},\"hi\":{},\"interpretation\":\"numerical square-root enclosure only\"}}",
        finite(value.approx),
        finite(value.enclosure[0]),
        finite(value.enclosure[1])
    )
}

/// Preserve full-mine or explicitly attributable ownership without converting its basis.
#[must_use]
pub fn ownership(value: Ownership) -> String {
    match value {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            rational(share)
        ),
    }
}

/// Preserve the supplied calendar support.
#[must_use]
pub fn period(value: Period) -> String {
    format!(
        "{{\"year\":{},\"first_month\":{},\"last_month\":{}}}",
        value.year, value.first_month, value.last_month
    )
}

/// Render a typed source or operating quantity with its exact rounding interval.
#[must_use]
pub fn quantity(value: &Quantity) -> String {
    format!(
        "{{\"subject\":{},\"period\":{},\"ownership\":{},\"kind\":{},\"estimate\":{}}}",
        js(&value.subject),
        period(value.period),
        ownership(value.ownership),
        js(&format!("{:?}", value.kind)),
        estimate(Estimate {
            point: value.point,
            interval: value.interval
        })
    )
}

/// Render retained training coefficients and their distinct descriptive variance summaries.
#[must_use]
pub fn fit(value: &Fit) -> String {
    format!(
        "{{\"subject\":{},\"ownership\":{},\"training_years\":[{}],\"mean_head_percent\":{},\"constant_tails_percent\":{},\"intercept_percent\":{},\"slope\":{},\"residual_sample_variance_at_printed\":{},\"residual_sample_sd_at_printed\":{},\"regression_error_variance_at_printed\":{},\"regression_error_sd_at_printed\":{}}}",
        js(&value.subject),
        ownership(value.ownership),
        value
            .training_years
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        estimate(value.mean_head),
        estimate(value.constant_tails),
        estimate(value.intercept),
        estimate(value.slope),
        rational(value.residual_sample_variance_at_printed),
        sqrt(value.residual_sample_sd_at_printed),
        rational(value.regression_error_variance_at_printed),
        sqrt(value.regression_error_sd_at_printed)
    )
}

/// Render one descriptive model projection without dropping the signs of its differences.
#[must_use]
pub fn projection(value: &ModelProjection) -> String {
    format!(
        "{{\"subject\":{},\"period\":{},\"ownership\":{},\"model\":{},\"expected_recovery_percent\":{},\"gap_pp\":{},\"copper_difference_kt\":{},\"fraction_of_reported_output\":{}}}",
        js(&value.subject),
        period(value.period),
        ownership(value.ownership),
        js(&format!("{:?}", value.model)),
        estimate(value.expected_recovery_percent),
        estimate(value.gap_pp),
        estimate(value.copper_difference_kt),
        estimate(value.fraction_of_reported_output)
    )
}
