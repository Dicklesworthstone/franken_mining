//! Definition-bound capital return over dated, source-bound financial Claims (BD-061, plan §21.6).
//! Determinism: Exact
//! Inherited floating metadata is retained by its exact bits, never used in arithmetic.
//!
//! DESIGN: ROCE is not a universal accounting definition. Earnings treatment, capital-employed basis,
//! averaging, ownership and annualization must be stated. Rounded source values produce closed
//! arithmetic enclosures, not confidence intervals. No tax, FX, interest or ownership bridge is inferred.

use std::fmt::Write as _;

use fmine_claim::{AdditivityClass, Claim};
use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Interval;
use fmine_time::{CivilDate, days_in_month};
use fmine_units::{Currency, ExactQty, UnitsError};

use super::declared::{Receipt, claim_json, join_json, js, ownership_json, qty};
use crate::guidance::{Ownership, Period};

pub mod source;
pub mod worksheet;

pub const INPUT_METRIC: &str = "capital-return.input/1";
pub const CAPITAL_METRIC: &str = "capital-return.capital-base/1";
pub const OUTPUT_METRIC: &str = "capital-return.assessment/1";
pub const NO_CLAIM: &str = "Exact arithmetic conditional on the cited earnings definition, capital-employed basis and common reporting scope. ROCE definitions are issuer-specific; no accounting audit, automatic adjustment, cross-definition comparability, calibrated uncertainty, investment recommendation or disclosure authority is established. Source-rounding enclosures are not confidence intervals.";

type Result<T> = std::result::Result<T, Refusal>;
const ONE: ExactQty = ExactQty::from_integer(1);
const TWO: ExactQty = ExactQty::from_integer(2);
const HUNDRED: ExactQty = ExactQty::from_integer(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxBasis {
    PreTax,
    AfterTax,
}

impl TaxBasis {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PreTax => "pre_tax",
            Self::AfterTax => "after_tax",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapitalBasis {
    Opening,
    Closing,
    OpeningClosingMean,
    ReportedAverage,
}

impl CapitalBasis {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Closing => "closing",
            Self::OpeningClosingMean => "opening_closing_mean",
            Self::ReportedAverage => "reported_average",
        }
    }

    fn roles(self) -> &'static [&'static str] {
        match self {
            Self::Opening => &["opening"],
            Self::Closing => &["closing"],
            Self::OpeningClosingMean => &["opening", "closing"],
            Self::ReportedAverage => &["average"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Annualization {
    None,
    /// A source-declared multiplier, never guessed from the number of reporting months.
    Explicit {
        multiplier: ExactQty,
        basis: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applicability {
    Applicable,
    /// An issuer's withheld/not-meaningful return remains unavailable even if operands exist.
    Excluded {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub id: String,
    pub version: String,
    pub earnings_basis: String,
    pub tax_basis: TaxBasis,
    pub capital_employed_basis: String,
    pub capital_basis: CapitalBasis,
    pub annualization: Annualization,
    pub applicability: Applicability,
}

impl Definition {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        blake3_hash(definition_json(self).as_bytes()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub issuer: String,
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    /// Names the common operating perimeter, e.g. continuing operations; no scope conversion.
    pub perimeter: String,
    pub accounting_basis: String,
    pub currency: Currency,
    /// Exactly 1, 1,000 or 1,000,000 major currency units per submitted unit. Inputs must match.
    pub monetary_scale: u32,
}

impl Scope {
    #[must_use]
    pub fn unit(&self) -> String {
        let suffix = match self.monetary_scale {
            1 => "",
            1_000 => "_k",
            1_000_000 => "_m",
            _ => "_invalid_scale",
        };
        format!("{:?}{suffix}", self.currency)
    }
}

/// An explicit zero step means exact declared input. Positive steps mean closed half-step rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amount {
    pub value: ExactQty,
    pub rounding_step: ExactQty,
}

impl Amount {
    fn enclosed(self) -> Result<Enclosed> {
        if self.rounding_step.signum() < 0 {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "rounding step must be nonnegative; zero explicitly denotes an exact input",
            ));
        }
        let half = exact(self.rounding_step.checked_div(TWO))?;
        Ok(Enclosed {
            point: self.value,
            bounds: closed(
                exact(self.value.checked_sub(half))?,
                exact(self.value.checked_add(half))?,
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValue {
    Definition(Definition),
    Earnings { period: Period, amount: Amount },
    Opening { as_of: CivilDate, amount: Amount },
    Closing { as_of: CivilDate, amount: Amount },
    Average { period: Period, amount: Amount },
}

impl InputValue {
    #[must_use]
    pub const fn role(&self) -> &'static str {
        match self {
            Self::Definition(_) => "definition",
            Self::Earnings { .. } => "earnings",
            Self::Opening { .. } => "opening",
            Self::Closing { .. } => "closing",
            Self::Average { .. } => "average",
        }
    }
}

/// Original agreed source interpretation is bound to the typed value, not just to a printed number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub quantity: String,
    pub raw_token: String,
    pub basis: String,
    pub readers: Vec<String>,
    pub published_at: String,
    pub published_at_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub scope: Scope,
    pub value: InputValue,
    pub source_record: Option<SourceRecord>,
}

impl Input {
    /// Balance snapshots and already averaged capital are not summable across reporting dates.
    #[must_use]
    pub const fn additivity(&self) -> AdditivityClass {
        match self.value {
            InputValue::Earnings { .. } => AdditivityClass::ExtensiveAdditive,
            _ => AdditivityClass::NonAdditive,
        }
    }

    #[must_use]
    pub fn canonical_tag(&self) -> String {
        blake3_hash(input_json(self).as_bytes()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enclosed {
    pub point: ExactQty,
    pub bounds: Interval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalBase {
    pub scope: Scope,
    pub definition_tag: String,
    pub basis: CapitalBasis,
    pub amount: Enclosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub scope: Scope,
    pub definition: Definition,
    pub definition_tag: String,
    pub earnings: Enclosed,
    pub capital_employed: Enclosed,
    /// Percent, not a fraction. This is always retained, even when annualization is requested.
    pub period_return_percent: Enclosed,
    pub annualized_return_percent: Option<Enclosed>,
}

/// Inputs and intermediate/output Claims are immutable after admission.
#[derive(Debug, Clone, PartialEq)]
pub struct CapitalReturnReceipt {
    inputs: Vec<Claim<Input>>,
    capital: Claim<CapitalBase>,
    claim: Claim<Summary>,
    digest: String,
}

impl CapitalReturnReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<Input>] {
        &self.inputs
    }
    #[must_use]
    pub fn capital_claim(&self) -> &Claim<CapitalBase> {
        &self.capital
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<Summary> {
        &self.claim
    }
    #[must_use]
    pub fn summary(&self) -> &Summary {
        &self.claim.value
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, detail: &str) -> Refusal {
    Refusal::new(code, format!("capital return: {detail}"))
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| bad(RefusalCode::NumericalCertificateFailed207, &e.to_string()))
}

fn named(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn closed(lo: ExactQty, hi: ExactQty) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn validate_scope(scope: &Scope) -> Result<()> {
    if [
        &scope.issuer,
        &scope.subject,
        &scope.perimeter,
        &scope.accounting_basis,
    ]
    .iter()
    .any(|s| !named(s))
        || !(1900..=9999).contains(&scope.period.year)
        || scope.period.first_month == 0
        || scope.period.first_month > scope.period.last_month
        || scope.period.last_month > 12
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "explicit named scope and valid calendar reporting window required",
        ));
    }
    if !matches!(scope.monetary_scale, 1 | 1_000 | 1_000_000) {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "monetary scale must be 1, 1000 or 1000000; no conversion is implicit",
        ));
    }
    if let Ownership::Attributable { share } = scope.ownership
        && (share.signum() <= 0 || exact(share.checked_cmp(ONE))?.is_gt())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "attributable share must be in (0, 1]",
        ));
    }
    Ok(())
}

fn validate_definition(definition: &Definition) -> Result<()> {
    if [
        &definition.id,
        &definition.version,
        &definition.earnings_basis,
        &definition.capital_employed_basis,
    ]
    .iter()
    .any(|s| !named(s))
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "definition requires an id, version, earnings treatment and capital-employed basis",
        ));
    }
    if let Annualization::Explicit { multiplier, basis } = &definition.annualization
        && (multiplier.signum() <= 0 || !named(basis))
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "annualization requires an explicit positive factor and named basis",
        ));
    }
    if let Applicability::Excluded { reason } = &definition.applicability {
        if !named(reason) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "excluded ROCE requires the stated reason",
            ));
        }
        return Err(bad(
            RefusalCode::SourceCellUnusable208,
            &format!("source withholds a meaningful return: {reason}"),
        ));
    }
    Ok(())
}

fn boundaries(period: Period) -> (CivilDate, CivilDate) {
    let (year, month) = if period.first_month == 1 {
        (i32::from(period.year) - 1, 12)
    } else {
        (i32::from(period.year), period.first_month - 1)
    };
    (
        CivilDate {
            year,
            month,
            day: days_in_month(year, month),
        },
        CivilDate {
            year: i32::from(period.year),
            month: period.last_month,
            day: days_in_month(i32::from(period.year), period.last_month),
        },
    )
}

fn mean(a: Enclosed, b: Enclosed) -> Result<Enclosed> {
    Ok(Enclosed {
        point: exact(exact(a.point.checked_add(b.point))?.checked_div(TWO))?,
        bounds: closed(
            exact(exact(a.bounds.lo.checked_add(b.bounds.lo))?.checked_div(TWO))?,
            exact(exact(a.bounds.hi.checked_add(b.bounds.hi))?.checked_div(TWO))?,
        ),
    })
}

fn scale(amount: Enclosed, factor: ExactQty) -> Result<Enclosed> {
    Ok(Enclosed {
        point: exact(amount.point.checked_mul(factor))?,
        bounds: closed(
            exact(amount.bounds.lo.checked_mul(factor))?,
            exact(amount.bounds.hi.checked_mul(factor))?,
        ),
    })
}

fn ratio(numerator: Enclosed, denominator: Enclosed) -> Result<Enclosed> {
    if denominator.bounds.lo.signum() <= 0 {
        return Err(bad(
            RefusalCode::NonIdentifiableAttribution104,
            "the complete selected capital-employed enclosure must be strictly positive",
        ));
    }
    let mut lo = exact(numerator.bounds.lo.checked_div(denominator.bounds.lo))?;
    let mut hi = lo;
    for n in [numerator.bounds.lo, numerator.bounds.hi] {
        for d in [denominator.bounds.lo, denominator.bounds.hi] {
            let value = exact(n.checked_div(d))?;
            if exact(value.checked_cmp(lo))?.is_lt() {
                lo = value;
            }
            if exact(value.checked_cmp(hi))?.is_gt() {
                hi = value;
            }
        }
    }
    scale(
        Enclosed {
            point: exact(numerator.point.checked_div(denominator.point))?,
            bounds: closed(lo, hi),
        },
        HUNDRED,
    )
}

/// Evaluate one explicitly defined return; no unused balances, inferred adjustments or fallback basis.
/// Negative earnings are valid. A positive selected average can include a zero/negative endpoint;
/// this follows the stated averaging definition and never changes an endpoint's sign.
pub fn evaluate(inputs: &[Claim<Input>]) -> Result<CapitalReturnReceipt> {
    if !(3..=4).contains(&inputs.len()) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "exactly one definition, earnings and the required one or two capital operands are needed",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by_key(|c| c.value.value.role());
    let scope = inputs[0].value.scope.clone();
    validate_scope(&scope)?;
    for input in &inputs {
        super::declared::admit(
            input,
            INPUT_METRIC,
            &input.value.canonical_tag(),
            input.value.additivity(),
            named,
            bad,
        )?;
        if input.value.scope != scope {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "issuer, subject, period, ownership, perimeter, accounting basis, currency or scale differs",
            ));
        }
    }
    if inputs
        .windows(2)
        .any(|p| p[0].value.value.role() == p[1].value.value.role())
    {
        return Err(bad(RefusalCode::BasisMismatch105, "duplicate input role"));
    }
    let definition = inputs
        .iter()
        .find_map(|c| match &c.value.value {
            InputValue::Definition(d) => Some(d.clone()),
            _ => None,
        })
        .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "missing definition Claim"))?;
    validate_definition(&definition)?;
    let mut expected = vec!["definition", "earnings"];
    expected.extend_from_slice(definition.capital_basis.roles());
    expected.sort();
    if inputs
        .iter()
        .map(|c| c.value.value.role())
        .collect::<Vec<_>>()
        != expected
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "capital operands differ from the stated basis; unused or missing balances cannot be substituted",
        ));
    }
    let (opening_date, closing_date) = boundaries(scope.period);
    let mut readings = std::collections::BTreeMap::new();
    for input in &inputs {
        let amount = match input.value.value {
            InputValue::Definition(_) => continue,
            InputValue::Earnings { period, amount } | InputValue::Average { period, amount } => {
                if period != scope.period {
                    return Err(bad(
                        RefusalCode::SupportMismatch102,
                        "earnings and reported average must cover the complete target period",
                    ));
                }
                amount
            }
            InputValue::Opening { as_of, amount } => {
                if as_of != opening_date {
                    return Err(bad(
                        RefusalCode::SupportMismatch102,
                        "opening capital must be dated at the preceding reporting close",
                    ));
                }
                amount
            }
            InputValue::Closing { as_of, amount } => {
                if as_of != closing_date {
                    return Err(bad(
                        RefusalCode::SupportMismatch102,
                        "closing capital must be dated at the target reporting close",
                    ));
                }
                amount
            }
        };
        readings.insert(input.value.value.role(), amount.enclosed()?);
    }
    let get = |role| {
        readings.get(role).copied().ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "missing required monetary operand",
            )
        })
    };
    let capital_employed = match definition.capital_basis {
        CapitalBasis::Opening => get("opening")?,
        CapitalBasis::Closing => get("closing")?,
        CapitalBasis::ReportedAverage => get("average")?,
        CapitalBasis::OpeningClosingMean => mean(get("opening")?, get("closing")?)?,
    };
    let earnings = get("earnings")?;
    let period_return_percent = ratio(earnings, capital_employed)?;
    let annualized_return_percent = match &definition.annualization {
        Annualization::None => None,
        Annualization::Explicit { multiplier, .. } => {
            Some(scale(period_return_percent, *multiplier)?)
        }
    };
    let definition_tag = definition.canonical_tag();
    let capital_value = CapitalBase {
        scope: scope.clone(),
        definition_tag: definition_tag.clone(),
        basis: definition.capital_basis,
        amount: capital_employed,
    };
    let capital_inputs: Vec<_> = inputs
        .iter()
        .filter(|c| c.value.value.role() != "earnings")
        .cloned()
        .collect();
    let mut capital_encoding = Encoder::new_frame(*b"RCAP", 1);
    for input in &capital_inputs {
        capital_encoding.put_str(&claim_json(input, &input_json(&input.value), false));
    }
    capital_encoding.put_str(&capital_json(&capital_value));
    let capital = Receipt::from_admitted(
        capital_inputs,
        CAPITAL_METRIC,
        capital_value,
        blake3_hash(&capital_encoding.finish()).to_hex(),
        AdditivityClass::NonAdditive,
        bad,
    )?
    .claim()
    .clone();
    let summary = Summary {
        scope,
        definition,
        definition_tag,
        earnings,
        capital_employed,
        period_return_percent,
        annualized_return_percent,
    };
    let mut encoding = Encoder::new_frame(*b"ROCE", 1);
    for input in &inputs {
        encoding.put_str(&claim_json(input, &input_json(&input.value), false));
    }
    encoding.put_str(&claim_json(&capital, &capital_json(&capital.value), false));
    encoding.put_str(&summary_json(&summary));
    let digest = blake3_hash(&encoding.finish()).to_hex();
    let combined = Receipt::from_admitted(
        inputs.clone(),
        OUTPUT_METRIC,
        summary,
        digest.clone(),
        AdditivityClass::NonAdditive,
        bad,
    )?;
    let prior = combined.claim();
    let mut parents = prior.parents.clone();
    parents.push(capital.claim_id);
    parents.sort();
    parents.dedup();
    let claim = Claim::new(
        OUTPUT_METRIC,
        prior.value.clone(),
        &digest,
        prior.representation.clone(),
        prior.support,
        AdditivityClass::NonAdditive,
        prior.evidence,
        prior.epistemic,
        prior.state,
        prior.confidentiality.clone(),
        prior.uu_terms.clone(),
        parents,
        prior.anchors.clone(),
    );
    Ok(CapitalReturnReceipt {
        inputs,
        capital,
        claim,
        digest,
    })
}

fn date_text(date: CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}
fn period_json(period: Period) -> String {
    format!(
        "{{\"year\":{},\"first_month\":{},\"last_month\":{}}}",
        period.year, period.first_month, period.last_month
    )
}
fn scope_json(scope: &Scope) -> String {
    format!(
        "{{\"issuer\":{},\"subject\":{},\"period\":{},\"ownership\":{},\"perimeter\":{},\"accounting_basis\":{},\"currency\":{},\"monetary_scale\":{},\"unit\":{}}}",
        js(&scope.issuer),
        js(&scope.subject),
        period_json(scope.period),
        ownership_json(scope.ownership),
        js(&scope.perimeter),
        js(&scope.accounting_basis),
        js(&format!("{:?}", scope.currency)),
        scope.monetary_scale,
        js(&scope.unit())
    )
}
fn definition_json(d: &Definition) -> String {
    let annualization = match &d.annualization {
        Annualization::None => "{\"kind\":\"none\"}".into(),
        Annualization::Explicit { multiplier, basis } => format!(
            "{{\"kind\":\"explicit\",\"multiplier\":{},\"basis\":{}}}",
            js(&qty(*multiplier)),
            js(basis)
        ),
    };
    let applicability = match &d.applicability {
        Applicability::Applicable => "{\"status\":\"applicable\"}".into(),
        Applicability::Excluded { reason } => {
            format!("{{\"status\":\"excluded\",\"reason\":{}}}", js(reason))
        }
    };
    format!(
        "{{\"id\":{},\"version\":{},\"earnings_basis\":{},\"tax_basis\":{},\"capital_employed_basis\":{},\"capital_basis\":{},\"annualization\":{annualization},\"applicability\":{applicability}}}",
        js(&d.id),
        js(&d.version),
        js(&d.earnings_basis),
        js(d.tax_basis.name()),
        js(&d.capital_employed_basis),
        js(d.capital_basis.name())
    )
}
fn enclosed_json(v: Enclosed) -> String {
    format!(
        "{{\"point\":{},\"lo\":{},\"hi\":{},\"bounds_basis\":\"closed_source_rounding_enclosure\"}}",
        js(&qty(v.point)),
        js(&qty(v.bounds.lo)),
        js(&qty(v.bounds.hi))
    )
}
fn input_json(input: &Input) -> String {
    let amount = |value: Amount| {
        format!(
            "{{\"value\":{},\"rounding_step\":{}}}",
            js(&qty(value.value)),
            js(&qty(value.rounding_step))
        )
    };
    let value = match &input.value {
        InputValue::Definition(d) => definition_json(d),
        InputValue::Earnings { period, amount: a } | InputValue::Average { period, amount: a } => {
            format!(
                "{{\"period\":{},\"amount\":{}}}",
                period_json(*period),
                amount(*a)
            )
        }
        InputValue::Opening { as_of, amount: a } | InputValue::Closing { as_of, amount: a } => {
            format!(
                "{{\"as_of\":{},\"amount\":{}}}",
                js(&date_text(*as_of)),
                amount(*a)
            )
        }
    };
    let record = input.source_record.as_ref().map_or_else(|| "null".into(), |s| format!("{{\"quantity\":{},\"raw_token\":{},\"basis\":{},\"readers\":[{}],\"published_at\":{},\"published_at_basis\":{}}}",
        js(&s.quantity),js(&s.raw_token),js(&s.basis),join_json(&s.readers,|r|js(r)),js(&s.published_at),js(&s.published_at_basis)));
    format!(
        "{{\"scope\":{},\"role\":{},\"value\":{value},\"source_record\":{record}}}",
        scope_json(&input.scope),
        js(input.value.role())
    )
}
fn capital_json(v: &CapitalBase) -> String {
    format!(
        "{{\"scope\":{},\"definition_tag\":{},\"basis\":{},\"amount\":{}}}",
        scope_json(&v.scope),
        js(&v.definition_tag),
        js(v.basis.name()),
        enclosed_json(v.amount)
    )
}
fn summary_json(v: &Summary) -> String {
    format!(
        "{{\"scope\":{},\"definition\":{},\"definition_tag\":{},\"earnings\":{},\"capital_employed\":{},\"period_return_percent\":{},\"annualized_return_percent\":{},\"calibration\":\"not_established\"}}",
        scope_json(&v.scope),
        definition_json(&v.definition),
        js(&v.definition_tag),
        enclosed_json(v.earnings),
        enclosed_json(v.capital_employed),
        enclosed_json(v.period_return_percent),
        v.annualized_return_percent
            .map_or_else(|| "null".into(), enclosed_json)
    )
}

impl CapitalReturnReceipt {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.capital-return.v1\",\"digest\":{},\"no_claim\":{},\"inputs\":[{}],\"capital_claim\":{},\"result\":{}}}",
            js(&self.digest),
            js(NO_CLAIM),
            join_json(&self.inputs, |c| claim_json(
                c,
                &input_json(&c.value),
                false
            )),
            claim_json(&self.capital, &capital_json(&self.capital.value), false),
            claim_json(&self.claim, &summary_json(self.summary()), false)
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let safe = |value: &str| {
            let mut escaped = String::new();
            for ch in value.chars() {
                match ch {
                    '&' => escaped.push_str("&amp;"),
                    '<' => escaped.push_str("&lt;"),
                    '>' => escaped.push_str("&gt;"),
                    '\\' | '|' | '`' | '[' | ']' | '*' | '_' | '~' => {
                        escaped.push('\\');
                        escaped.push(ch);
                    }
                    ch if ch.is_control() => escaped.push(' '),
                    ch => escaped.push(ch),
                }
            }
            escaped
        };
        let show = |v: Enclosed| {
            format!(
                "{} [{}; {}]",
                qty(v.point),
                qty(v.bounds.lo),
                qty(v.bounds.hi)
            )
        };
        let mut out = format!(
            "# Capital return\n\n{NO_CLAIM}\n\n{} / {}; {} months {}–{}; ownership `{}`; perimeter {}; accounting basis {}; unit **{}**.\n\nDefinition **{} / {}** (`{}`): {} earnings, {}; capital employed: {}; denominator **{}**.\n\n| Measure | Point [closed lower; upper] |\n|---|---|\n| Earnings ({}) | {} |\n| Capital employed ({}) | {} |\n| Period return (%) | {} |\n",
            safe(&s.scope.issuer),
            safe(&s.scope.subject),
            s.scope.period.year,
            s.scope.period.first_month,
            s.scope.period.last_month,
            ownership_json(s.scope.ownership),
            safe(&s.scope.perimeter),
            safe(&s.scope.accounting_basis),
            s.scope.unit(),
            safe(&s.definition.id),
            safe(&s.definition.version),
            s.definition_tag,
            s.definition.tax_basis.name(),
            safe(&s.definition.earnings_basis),
            safe(&s.definition.capital_employed_basis),
            s.definition.capital_basis.name(),
            s.scope.unit(),
            show(s.earnings),
            s.scope.unit(),
            show(s.capital_employed),
            show(s.period_return_percent)
        );
        if let Some(annualized) = s.annualized_return_percent {
            let _ = writeln!(out, "| Annualized return (%) | {} |", show(annualized));
        }
        match &s.definition.annualization {
            Annualization::None => out.push_str("\nAnnualization: **none**.\n"),
            Annualization::Explicit { multiplier, basis } => {
                let _ = writeln!(
                    out,
                    "\nAnnualization: ×{}; {}. Raw period return is retained above.",
                    qty(*multiplier),
                    safe(basis)
                );
            }
        }
        out.push_str("\n## Source Claims\n\n| Role | Stated input | Claim | Evidence | Sources |\n|---|---|---|---|---|\n");
        for input in &self.inputs {
            let evidence = format!("{:?}; {}", input.evidence, input.confidentiality);
            let sources = input
                .anchors
                .iter()
                .map(|a| {
                    format!(
                        "{}:{} (SHA-256 {})",
                        safe(&a.artifact_path),
                        safe(&a.locator),
                        fmine_core::Digest32(a.artifact_sha256)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            let _ = writeln!(
                out,
                "| {} | {} | `{}` | {} | {} |",
                input.value.value.role(),
                safe(&input_json(&input.value)),
                input.claim_id,
                safe(&evidence),
                sources
            );
        }
        let _ = writeln!(
            out,
            "\nCapital-base Claim `{}`; result Claim `{}`; digest `{}`. Output additivity: NonAdditive.\n",
            self.capital.claim_id, self.claim.claim_id, self.digest
        );
        // Each JSON envelope is one escaped line; source text cannot create a closing fence.
        out.push_str("## Source Claim receipts\n\nEach envelope preserves the original Claim and its ancestry.\n");
        for input in &self.inputs {
            let _ = writeln!(
                out,
                "\n### Input: {}\n\n```json\n{}\n```",
                input.value.value.role(),
                claim_json(input, &input_json(&input.value), false)
            );
        }
        let _ = writeln!(
            out,
            "\n## Derived Claim receipts\n\nThe exact envelopes retain evidence, confidentiality, UU terms, parents and anchors.\n\n### Capital base\n\n```json\n{}\n```\n\n### Return assessment\n\n```json\n{}\n```",
            claim_json(&self.capital, &capital_json(&self.capital.value), false),
            claim_json(&self.claim, &summary_json(self.summary()), false)
        );
        out
    }
}

#[cfg(test)]
mod tests;
