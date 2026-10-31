//! Published cut-off reproduction (`BD-274`, plan §15; E182–E184).
//! Determinism: DeterministicPerIsa (the grades are exact rationals; only the solver cross-check uses floats).
//!
//! A technical report prints a cut-off grade and the inputs behind it. This recomputes the grade from those
//! inputs alone, on exact rationals, and says how the printed figure stands against the result:
//!
//! - `Reproduced`: the value at the printed inputs rounds to the printed cut-off;
//! - `WithinInputRounding`: it does not, but the range the inputs' own rounding allows reaches the printed cut-off;
//! - `Conflict`: no input within its rounding reaches it.
//!
//! The range is exact because the grade is monotone in every input. It is a statement about the printed inputs: it
//! says nothing about whether those inputs are right for the mine.

use std::collections::BTreeSet;
use std::fmt;

use fmine_registry::toml_subset::{TomlRow, TomlValue, parse_subset};
use fmine_units::{ExactQty, TROY_OZ_TO_GRAMS_DEN, TROY_OZ_TO_GRAMS_NUM, UnitsError};

use crate::solve_cutoff_grade;

/// Schema id `registries/published_cutoffs.toml` must declare.
pub const CUTOFFS_SCHEMA: &str = "fmine.published_cutoffs.v1";

/// Largest step the solver cross-check may differ from the exact fixed point by, in g/t.
const SOLVER_AGREEMENT_G_PER_T: f64 = 1e-6;

/// Why a registry row could not be read or a cut-off could not be computed.
#[derive(Debug, Clone, PartialEq)]
pub enum CutoffFailure {
    /// The registry text does not parse, or its schema line is wrong.
    Registry(String),
    /// A row is missing a field, repeats an id, or holds a malformed value.
    Row { line: usize, detail: String },
    /// Exact arithmetic overflowed.
    Exact(UnitsError),
    /// The net value per ounce is not positive, so no break-even grade exists.
    NoPositiveNet { id: String },
    /// The recovery term is not positive, so no break-even grade exists.
    NoPositiveRecovery { id: String },
    /// The float solver failed, or disagrees with the exact fixed point.
    Solver { id: String, detail: String },
}

impl fmt::Display for CutoffFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registry(d) => write!(f, "published cut-off registry: {d}"),
            Self::Row { line, detail } => {
                write!(f, "published cut-off registry, line {line}: {detail}")
            }
            Self::Exact(e) => write!(f, "exact arithmetic failed: {e:?}"),
            Self::NoPositiveNet { id } => {
                write!(f, "{id}: the net value per ounce is not positive")
            }
            Self::NoPositiveRecovery { id } => write!(f, "{id}: recovery is not positive"),
            Self::Solver { id, detail } => write!(f, "{id}: fixed-point solver: {detail}"),
        }
    }
}

impl std::error::Error for CutoffFailure {}

impl From<UnitsError> for CutoffFailure {
    fn from(e: UnitsError) -> Self {
        Self::Exact(e)
    }
}

/// A token as printed: its value and half a unit of its last place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Printed {
    pub value: ExactQty,
    pub half_step: ExactQty,
}

impl Printed {
    /// Reads a plain decimal token. Its decimal places give its rounding step.
    pub fn parse(token: &str) -> Result<Self, UnitsError> {
        let places = token.split_once('.').map_or(0, |(_, frac)| frac.len());
        let places = u32::try_from(places).map_err(|_| UnitsError::Overflow)?;
        let scale = 10_i128.checked_pow(places).ok_or(UnitsError::Overflow)?;
        Ok(Self {
            value: ExactQty::from_decimal_str(token)?,
            half_step: ExactQty::new(1, scale.checked_mul(2).ok_or(UnitsError::Overflow)?)?,
        })
    }

    /// `value − half_step`.
    pub fn low(self) -> Result<ExactQty, UnitsError> {
        self.value.checked_sub(self.half_step)
    }

    /// `value + half_step`.
    pub fn high(self) -> Result<ExactQty, UnitsError> {
        self.value.checked_add(self.half_step)
    }
}

/// The unit a report prints its cut-off in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradeUnit {
    GramsPerTonne,
    OuncesPerShortTon,
}

impl GradeUnit {
    fn symbol(self) -> &'static str {
        match self {
            Self::GramsPerTonne => "g/t",
            Self::OuncesPerShortTon => "oz/st",
        }
    }
}

/// How recovery enters the cut-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// One printed recovery, in percent.
    Percent(Printed),
    /// Recovery that depends on the grade itself, `R% = intercept − slope · TCM / Au`, with TCM in percent and Au
    /// in g/t. The cut-off is then the fixed point of `g = c / R(g)` (E182).
    TcmOverAu {
        intercept: Printed,
        slope: Printed,
        tcm_percent: Printed,
    },
}

/// One registered cut-off and its printed inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoffCase {
    pub id: String,
    pub document: String,
    pub page: i64,
    pub table: String,
    pub evidence: String,
    pub unit: GradeUnit,
    pub published: Printed,
    /// The same cut-off as the report also prints it in g/t, for a report that works in oz/st.
    pub published_metric: Option<Printed>,
    /// The report's assumed price, taken as exact.
    pub gold_price_usd_per_oz: ExactQty,
    /// Cost terms in US$/t, added together.
    pub costs_usd_per_t: Vec<Printed>,
    /// Terms taken as a percentage of revenue.
    pub revenue_percent: Vec<Printed>,
    /// Terms taken per recovered ounce.
    pub usd_per_oz: Vec<Printed>,
    pub recovery: Recovery,
}

/// How the printed cut-off stands against the printed inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoffState {
    Reproduced,
    WithinInputRounding,
    Conflict,
}

impl CutoffState {
    /// The state as the robot output spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reproduced => "reproduced",
            Self::WithinInputRounding => "within_input_rounding",
            Self::Conflict => "conflict",
        }
    }
}

/// The result for one registered cut-off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoffReport {
    pub id: String,
    pub document: String,
    pub page: i64,
    pub table: String,
    pub evidence: String,
    pub unit: GradeUnit,
    pub published: Printed,
    /// The grade at the printed inputs, in the report's unit.
    pub computed: ExactQty,
    /// The grade with every input at the end of its rounding that lowers it.
    pub low: ExactQty,
    /// The grade with every input at the end of its rounding that raises it.
    pub high: ExactQty,
    pub state: CutoffState,
    /// Iterations the fixed-point solver took, for a grade-dependent recovery.
    pub solver_iterations: Option<usize>,
    /// For a report working in oz/st: its printed g/t equals its printed oz/st, already rounded, times 240/7.
    pub metric_from_rounded: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Corner {
    Printed,
    Low,
    High,
}

/// An input's value at `corner`. `raises` says whether a larger input gives a larger grade.
fn at(p: Printed, raises: bool, corner: Corner) -> Result<ExactQty, UnitsError> {
    match (corner, raises) {
        (Corner::Printed, _) => Ok(p.value),
        (Corner::High, true) | (Corner::Low, false) => p.high(),
        (Corner::High, false) | (Corner::Low, true) => p.low(),
    }
}

fn sum(terms: &[Printed], corner: Corner) -> Result<ExactQty, UnitsError> {
    terms
        .iter()
        .try_fold(ExactQty::from_integer(0), |total, p| {
            total.checked_add(at(*p, true, corner)?)
        })
}

/// Revenue kept per recovered ounce: the price, less its percentage terms, less its per-ounce terms.
fn net_per_oz(case: &CutoffCase, corner: Corner) -> Result<ExactQty, UnitsError> {
    let share = sum(&case.revenue_percent, corner)?.checked_div(ExactQty::from_integer(100))?;
    case.gold_price_usd_per_oz
        .checked_mul(ExactQty::from_integer(1).checked_sub(share)?)?
        .checked_sub(sum(&case.usd_per_oz, corner)?)
}

/// The grade in g/t at `corner`. With `Recovery::TcmOverAu`, `g·R(g) = c` is linear in `g`, so its fixed point has
/// a closed form and stays exact.
fn grade_g_per_t(case: &CutoffCase, corner: Corner) -> Result<ExactQty, CutoffFailure> {
    let hundred = ExactQty::from_integer(100);
    let net_per_oz = net_per_oz(case, corner)?;
    if net_per_oz.signum() <= 0 {
        return Err(CutoffFailure::NoPositiveNet {
            id: case.id.clone(),
        });
    }
    let grams_per_oz = ExactQty::new(TROY_OZ_TO_GRAMS_NUM, TROY_OZ_TO_GRAMS_DEN)?;
    let c =
        sum(&case.costs_usd_per_t, corner)?.checked_div(net_per_oz.checked_div(grams_per_oz)?)?;
    let (numerator, recovery) = match case.recovery {
        Recovery::Percent(r) => (c, at(r, false, corner)?),
        Recovery::TcmOverAu {
            intercept,
            slope,
            tcm_percent,
        } => {
            let loss = at(slope, true, corner)?
                .checked_mul(at(tcm_percent, true, corner)?)?
                .checked_div(hundred)?;
            (c.checked_add(loss)?, at(intercept, false, corner)?)
        }
    };
    if recovery.signum() <= 0 {
        return Err(CutoffFailure::NoPositiveRecovery {
            id: case.id.clone(),
        });
    }
    Ok(numerator.checked_div(recovery.checked_div(hundred)?)?)
}

fn in_unit(g_per_t: ExactQty, unit: GradeUnit) -> Result<ExactQty, UnitsError> {
    match unit {
        GradeUnit::GramsPerTonne => Ok(g_per_t),
        GradeUnit::OuncesPerShortTon => g_per_t.g_per_t_to_oz_per_st(),
    }
}

/// Runs the float fixed-point solver on a grade-dependent recovery and holds it to the exact answer.
fn solver_check(
    case: &CutoffCase,
    exact_g_per_t: ExactQty,
) -> Result<Option<usize>, CutoffFailure> {
    let Recovery::TcmOverAu {
        intercept,
        slope,
        tcm_percent,
    } = case.recovery
    else {
        return Ok(None);
    };
    let net_per_g = net_per_oz(case, Corner::Printed)?
        .checked_div(ExactQty::new(TROY_OZ_TO_GRAMS_NUM, TROY_OZ_TO_GRAMS_DEN)?)?;
    let cost = sum(&case.costs_usd_per_t, Corner::Printed)?.to_f64();
    let (i, s, t) = (
        intercept.value.to_f64(),
        slope.value.to_f64(),
        tcm_percent.value.to_f64(),
    );
    let solved = solve_cutoff_grade(cost, net_per_g.to_f64(), exact_g_per_t.to_f64(), |g| {
        (i - s * t / g) / 100.0
    })
    .map_err(|e| CutoffFailure::Solver {
        id: case.id.clone(),
        detail: e.to_string(),
    })?;
    let gap = (solved.grade - exact_g_per_t.to_f64()).abs();
    if gap > SOLVER_AGREEMENT_G_PER_T {
        return Err(CutoffFailure::Solver {
            id: case.id.clone(),
            detail: format!("differs from the exact fixed point by {gap}"),
        });
    }
    Ok(Some(solved.iterations))
}

/// Whether a report's printed g/t equals its printed oz/st, already rounded, times 240/7, at the g/t's own step.
fn metric_from_rounded(published: Printed, metric: Printed) -> Result<bool, UnitsError> {
    rounds_to(published.value.oz_per_st_to_g_per_t()?, metric)
}

/// `x` rounds to `p`, half up.
fn rounds_to(x: ExactQty, p: Printed) -> Result<bool, UnitsError> {
    use std::cmp::Ordering::Less;
    Ok(x.checked_cmp(p.low()?)? != Less && x.checked_cmp(p.high()?)? == Less)
}

/// The range `[low, high]` holds a value that rounds to `p`.
fn reaches(low: ExactQty, high: ExactQty, p: Printed) -> Result<bool, UnitsError> {
    use std::cmp::Ordering::Less;
    Ok(low.checked_cmp(p.high()?)? == Less && high.checked_cmp(p.low()?)? != Less)
}

/// Recomputes one cut-off from its printed inputs.
pub fn reproduce(case: &CutoffCase) -> Result<CutoffReport, CutoffFailure> {
    let printed_g_per_t = grade_g_per_t(case, Corner::Printed)?;
    let computed = in_unit(printed_g_per_t, case.unit)?;
    let low = in_unit(grade_g_per_t(case, Corner::Low)?, case.unit)?;
    let high = in_unit(grade_g_per_t(case, Corner::High)?, case.unit)?;
    let state = if rounds_to(computed, case.published)? {
        CutoffState::Reproduced
    } else if reaches(low, high, case.published)? {
        CutoffState::WithinInputRounding
    } else {
        CutoffState::Conflict
    };
    Ok(CutoffReport {
        id: case.id.clone(),
        document: case.document.clone(),
        page: case.page,
        table: case.table.clone(),
        evidence: case.evidence.clone(),
        unit: case.unit,
        published: case.published,
        computed,
        low,
        high,
        state,
        solver_iterations: solver_check(case, printed_g_per_t)?,
        metric_from_rounded: case
            .published_metric
            .map(|m| metric_from_rounded(case.published, m))
            .transpose()?,
    })
}

/// `x` to `places` decimal places, rounded half up, from the exact value: never through a float.
fn fixed(x: ExactQty, places: u32) -> Result<String, UnitsError> {
    let scale = 10_i128.checked_pow(places).ok_or(UnitsError::Overflow)?;
    let scaled = x.checked_mul(ExactQty::from_integer(scale))?;
    let doubled = scaled
        .num()
        .checked_mul(2)
        .and_then(|n| n.checked_add(scaled.den()))
        .ok_or(UnitsError::Overflow)?;
    let rounded = doubled.div_euclid(scaled.den().checked_mul(2).ok_or(UnitsError::Overflow)?);
    let (whole, frac) = (rounded.div_euclid(scale), rounded.rem_euclid(scale));
    let width = usize::try_from(places).map_err(|_| UnitsError::Overflow)?;
    Ok(format!("{whole}.{frac:0width$}"))
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

impl CutoffReport {
    /// One JSON object: identifiers, the printed token's value and the exact results to six places.
    pub fn to_json(&self) -> Result<String, UnitsError> {
        let unit = self.unit.symbol();
        let (id, doc, table, ev) = (
            json_escape(&self.id),
            json_escape(&self.document),
            json_escape(&self.table),
            json_escape(&self.evidence),
        );
        let (published, computed, low, high) = (
            fixed(self.published.value, 6)?,
            fixed(self.computed, 6)?,
            fixed(self.low, 6)?,
            fixed(self.high, 6)?,
        );
        let solver = self
            .solver_iterations
            .map_or("null".to_string(), |n| n.to_string());
        let metric = self
            .metric_from_rounded
            .map_or("null".to_string(), |b| b.to_string());
        Ok(format!(
            "{{\"id\":\"{id}\",\"document\":\"{doc}\",\"page\":{},\"table\":\"{table}\",\"evidence\":\"{ev}\",\
             \"unit\":\"{unit}\",\"published\":\"{published}\",\"computed\":\"{computed}\",\"low\":\"{low}\",\
             \"high\":\"{high}\",\"state\":\"{}\",\"solver_iterations\":{solver},\"metric_from_rounded\":{metric}}}",
            self.page,
            self.state.as_str(),
        ))
    }
}

fn text<'a>(row: &'a TomlRow, key: &str) -> Result<&'a str, CutoffFailure> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) => Ok(s),
        _ => Err(CutoffFailure::Row {
            line: row.line,
            detail: format!("needs the text field {key}"),
        }),
    }
}

fn printed(row: &TomlRow, key: &str) -> Result<Option<Printed>, CutoffFailure> {
    let Some(field) = row.get(key) else {
        return Ok(None);
    };
    let TomlValue::Str(token) = &field.value else {
        return Err(CutoffFailure::Row {
            line: field.line,
            detail: format!("{key} must be a printed token in quotes"),
        });
    };
    Printed::parse(token)
        .map(Some)
        .map_err(|e| CutoffFailure::Row {
            line: field.line,
            detail: format!("{key} is not a decimal: {e:?}"),
        })
}

fn needed(row: &TomlRow, key: &str) -> Result<Printed, CutoffFailure> {
    printed(row, key)?.ok_or_else(|| CutoffFailure::Row {
        line: row.line,
        detail: format!("needs the field {key}"),
    })
}

fn case_from_row(row: &TomlRow) -> Result<CutoffCase, CutoffFailure> {
    let unit = match text(row, "unit")? {
        "g/t" => GradeUnit::GramsPerTonne,
        "oz/st" => GradeUnit::OuncesPerShortTon,
        other => {
            return Err(CutoffFailure::Row {
                line: row.line,
                detail: format!("unknown unit {other}"),
            });
        }
    };
    let page = match row.get("page").map(|f| &f.value) {
        Some(TomlValue::Int(p)) => *p,
        _ => {
            return Err(CutoffFailure::Row {
                line: row.line,
                detail: "needs the integer field page".to_string(),
            });
        }
    };
    let recovery = match printed(row, "recovery_percent")? {
        Some(r) => {
            if row.get("recovery_slope").is_some() {
                return Err(CutoffFailure::Row {
                    line: row.line,
                    detail: "states both a fixed and a grade-dependent recovery".to_string(),
                });
            }
            Recovery::Percent(r)
        }
        None => Recovery::TcmOverAu {
            intercept: needed(row, "recovery_intercept_percent")?,
            slope: needed(row, "recovery_slope")?,
            tcm_percent: needed(row, "tcm_percent")?,
        },
    };
    let present = |keys: &[&str]| -> Result<Vec<Printed>, CutoffFailure> {
        keys.iter()
            .filter_map(|k| printed(row, k).transpose())
            .collect()
    };
    Ok(CutoffCase {
        id: text(row, "id")?.to_string(),
        document: text(row, "document")?.to_string(),
        page,
        table: text(row, "table")?.to_string(),
        evidence: text(row, "evidence")?.to_string(),
        unit,
        published: needed(row, "published")?,
        published_metric: printed(row, "published_metric")?,
        gold_price_usd_per_oz: ExactQty::from_decimal_str(text(row, "gold_price_usd_per_oz")?)?,
        costs_usd_per_t: {
            let mut costs = vec![needed(row, "cost_usd_per_t")?];
            costs.extend(present(&["sustaining_usd_per_t"])?);
            costs
        },
        revenue_percent: present(&["royalty_percent", "tax_percent"])?,
        usd_per_oz: present(&["royalty_usd_per_oz", "refining_usd_per_oz"])?,
        recovery,
    })
}

/// Reads `registries/published_cutoffs.toml`: schema, one unique id per row, every value well formed.
pub fn parse_cases(toml: &str) -> Result<Vec<CutoffCase>, CutoffFailure> {
    let doc = parse_subset(toml).map_err(|e| CutoffFailure::Registry(e.to_string()))?;
    match doc.top_get("schema").map(|f| &f.value) {
        Some(TomlValue::Str(s)) if s == CUTOFFS_SCHEMA => {}
        _ => {
            return Err(CutoffFailure::Registry(format!(
                "schema must be {CUTOFFS_SCHEMA}"
            )));
        }
    }
    let mut seen = BTreeSet::new();
    let mut cases = Vec::new();
    for row in doc.rows_of("cutoff") {
        let case = case_from_row(row)?;
        if !seen.insert(case.id.clone()) {
            return Err(CutoffFailure::Row {
                line: row.line,
                detail: format!("duplicate id {}", case.id),
            });
        }
        cases.push(case);
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = include_str!("../../../registries/published_cutoffs.toml");

    fn cases() -> Vec<CutoffCase> {
        parse_cases(REGISTRY).expect("registry parses")
    }

    fn state_of(case: &CutoffCase) -> CutoffState {
        reproduce(case).expect("reproduces").state
    }

    /// E182–E184: the seven corpus cut-offs, each state stated. Six round to the printed figure, Turquoise Ridge by a
    /// hair (5.66495 against a rounding edge of 5.665). The seventh, the Cortez Hills longhole roaster row, is 0.1336
    /// against a printed 0.133, E183's "off by 0.001": it sits within the rounding of its inputs, not on the figure.
    #[test]
    fn test_seven_corpus_cutoffs_e182_e184() {
        let reports: Vec<CutoffReport> = cases()
            .iter()
            .map(|c| reproduce(c).expect("reproduces"))
            .collect();
        let got: Vec<(&str, CutoffState, String)> = reports
            .iter()
            .map(|r| (r.id.as_str(), r.state, fixed(r.computed, 4).expect("fixed")))
            .collect();
        for line in &reports {
            println!("{}", line.to_json().expect("json"));
        }
        let want = [
            (
                "turquoise-ridge-ug-resource",
                CutoffState::Reproduced,
                "5.6649",
            ),
            (
                "carlin-gold-quarry-roaster-in-pit",
                CutoffState::Reproduced,
                "1.0837",
            ),
            (
                "cortez-hills-ug-cut-and-fill-oxide",
                CutoffState::Reproduced,
                "0.1176",
            ),
            (
                "cortez-hills-ug-cut-and-fill-roaster",
                CutoffState::Reproduced,
                "0.1521",
            ),
            (
                "cortez-hills-ug-longhole-oxide",
                CutoffState::Reproduced,
                "0.0990",
            ),
            (
                "cortez-hills-ug-longhole-roaster",
                CutoffState::WithinInputRounding,
                "0.1336",
            ),
            (
                "goldrush-longhole-roaster",
                CutoffState::Reproduced,
                "0.1174",
            ),
        ];
        assert_eq!(got.len(), want.len());
        for ((id, state, value), (want_id, want_state, want_value)) in got.iter().zip(want) {
            assert_eq!(
                (*id, *state, value.as_str()),
                (want_id, want_state, want_value)
            );
        }
    }

    /// E182: the printed recovery is the fixed point's, and the float solver lands on the same grade.
    #[test]
    fn test_recovery_at_the_fixed_point_is_the_printed_79_6() {
        let case = &cases()[0];
        let report = reproduce(case).expect("reproduces");
        assert!(report.solver_iterations.is_some_and(|n| n <= 100));
        let g = report.computed.to_f64();
        let r = 100.79 - 155.7 * 0.771 / g;
        assert!((r - 79.6).abs() < 0.05, "recovery at the fixed point {r}");
    }

    /// E183: the printed g/t of every Cortez row is its printed oz/st, already rounded, times 240/7.
    #[test]
    fn test_cortez_metric_values_are_double_rounded() {
        let flags: Vec<Option<bool>> = cases()
            .iter()
            .filter(|c| c.unit == GradeUnit::OuncesPerShortTon)
            .map(|c| reproduce(c).expect("reproduces").metric_from_rounded)
            .collect();
        assert_eq!(flags, vec![Some(true); 5]);
    }

    /// The trap the check exists to catch (E183): reading $/t as $/st, which is what omitting the conversion does,
    /// moves every Cortez row off its printed cut-off.
    #[test]
    fn test_planted_tonne_for_short_ton_conflicts_on_every_cortez_row() {
        for mut case in cases()
            .into_iter()
            .filter(|c| c.unit == GradeUnit::OuncesPerShortTon)
        {
            let tonnes_per_short_ton = ExactQty::from_decimal_str("0.90718474").expect("decimal");
            for cost in &mut case.costs_usd_per_t {
                cost.value = cost
                    .value
                    .checked_div(tonnes_per_short_ton)
                    .expect("divide");
                cost.half_step = cost
                    .half_step
                    .checked_div(tonnes_per_short_ton)
                    .expect("divide");
            }
            assert_eq!(state_of(&case), CutoffState::Conflict, "{}", case.id);
        }
    }

    /// E183: Cortez lists AB495 at 0.96% and leaves it out. Adding it moves the Cut-and-Fill oxide row off its figure.
    #[test]
    fn test_planted_ab495_moves_a_cortez_row() {
        let mut case = cases()
            .into_iter()
            .find(|c| c.id == "cortez-hills-ug-cut-and-fill-oxide")
            .expect("row");
        assert_eq!(state_of(&case), CutoffState::Reproduced);
        case.revenue_percent
            .push(Printed::parse("0.96").expect("token"));
        assert_ne!(state_of(&case), CutoffState::Reproduced);
    }

    /// A printed cut-off beyond the inputs' rounding is a conflict, in both directions.
    #[test]
    fn test_planted_published_figure_conflicts_both_ways() {
        for token in ["1.20", "0.95"] {
            let mut case = cases()[1].clone();
            case.published = Printed::parse(token).expect("token");
            assert_eq!(state_of(&case), CutoffState::Conflict, "{token}");
        }
    }

    /// A printed figure covers `[p − h, p + h)`, so a range that starts at `p + h` misses it and one that ends at
    /// `p − h` reaches it.
    #[test]
    fn test_reaches_respects_the_half_open_rounding_interval() {
        let p = Printed::parse("1.0").expect("token");
        let at = |t: &str| ExactQty::from_decimal_str(t).expect("decimal");
        let reached = |a: &str, b: &str| reaches(at(a), at(b), p).expect("compare");
        assert!(!reached("1.05", "1.20"));
        assert!(reached("1.0499", "1.20"));
        assert!(reached("0.80", "0.95"));
        assert!(!reached("0.80", "0.9499"));
    }

    #[test]
    fn test_range_holds_the_printed_inputs_value() {
        for case in cases() {
            let r = reproduce(&case).expect("reproduces");
            assert_eq!(
                r.low.checked_cmp(r.computed).expect("cmp"),
                std::cmp::Ordering::Less,
                "{}",
                r.id
            );
            assert_eq!(
                r.computed.checked_cmp(r.high).expect("cmp"),
                std::cmp::Ordering::Less,
                "{}",
                r.id
            );
        }
    }

    #[test]
    fn test_registry_refusals() {
        let header = format!("schema = \"{CUTOFFS_SCHEMA}\"\n");
        let row = "[[cutoff]]\nid = \"a\"\ndocument = \"d\"\npage = 1\ntable = \"t\"\nevidence = \"E1\"\nunit = \"g/t\"\n\
                   published = \"1.0\"\ngold_price_usd_per_oz = \"1000\"\ncost_usd_per_t = \"10\"\nrecovery_percent = \"90\"\n";
        assert!(parse_cases(&format!("{header}{row}")).is_ok());
        assert!(matches!(parse_cases(row), Err(CutoffFailure::Registry(_))));
        assert!(matches!(
            parse_cases(&format!("{header}{row}{row}")),
            Err(CutoffFailure::Row { detail, .. }) if detail.contains("duplicate")
        ));
        assert!(matches!(
            parse_cases(&format!("{header}{}", row.replace("g/t", "oz/t"))),
            Err(CutoffFailure::Row { detail, .. }) if detail.contains("unknown unit")
        ));
        assert!(matches!(
            parse_cases(&format!("{header}{}", row.replace("\"1.0\"", "\"one\""))),
            Err(CutoffFailure::Row { detail, .. }) if detail.contains("published")
        ));
        assert!(matches!(
            parse_cases(&format!("{header}{}recovery_slope = \"1\"\n", row)),
            Err(CutoffFailure::Row { detail, .. }) if detail.contains("both")
        ));
    }

    #[test]
    fn test_no_positive_net_is_refused_not_clamped() {
        let mut case = cases()[2].clone();
        case.usd_per_oz.push(Printed::parse("2000").expect("token"));
        assert!(matches!(
            reproduce(&case),
            Err(CutoffFailure::NoPositiveNet { .. })
        ));
    }

    #[test]
    fn test_fixed_rounds_half_up_from_the_exact_value() {
        let x = ExactQty::new(1, 3).expect("rat");
        assert_eq!(fixed(x, 4).expect("fixed"), "0.3333");
        assert_eq!(
            fixed(ExactQty::new(5, 1000).expect("rat"), 2).expect("fixed"),
            "0.01"
        );
        assert_eq!(fixed(ExactQty::from_integer(2), 3).expect("fixed"), "2.000");
    }
}
