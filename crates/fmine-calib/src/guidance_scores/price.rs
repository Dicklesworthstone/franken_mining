//! DESIGN (BD-341.S5): an exact, source-bound price sensitivity beside the unconditional guidance score.
//! The gate tests only the supplied quarterly population. It neither establishes causality nor authenticates
//! documents, their dates, or the completeness of that population. No failed gate produces an adjustment.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};

use super::{
    Guidance, GuidanceScore, Interval, PrintedOutcome, Rat, interval_json, rat_json, score,
};

const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    GoldAisc,
    CopperAisc,
    TotalCashCost,
    C1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateUnit {
    UsdPerTroyOunce,
    UsdPerPound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub locator: String,
    pub sha256: Digest32,
}

/// `basis` is an explicit denominator/ownership convention, compared literally; no conversion is inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedValue {
    pub printed: PrintedOutcome,
    pub unit: RateUnit,
    pub subject: String,
    pub period: String,
    pub basis: String,
    pub citation: Citation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoyaltyQuarter {
    /// A unique YYYYQn; both values must cover this same quarter.
    pub id: String,
    pub royalty: CitedValue,
    pub realized_price: CitedValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceInput {
    pub metric: Metric,
    pub guidance: Guidance,
    pub guidance_citation: Citation,
    pub outcome: CitedValue,
    pub royalty: CitedValue,
    pub assumed_price: Option<CitedValue>,
    pub realized_price: CitedValue,
    /// Explicitly names the price subject; a company proxy is retained when different from the cost subject.
    pub price_subject: String,
    pub quarters: Vec<RoyaltyQuarter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    Supported {
        beta: Interval,
        quarters: Vec<String>,
    },
    NotSupported {
        quarters: [String; 2],
        bounds: [Interval; 2],
    },
    InsufficientHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsplitReason {
    UnsupportedMetric,
    NoStatedPriceAssumption,
    GuidanceHasNoRange,
    ProportionalityRefuted,
    InsufficientHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjustment {
    /// Signed r × (1 − assumed / realized), with an enclosure of the supplied rounding boxes.
    pub contribution: PrintedOutcome,
    pub conditional: GuidanceScore,
}

/// Immutable paired result: the conditional score cannot be emitted by this renderer without the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalReport {
    input: PriceInput,
    unconditional: GuidanceScore,
    gate: Gate,
    adjustment: Option<Adjustment>,
    unsplit: Option<UnsplitReason>,
    digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriceError {
    InvalidCitation,
    InvalidContext,
    ContextMismatch(&'static str),
    InvalidValue,
    NonpositivePrice,
    NegativeRoyalty,
    DuplicateQuarter,
    ArithmeticOverflow,
    InvalidGuidance,
    AlteredReport,
}

impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "price sensitivity refused: {self:?}")
    }
}
impl std::error::Error for PriceError {}
type Result<T> = std::result::Result<T, PriceError>;

fn exact<T, E>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|_| PriceError::ArithmeticOverflow)
}
fn cmp(a: Rat, b: Rat) -> Result<Ordering> {
    exact(a.checked_cmp(b))
}
fn valid_text(s: &str) -> bool {
    !s.trim().is_empty() && !s.chars().any(char::is_control)
}
fn citation(c: &Citation) -> Result<()> {
    if !valid_text(&c.locator) {
        return Err(PriceError::InvalidCitation);
    }
    Ok(())
}
fn value(v: &CitedValue) -> Result<()> {
    citation(&v.citation)?;
    if ![&v.subject, &v.period, &v.basis]
        .into_iter()
        .all(|s| valid_text(s))
    {
        return Err(PriceError::InvalidContext);
    }
    score(
        Guidance::Point {
            value: v.printed.value,
        },
        v.printed,
    )
    .map_err(|_| PriceError::InvalidValue)?;
    Ok(())
}
fn positive_price(v: &CitedValue) -> Result<()> {
    value(v)?;
    if cmp(v.printed.interval.lo, ZERO)? != Ordering::Greater {
        return Err(PriceError::NonpositivePrice);
    }
    Ok(())
}
fn royalty(v: &CitedValue) -> Result<()> {
    value(v)?;
    if cmp(v.printed.interval.lo, ZERO)? == Ordering::Less {
        return Err(PriceError::NegativeRoyalty);
    }
    Ok(())
}
fn same(a: &CitedValue, b: &CitedValue, subject: bool, period: bool) -> Result<()> {
    if a.unit != b.unit {
        return Err(PriceError::ContextMismatch("unit"));
    }
    if a.basis != b.basis {
        return Err(PriceError::ContextMismatch("basis"));
    }
    if subject && a.subject != b.subject {
        return Err(PriceError::ContextMismatch("subject"));
    }
    if period && a.period != b.period {
        return Err(PriceError::ContextMismatch("period"));
    }
    Ok(())
}
fn quarter(q: &RoyaltyQuarter) -> Result<()> {
    let id = q.id.as_bytes();
    if id.len() != 6
        || !id[..4].iter().all(u8::is_ascii_digit)
        || id[4] != b'Q'
        || !(b'1'..=b'4').contains(&id[5])
        || q.royalty.period != q.id
        || q.realized_price.period != q.id
    {
        return Err(PriceError::InvalidContext);
    }
    royalty(&q.royalty)?;
    positive_price(&q.realized_price)?;
    same(&q.royalty, &q.realized_price, false, true)
}
fn beta(q: &RoyaltyQuarter) -> Result<Interval> {
    let (r, p) = (
        q.royalty.printed.interval,
        q.realized_price.printed.interval,
    );
    Ok(Interval {
        lo: exact(r.lo.checked_div(p.hi))?,
        hi: exact(r.hi.checked_div(p.lo))?,
        lo_strict: r.lo_strict || (r.lo != ZERO && p.hi_strict),
        hi_strict: r.hi_strict || (r.hi != ZERO && p.lo_strict),
    })
}

/// Exact feasibility of one constant royalty/price ratio across the supplied quarters. Input order is immaterial.
/// A conflicting pair is a certificate of empty intersection, never a ranking or correction of either quarter.
pub fn proportionality(quarters: &[RoyaltyQuarter]) -> Result<Gate> {
    let mut sorted: Vec<_> = quarters.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut ids = BTreeSet::new();
    for q in &sorted {
        quarter(q)?;
        if !ids.insert(&q.id) {
            return Err(PriceError::DuplicateQuarter);
        }
        if let Some(first) = sorted.first() {
            same(&first.royalty, &q.royalty, true, false)?;
            same(&first.realized_price, &q.realized_price, true, false)?;
        }
    }
    if sorted.len() < 2 {
        return Ok(Gate::InsufficientHistory);
    }
    let mut bounds = Vec::with_capacity(sorted.len());
    for q in &sorted {
        bounds.push(beta(q)?);
    }
    let (mut low, mut high) = (0, 0);
    for (i, b) in bounds.iter().enumerate().skip(1) {
        let l = cmp(b.lo, bounds[low].lo)?;
        if l == Ordering::Greater || (l == Ordering::Equal && b.lo_strict && !bounds[low].lo_strict)
        {
            low = i;
        }
        let h = cmp(b.hi, bounds[high].hi)?;
        if h == Ordering::Less || (h == Ordering::Equal && b.hi_strict && !bounds[high].hi_strict) {
            high = i;
        }
    }
    let intersection = Interval {
        lo: bounds[low].lo,
        hi: bounds[high].hi,
        lo_strict: bounds[low].lo_strict,
        hi_strict: bounds[high].hi_strict,
    };
    let order = cmp(intersection.lo, intersection.hi)?;
    if order == Ordering::Greater
        || (order == Ordering::Equal && (intersection.lo_strict || intersection.hi_strict))
    {
        return Ok(Gate::NotSupported {
            quarters: [sorted[low].id.clone(), sorted[high].id.clone()],
            bounds: [bounds[low], bounds[high]],
        });
    }
    Ok(Gate::Supported {
        beta: intersection,
        quarters: sorted.iter().map(|q| q.id.clone()).collect(),
    })
}

fn validate_input(i: &PriceInput) -> Result<()> {
    citation(&i.guidance_citation)?;
    value(&i.outcome)?;
    royalty(&i.royalty)?;
    positive_price(&i.realized_price)?;
    same(&i.outcome, &i.royalty, true, true)?;
    same(&i.outcome, &i.realized_price, false, true)?;
    if !valid_text(&i.price_subject) || i.realized_price.subject != i.price_subject {
        return Err(PriceError::ContextMismatch("price subject"));
    }
    if let Some(p) = &i.assumed_price {
        positive_price(p)?;
        same(&i.realized_price, p, true, true)?;
    }
    let unit = match i.metric {
        Metric::GoldAisc | Metric::TotalCashCost => RateUnit::UsdPerTroyOunce,
        Metric::CopperAisc | Metric::C1 => RateUnit::UsdPerPound,
    };
    if i.outcome.unit != unit {
        return Err(PriceError::ContextMismatch("metric unit"));
    }
    for q in &i.quarters {
        same(&i.royalty, &q.royalty, true, false)?;
        same(&i.realized_price, &q.realized_price, true, false)?;
    }
    Ok(())
}

fn contribution(i: &PriceInput, assumed: &CitedValue) -> Result<PrintedOutcome> {
    let formula = |r: Rat, a: Rat, p: Rat| {
        exact(r.checked_mul(exact(ONE.checked_sub(exact(a.checked_div(p))?))?))
    };
    let printed = formula(
        i.royalty.printed.value,
        assumed.printed.value,
        i.realized_price.printed.value,
    )?;
    // The function is multiaffine in r, a and 1/p on a positive-price box: extrema are at its corners.
    // Closed enclosures intentionally include unattained open endpoints; they never overstate precision.
    let mut lo = printed;
    let mut hi = printed;
    for r in [i.royalty.printed.interval.lo, i.royalty.printed.interval.hi] {
        for a in [assumed.printed.interval.lo, assumed.printed.interval.hi] {
            for p in [
                i.realized_price.printed.interval.lo,
                i.realized_price.printed.interval.hi,
            ] {
                let x = formula(r, a, p)?;
                if cmp(x, lo)? == Ordering::Less {
                    lo = x;
                }
                if cmp(x, hi)? == Ordering::Greater {
                    hi = x;
                }
            }
        }
    }
    Ok(PrintedOutcome {
        value: printed,
        interval: Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        },
    })
}

/// Preserve the unconditional result and apply a sensitivity only after the exact proportionality gate passes.
/// Citations are bound, not independently verified: the caller must verify source bytes before using this API.
pub fn conditional(input: &PriceInput) -> Result<ConditionalReport> {
    validate_input(input)?;
    let unconditional =
        score(input.guidance, input.outcome.printed).map_err(|_| PriceError::InvalidGuidance)?;
    let gate = proportionality(&input.quarters)?;
    let unsplit = if !matches!(input.metric, Metric::GoldAisc | Metric::CopperAisc) {
        Some(UnsplitReason::UnsupportedMetric)
    } else if input.assumed_price.is_none() {
        Some(UnsplitReason::NoStatedPriceAssumption)
    } else if !matches!(input.guidance, Guidance::Range { .. }) {
        Some(UnsplitReason::GuidanceHasNoRange)
    } else {
        match &gate {
            Gate::Supported { .. } => None,
            Gate::NotSupported { .. } => Some(UnsplitReason::ProportionalityRefuted),
            Gate::InsufficientHistory => Some(UnsplitReason::InsufficientHistory),
        }
    };
    let adjustment = match (&input.assumed_price, unsplit) {
        (Some(a), None) => {
            let p = contribution(input, a)?;
            let y = input.outcome.printed;
            let adjusted = PrintedOutcome {
                value: exact(y.value.checked_sub(p.value))?,
                interval: Interval {
                    lo: exact(y.interval.lo.checked_sub(p.interval.hi))?,
                    hi: exact(y.interval.hi.checked_sub(p.interval.lo))?,
                    lo_strict: y.interval.lo_strict,
                    hi_strict: y.interval.hi_strict,
                },
            };
            Some(Adjustment {
                contribution: p,
                conditional: score(input.guidance, adjusted)
                    .map_err(|_| PriceError::InvalidValue)?,
            })
        }
        _ => None,
    };
    let mut input = input.clone();
    input.quarters.sort_by(|a, b| a.id.cmp(&b.id));
    let mut report = ConditionalReport {
        input,
        unconditional,
        gate,
        adjustment,
        unsplit,
        digest: Digest32::default(),
    };
    report.digest = report.compute_digest();
    Ok(report)
}

fn quote(s: &str) -> String {
    let mut result = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(result, "\\u{:04x}", u32::from(c));
            }
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}
fn cited_json(v: &CitedValue) -> String {
    format!(
        "{{\"v\":{},\"interval\":{},\"at\":{},\"sha256\":\"{}\",\"subject\":{},\"period\":{},\"basis\":{},\"unit\":{}}}",
        rat_json(v.printed.value),
        interval_json(Some(v.printed.interval)),
        quote(&v.citation.locator),
        v.citation.sha256,
        quote(&v.subject),
        quote(&v.period),
        quote(&v.basis),
        quote(match v.unit {
            RateUnit::UsdPerTroyOunce => "USD/troy_oz",
            RateUnit::UsdPerPound => "USD/lb",
        })
    )
}
fn gate_json(gate: &Gate) -> String {
    match gate {
        Gate::Supported { beta, quarters } => format!(
            "{{\"state\":\"supported_on_supplied_quarters\",\"beta\":{},\"quarters\":[{}]}}",
            interval_json(Some(*beta)),
            quarters
                .iter()
                .map(|q| quote(q))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Gate::NotSupported { quarters, bounds } => format!(
            "{{\"state\":\"not_supported\",\"witness_quarters\":[{},{}],\"bounds\":[{},{}]}}",
            quote(&quarters[0]),
            quote(&quarters[1]),
            interval_json(Some(bounds[0])),
            interval_json(Some(bounds[1]))
        ),
        Gate::InsufficientHistory => "{\"state\":\"insufficient_history\"}".to_string(),
    }
}
impl ConditionalReport {
    pub fn unconditional(&self) -> &GuidanceScore {
        &self.unconditional
    }
    pub fn adjustment(&self) -> Option<&Adjustment> {
        self.adjustment.as_ref()
    }
    pub fn gate(&self) -> &Gate {
        &self.gate
    }
    pub fn unsplit_reason(&self) -> Option<UnsplitReason> {
        self.unsplit
    }
    pub fn digest(&self) -> Digest32 {
        self.digest
    }
    pub fn validate(&self) -> Result<()> {
        if *self != conditional(&self.input)? {
            return Err(PriceError::AlteredReport);
        }
        Ok(())
    }
    fn inputs_json(&self) -> String {
        let i = &self.input;
        let guidance = match i.guidance {
            Guidance::Range { lo, hi } => {
                format!("{{\"range\":[{},{}]}}", rat_json(lo), rat_json(hi))
            }
            Guidance::Point { value } => format!("{{\"point\":{}}}", rat_json(value)),
            Guidance::AtMost { upper } => format!("{{\"at_most\":{}}}", rat_json(upper)),
            Guidance::AtLeast { lower } => format!("{{\"at_least\":{}}}", rat_json(lower)),
        };
        let quarters = i
            .quarters
            .iter()
            .map(|q| {
                format!(
                    "{{\"id\":{},\"royalty\":{},\"realized_price\":{}}}",
                    quote(&q.id),
                    cited_json(&q.royalty),
                    cited_json(&q.realized_price)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"metric\":{},\"guidance\":{},\"guidance_at\":{},\"guidance_sha256\":\"{}\",\"outcome\":{},\"royalty\":{},\"assumed_price\":{},\"realized_price\":{},\"price_subject\":{},\"price_proxy\":{},\"quarters\":[{quarters}]}}",
            quote(match i.metric {
                Metric::GoldAisc => "gold_aisc",
                Metric::CopperAisc => "copper_aisc",
                Metric::TotalCashCost => "tcc",
                Metric::C1 => "c1",
            }),
            guidance,
            quote(&i.guidance_citation.locator),
            i.guidance_citation.sha256,
            cited_json(&i.outcome),
            cited_json(&i.royalty),
            i.assumed_price
                .as_ref()
                .map_or_else(|| "null".to_string(), cited_json),
            cited_json(&i.realized_price),
            quote(&i.price_subject),
            i.price_subject != i.outcome.subject
        )
    }
    fn result_json(&self) -> String {
        let adjustment = self.adjustment.as_ref().map_or_else(
            || "null".to_string(),
            |a| {
                format!(
                    "{{\"price_contribution\":{},\"rounding_enclosure\":{},\"conditional\":{}}}",
                    rat_json(a.contribution.value),
                    interval_json(Some(a.contribution.interval)),
                    a.conditional.to_json()
                )
            },
        );
        format!(
            "{{\"unconditional\":{},\"gate\":{},\"adjustment\":{},\"unsplit_reason\":{}}}",
            self.unconditional.to_json(),
            gate_json(&self.gate),
            adjustment,
            self.unsplit.map_or_else(
                || "null".to_string(),
                |r| quote(match r {
                    UnsplitReason::UnsupportedMetric => "unsupported_metric",
                    UnsplitReason::NoStatedPriceAssumption => "no_stated_price_assumption",
                    UnsplitReason::GuidanceHasNoRange => "guidance_has_no_range",
                    UnsplitReason::ProportionalityRefuted => "proportionality_refuted",
                    UnsplitReason::InsufficientHistory => "insufficient_history",
                })
            )
        )
    }
    fn compute_digest(&self) -> Digest32 {
        let mut e = Encoder::new_frame(*b"GPSP", 1);
        e.put_str(&self.inputs_json());
        e.put_str(&self.result_json());
        blake3_hash(&e.finish())
    }
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.guidance.price.v1\",\"digest\":\"{}\",\"inputs\":{},\"scores\":{},\"no_claim\":\"Descriptive sensitivity on supplied cited inputs; no causal attribution, controllability, source authentication or calibration claim.\"}}",
            self.digest,
            self.inputs_json(),
            self.result_json()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(n: i128) -> Rat {
        Rat::from_integer(n)
    }
    fn r(n: i128, d: i128) -> Rat {
        Rat::new(n, d).expect("rational")
    }
    fn source(s: &str) -> Citation {
        Citation {
            locator: s.into(),
            sha256: fmine_core::hash::sha256(s.as_bytes()),
        }
    }
    fn amount(subject: &str, period: &str, v: i128) -> CitedValue {
        CitedValue {
            printed: PrintedOutcome {
                value: n(v),
                interval: Interval {
                    lo: n(v),
                    hi: n(v),
                    lo_strict: false,
                    hi_strict: false,
                },
            },
            unit: RateUnit::UsdPerTroyOunce,
            subject: subject.into(),
            period: period.into(),
            basis: "sold gold; stated ownership basis".into(),
            citation: source(&format!("synthetic:{subject}:{period}:{v}")),
        }
    }
    fn input() -> PriceInput {
        PriceInput {
            metric: Metric::GoldAisc,
            guidance: Guidance::Range {
                lo: n(100),
                hi: n(120),
            },
            guidance_citation: source("synthetic:guidance:p1"),
            outcome: amount("mine", "2030", 125),
            royalty: amount("mine", "2030", 100),
            assumed_price: Some(amount("mine", "2030", 1500)),
            realized_price: amount("mine", "2030", 2000),
            price_subject: "mine".into(),
            quarters: vec![
                RoyaltyQuarter {
                    id: "2029Q1".into(),
                    royalty: amount("mine", "2029Q1", 50),
                    realized_price: amount("mine", "2029Q1", 1000),
                },
                RoyaltyQuarter {
                    id: "2029Q2".into(),
                    royalty: amount("mine", "2029Q2", 100),
                    realized_price: amount("mine", "2029Q2", 2000),
                },
            ],
        }
    }
    #[test]
    fn synthetic_known_answer_25_retains_unconditional() {
        let report = conditional(&input()).expect("split");
        let adjustment = report.adjustment().expect("supported");
        assert_eq!(adjustment.contribution.value, n(25));
        assert_eq!(report.unconditional().z, Some(r(3, 2)));
        assert_eq!(adjustment.conditional.z, Some(n(-1)));
        assert!(report.to_json().contains("\"unconditional\":"));
        assert!(report.to_json().contains("\"conditional\":"));
        report.validate().expect("paired report");
    }
    #[test]
    fn equal_prices_are_identity_including_negative_price_move() {
        let mut i = input();
        for price in [1, 1500, 2000, 100_000] {
            i.assumed_price = Some(amount("mine", "2030", price));
            i.realized_price = amount("mine", "2030", price);
            let report = conditional(&i).expect("identity");
            let a = report.adjustment().expect("supported");
            assert_eq!(a.contribution.value, ZERO);
            assert_eq!(&a.conditional, report.unconditional());
        }
        i.assumed_price = Some(amount("mine", "2030", 3000));
        i.realized_price = amount("mine", "2030", 2000);
        assert_eq!(
            conditional(&i)
                .expect("signed")
                .adjustment()
                .expect("split")
                .contribution
                .value,
            n(-50)
        );
    }
    #[test]
    fn proportionality_conflict_witness_is_order_independent() {
        let mut i = input();
        i.quarters[1].royalty = amount("mine", "2029Q2", 101);
        let report = conditional(&i).expect("refuted gate");
        assert_eq!(
            report.unsplit_reason(),
            Some(UnsplitReason::ProportionalityRefuted)
        );
        assert!(report.adjustment().is_none());
        let Gate::NotSupported { quarters, bounds } = report.gate() else {
            panic!("conflict");
        };
        assert_eq!(quarters, &["2029Q2", "2029Q1"]);
        assert!(cmp(bounds[0].lo, bounds[1].hi).expect("cmp") == Ordering::Greater);
        i.quarters.reverse();
        assert_eq!(report, conditional(&i).expect("same"));
    }
    #[test]
    fn proportionality_respects_open_ends_and_zero_royalty() {
        let mut i = input();
        i.quarters[0].royalty.printed.interval.hi = n(100);
        i.quarters[0].royalty.printed.interval.hi_strict = true;
        i.quarters[1].royalty = amount("mine", "2029Q2", 200);
        assert!(matches!(
            proportionality(&i.quarters).expect("open conflict"),
            Gate::NotSupported { .. }
        ));
        i.quarters[0].royalty.printed.interval.hi_strict = false;
        let Gate::Supported { beta, .. } = proportionality(&i.quarters).expect("closed") else {
            panic!("supported");
        };
        assert_eq!(beta.lo, r(1, 10));
        assert_eq!(beta.hi, r(1, 10));
        for q in &mut i.quarters {
            q.royalty = amount("mine", &q.id, 0);
            q.realized_price.printed.interval.hi = n(3000);
            q.realized_price.printed.interval.hi_strict = true;
        }
        let Gate::Supported { beta, .. } = proportionality(&i.quarters).expect("zero") else {
            panic!("zero beta");
        };
        assert_eq!(
            beta,
            Interval {
                lo: ZERO,
                hi: ZERO,
                lo_strict: false,
                hi_strict: false
            }
        );
    }
    #[test]
    fn missing_assumption_unsupported_metrics_and_sparse_history_stay_unsplit() {
        let mut i = input();
        i.assumed_price = None;
        assert_eq!(
            conditional(&i).expect("missing").unsplit_reason(),
            Some(UnsplitReason::NoStatedPriceAssumption)
        );
        i = input();
        i.metric = Metric::TotalCashCost;
        assert_eq!(
            conditional(&i).expect("TCC").unsplit_reason(),
            Some(UnsplitReason::UnsupportedMetric)
        );
        i.metric = Metric::C1;
        i.outcome.unit = RateUnit::UsdPerPound;
        i.royalty.unit = RateUnit::UsdPerPound;
        i.realized_price.unit = RateUnit::UsdPerPound;
        i.assumed_price.as_mut().expect("price").unit = RateUnit::UsdPerPound;
        for q in &mut i.quarters {
            q.royalty.unit = RateUnit::UsdPerPound;
            q.realized_price.unit = RateUnit::UsdPerPound;
        }
        assert_eq!(
            conditional(&i).expect("C1").unsplit_reason(),
            Some(UnsplitReason::UnsupportedMetric)
        );
        i = input();
        i.quarters.truncate(1);
        assert_eq!(
            conditional(&i).expect("sparse").unsplit_reason(),
            Some(UnsplitReason::InsufficientHistory)
        );
    }
    #[test]
    fn metadata_mismatch_and_invalid_values_refuse() {
        let mut i = input();
        i.royalty.basis = "produced".into();
        assert_eq!(conditional(&i), Err(PriceError::ContextMismatch("basis")));
        i = input();
        i.realized_price.period = "2031".into();
        assert_eq!(conditional(&i), Err(PriceError::ContextMismatch("period")));
        i = input();
        i.assumed_price.as_mut().expect("price").unit = RateUnit::UsdPerPound;
        assert_eq!(conditional(&i), Err(PriceError::ContextMismatch("unit")));
        i = input();
        i.outcome.citation.locator.clear();
        assert_eq!(conditional(&i), Err(PriceError::InvalidCitation));
        i = input();
        i.realized_price.printed.interval.lo = ZERO;
        assert_eq!(conditional(&i), Err(PriceError::NonpositivePrice));
        i = input();
        i.quarters.push(i.quarters[0].clone());
        assert_eq!(conditional(&i), Err(PriceError::DuplicateQuarter));
    }
    #[test]
    fn rounding_enclosure_contains_all_independent_corner_results() {
        let mut i = input();
        i.royalty.printed.interval = Interval {
            lo: n(99),
            hi: n(101),
            lo_strict: true,
            hi_strict: true,
        };
        let p = i.assumed_price.as_mut().expect("price");
        p.printed.interval.lo = n(1490);
        p.printed.interval.hi = n(1510);
        i.realized_price.printed.interval.lo = n(1990);
        i.realized_price.printed.interval.hi = n(2010);
        let report = conditional(&i).expect("bounded");
        let p = report.adjustment().expect("split").contribution.interval;
        assert_eq!(p.lo, r(4752, 199));
        assert_eq!(p.hi, r(5252, 201));
        assert!(p.contains(n(25)).expect("printed enclosed"));
    }
    #[test]
    fn report_binds_source_hashes_context_and_both_scores() {
        let i = input();
        let report = conditional(&i).expect("base");
        let mut edited = i.clone();
        edited.royalty.citation.sha256.0[0] ^= 1;
        assert_ne!(
            report.digest(),
            conditional(&edited).expect("source change").digest()
        );
        edited = i.clone();
        edited.price_subject = "company".into();
        edited.realized_price.subject = "company".into();
        edited.assumed_price.as_mut().expect("price").subject = "company".into();
        for q in &mut edited.quarters {
            q.realized_price.subject = "company".into();
        }
        let proxy = conditional(&edited).expect("explicit proxy");
        assert!(proxy.to_json().contains("\"price_proxy\":true"));
        assert_ne!(report.digest(), proxy.digest());
        let mut changed = report.clone();
        changed.unconditional.z = None;
        assert_eq!(changed.validate(), Err(PriceError::AlteredReport));
        let mut reordered = i;
        reordered.quarters.reverse();
        assert_eq!(report, conditional(&reordered).expect("stable"));
    }

    #[test]
    fn exact_common_scale_preserves_both_scores_and_gate() {
        let i = input();
        let base = conditional(&i).expect("base");
        let mut scaled = i;
        let factor = r(7, 3);
        let scale = |v: &mut CitedValue| {
            v.printed.value = v.printed.value.checked_mul(factor).expect("scale");
            v.printed.interval.lo = v.printed.interval.lo.checked_mul(factor).expect("scale");
            v.printed.interval.hi = v.printed.interval.hi.checked_mul(factor).expect("scale");
        };
        scaled.guidance = Guidance::Range {
            lo: n(100).checked_mul(factor).expect("scale"),
            hi: n(120).checked_mul(factor).expect("scale"),
        };
        scale(&mut scaled.outcome);
        scale(&mut scaled.royalty);
        scale(&mut scaled.realized_price);
        scale(scaled.assumed_price.as_mut().expect("price"));
        for q in &mut scaled.quarters {
            scale(&mut q.royalty);
            scale(&mut q.realized_price);
        }
        let other = conditional(&scaled).expect("exact scale");
        assert_eq!(other.gate(), base.gate());
        assert_eq!(other.unconditional().z, base.unconditional().z);
        assert_eq!(
            other.adjustment().expect("split").conditional.z,
            base.adjustment().expect("split").conditional.z
        );
        assert_eq!(
            other.adjustment().expect("split").contribution.value,
            n(25).checked_mul(factor).expect("scaled known answer")
        );
    }

    #[test]
    fn arithmetic_overflow_refuses_instead_of_losing_the_unconditional_record() {
        let mut i = input();
        i.assumed_price = Some(amount("mine", "2030", i128::MAX));
        i.realized_price = amount("mine", "2030", 1);
        assert_eq!(conditional(&i), Err(PriceError::ArithmeticOverflow));
        assert_eq!(
            score(i.guidance, i.outcome.printed)
                .expect("original still scoreable")
                .z,
            Some(r(3, 2))
        );
    }
}
