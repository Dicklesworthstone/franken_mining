//! Exact, source-bound reserve replacement (BD-063).
//!
//! Annual net changes are summed before division by explicitly signed depletion. Reported GEO values
//! already include the registered annual decks; retaining those decks is not a second conversion.
//! Separate aggregate totals constrain the annual rounding boxes without replacing their printed sums.
//! Determinism: Exact (checked rationals, canonical declarations and evidence; no I/O or clocks).

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_numerics::exact_feas::Rat;
use fmine_registry::metrics::{
    GeoDeck, MetricDefinition, MetricFamily, MetricRegistry, ReserveMetal, ReserveReplacementRule,
};
use fmine_registry::reserve_replacement::{
    DepletionSign, HeadlineKind, ReplacementRegistry, Series,
};
use fmine_registry::sources::{DocumentRow, is_date};
use fmine_registry::transcriptions::{Accepted, Token, parse_token};

pub const SCHEMA: &str = "fmine.reserve-replacement-report.v1";
pub const NO_CLAIM: &str = "Exact arithmetic on registered issuer assertions, under closed half-step rounding bounds and the declared annual window, metal, attributable ownership and acquisition/divestment exclusions. It does not verify reserve truth or classification, infer omitted years, select a preferred vintage, reconstruct GEO from metals, or invent a tolerance for approximate language. Document-byte authentication and independent reader agreement remain caller obligations.";
const OVERFLOW: &str = "exact reserve-replacement arithmetic overflowed";
const ZERO: Rat = Rat::from_integer(0);
const HUNDRED: Rat = Rat::from_integer(100);

/// Closed display-rounding enclosure, not a confidence interval or headline-language tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub lo: Rat,
    pub hi: Rat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactValue {
    pub printed: Rat,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub source: Accepted,
    pub document: DocumentRow,
    pub value: ExactValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnualResult {
    pub year: u16,
    pub net_change: Reading,
    pub depletion: Reading,
    pub normalized_depletion: ExactValue,
    pub geo_deck: Option<GeoDeck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub net_change: ExactValue,
    /// Positive depletion after the registered sign convention; the original tokens remain cited.
    pub depletion: ExactValue,
    pub percent: ExactValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateResult {
    pub net_change: Reading,
    pub depletion: Reading,
    pub totals: Totals,
}

/// Joint annual/aggregate constraints have no distinguished point estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JointBounds {
    pub net_change: Bounds,
    pub depletion: Bounds,
    pub percent: Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlineState {
    RoundedIntervalCompatible,
    Incompatible,
    ApproximateUnassessed,
}

impl HeadlineState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RoundedIntervalCompatible => "rounded_interval_compatible",
            Self::Incompatible => "incompatible",
            Self::ApproximateUnassessed => "approximate_unassessed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlineResult {
    pub reading: Reading,
    pub kind: HeadlineKind,
    pub state: HeadlineState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesResult {
    pub selection: Series,
    pub definition: MetricDefinition,
    pub rule: ReserveReplacementRule,
    pub geo_definition: Option<MetricDefinition>,
    pub years: Vec<AnnualResult>,
    pub annual: Totals,
    pub aggregate: Option<AggregateResult>,
    pub joint: JointBounds,
    pub headline: Option<HeadlineResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesRefusal {
    pub selection: Series,
    pub reason: String,
    /// All candidates, including duplicates or malformed bindings, remain available for review.
    pub evidence: Vec<Accepted>,
    pub documents: Vec<DocumentRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeriesAssessment {
    Computed(Box<SeriesResult>),
    Refused(Box<SeriesRefusal>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementReport {
    pub issuer: String,
    pub registry_refusal: Option<String>,
    /// Selected definition dependencies are retained even when a series refuses before calculation.
    pub definitions: Vec<MetricDefinition>,
    pub rules: Vec<ReserveReplacementRule>,
    pub geo_decks: Vec<GeoDeck>,
    pub series: Vec<SeriesAssessment>,
}

fn arithmetic<T>(value: Result<T, fmine_units::UnitsError>) -> Result<T, String> {
    value.map_err(|_| OVERFLOW.into())
}

fn compare(a: Rat, b: Rat) -> Result<Ordering, String> {
    arithmetic(a.checked_cmp(b))
}

impl Bounds {
    fn add(self, other: Self) -> Result<Self, String> {
        Ok(Self {
            lo: arithmetic(self.lo.checked_add(other.lo))?,
            hi: arithmetic(self.hi.checked_add(other.hi))?,
        })
    }

    fn intersect(self, other: Self) -> Result<Option<Self>, String> {
        let lo = if compare(self.lo, other.lo)?.is_gt() {
            self.lo
        } else {
            other.lo
        };
        let hi = if compare(self.hi, other.hi)?.is_lt() {
            self.hi
        } else {
            other.hi
        };
        Ok((!compare(lo, hi)?.is_gt()).then_some(Self { lo, hi }))
    }
}

impl ExactValue {
    fn add(self, other: Self) -> Result<Self, String> {
        Ok(Self {
            printed: arithmetic(self.printed.checked_add(other.printed))?,
            bounds: self.bounds.add(other.bounds)?,
        })
    }

    fn depletion(self, sign: DepletionSign) -> Result<Self, String> {
        let value = match sign {
            DepletionSign::Positive => self,
            DepletionSign::Negative => Self {
                printed: arithmetic(self.printed.checked_neg())?,
                bounds: Bounds {
                    lo: arithmetic(self.bounds.hi.checked_neg())?,
                    hi: arithmetic(self.bounds.lo.checked_neg())?,
                },
            },
        };
        if value.printed.signum() < 0 {
            return Err("depletion has the opposite sign to the registered convention".into());
        }
        Ok(value)
    }
}

fn percent_bounds(net: Bounds, depletion: Bounds) -> Result<Bounds, String> {
    if depletion.lo.signum() <= 0 {
        return Err(
            "cumulative depletion rounding bounds include zero or negative depletion".into(),
        );
    }
    let ratio = |n: Rat, d: Rat| arithmetic(n.checked_div(d).and_then(|q| q.checked_mul(HUNDRED)));
    // Signed net changes need all four corners; a positive-only numerator shortcut is unsound.
    let mut lo = ratio(net.lo, depletion.lo)?;
    let mut hi = lo;
    for (n, d) in [
        (net.lo, depletion.hi),
        (net.hi, depletion.lo),
        (net.hi, depletion.hi),
    ] {
        let value = ratio(n, d)?;
        if compare(value, lo)?.is_lt() {
            lo = value;
        }
        if compare(value, hi)?.is_gt() {
            hi = value;
        }
    }
    Ok(Bounds { lo, hi })
}

fn totals(net_change: ExactValue, depletion: ExactValue) -> Result<Totals, String> {
    let bounds = percent_bounds(net_change.bounds, depletion.bounds)?;
    Ok(Totals {
        net_change,
        depletion,
        percent: ExactValue {
            printed: arithmetic(
                net_change
                    .printed
                    .checked_div(depletion.printed)
                    .and_then(|q| q.checked_mul(HUNDRED)),
            )?,
            bounds,
        },
    })
}

struct Binder<'a> {
    basis: &'a str,
    readings: &'a [Accepted],
    documents: &'a [DocumentRow],
}

impl Binder<'_> {
    fn bind(
        &self,
        doc: &str,
        quantity: &str,
        period: &str,
        end_year: u16,
        unit: &str,
    ) -> Result<Reading, String> {
        let Self {
            basis,
            readings,
            documents,
        } = *self;
        let matches: Vec<_> = readings
            .iter()
            .filter(|a| a.locator.doc == doc && a.quantity == quantity)
            .collect();
        let a = match matches.as_slice() {
            [a] => *a,
            [] => return Err(format!("no agreed reading for {doc}/{quantity}")),
            _ => return Err(format!("multiple candidate readings for {doc}/{quantity}")),
        };
        let refuse = |why: &str| format!("{}: {why}", a.locator);
        let docs: Vec<_> = documents.iter().filter(|d| d.file == doc).collect();
        let d = match docs.as_slice() {
            [d] => *d,
            [] => return Err(refuse("document is not registered")),
            _ => return Err(refuse("document registration is ambiguous")),
        };
        if readings
            .iter()
            .filter(|other| other.locator == a.locator)
            .count()
            != 1
            || readings
                .iter()
                .filter(|other| other.locator.to_string() == a.locator.to_string())
                .count()
                != 1
        {
            return Err(refuse(
                "source locator appears more than once or renders ambiguously",
            ));
        }
        if a.unit != unit || a.basis != basis || a.period != period {
            return Err(refuse(
                "unit, metal/ownership basis or period differs from the explicit declaration",
            ));
        }
        if a.doc_sha256 != d.sha256
            || d.sha256.len() != 64
            || !d
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !is_date(&d.published_at)
            || d.published_at.as_str() < format!("{end_year:04}-12-31").as_str()
            || d.kind.trim().is_empty()
            || d.published_at_basis.trim().is_empty()
            || a.locator.page == 0
            || a.locator.page > d.pages
            || [&a.locator.table, &a.locator.row, &a.locator.col]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(refuse(
                "invalid source locator, publication date or registered digest",
            ));
        }
        let readers: BTreeSet<_> = a.readers.iter().collect();
        if readers.len() < 2
            || readers.len() != a.readers.len()
            || readers.iter().any(|r| {
                r.is_empty()
                    || !r.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-')
                    })
            })
        {
            return Err(refuse(
                "at least two distinct reader role identities are required",
            ));
        }
        if parse_token(&a.token).as_ref() != Ok(&a.value) {
            return Err(refuse("printed token and exact admitted value disagree"));
        }
        let Token::Point(decimal) = a.value else {
            return Err(refuse(
                "reserve amounts and headlines require a printed point",
            ));
        };
        let den = 10_i128
            .checked_pow(decimal.scale)
            .ok_or_else(|| refuse("printed precision overflow"))?;
        let printed = arithmetic(Rat::new(decimal.mantissa, den))?;
        let half =
            arithmetic(Rat::new(1, den).and_then(|step| step.checked_div(Rat::from_integer(2))))?;
        Ok(Reading {
            source: a.clone(),
            document: d.clone(),
            value: ExactValue {
                printed,
                bounds: Bounds {
                    lo: arithmetic(printed.checked_sub(half))?,
                    hi: arithmetic(printed.checked_add(half))?,
                },
            },
        })
    }
}

fn definition<'a>(
    rules: &'a MetricRegistry,
    id: &str,
    family: MetricFamily,
) -> Result<&'a MetricDefinition, String> {
    let matches: Vec<_> = rules.definitions.iter().filter(|d| d.id == id).collect();
    let [definition] = matches.as_slice() else {
        return Err(format!(
            "definition {id} must have exactly one registered version"
        ));
    };
    if definition.family != family
        || definition.source.trim().is_empty()
        || definition.evidence.trim().is_empty()
        || definition.formula.trim().is_empty()
    {
        return Err(format!(
            "definition {id} has incompatible family or missing evidence"
        ));
    }
    Ok(definition)
}

fn compute(
    series: &Series,
    rules: &MetricRegistry,
    readings: &[Accepted],
    documents: &[DocumentRow],
) -> Result<SeriesResult, String> {
    let definition = definition(rules, &series.definition, MetricFamily::ReserveReplacement)?;
    let matched: Vec<_> = rules
        .reserve_replacements
        .iter()
        .filter(|r| r.definition == series.definition)
        .collect();
    let [rule] = matched.as_slice() else {
        return Err("reserve replacement requires exactly one explicit metric rule".into());
    };
    let expected_basis = match rule.metal {
        ReserveMetal::Gold => "attributable_gold_ex_ad",
        ReserveMetal::GoldEquivalent => "attributable_geo_ex_ad",
    };
    if definition.unit != "percent"
        || !matches!(series.unit.as_str(), "oz" | "koz" | "Moz")
        || rule.amount_unit != series.unit
        || rule.basis != series.basis
        || rule.basis != expected_basis
        || !rule.excludes_acquisitions_divestments
        || rule.window_years != series.end_year - series.start_year + 1
    {
        return Err("window, unit, metal or attributable ex-acquisition/divestment basis differs from the supported registered rule".into());
    }
    let geo_definition = match (rule.metal, &rule.geo_definition) {
        (ReserveMetal::Gold, None) => None,
        (ReserveMetal::GoldEquivalent, Some(id)) => {
            let d = definition_geo(rules, id)?;
            Some(d.clone())
        }
        _ => return Err("metal and registered GEO definition disagree".into()),
    };
    let binder = Binder {
        basis: &series.basis,
        readings,
        documents,
    };
    let mut years = Vec::new();
    let empty = ExactValue {
        printed: ZERO,
        bounds: Bounds { lo: ZERO, hi: ZERO },
    };
    let mut net = empty;
    let mut depletion = empty;
    for selected in &series.years {
        let period = selected.year.to_string();
        let n = binder.bind(
            &selected.doc,
            &selected.net_change,
            &period,
            selected.year,
            &series.unit,
        )?;
        let d = binder.bind(
            &selected.doc,
            &selected.depletion,
            &period,
            selected.year,
            &series.unit,
        )?;
        let normalized = d.value.depletion(series.depletion_sign)?;
        let geo_deck = if let Some(definition) = &geo_definition {
            let matches: Vec<_> = rules
                .geo_decks
                .iter()
                .filter(|deck| {
                    deck.definition == definition.id && deck.year == i64::from(selected.year)
                })
                .collect();
            let [deck] = matches.as_slice() else {
                return Err(format!(
                    "GEO year {} requires exactly one named reserve-price deck",
                    selected.year
                ));
            };
            for price in [&deck.gold_usd_per_oz, &deck.copper_usd_per_lb] {
                if arithmetic(Rat::from_decimal_str(price))?.signum() <= 0 {
                    return Err("GEO reserve prices must be positive exact decimals".into());
                }
            }
            Some((*deck).clone())
        } else {
            None
        };
        net = net.add(n.value)?;
        depletion = depletion.add(normalized)?;
        years.push(AnnualResult {
            year: selected.year,
            net_change: n,
            depletion: d,
            normalized_depletion: normalized,
            geo_deck,
        });
    }
    let annual = totals(net, depletion)?;
    let aggregate = series
        .aggregate
        .as_ref()
        .map(|selected| {
            let n = binder.bind(
                &selected.doc,
                &selected.net_change,
                &selected.period,
                series.end_year,
                &series.unit,
            )?;
            let d = binder.bind(
                &selected.doc,
                &selected.depletion,
                &selected.period,
                series.end_year,
                &series.unit,
            )?;
            let totals = totals(n.value, d.value.depletion(series.depletion_sign)?)?;
            Ok::<_, String>(AggregateResult {
                net_change: n,
                depletion: d,
                totals,
            })
        })
        .transpose()?;
    let (joint_net, joint_depletion) = match &aggregate {
        Some(a) => (
            net.bounds
                .intersect(a.totals.net_change.bounds)?
                .ok_or("annual and aggregate net-change rounding constraints are disjoint")?,
            depletion
                .bounds
                .intersect(a.totals.depletion.bounds)?
                .ok_or("annual and aggregate depletion rounding constraints are disjoint")?,
        ),
        None => (net.bounds, depletion.bounds),
    };
    let joint = JointBounds {
        net_change: joint_net,
        depletion: joint_depletion,
        percent: percent_bounds(joint_net, joint_depletion)?,
    };
    let headline = series
        .headline
        .as_ref()
        .map(|selected| {
            let reading = binder.bind(
                &selected.doc,
                &selected.quantity,
                &selected.period,
                series.end_year,
                "percent",
            )?;
            let state = match selected.kind {
                HeadlineKind::Approximate => HeadlineState::ApproximateUnassessed,
                HeadlineKind::Rounded => {
                    if joint.percent.intersect(reading.value.bounds)?.is_some() {
                        HeadlineState::RoundedIntervalCompatible
                    } else {
                        HeadlineState::Incompatible
                    }
                }
            };
            Ok::<_, String>(HeadlineResult {
                reading,
                kind: selected.kind,
                state,
            })
        })
        .transpose()?;
    Ok(SeriesResult {
        selection: series.clone(),
        definition: definition.clone(),
        rule: (*rule).clone(),
        geo_definition,
        years,
        annual,
        aggregate,
        joint,
        headline,
    })
}

fn definition_geo<'a>(rules: &'a MetricRegistry, id: &str) -> Result<&'a MetricDefinition, String> {
    let d = definition(rules, id, MetricFamily::Geo)?;
    if d.unit != "oz" {
        return Err("the GEO definition must explicitly produce gold-equivalent ounces".into());
    }
    Ok(d)
}

fn selectors(series: &Series) -> BTreeSet<(&str, &str)> {
    let mut out = BTreeSet::new();
    for year in &series.years {
        out.insert((year.doc.as_str(), year.net_change.as_str()));
        out.insert((year.doc.as_str(), year.depletion.as_str()));
    }
    if let Some(total) = &series.aggregate {
        out.insert((&total.doc, &total.net_change));
        out.insert((&total.doc, &total.depletion));
    }
    if let Some(headline) = &series.headline {
        out.insert((&headline.doc, &headline.quantity));
    }
    out
}

/// Rechecks pure bindings. The caller must first authenticate document bytes and independent readings.
#[must_use]
pub fn assess(
    binding: &ReplacementRegistry,
    rules: &MetricRegistry,
    accepted: &[Accepted],
    documents: &[DocumentRow],
) -> ReplacementReport {
    let mut binding = binding.clone();
    for series in &mut binding.series {
        series.years.sort_by(|a, b| {
            (a.year, &a.doc, &a.net_change, &a.depletion).cmp(&(
                b.year,
                &b.doc,
                &b.net_change,
                &b.depletion,
            ))
        });
    }
    binding.series.sort_by_cached_key(selection_json);
    let registry_refusal = binding.validate().err().map(|e| e.to_string());
    let mut accepted = accepted.to_vec();
    for a in &mut accepted {
        a.readers.sort();
    }
    accepted.sort_by_cached_key(accepted_json);
    let mut documents = documents.to_vec();
    documents.sort_by_cached_key(document_json);
    let mut selected_rules: Vec<_> = rules
        .reserve_replacements
        .iter()
        .filter(|rule| {
            binding
                .series
                .iter()
                .any(|s| s.definition == rule.definition)
        })
        .cloned()
        .collect();
    selected_rules.sort_by_cached_key(rule_json);
    let mut definitions: Vec<_> = rules
        .definitions
        .iter()
        .filter(|d| {
            binding.series.iter().any(|s| s.definition == d.id)
                || selected_rules
                    .iter()
                    .any(|r| r.geo_definition.as_deref() == Some(d.id.as_str()))
        })
        .cloned()
        .collect();
    definitions.sort_by_cached_key(definition_json);
    let mut geo_decks: Vec<_> = rules
        .geo_decks
        .iter()
        .filter(|d| {
            selected_rules
                .iter()
                .any(|r| r.geo_definition.as_deref() == Some(d.definition.as_str()))
        })
        .cloned()
        .collect();
    geo_decks.sort_by_cached_key(deck_json);
    let series = binding
        .series
        .iter()
        .map(|selection| {
            let result = match &registry_refusal {
                Some(reason) => Err(format!("invalid replacement registry: {reason}")),
                None => compute(selection, rules, &accepted, &documents),
            };
            match result {
                Ok(result) => SeriesAssessment::Computed(Box::new(result)),
                Err(reason) => {
                    let selected = selectors(selection);
                    SeriesAssessment::Refused(Box::new(SeriesRefusal {
                        selection: selection.clone(),
                        reason,
                        evidence: accepted
                            .iter()
                            .filter(|a| {
                                selected.contains(&(a.locator.doc.as_str(), a.quantity.as_str()))
                            })
                            .cloned()
                            .collect(),
                        documents: documents
                            .iter()
                            .filter(|d| selected.iter().any(|(doc, _)| *doc == d.file))
                            .cloned()
                            .collect(),
                    }))
                }
            }
        })
        .collect();
    ReplacementReport {
        issuer: binding.issuer,
        registry_refusal,
        definitions,
        rules: selected_rules,
        geo_decks,
        series,
    }
}

fn js(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn array<T>(values: &[T], encode: impl Fn(&T) -> String) -> String {
    format!(
        "[{}]",
        values.iter().map(encode).collect::<Vec<_>>().join(",")
    )
}
fn optional<T>(value: Option<&T>, encode: impl Fn(&T) -> String) -> String {
    value.map_or_else(|| "null".into(), encode)
}
fn rat(value: Rat) -> String {
    format!(
        "{{\"num\":\"{}\",\"den\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn bounds(value: Bounds) -> String {
    format!(
        "{{\"lo\":{},\"hi\":{},\"closed\":true}}",
        rat(value.lo),
        rat(value.hi)
    )
}
fn exact(value: ExactValue) -> String {
    format!(
        "{{\"printed\":{},\"rounding\":{}}}",
        rat(value.printed),
        bounds(value.bounds)
    )
}
fn totals_json(value: &Totals) -> String {
    format!(
        "{{\"net_change\":{},\"depletion\":{},\"percent\":{}}}",
        exact(value.net_change),
        exact(value.depletion),
        exact(value.percent)
    )
}

fn accepted_json(a: &Accepted) -> String {
    let decimal = |d: fmine_registry::transcriptions::Decimal| {
        format!("{{\"mantissa\":\"{}\",\"scale\":{}}}", d.mantissa, d.scale)
    };
    let admitted = match a.value {
        Token::Point(d) => format!("{{\"kind\":\"point\",\"value\":{}}}", decimal(d)),
        Token::Range(lo, hi) => format!(
            "{{\"kind\":\"range\",\"lo\":{},\"hi\":{}}}",
            decimal(lo),
            decimal(hi)
        ),
    };
    format!(
        "{{\"locator\":{},\"doc\":{},\"page\":{},\"table\":{},\"row\":{},\"col\":{},\"sha256\":{},\"token\":{},\"admitted_value\":{admitted},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"readers\":{}}}",
        js(&a.locator.to_string()),
        js(&a.locator.doc),
        a.locator.page,
        js(&a.locator.table),
        js(&a.locator.row),
        js(&a.locator.col),
        js(&a.doc_sha256),
        js(&a.token),
        js(&a.quantity),
        js(&a.unit),
        js(&a.basis),
        js(&a.period),
        array(&a.readers, |s| js(s))
    )
}

fn document_json(d: &DocumentRow) -> String {
    format!(
        "{{\"file\":{},\"dir\":{},\"kind\":{},\"url\":{},\"url_basis\":{},\"published_at\":{},\"published_at_basis\":{},\"pages\":{},\"bytes\":{},\"sha256\":{}}}",
        js(&d.file),
        js(&d.dir),
        js(&d.kind),
        js(&d.url),
        js(&d.url_basis),
        js(&d.published_at),
        js(&d.published_at_basis),
        d.pages,
        d.bytes,
        js(&d.sha256)
    )
}
fn reading_json(r: &Reading) -> String {
    format!(
        "{{\"source\":{},\"document\":{},\"value\":{}}}",
        accepted_json(&r.source),
        document_json(&r.document),
        exact(r.value)
    )
}
fn definition_json(d: &MetricDefinition) -> String {
    let family = match d.family {
        MetricFamily::AllInCost => "all_in_cost",
        MetricFamily::Geo => "geo",
        MetricFamily::ReserveReplacement => "reserve_replacement",
    };
    let credit = match d.credit_basis {
        fmine_registry::metrics::CreditBasis::ByProduct => "by_product",
        fmine_registry::metrics::CreditBasis::CoProduct => "co_product",
    };
    format!(
        "{{\"id\":{},\"family\":{},\"unit\":{},\"formula\":{},\"source\":{},\"evidence\":{},\"credit_basis\":{},\"extends\":{},\"supersedes\":{}}}",
        js(&d.id),
        js(family),
        js(&d.unit),
        js(&d.formula),
        js(&d.source),
        js(&d.evidence),
        js(credit),
        array(&d.extends, |s| js(s)),
        optional(d.supersedes.as_ref(), |s| js(s))
    )
}
fn rule_json(r: &ReserveReplacementRule) -> String {
    format!(
        "{{\"definition\":{},\"metal\":{},\"window_years\":{},\"amount_unit\":{},\"basis\":{},\"excludes_acquisitions_divestments\":{},\"geo_definition\":{}}}",
        js(&r.definition),
        js(r.metal.as_str()),
        r.window_years,
        js(&r.amount_unit),
        js(&r.basis),
        r.excludes_acquisitions_divestments,
        optional(r.geo_definition.as_ref(), |s| js(s))
    )
}
fn deck_json(d: &GeoDeck) -> String {
    format!(
        "{{\"definition\":{},\"year\":{},\"gold_usd_per_oz\":{},\"copper_usd_per_lb\":{}}}",
        js(&d.definition),
        d.year,
        js(&d.gold_usd_per_oz),
        js(&d.copper_usd_per_lb)
    )
}
fn selection_json(s: &Series) -> String {
    let years = array(&s.years, |y| {
        format!(
            "{{\"year\":{},\"doc\":{},\"net_change\":{},\"depletion\":{}}}",
            y.year,
            js(&y.doc),
            js(&y.net_change),
            js(&y.depletion)
        )
    });
    let aggregate = optional(s.aggregate.as_ref(), |a| {
        format!(
            "{{\"doc\":{},\"net_change\":{},\"depletion\":{},\"period\":{}}}",
            js(&a.doc),
            js(&a.net_change),
            js(&a.depletion),
            js(&a.period)
        )
    });
    let headline = optional(s.headline.as_ref(), |h| {
        format!(
            "{{\"doc\":{},\"quantity\":{},\"period\":{},\"kind\":{}}}",
            js(&h.doc),
            js(&h.quantity),
            js(&h.period),
            js(h.kind.as_str())
        )
    });
    format!(
        "{{\"id\":{},\"definition\":{},\"start_year\":{},\"end_year\":{},\"unit\":{},\"basis\":{},\"depletion_sign\":{},\"years\":{years},\"aggregate\":{aggregate},\"headline\":{headline}}}",
        js(&s.id),
        js(&s.definition),
        s.start_year,
        s.end_year,
        js(&s.unit),
        js(&s.basis),
        js(s.depletion_sign.as_str())
    )
}
fn result_json(r: &SeriesAssessment) -> String {
    match r {
        SeriesAssessment::Refused(r) => format!(
            "{{\"state\":\"refused\",\"selection\":{},\"reason\":{},\"evidence\":{},\"documents\":{}}}",
            selection_json(&r.selection),
            js(&r.reason),
            array(&r.evidence, accepted_json),
            array(&r.documents, document_json)
        ),
        SeriesAssessment::Computed(r) => {
            let years = array(&r.years, |y| {
                format!(
                    "{{\"year\":{},\"net_change\":{},\"depletion\":{},\"normalized_depletion\":{},\"geo_deck\":{}}}",
                    y.year,
                    reading_json(&y.net_change),
                    reading_json(&y.depletion),
                    exact(y.normalized_depletion),
                    optional(y.geo_deck.as_ref(), deck_json)
                )
            });
            let aggregate = optional(r.aggregate.as_ref(), |a| {
                format!(
                    "{{\"net_change\":{},\"depletion\":{},\"totals\":{}}}",
                    reading_json(&a.net_change),
                    reading_json(&a.depletion),
                    totals_json(&a.totals)
                )
            });
            let headline = optional(r.headline.as_ref(), |h| {
                format!(
                    "{{\"reading\":{},\"kind\":{},\"state\":{},\"comparison_interval\":{}}}",
                    reading_json(&h.reading),
                    js(h.kind.as_str()),
                    js(h.state.as_str()),
                    if h.kind == HeadlineKind::Rounded {
                        bounds(h.reading.value.bounds)
                    } else {
                        "null".into()
                    }
                )
            });
            format!(
                "{{\"state\":\"computed\",\"selection\":{},\"definition\":{},\"rule\":{},\"geo_definition\":{},\"years\":{years},\"annual\":{},\"aggregate\":{aggregate},\"joint\":{{\"net_change\":{},\"depletion\":{},\"percent\":{}}},\"headline\":{headline}}}",
                selection_json(&r.selection),
                definition_json(&r.definition),
                rule_json(&r.rule),
                optional(r.geo_definition.as_ref(), definition_json),
                totals_json(&r.annual),
                bounds(r.joint.net_change),
                bounds(r.joint.depletion),
                bounds(r.joint.percent)
            )
        }
    }
}

impl ReplacementReport {
    fn body(&self) -> String {
        format!(
            "\"schema\":{},\"issuer\":{},\"registry_refusal\":{},\"definitions\":{},\"rules\":{},\"geo_decks\":{},\"series\":{},\"no_claim\":{}",
            js(SCHEMA),
            js(&self.issuer),
            optional(self.registry_refusal.as_ref(), |s| js(s)),
            array(&self.definitions, definition_json),
            array(&self.rules, rule_json),
            array(&self.geo_decks, deck_json),
            array(&self.series, result_json),
            js(NO_CLAIM)
        )
    }

    /// SHA-256 binds the full canonical report, excluding only its own digest field.
    #[must_use]
    pub fn digest(&self) -> String {
        sha256(format!("{{{}}}", self.body()).as_bytes()).to_hex()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!("{{{},\"digest\":{}}}", self.body(), js(&self.digest()))
    }
}

#[cfg(test)]
#[path = "../../xtask/testdata/reserve_replacement_unit_tests.rs"]
mod tests;
