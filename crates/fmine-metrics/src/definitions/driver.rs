//! DESIGN: single-driver fixed-plus-variable cost estimation (BD-061, plan §21.6).
//! Exact OLS on declared comparable periods; covariance is conditional on the declared
//! fixed-driver, iid homoskedastic cost-error model, never calibrated coverage.
//! Determinism: Exact

use fmine_claim::{AdditivityClass, Claim, ClaimId, Representation, SpatialSupport};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{Currency, ExactQty as Rat, Money, UnitsError};

use super::{
    claims::{self, CostInput, CostValue, DeclaredDriver, Scope},
    declared,
};
use crate::guidance::{Ownership, Period};

#[cfg(test)]
mod tests;
pub mod worksheet;

pub const INPUT_METRIC: &str = "cost.driver.input/1";
pub const OUTPUT_METRIC: &str = "cost.driver.fitted/1";
pub const ERROR_BASIS: &str = "iid_homoskedastic_cost_errors_fixed_driver";
pub const ROUNDING: &str = "nearest_ties_even_0_0001_usd";
pub const NO_CLAIM: &str = "Conditional single-driver OLS cost estimate. Declared historical costs, accounting certificate references, availability and driver quantities are not independently authenticated. Signed coefficients and residuals are retained. Parameter and forecast variances are algebraic estimates under the declared iid homoskedastic cost-error and fixed-driver assumptions; zero residual variance is not proof of certainty. No calibrated interval, source audit, prospective registration, price-linked cost model or operating authorization is supplied.";
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

/// Every row carries its own physical period and common accounting/ownership scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverInput {
    pub scope: Scope,
    pub value: DriverValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverValue {
    Model {
        item: String,
        driver: String,
        unit: String,
        quantity_domain: [Rat; 2],
        valid_from: Period,
        valid_through: Period,
        information_cut: Period,
        error_basis: String,
        monetary_rounding: String,
    },
    Observation {
        quantity: Rat,
        usd: Money,
        available: Period,
        accounting_certificate: String,
    },
    Target {
        quantity: Rat,
    },
}

fn put_period(enc: &mut Encoder, p: Period) {
    enc.put_u16(p.year);
    enc.put_u8(p.first_month);
    enc.put_u8(p.last_month);
}
fn put_rat(enc: &mut Encoder, q: Rat) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

impl DriverInput {
    fn key(&self) -> (u8, u16, u8, u8) {
        let role = match self.value {
            DriverValue::Model { .. } => 0,
            DriverValue::Observation { .. } => 1,
            DriverValue::Target { .. } => 2,
        };
        let p = self.scope.period;
        (role, p.year, p.first_month, p.last_month)
    }
    pub fn additivity(&self) -> AdditivityClass {
        AdditivityClass::NonAdditive
    }
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"CDRI", 1);
        claims::put_scope(&mut e, &self.scope);
        e.put_u8(self.key().0);
        match &self.value {
            DriverValue::Model {
                item,
                driver,
                unit,
                quantity_domain,
                valid_from,
                valid_through,
                information_cut,
                error_basis,
                monetary_rounding,
            } => {
                for s in [item, driver, unit, error_basis, monetary_rounding] {
                    e.put_str(s);
                }
                for q in quantity_domain {
                    put_rat(&mut e, *q);
                }
                for p in [valid_from, valid_through, information_cut] {
                    put_period(&mut e, *p);
                }
            }
            DriverValue::Observation {
                quantity,
                usd,
                available,
                accounting_certificate,
            } => {
                put_rat(&mut e, *quantity);
                e.put_str(&format!("{:?}", usd.ccy));
                e.put_i128(usd.minor);
                put_period(&mut e, *available);
                e.put_str(accounting_certificate);
            }
            DriverValue::Target { quantity } => put_rat(&mut e, *quantity),
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FittedRow {
    pub input_claim_id: ClaimId,
    pub period: Period,
    pub quantity: Rat,
    pub observed_usd: Rat,
    pub fitted_usd: Rat,
    /// Observed minus fitted; neither signs nor outliers are removed.
    pub residual_usd: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverSummary {
    pub scope: Scope,
    pub item: String,
    pub driver: String,
    pub unit: String,
    pub quantity_domain: [Rat; 2],
    pub information_cut: Period,
    pub valid_from: Period,
    pub valid_through: Period,
    pub fixed_usd: Rat,
    pub usd_per_unit: Rat,
    pub mean_driver: Rat,
    pub centered_driver_sum_squares: Rat,
    pub residual_sum_squares_usd2: Rat,
    pub residual_variance_usd2: Rat,
    /// Row-major 2x2 matrix, ordered (fixed USD, USD per driver unit).
    pub coefficient_covariance: [Rat; 4],
    pub rows: Vec<FittedRow>,
    pub target_quantity: Rat,
    pub mean_usd: Rat,
    pub mean_variance_usd2: Rat,
    pub observation_variance_usd2: Rat,
    pub rounded_usd: Money,
    pub rounding_delta_usd: Rat,
}
pub type DriverReceipt = declared::Receipt<DriverInput, DriverSummary>;

fn bad(code: RefusalCode, text: &str) -> Refusal {
    Refusal::new(code, format!("cost driver: {text}"))
}
fn exact<T>(v: std::result::Result<T, UnitsError>) -> Result<T> {
    v.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact arithmetic overflow",
        )
    })
}
fn named(s: &str) -> bool {
    !s.is_empty() && s.len() <= 4096 && s.trim() == s && !s.chars().any(char::is_control)
}
fn start(p: Period) -> u32 {
    u32::from(p.year) * 12 + u32::from(p.first_month)
}
fn end(p: Period) -> u32 {
    u32::from(p.year) * 12 + u32::from(p.last_month)
}
fn period_ok(p: Period) -> bool {
    p.year > 0
        && p.year <= 9999
        && p.first_month > 0
        && p.first_month <= p.last_month
        && p.last_month <= 12
}
fn square(q: Rat) -> Result<Rat> {
    exact(q.checked_mul(q))
}

/// Exact monetary boundary selected explicitly in the model declaration.
fn round_money(usd: Rat) -> Result<Money> {
    let scaled = exact(usd.checked_mul(Rat::from_integer(Money::SCALE)))?;
    let (n, d) = (scaled.num(), scaled.den());
    let floor = n.div_euclid(d);
    let remainder = n.rem_euclid(d);
    let round_up = remainder > d - remainder || (remainder == d - remainder && floor % 2 != 0);
    let minor = if round_up {
        floor.checked_add(1).ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "monetary rounding overflow",
            )
        })?
    } else {
        floor
    };
    Ok(Money::from_minor(Currency::USD, minor))
}

/// Fit and forecast one comparable cost pool. Three to 128 distinct, nonoverlapping
/// periods are required; there is no automatic outlier removal or extrapolation.
pub fn fit(inputs: &[Claim<DriverInput>]) -> Result<DriverReceipt> {
    if !(5..=130).contains(&inputs.len()) {
        return Err(bad(
            RefusalCode::NotObservable202,
            "one model, one target and 3..128 observations are required",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by_key(|c| c.value.key());
    for c in &inputs {
        declared::admit(
            c,
            INPUT_METRIC,
            &c.value.canonical_tag(),
            c.value.additivity(),
            named,
            bad,
        )?;
        let s = &c.value.scope;
        if !period_ok(s.period) || !named(&s.issuer) || !named(&s.subject) || !named(&s.basis) {
            return Err(bad(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "named scopes and valid calendar periods required",
            ));
        }
    }
    let first = &inputs[0].value;
    let DriverValue::Model {
        item,
        driver,
        unit,
        quantity_domain,
        valid_from,
        valid_through,
        information_cut,
        error_basis,
        monetary_rounding,
    } = &first.value
    else {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "model declaration missing",
        ));
    };
    let last = &inputs[inputs.len() - 1].value;
    let DriverValue::Target { quantity: target } = last.value else {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "target quantity missing",
        ));
    };
    let scope = &last.scope;
    if first.scope != *scope
        || inputs.iter().any(|c| {
            let s = &c.value.scope;
            s.issuer != scope.issuer
                || s.subject != scope.subject
                || s.basis != scope.basis
                || s.ownership != scope.ownership
                || s.period.last_month - s.period.first_month
                    != scope.period.last_month - scope.period.first_month
        })
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "issuer, site, basis, ownership and period duration must agree",
        ));
    }
    if let Ownership::Attributable { share } = scope.ownership
        && (share.signum() <= 0 || exact(share.checked_cmp(ONE))?.is_gt())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "ownership share must lie in (0,1]",
        ));
    }
    DeclaredDriver {
        kind: driver.clone(),
        unit: unit.clone(),
        quantity: ZERO,
        usd_per_unit: ZERO,
        fixed_usd: Money::from_minor(Currency::USD, 0),
    }
    .amount()?;
    if !named(item) || error_basis != ERROR_BASIS || monetary_rounding != ROUNDING {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare the cost item, iid fixed-driver error basis and nearest-even monetary rounding",
        ));
    }
    if !period_ok(*valid_from)
        || !period_ok(*valid_through)
        || !period_ok(*information_cut)
        || start(*valid_from) > start(scope.period)
        || end(scope.period) > end(*valid_through)
        || end(*information_cut) >= start(scope.period)
    {
        return Err(bad(
            RefusalCode::AssayLookahead108,
            "target must follow the information cut and lie inside the declared validity dates",
        ));
    }
    let [low, high] = *quantity_domain;
    if low.signum() < 0
        || exact(low.checked_cmp(high))?.is_gt()
        || exact(target.checked_cmp(low))?.is_lt()
        || exact(target.checked_cmp(high))?.is_gt()
    {
        return Err(bad(
            RefusalCode::OutsideAoa203,
            "target is outside the declared nonnegative driver domain",
        ));
    }
    let mut rows = Vec::new();
    let (mut sum_x, mut sum_y) = (ZERO, ZERO);
    let (mut min_x, mut max_x) = (None, None);
    let mut previous_end = 0;
    for c in &inputs[1..inputs.len() - 1] {
        let DriverValue::Observation {
            quantity,
            usd,
            available,
            accounting_certificate,
        } = &c.value.value
        else {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "exactly one model and target are required",
            ));
        };
        let p = c.value.scope.period;
        if start(p) <= previous_end
            || !period_ok(*available)
            || end(*available) < end(p)
            || end(*available) > end(*information_cut)
        {
            return Err(bad(
                RefusalCode::AssayLookahead108,
                "training periods overlap or an observation is unavailable by the information cut",
            ));
        }
        previous_end = end(p);
        if quantity.signum() < 0 || usd.ccy != Currency::USD || !named(accounting_certificate) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "nonnegative drivers, USD costs and explicit accounting certificate references required",
            ));
        }
        min_x = Some(match min_x {
            Some(v) if exact(quantity.checked_cmp(v))?.is_ge() => v,
            _ => *quantity,
        });
        max_x = Some(match max_x {
            Some(v) if exact(quantity.checked_cmp(v))?.is_le() => v,
            _ => *quantity,
        });
        let y = exact(Rat::new(usd.minor, Money::SCALE))?;
        sum_x = exact(sum_x.checked_add(*quantity))?;
        sum_y = exact(sum_y.checked_add(y))?;
        rows.push(FittedRow {
            input_claim_id: c.claim_id,
            period: p,
            quantity: *quantity,
            observed_usd: y,
            fitted_usd: ZERO,
            residual_usd: ZERO,
        });
    }
    let (Some(min_x), Some(max_x)) = (min_x, max_x) else {
        return Err(bad(RefusalCode::NotObservable202, "no observations"));
    };
    if exact(low.checked_cmp(min_x))?.is_lt() || exact(high.checked_cmp(max_x))?.is_gt() {
        return Err(bad(
            RefusalCode::OutsideAoa203,
            "declared domain extends beyond observed driver support",
        ));
    }
    let n = Rat::from_integer(rows.len() as i128);
    let mean_x = exact(sum_x.checked_div(n))?;
    let mean_y = exact(sum_y.checked_div(n))?;
    let (mut sxx, mut sxy) = (ZERO, ZERO);
    for row in &rows {
        let dx = exact(row.quantity.checked_sub(mean_x))?;
        sxx = exact(sxx.checked_add(square(dx)?))?;
        sxy = exact(sxy.checked_add(exact(
            dx.checked_mul(exact(row.observed_usd.checked_sub(mean_y))?),
        )?))?;
    }
    if sxx == ZERO {
        return Err(bad(
            RefusalCode::NonIdentifiableAttribution104,
            "constant driver cannot identify separate fixed and variable costs",
        ));
    }
    let rate = exact(sxy.checked_div(sxx))?;
    let fixed = exact(mean_y.checked_sub(exact(rate.checked_mul(mean_x))?))?;
    let mut sse = ZERO;
    for row in &mut rows {
        row.fitted_usd = exact(fixed.checked_add(exact(rate.checked_mul(row.quantity))?))?;
        row.residual_usd = exact(row.observed_usd.checked_sub(row.fitted_usd))?;
        sse = exact(sse.checked_add(square(row.residual_usd)?))?;
    }
    let variance = exact(sse.checked_div(Rat::from_integer(rows.len() as i128 - 2)))?;
    let slope_variance = exact(variance.checked_div(sxx))?;
    let intercept_variance = exact(
        exact(variance.checked_div(n))?
            .checked_add(exact(square(mean_x)?.checked_mul(slope_variance))?),
    )?;
    let cross = exact(exact(mean_x.checked_mul(slope_variance))?.checked_neg())?;
    let mean = exact(fixed.checked_add(exact(rate.checked_mul(target))?))?;
    let mean_variance = exact(exact(variance.checked_div(n))?.checked_add(exact(
        square(exact(target.checked_sub(mean_x))?)?.checked_mul(slope_variance),
    )?))?;
    let rounded = round_money(mean)?;
    let summary = DriverSummary {
        scope: scope.clone(),
        item: item.clone(),
        driver: driver.clone(),
        unit: unit.clone(),
        quantity_domain: *quantity_domain,
        information_cut: *information_cut,
        valid_from: *valid_from,
        valid_through: *valid_through,
        fixed_usd: fixed,
        usd_per_unit: rate,
        mean_driver: mean_x,
        centered_driver_sum_squares: sxx,
        residual_sum_squares_usd2: sse,
        residual_variance_usd2: variance,
        coefficient_covariance: [intercept_variance, cross, cross, slope_variance],
        rows,
        target_quantity: target,
        mean_usd: mean,
        mean_variance_usd2: mean_variance,
        observation_variance_usd2: exact(variance.checked_add(mean_variance))?,
        rounded_usd: rounded,
        rounding_delta_usd: exact(exact(Rat::new(rounded.minor, Money::SCALE))?.checked_sub(mean))?,
    };
    let mut e = Encoder::new_frame(*b"CDRF", 1);
    e.put_str(&summary.to_json());
    for input in &inputs {
        claims::put_envelope(&mut e, input);
    }
    DriverReceipt::from_modelled(
        inputs,
        OUTPUT_METRIC,
        summary,
        blake3_hash(&e.finish()).to_hex(),
        bad,
    )
}

impl DriverReceipt {
    /// Feed the fitted line through the existing exact definition owner. The model receipt
    /// remains its parent; rounded and unrounded means are separate retained values.
    pub fn predicted_line(&self) -> Claim<CostInput> {
        let s = self.summary();
        let input = CostInput {
            scope: s.scope.clone(),
            value: CostValue::Line {
                item: s.item.clone(),
                amount: s.rounded_usd,
            },
        };
        let tag = input.canonical_tag();
        let c = self.claim();
        Claim::new(
            claims::INPUT_METRIC,
            input,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            c.evidence,
            c.epistemic,
            c.state,
            c.confidentiality.clone(),
            c.uu_terms.clone(),
            vec![c.claim_id],
            c.anchors.clone(),
        )
    }

    pub fn to_json(&self) -> String {
        use declared::{claim_json, join_json, js};
        let summary = self.summary().to_json();
        format!(
            "{{\"schema\":\"fmine.cost-driver-result.v1\",\"digest\":{},\"calibrated\":false,\"error_basis\":{},\"monetary_rounding\":{},\"claim\":{},\"inputs\":[{}],\"no_claim\":{}}}",
            js(self.digest()),
            js(ERROR_BASIS),
            js(ROUNDING),
            claim_json(self.claim(), &summary, true),
            join_json(self.inputs(), |c| claim_json(c, &c.value.to_json(), true)),
            js(NO_CLAIM)
        )
    }

    pub fn to_markdown(&self) -> String {
        use declared::qty;
        use std::fmt::Write as _;
        let s = self.summary();
        let text = |v: &str| {
            v.replace('\\', "\\\\")
                .replace('`', "\\`")
                .replace('|', "\\|")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let mut out = format!(
            "## Fitted cost driver\n\n{} / {} / {}. Driver: {} ({}).\n\nCost = {} USD + {} USD/{} × quantity. Coefficients retain their fitted signs.\n\n{} training periods; residual degrees of freedom {}. Residual variance: {} USD².\n\nTarget quantity {}; mean cost {} USD; rounded cost {}/10000 USD; rounding difference {} USD.\n\nVariance of fitted mean: {} USD². Variance of a new comparable cost observation: {} USD². These are conditional variance estimates, not calibrated intervals.\n\n| Period | Driver | Observed USD | Fitted USD | Observed minus fitted USD |\n|---|---:|---:|---:|---:|\n",
            text(&s.scope.issuer),
            text(&s.scope.subject),
            text(&s.item),
            text(&s.driver),
            text(&s.unit),
            qty(s.fixed_usd),
            qty(s.usd_per_unit),
            text(&s.unit),
            s.rows.len(),
            s.rows.len() - 2,
            qty(s.residual_variance_usd2),
            qty(s.target_quantity),
            qty(s.mean_usd),
            s.rounded_usd.minor,
            qty(s.rounding_delta_usd),
            qty(s.mean_variance_usd2),
            qty(s.observation_variance_usd2)
        );
        for r in &s.rows {
            let _ = writeln!(
                out,
                "| {} months {}–{} | {} | {} | {} | {} |",
                r.period.year,
                r.period.first_month,
                r.period.last_month,
                qty(r.quantity),
                qty(r.observed_usd),
                qty(r.fitted_usd),
                qty(r.residual_usd)
            );
        }
        let _ = writeln!(
            out,
            "\nInformation cut: {} months {}–{}. Driver validity: [{}, {}] {}; calendar validity {} months {}–{} through {} months {}–{}.\n\nCoefficient covariance (fixed, variable): [{}, {}; {}, {}].\n\nModel Claim `{}`; receipt `{}`.\n\n{}\n\n### Training and declaration sources\n",
            s.information_cut.year,
            s.information_cut.first_month,
            s.information_cut.last_month,
            qty(s.quantity_domain[0]),
            qty(s.quantity_domain[1]),
            text(&s.unit),
            s.valid_from.year,
            s.valid_from.first_month,
            s.valid_from.last_month,
            s.valid_through.year,
            s.valid_through.first_month,
            s.valid_through.last_month,
            qty(s.coefficient_covariance[0]),
            qty(s.coefficient_covariance[1]),
            qty(s.coefficient_covariance[2]),
            qty(s.coefficient_covariance[3]),
            self.claim().claim_id,
            self.digest(),
            NO_CLAIM
        );
        for input in self.inputs() {
            if let DriverValue::Observation {
                accounting_certificate,
                available,
                ..
            } = &input.value.value
            {
                let _ = writeln!(
                    out,
                    "- Accounting reference `{}`; declared available {} months {}–{}; Claim `{}`.",
                    text(accounting_certificate),
                    available.year,
                    available.first_month,
                    available.last_month,
                    input.claim_id
                );
            }
            for a in &input.anchors {
                let _ = writeln!(
                    out,
                    "- {}: {}; SHA-256 `{}`.",
                    text(&a.artifact_path),
                    text(&a.locator),
                    fmine_core::Digest32(a.artifact_sha256).to_hex()
                );
            }
        }
        out
    }
}

fn period_json(p: Period) -> String {
    format!(
        "{{\"year\":{},\"first_month\":{},\"last_month\":{}}}",
        p.year, p.first_month, p.last_month
    )
}

impl DriverInput {
    fn to_json(&self) -> String {
        use declared::{js, qty};
        let value = match &self.value {
            DriverValue::Model {
                item,
                driver,
                unit,
                quantity_domain,
                valid_from,
                valid_through,
                information_cut,
                error_basis,
                monetary_rounding,
            } => format!(
                "{{\"kind\":\"model\",\"item\":{},\"driver\":{},\"unit\":{},\"quantity_domain\":[{},{}],\"valid_from\":{},\"valid_through\":{},\"information_cut\":{},\"error_basis\":{},\"monetary_rounding\":{}}}",
                js(item),
                js(driver),
                js(unit),
                js(&qty(quantity_domain[0])),
                js(&qty(quantity_domain[1])),
                period_json(*valid_from),
                period_json(*valid_through),
                period_json(*information_cut),
                js(error_basis),
                js(monetary_rounding)
            ),
            DriverValue::Observation {
                quantity,
                usd,
                available,
                accounting_certificate,
            } => format!(
                "{{\"kind\":\"observation\",\"quantity\":{},\"cost_usd\":{},\"available\":{},\"accounting_certificate\":{}}}",
                js(&qty(*quantity)),
                js(&format!("{}/10000", usd.minor)),
                period_json(*available),
                js(accounting_certificate)
            ),
            DriverValue::Target { quantity } => format!(
                "{{\"kind\":\"target\",\"quantity\":{}}}",
                js(&qty(*quantity))
            ),
        };
        format!(
            "{{\"scope\":{},\"value\":{value}}}",
            super::worksheet::scope_json(&self.scope)
        )
    }
}

impl DriverSummary {
    fn to_json(&self) -> String {
        use declared::{join_json, js, qty};
        let rows = join_json(&self.rows, |r| {
            format!(
                "{{\"input_claim_id\":{},\"period\":{},\"quantity\":{},\"observed_usd\":{},\"fitted_usd\":{},\"residual_usd\":{}}}",
                js(&r.input_claim_id.to_string()),
                period_json(r.period),
                js(&qty(r.quantity)),
                js(&qty(r.observed_usd)),
                js(&qty(r.fitted_usd)),
                js(&qty(r.residual_usd))
            )
        });
        format!(
            "{{\"scope\":{},\"item\":{},\"driver\":{},\"unit\":{},\"quantity_domain\":[{},{}],\"information_cut\":{},\"valid_from\":{},\"valid_through\":{},\"n\":{},\"residual_degrees_of_freedom\":{},\"fixed_usd\":{},\"usd_per_unit\":{},\"mean_driver\":{},\"centered_driver_sum_squares\":{},\"residual_sum_squares_usd2\":{},\"residual_variance_usd2\":{},\"coefficient_order\":[\"fixed_usd\",\"usd_per_unit\"],\"coefficient_covariance_layout\":\"row_major_2x2\",\"coefficient_covariance\":[{}],\"rows\":[{rows}],\"target_quantity\":{},\"mean_usd\":{},\"mean_variance_usd2\":{},\"observation_variance_usd2\":{},\"rounded_minor_usd\":{},\"rounding_delta_usd\":{}}}",
            super::worksheet::scope_json(&self.scope),
            js(&self.item),
            js(&self.driver),
            js(&self.unit),
            js(&qty(self.quantity_domain[0])),
            js(&qty(self.quantity_domain[1])),
            period_json(self.information_cut),
            period_json(self.valid_from),
            period_json(self.valid_through),
            self.rows.len(),
            self.rows.len() - 2,
            js(&qty(self.fixed_usd)),
            js(&qty(self.usd_per_unit)),
            js(&qty(self.mean_driver)),
            js(&qty(self.centered_driver_sum_squares)),
            js(&qty(self.residual_sum_squares_usd2)),
            js(&qty(self.residual_variance_usd2)),
            join_json(&self.coefficient_covariance, |q| js(&qty(*q))),
            js(&qty(self.target_quantity)),
            js(&qty(self.mean_usd)),
            js(&qty(self.mean_variance_usd2)),
            js(&qty(self.observation_variance_usd2)),
            js(&self.rounded_usd.minor.to_string()),
            js(&qty(self.rounding_delta_usd))
        )
    }
}
