//! Determinism: BitwisePortable
//! Developer cash-flow replay, exact rational discounting-convention discovery with transcendental-free
//! square-root enclosures, and refusing bracketed IRR (`BD-350.S4` / `bd-gqu3.3`, plan §39 WP-15).
//!
//! All public signatures use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6) and
//! zero std transcendentals (`BitwisePortable`, `xtask check-det`).
//!
//! Conventions evaluated:
//! - `EndYear`: `Σ_{t=0..n} CF_t / (1 + r)^t`
//! - `MidYearAll`: `Σ_{t=0..n} CF_t / (1 + r)^(t - 1/2) = NPV_end_year · √(1 + r)`
//! - `Year0UndiscountedMidYearOps`: `CF_0 + Σ_{t=1..n} CF_t / (1 + r)^(t - 1/2)`
//!
//! Every verdict is `"reproduces"` or `"does not reproduce under the stated inputs"`, never
//! `"the issuer is wrong"`.

use fmine_numerics::{RootError, brent_root};
use fmine_units::ExactQty;

use crate::EconError;

/// Maximum base-10 scale exponent used when refining the rational enclosure of `√(1 + r)`.
const SQRT_SCALE_POWERS: [u32; 3] = [6, 9, 12];

/// Declared discounting conventions tested during convention discovery (`BD-350.S4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiscountConvention {
    EndYear,
    MidYearAll,
    Year0UndiscountedMidYearOps,
}

impl DiscountConvention {
    /// Canonical JSON / log label matching `bd-gqu3.3`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EndYear => "EndYear",
            Self::MidYearAll => "MidYearAll",
            Self::Year0UndiscountedMidYearOps => "Year0UndiscountedMidYearOps",
        }
    }

    /// All declared conventions in canonical evaluation order.
    pub const ALL: [Self; 3] = [
        Self::EndYear,
        Self::MidYearAll,
        Self::Year0UndiscountedMidYearOps,
    ];
}

/// Tax basis of the cash-flow replay (`BD-350.S4`: after-tax replay without a schedule refuses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxBasis {
    PreTax,
    /// After-tax replay requires an explicit tax schedule (`None` refuses!).
    AfterTax {
        /// Optional per-period exact tax cash outflows (same length as `flows`).
        tax_outflows: Option<Vec<ExactQty>>,
    },
}

/// Closed exact rational interval `[lo, hi]` with `lo <= hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactInterval {
    pub lo: ExactQty,
    pub hi: ExactQty,
}

impl ExactInterval {
    /// Construct a validated exact interval (`lo <= hi`).
    pub fn new(lo: ExactQty, hi: ExactQty) -> Result<Self, EconError> {
        if lo.checked_cmp(hi)?.is_gt() {
            return Err(EconError::InvalidDomain {
                field: "interval",
                detail: "interval lower bound exceeds upper bound".to_string(),
            });
        }
        Ok(Self { lo, hi })
    }

    /// Construct a point interval `[x, x]`.
    #[must_use]
    pub const fn point(x: ExactQty) -> Self {
        Self { lo: x, hi: x }
    }

    /// Construct a rounding interval `[printed - step/2, printed + step/2]` for a published number.
    pub fn from_rounding_step(printed: ExactQty, step: ExactQty) -> Result<Self, EconError> {
        if step.signum() < 0 {
            return Err(EconError::InvalidDomain {
                field: "rounding_step",
                detail: "rounding step must be >= 0".to_string(),
            });
        }
        let half = step.checked_div(ExactQty::from_integer(2))?;
        Self::new(printed.checked_sub(half)?, printed.checked_add(half)?)
    }

    /// Interval addition `[a, b] + [c, d] = [a + c, b + d]`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, EconError> {
        Self::new(self.lo.checked_add(rhs.lo)?, self.hi.checked_add(rhs.hi)?)
    }

    /// Interval subtraction `[a, b] - [c, d] = [a - d, b - c]`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, EconError> {
        Self::new(self.lo.checked_sub(rhs.hi)?, self.hi.checked_sub(rhs.lo)?)
    }

    /// Exact interval multiplication over all four endpoint products.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, EconError> {
        let p0 = self.lo.checked_mul(rhs.lo)?;
        let p1 = self.lo.checked_mul(rhs.hi)?;
        let p2 = self.hi.checked_mul(rhs.lo)?;
        let p3 = self.hi.checked_mul(rhs.hi)?;
        let mut lo = p0;
        let mut hi = p0;
        for p in [p1, p2, p3] {
            if p.checked_cmp(lo)?.is_lt() {
                lo = p;
            }
            if p.checked_cmp(hi)?.is_gt() {
                hi = p;
            }
        }
        Self::new(lo, hi)
    }

    /// Returns `true` iff `self` and `other` intersect (`self.lo <= other.hi && other.lo <= self.hi`).
    pub fn overlaps(self, other: Self) -> Result<bool, EconError> {
        Ok(self.lo.checked_cmp(other.hi)?.is_le() && other.lo.checked_cmp(self.hi)?.is_le())
    }

    /// Returns `true` iff `self` lies completely inside `other` (`other.lo <= self.lo && self.hi <= other.hi`).
    pub fn contained_in(self, other: Self) -> Result<bool, EconError> {
        Ok(other.lo.checked_cmp(self.lo)?.is_le() && self.hi.checked_cmp(other.hi)?.is_le())
    }

    /// Exact midpoint `(lo + hi) / 2`.
    pub fn midpoint(self) -> Result<ExactQty, EconError> {
        Ok(self
            .lo
            .checked_add(self.hi)?
            .checked_div(ExactQty::from_integer(2))?)
    }
}

/// Exact 256-bit unsigned product `(hi, lo)` of two `u128` values (no overflow, portable).
fn mul_u128_wide(a: u128, b: u128) -> (u128, u128) {
    let a_lo = a & 0xFFFF_FFFF_FFFF_FFFF;
    let a_hi = a >> 64;
    let b_lo = b & 0xFFFF_FFFF_FFFF_FFFF;
    let b_hi = b >> 64;

    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;

    let mid1 = (p0 >> 64) + (p1 & 0xFFFF_FFFF_FFFF_FFFF) + (p2 & 0xFFFF_FFFF_FFFF_FFFF);
    let lo = (p0 & 0xFFFF_FFFF_FFFF_FFFF) | (mid1 << 64);
    let hi = p3 + (p1 >> 64) + (p2 >> 64) + (mid1 >> 64);
    (hi, lo)
}

/// Compare `a * b` vs `c * d` exactly in 256-bit unsigned arithmetic.
fn cmp_prod_u128(a: u128, b: u128, c: u128, d: u128) -> core::cmp::Ordering {
    mul_u128_wide(a, b).cmp(&mul_u128_wide(c, d))
}

/// Compare `(k / scale)^2 <= num / den` exactly using `u128` wide products (`k^2 * den <= num * scale^2`).
/// Works for any `k, scale < 2^64` (so `k^2` and `scale^2` fit cleanly in `u128`).
fn sq_le_ratio(k: u128, scale: u128, num: u128, den: u128) -> bool {
    let k_sq = k * k;
    let s_sq = scale * scale;
    cmp_prod_u128(k_sq, den, num, s_sq).is_le()
}

fn pow10_u128(exp: u32) -> Result<u128, EconError> {
    let mut acc = 1_u128;
    for _ in 0..exp {
        acc = acc.checked_mul(10).ok_or(EconError::Overflow)?;
    }
    Ok(acc)
}

/// Compute an exact rational enclosure `[lo, hi]` for `√x` satisfying `lo^2 <= x <= hi^2` with
/// width `<= 10^(-scale_pow10)`, using strictly integer bisection (ZERO transcendentals,
/// `BitwisePortable`).
pub fn sqrt_enclosure(x: ExactQty, scale_pow10: u32) -> Result<ExactInterval, EconError> {
    if x.signum() < 0 {
        return Err(EconError::InvalidDomain {
            field: "sqrt_arg",
            detail: "cannot take square root of negative quantity".to_string(),
        });
    }
    if x.signum() == 0 {
        return Ok(ExactInterval::point(ExactQty::from_integer(0)));
    }
    if scale_pow10 > 15 {
        return Err(EconError::InvalidDomain {
            field: "scale_pow10",
            detail: format!("scale_pow10 must be <= 15, got {scale_pow10}"),
        });
    }
    let scale = pow10_u128(scale_pow10)?;
    let num = u128::try_from(x.num()).map_err(|_| EconError::Overflow)?;
    let den = u128::try_from(x.den()).map_err(|_| EconError::Overflow)?;

    // Choose initial upper integer bound `hi_k` such that `(hi_k / scale)^2 >= num / den`,
    // staying below 2^63 so `k * k` never overflows u128.
    let mut hi_k = scale;
    let max_k = (1_u128 << 63) - 1;
    while sq_le_ratio(hi_k, scale, num, den) {
        if hi_k > max_k / 2 {
            return Err(EconError::Overflow);
        }
        hi_k *= 2;
    }

    // Binary search for largest `lo_k` with `(lo_k / scale)^2 <= num / den`.
    let mut lo_k = 0_u128;
    while lo_k + 1 < hi_k {
        let mid = lo_k + (hi_k - lo_k) / 2;
        if sq_le_ratio(mid, scale, num, den) {
            lo_k = mid;
        } else {
            hi_k = mid;
        }
    }

    let scale_i128 = i128::try_from(scale).map_err(|_| EconError::Overflow)?;
    let lo_i128 = i128::try_from(lo_k).map_err(|_| EconError::Overflow)?;
    let hi_i128 = if cmp_prod_u128(lo_k * lo_k, den, num, scale * scale).is_eq() {
        // Exact rational square root!
        lo_i128
    } else {
        i128::try_from(lo_k + 1).map_err(|_| EconError::Overflow)?
    };
    ExactInterval::new(
        ExactQty::new(lo_i128, scale_i128)?,
        ExactQty::new(hi_i128, scale_i128)?,
    )
}

/// Compute exact rational `(1 + r)^(-t)` for integer `t >= 0` (`r > -1`).
fn exact_discount_factor_int(rate: ExactQty, t: usize) -> Result<ExactQty, EconError> {
    let one = ExactQty::from_integer(1);
    let base = one.checked_add(rate)?;
    if base.signum() <= 0 {
        return Err(EconError::InvalidDomain {
            field: "discount_rate",
            detail: "discount rate must satisfy 1 + r > 0".to_string(),
        });
    }
    let inv_base = one.checked_div(base)?;
    let mut acc = one;
    let mut power = inv_base;
    let mut exp = t;
    while exp > 0 {
        if (exp & 1) == 1 {
            acc = acc.checked_mul(power)?;
        }
        exp >>= 1;
        if exp > 0 {
            power = power.checked_mul(power)?;
        }
    }
    Ok(acc)
}

/// Apply tax basis: pre-tax keeps `flows` as-is; after-tax without a schedule refuses (`FMINE-REF-107`).
fn resolve_effective_flows(
    flows: &[ExactQty],
    tax_basis: &TaxBasis,
) -> Result<Vec<ExactQty>, EconError> {
    if flows.is_empty() {
        return Err(EconError::InvalidDomain {
            field: "flows",
            detail: "cash-flow schedule must contain at least one period".to_string(),
        });
    }
    match tax_basis {
        TaxBasis::PreTax => Ok(flows.to_vec()),
        TaxBasis::AfterTax { tax_outflows: None } => Err(EconError::MissingTaxSchedule {
            detail: "after-tax cash-flow replay refused: no tax schedule supplied".to_string(),
        }),
        TaxBasis::AfterTax {
            tax_outflows: Some(taxes),
        } => {
            if taxes.len() != flows.len() {
                return Err(EconError::InvalidDomain {
                    field: "tax_outflows",
                    detail: format!(
                        "tax schedule length {} != cash-flow length {}",
                        taxes.len(),
                        flows.len()
                    ),
                });
            }
            let mut net = Vec::with_capacity(flows.len());
            for (&cf, &tx) in flows.iter().zip(taxes.iter()) {
                if tx.signum() < 0 {
                    return Err(EconError::InvalidDomain {
                        field: "tax_outflows",
                        detail: "tax outflow entry must be >= 0".to_string(),
                    });
                }
                net.push(cf.checked_sub(tx)?);
            }
            Ok(net)
        }
    }
}

/// Evaluate the exact rational NPV interval for one [`DiscountConvention`] (`BD-350.S4`),
/// propagating per-flow rounding step `flow_rounding_step` and using `sqrt_enclosure(1 + r, sqrt_scale_pow10)`
/// for half-year conventions.
pub fn npv_interval(
    flows: &[ExactQty],
    flow_rounding_step: ExactQty,
    discount_rate: ExactQty,
    convention: DiscountConvention,
    tax_basis: &TaxBasis,
    sqrt_scale_pow10: u32,
) -> Result<ExactInterval, EconError> {
    let effective = resolve_effective_flows(flows, tax_basis)?;
    let one = ExactQty::from_integer(1);
    let one_plus_r = one.checked_add(discount_rate)?;
    if one_plus_r.signum() <= 0 {
        return Err(EconError::InvalidDomain {
            field: "discount_rate",
            detail: "discount rate must satisfy 1 + r > 0".to_string(),
        });
    }

    let cf0 = ExactInterval::from_rounding_step(effective[0], flow_rounding_step)?;
    let mut ops_end = ExactInterval::point(ExactQty::from_integer(0));
    for (t, &cf) in effective.iter().enumerate().skip(1) {
        let cf_ivl = ExactInterval::from_rounding_step(cf, flow_rounding_step)?;
        let df = exact_discount_factor_int(discount_rate, t)?;
        let pv = cf_ivl.checked_mul(ExactInterval::point(df))?;
        ops_end = ops_end.checked_add(pv)?;
    }

    match convention {
        DiscountConvention::EndYear => cf0.checked_add(ops_end),
        DiscountConvention::MidYearAll => {
            let end_total = cf0.checked_add(ops_end)?;
            let half_factor = sqrt_enclosure(one_plus_r, sqrt_scale_pow10)?;
            end_total.checked_mul(half_factor)
        }
        DiscountConvention::Year0UndiscountedMidYearOps => {
            let half_factor = sqrt_enclosure(one_plus_r, sqrt_scale_pow10)?;
            let ops_mid = ops_end.checked_mul(half_factor)?;
            cf0.checked_add(ops_mid)
        }
    }
}

/// Per-convention evaluation row emitted during convention discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionEvalRow {
    pub convention: DiscountConvention,
    pub npv_interval: ExactInterval,
    pub matches_printed: bool,
    pub log_line: String,
}

/// Successful unique discounting-convention discovery verdict (`BD-350.S4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionDiscovery {
    pub discovered: DiscountConvention,
    pub printed_interval: ExactInterval,
    pub evaluations: Vec<ConventionEvalRow>,
    pub summary_log_line: String,
}

fn fmt_exact_f64_6dp(q: ExactQty) -> String {
    format!("{:.6}", q.to_f64())
}

/// Input parameters for [`discover_discounting_convention`].
#[derive(Debug, Clone)]
pub struct ConventionDiscoveryRequest<'a> {
    pub report_id: &'a str,
    pub report_sha256: &'a str,
    pub flows: &'a [ExactQty],
    pub flow_rounding_step: ExactQty,
    pub discount_rate: ExactQty,
    pub printed_npv: ExactQty,
    pub printed_step: ExactQty,
    pub tax_basis: &'a TaxBasis,
}

/// Discover which of the three declared discounting conventions reproduces `printed_npv` within
/// `printed_step` (`[printed_npv - printed_step/2, printed_npv + printed_step/2]`).
///
/// Refines the rational square-root enclosure if a boundary straddle occurs (up to `10^-12`).
/// Refuses with `FMINE-REF-104` (`NonIdentifiableConvention`, `"convention not discovered"`) when
/// zero OR two-or-more conventions match.
pub fn discover_discounting_convention(
    req: &ConventionDiscoveryRequest<'_>,
) -> Result<ConventionDiscovery, EconError> {
    let printed_interval = ExactInterval::from_rounding_step(req.printed_npv, req.printed_step)?;
    // Validate tax schedule & flows up front before looping conventions.
    let _ = resolve_effective_flows(req.flows, req.tax_basis)?;

    let mut rows = Vec::with_capacity(DiscountConvention::ALL.len());
    let mut matched = Vec::new();

    for conv in DiscountConvention::ALL {
        let mut chosen_ivl = npv_interval(
            req.flows,
            req.flow_rounding_step,
            req.discount_rate,
            conv,
            req.tax_basis,
            SQRT_SCALE_POWERS[0],
        )?;
        // If half-year enclosure straddles the printed interval boundary, refine up to 10^-12.
        if conv != DiscountConvention::EndYear {
            for &scale_pow in &SQRT_SCALE_POWERS[1..] {
                let overlaps = chosen_ivl.overlaps(printed_interval)?;
                let inside = chosen_ivl.contained_in(printed_interval)?;
                if !overlaps || inside {
                    break;
                }
                chosen_ivl = npv_interval(
                    req.flows,
                    req.flow_rounding_step,
                    req.discount_rate,
                    conv,
                    req.tax_basis,
                    scale_pow,
                )?;
            }
        }
        let matches_printed = chosen_ivl.overlaps(printed_interval)?;
        if matches_printed {
            matched.push(conv);
        }
        let log_line = format!(
            "{{\"event\":\"npv_convention\",\"convention\":\"{}\",\"npv_lo\":{},\"npv_hi\":{},\
             \"printed\":{},\"printed_step\":{},\"match\":{matches_printed}}}",
            conv.as_str(),
            fmt_exact_f64_6dp(chosen_ivl.lo),
            fmt_exact_f64_6dp(chosen_ivl.hi),
            fmt_exact_f64_6dp(req.printed_npv),
            fmt_exact_f64_6dp(req.printed_step),
        );
        rows.push(ConventionEvalRow {
            convention: conv,
            npv_interval: chosen_ivl,
            matches_printed,
            log_line,
        });
    }

    if matched.len() != 1 {
        return Err(EconError::NonIdentifiableConvention {
            detail: format!(
                "convention not discovered: {} of 3 conventions matched printed NPV [{}..{}]",
                matched.len(),
                fmt_exact_f64_6dp(printed_interval.lo),
                fmt_exact_f64_6dp(printed_interval.hi),
            ),
        });
    }

    let discovered = matched[0];
    let summary_log_line = format!(
        "{{\"event\":\"npv_replay\",\"report\":\"{}\",\"sha256\":\"{}\",\
         \"convention\":\"{}\",\"irr_state\":\"not_requested\"}}",
        req.report_id,
        req.report_sha256,
        discovered.as_str()
    );
    Ok(ConventionDiscovery {
        discovered,
        printed_interval,
        evaluations: rows,
        summary_log_line,
    })
}

/// Converged bracketed IRR solution returned by [`solve_irr_bracketed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrrRoot {
    /// Converged IRR represented as an exact rational (scaled to `10^-9`).
    pub rate: ExactQty,
    pub bracket_lo: ExactQty,
    pub bracket_hi: ExactQty,
    pub iterations: usize,
}

/// Count Descartes sign changes in a cash-flow sequence (ignoring zero entries).
#[must_use]
pub fn count_cash_flow_sign_changes(flows: &[ExactQty]) -> usize {
    let mut changes = 0_usize;
    let mut prev_sign = 0_i128;
    for &cf in flows {
        let s = cf.signum();
        if s == 0 {
            continue;
        }
        if prev_sign != 0 && s != prev_sign {
            changes += 1;
        }
        prev_sign = s;
    }
    changes
}

/// Pure transcendental-free `f64` NPV evaluator for root-finding (`+ - * /` and Newton sqrt only).
fn sqrt_f64_portable(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return f64::NAN;
    }
    // 60 iterations of Newton-Raphson on [0, max(1, x)] uses only `+ - * /` (BitwisePortable).
    let mut g = if x > 1.0 { x } else { 1.0 };
    for _ in 0..60 {
        g = 0.5 * (g + x / g);
    }
    g
}

fn npv_f64_eval(flows_f64: &[f64], r: f64, convention: DiscountConvention) -> f64 {
    let base = 1.0 + r;
    if !base.is_finite() || base <= 0.0 {
        return f64::NAN;
    }
    let inv = 1.0 / base;
    let mut ops_end = 0.0_f64;
    let mut df = inv;
    for &cf in flows_f64.iter().skip(1) {
        ops_end += cf * df;
        df *= inv;
    }
    let cf0 = flows_f64[0];
    match convention {
        DiscountConvention::EndYear => cf0 + ops_end,
        DiscountConvention::MidYearAll => (cf0 + ops_end) * sqrt_f64_portable(base),
        DiscountConvention::Year0UndiscountedMidYearOps => cf0 + ops_end * sqrt_f64_portable(base),
    }
}

fn f64_to_exact_1e9(v: f64) -> Result<ExactQty, EconError> {
    if !v.is_finite() {
        return Err(EconError::NumericalRootFailed {
            detail: "IRR iterate was non-finite".to_string(),
        });
    }
    let scaled = (v * 1_000_000_000.0).round();
    if !scaled.is_finite() || scaled.abs() > 1e30 {
        return Err(EconError::Overflow);
    }
    Ok(ExactQty::new(scaled as i128, 1_000_000_000)?)
}

/// Solve for a single unique bracketed IRR in `[bracket_lo, bracket_hi]` using `fmine_numerics::brent_root`.
///
/// Refuses:
/// - `FMINE-REF-104` when `flows` has more than one sign change (multiple roots possible; never
///   silently pick one — use [`find_all_bracketed_irrs`] to enumerate all bracketed roots);
/// - `FMINE-REF-207` on no sign change, non-finite evaluation, or non-convergence (`bd-t1dk`);
/// - `FMINE-REF-107` if `TaxBasis::AfterTax` lacks a tax schedule.
pub fn solve_irr_bracketed(
    flows: &[ExactQty],
    convention: DiscountConvention,
    tax_basis: &TaxBasis,
    bracket_lo: ExactQty,
    bracket_hi: ExactQty,
    tol: ExactQty,
    max_iter: usize,
) -> Result<IrrRoot, EconError> {
    let effective = resolve_effective_flows(flows, tax_basis)?;
    let sign_changes = count_cash_flow_sign_changes(&effective);
    if sign_changes == 0 {
        return Err(EconError::NumericalRootFailed {
            detail: "cash-flow schedule has zero sign changes; IRR is undefined".to_string(),
        });
    }
    if sign_changes > 1 {
        return Err(EconError::NonIdentifiableConvention {
            detail: format!(
                "cash-flow schedule has {sign_changes} sign changes (multiple IRRs possible); \
                 refusing to pick a single root — use find_all_bracketed_irrs"
            ),
        });
    }
    solve_irr_single_bracket_unchecked_sign_changes(
        &effective, convention, bracket_lo, bracket_hi, tol, max_iter,
    )
}

fn solve_irr_single_bracket_unchecked_sign_changes(
    effective: &[ExactQty],
    convention: DiscountConvention,
    bracket_lo: ExactQty,
    bracket_hi: ExactQty,
    tol: ExactQty,
    max_iter: usize,
) -> Result<IrrRoot, EconError> {
    if bracket_lo.checked_cmp(bracket_hi)?.is_ge() {
        return Err(EconError::InvalidDomain {
            field: "irr_bracket",
            detail: "IRR bracket requires bracket_lo < bracket_hi".to_string(),
        });
    }
    let neg_one = ExactQty::from_integer(-1);
    if bracket_lo.checked_cmp(neg_one)?.is_le() {
        return Err(EconError::InvalidDomain {
            field: "irr_bracket",
            detail: "IRR bracket_lo must be strictly greater than -1".to_string(),
        });
    }
    if tol.signum() <= 0 {
        return Err(EconError::InvalidDomain {
            field: "tol",
            detail: "IRR tolerance must be strictly positive".to_string(),
        });
    }
    if max_iter == 0 {
        return Err(EconError::NumericalRootFailed {
            detail: "IRR max_iter must be >= 1".to_string(),
        });
    }

    let flows_f64: Vec<f64> = effective.iter().map(|q| q.to_f64()).collect();
    let root = brent_root(
        bracket_lo.to_f64(),
        bracket_hi.to_f64(),
        tol.to_f64(),
        max_iter,
        |r| npv_f64_eval(&flows_f64, r, convention),
    )
    .map_err(|err| match err {
        RootError::NoSignChange { fa, fb } => EconError::NumericalRootFailed {
            detail: format!("IRR bracket has no sign change (f(lo)={fa}, f(hi)={fb})"),
        },
        RootError::NonFinite { at } => EconError::NumericalRootFailed {
            detail: format!("IRR NPV evaluation non-finite at r={at}"),
        },
        RootError::NotConverged {
            iterations,
            bracket_width,
            f_b,
        } => EconError::NumericalRootFailed {
            detail: format!(
                "IRR did not converge after {iterations} iterations (width={bracket_width}, f={f_b})"
            ),
        },
    })?;

    Ok(IrrRoot {
        rate: f64_to_exact_1e9(root.x)?,
        bracket_lo: f64_to_exact_1e9(root.bracket.0)?,
        bracket_hi: f64_to_exact_1e9(root.bracket.1)?,
        iterations: root.iterations,
    })
}

/// Parameters for multi-root IRR scanning across `[scan_lo, scan_hi]`.
#[derive(Debug, Clone)]
pub struct IrrScanRequest<'a> {
    pub flows: &'a [ExactQty],
    pub convention: DiscountConvention,
    pub tax_basis: &'a TaxBasis,
    pub scan_lo: ExactQty,
    pub scan_hi: ExactQty,
    pub subdivisions: u32,
    pub tol: ExactQty,
    pub max_iter: usize,
}

/// Enumerate every bracketed IRR across `subdivisions` uniform sub-brackets on `[scan_lo, scan_hi]`
/// (for non-conventional multi-sign-change cash flows; never silently picks a single root).
pub fn find_all_bracketed_irrs(req: &IrrScanRequest<'_>) -> Result<Vec<IrrRoot>, EconError> {
    let effective = resolve_effective_flows(req.flows, req.tax_basis)?;
    if count_cash_flow_sign_changes(&effective) == 0 {
        return Err(EconError::NumericalRootFailed {
            detail: "cash-flow schedule has zero sign changes; IRR is undefined".to_string(),
        });
    }
    if req.subdivisions == 0 {
        return Err(EconError::InvalidDomain {
            field: "subdivisions",
            detail: "IRR scan subdivisions must be >= 1".to_string(),
        });
    }
    if req.scan_lo.checked_cmp(req.scan_hi)?.is_ge() {
        return Err(EconError::InvalidDomain {
            field: "scan_bracket",
            detail: "scan_lo must be < scan_hi".to_string(),
        });
    }
    let step = req
        .scan_hi
        .checked_sub(req.scan_lo)?
        .checked_div(ExactQty::from_integer(i128::from(req.subdivisions)))?;
    let flows_f64: Vec<f64> = effective.iter().map(|q| q.to_f64()).collect();

    let mut roots: Vec<IrrRoot> = Vec::new();
    for i in 0..req.subdivisions {
        let a = req
            .scan_lo
            .checked_add(step.checked_mul(ExactQty::from_integer(i128::from(i)))?)?;
        let b = if i + 1 == req.subdivisions {
            req.scan_hi
        } else {
            req.scan_lo
                .checked_add(step.checked_mul(ExactQty::from_integer(i128::from(i + 1)))?)?
        };
        let fa = npv_f64_eval(&flows_f64, a.to_f64(), req.convention);
        let fb = npv_f64_eval(&flows_f64, b.to_f64(), req.convention);
        if !fa.is_finite() || !fb.is_finite() {
            return Err(EconError::NumericalRootFailed {
                detail: "non-finite NPV during multi-root IRR scan".to_string(),
            });
        }
        if fa == 0.0 || fb == 0.0 || (fa < 0.0) != (fb < 0.0) {
            let root = solve_irr_single_bracket_unchecked_sign_changes(
                &effective,
                req.convention,
                a,
                b,
                req.tol,
                req.max_iter,
            )?;
            // Deduplicate exact boundary hits shared between adjacent sub-brackets.
            let is_dup = roots.last().is_some_and(|prev| {
                prev.rate
                    .checked_sub(root.rate)
                    .ok()
                    .and_then(|d| {
                        let abs_d = if d.signum() < 0 {
                            ExactQty::from_integer(0).checked_sub(d).ok()?
                        } else {
                            d
                        };
                        abs_d
                            .checked_cmp(req.tol.checked_mul(ExactQty::from_integer(10)).ok()?)
                            .ok()
                    })
                    .is_some_and(|ord| ord.is_le())
            });
            if !is_dup {
                roots.push(root);
            }
        }
    }
    if roots.is_empty() {
        return Err(EconError::NumericalRootFailed {
            detail: "no bracketed IRR found across scan interval".to_string(),
        });
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_evidence::RefusalCode;

    fn q(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).unwrap()
    }

    /// Synthetic known-answer cash flows from `bd-gqu3.3`: `[-100, 30, 40, 50, 20]` ($M) at `r = 5%`.
    fn synthetic_flows() -> Vec<ExactQty> {
        vec![
            ExactQty::from_integer(-100),
            ExactQty::from_integer(30),
            ExactQty::from_integer(40),
            ExactQty::from_integer(50),
            ExactQty::from_integer(20),
        ]
    }

    #[test]
    fn test_npv_end_year_24_50() {
        let ivl = npv_interval(
            &synthetic_flows(),
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::EndYear,
            &TaxBasis::PreTax,
            9,
        )
        .unwrap();
        assert_eq!(ivl.lo, ivl.hi);
        assert!((ivl.lo.to_f64() - 24.498_537).abs() < 1e-5);
        assert_eq!(format!("{:.2}", ivl.lo.to_f64()), "24.50");
    }

    #[test]
    fn test_npv_mid_year_all_25_10() {
        let ivl = npv_interval(
            &synthetic_flows(),
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::MidYearAll,
            &TaxBasis::PreTax,
            9,
        )
        .unwrap();
        let mid = ivl.midpoint().unwrap().to_f64();
        assert!((mid - 25.103_530).abs() < 1e-5, "got {mid}");
        assert_eq!(format!("{mid:.2}"), "25.10");
    }

    #[test]
    fn test_npv_year0_undiscounted_27_57() {
        let ivl = npv_interval(
            &synthetic_flows(),
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::Year0UndiscountedMidYearOps,
            &TaxBasis::PreTax,
            9,
        )
        .unwrap();
        let mid = ivl.midpoint().unwrap().to_f64();
        assert!((mid - 27.573_038).abs() < 1e-5, "got {mid}");
        assert_eq!(format!("{mid:.2}"), "27.57");
    }

    #[test]
    fn test_printed_25_1_identifies_mid_year_all() {
        let flows = synthetic_flows();
        let res = discover_discounting_convention(&ConventionDiscoveryRequest {
            report_id: "synthetic-dev-report",
            report_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            flows: &flows,
            flow_rounding_step: ExactQty::from_integer(0),
            discount_rate: q(5, 100),
            printed_npv: q(251, 10), // printed "25.1" -> [25.05, 25.15]
            printed_step: q(1, 10),
            tax_basis: &TaxBasis::PreTax,
        })
        .unwrap();
        assert_eq!(res.discovered, DiscountConvention::MidYearAll);
        assert_eq!(res.evaluations.len(), 3);
        assert!(res.evaluations[1].matches_printed);
        assert!(!res.evaluations[0].matches_printed);
        assert!(!res.evaluations[2].matches_printed);
        assert!(
            res.evaluations[1]
                .log_line
                .contains("\"event\":\"npv_convention\"")
        );
        assert!(res.summary_log_line.contains("\"event\":\"npv_replay\""));
    }

    #[test]
    fn test_convention_not_discovered_zero_or_several() {
        let flows = synthetic_flows();
        // Zero matches: printed "10.0" ± 0.05 matches none of {24.50, 25.10, 27.57}.
        let err_zero = discover_discounting_convention(&ConventionDiscoveryRequest {
            report_id: "syn",
            report_sha256: "00",
            flows: &flows,
            flow_rounding_step: ExactQty::from_integer(0),
            discount_rate: q(5, 100),
            printed_npv: ExactQty::from_integer(10),
            printed_step: q(1, 10),
            tax_basis: &TaxBasis::PreTax,
        })
        .unwrap_err();
        assert_eq!(
            err_zero.to_refusal().code,
            RefusalCode::NonIdentifiableAttribution104.spec().code
        );
        assert!(err_zero.to_string().contains("convention not discovered"));

        // Several matches: coarse printed step ±5.0 covers all three conventions.
        let err_many = discover_discounting_convention(&ConventionDiscoveryRequest {
            report_id: "syn",
            report_sha256: "00",
            flows: &flows,
            flow_rounding_step: ExactQty::from_integer(0),
            discount_rate: q(5, 100),
            printed_npv: ExactQty::from_integer(25),
            printed_step: ExactQty::from_integer(10),
            tax_basis: &TaxBasis::PreTax,
        })
        .unwrap_err();
        assert_eq!(
            err_many.to_refusal().code,
            RefusalCode::NonIdentifiableAttribution104.spec().code
        );
    }

    #[test]
    fn test_irr_refuses_no_sign_change_bracket() {
        // Synthetic example from bd-gqu3.3: -100, +230, -132 has roots at 10% and 20%;
        // bracket [0.30, 0.50] has no sign change.
        let flows = vec![
            ExactQty::from_integer(-100),
            ExactQty::from_integer(230),
            ExactQty::from_integer(-132),
        ];
        let err = find_all_bracketed_irrs(&IrrScanRequest {
            flows: &flows,
            convention: DiscountConvention::EndYear,
            tax_basis: &TaxBasis::PreTax,
            scan_lo: q(30, 100),
            scan_hi: q(50, 100),
            subdivisions: 10,
            tol: q(1, 1_000_000_000),
            max_iter: 100,
        })
        .unwrap_err();
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::NumericalCertificateFailed207.spec().code
        );

        // Single-sign-change flows on a bracket with no sign change also refuse.
        let err2 = solve_irr_bracketed(
            &synthetic_flows(),
            DiscountConvention::EndYear,
            &TaxBasis::PreTax,
            q(50, 100),
            q(80, 100),
            q(1, 1_000_000_000),
            100,
        )
        .unwrap_err();
        assert_eq!(
            err2.to_refusal().code,
            RefusalCode::NumericalCertificateFailed207.spec().code
        );
    }

    #[test]
    fn test_irr_two_sign_changes_never_picks_one() {
        let flows = vec![
            ExactQty::from_integer(-100),
            ExactQty::from_integer(230),
            ExactQty::from_integer(-132),
        ];
        // Single-root solver refuses multi-sign-change schedule with FMINE-REF-104 (never picks one!).
        let err = solve_irr_bracketed(
            &flows,
            DiscountConvention::EndYear,
            &TaxBasis::PreTax,
            q(0, 1),
            q(50, 100),
            q(1, 1_000_000_000),
            100,
        )
        .unwrap_err();
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::NonIdentifiableAttribution104.spec().code
        );

        // Multi-root enumerator reports BOTH exact roots (10% and 20%) cleanly.
        let roots = find_all_bracketed_irrs(&IrrScanRequest {
            flows: &flows,
            convention: DiscountConvention::EndYear,
            tax_basis: &TaxBasis::PreTax,
            scan_lo: q(0, 1),
            scan_hi: q(50, 100),
            subdivisions: 10,
            tol: q(1, 1_000_000_000),
            max_iter: 100,
        })
        .unwrap();
        assert_eq!(roots.len(), 2);
        assert!((roots[0].rate.to_f64() - 0.10).abs() < 1e-7);
        assert!((roots[1].rate.to_f64() - 0.20).abs() < 1e-7);
    }

    #[test]
    fn test_irr_refuses_not_converged() {
        // Only 1 iteration on wide bracket [0.0, 0.5] with tight tol 1e-12 cannot converge -> refuses!
        let err = solve_irr_bracketed(
            &synthetic_flows(),
            DiscountConvention::EndYear,
            &TaxBasis::PreTax,
            q(0, 1),
            q(50, 100),
            q(1, 1_000_000_000_000),
            1,
        )
        .unwrap_err();
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::NumericalCertificateFailed207.spec().code
        );
        assert!(err.to_string().contains("did not converge"));
    }

    #[test]
    fn test_after_tax_without_schedule_refuses() {
        let err = npv_interval(
            &synthetic_flows(),
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::EndYear,
            &TaxBasis::AfterTax { tax_outflows: None },
            9,
        )
        .unwrap_err();
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );
    }

    #[test]
    fn test_scaling_flows_by_k_scales_npv_keeps_convention() {
        let k = ExactQty::from_integer(7);
        let scaled: Vec<ExactQty> = synthetic_flows()
            .into_iter()
            .map(|c| c.checked_mul(k).unwrap())
            .collect();
        let base_mid = npv_interval(
            &synthetic_flows(),
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::MidYearAll,
            &TaxBasis::PreTax,
            9,
        )
        .unwrap();
        let scaled_mid = npv_interval(
            &scaled,
            ExactQty::from_integer(0),
            q(5, 100),
            DiscountConvention::MidYearAll,
            &TaxBasis::PreTax,
            9,
        )
        .unwrap();
        assert_eq!(scaled_mid.lo, base_mid.lo.checked_mul(k).unwrap());
        assert_eq!(scaled_mid.hi, base_mid.hi.checked_mul(k).unwrap());

        let disc = discover_discounting_convention(&ConventionDiscoveryRequest {
            report_id: "scaled",
            report_sha256: "00",
            flows: &scaled,
            flow_rounding_step: ExactQty::from_integer(0),
            discount_rate: q(5, 100),
            printed_npv: q(251, 10).checked_mul(k).unwrap(),
            printed_step: q(1, 10).checked_mul(k).unwrap(),
            tax_basis: &TaxBasis::PreTax,
        })
        .unwrap();
        assert_eq!(disc.discovered, DiscountConvention::MidYearAll);
    }

    #[test]
    fn test_mid_year_factor_enclosure_contains_root() {
        for r_bps in [100_i128, 500, 800, 1000, 1250] {
            let one_plus_r = ExactQty::new(10_000 + r_bps, 10_000).unwrap();
            let ivl = sqrt_enclosure(one_plus_r, 9).unwrap();
            let lo_sq = ivl.lo.checked_mul(ivl.lo).unwrap();
            let hi_sq = ivl.hi.checked_mul(ivl.hi).unwrap();
            assert!(
                lo_sq.checked_cmp(one_plus_r).unwrap().is_le(),
                "lo^2 must be <= 1 + r"
            );
            assert!(
                hi_sq.checked_cmp(one_plus_r).unwrap().is_ge(),
                "hi^2 must be >= 1 + r"
            );
            let width = ivl.hi.checked_sub(ivl.lo).unwrap();
            assert!(
                width
                    .checked_cmp(ExactQty::new(1, 1_000_000_000).unwrap())
                    .unwrap()
                    .is_le()
            );
        }
    }
}
